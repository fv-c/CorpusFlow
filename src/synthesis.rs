use crate::{
    audio::MonoBuffer,
    config::{
        EnvelopeAdaptationMode, GainAdaptationMode, OverlapScheduleMode, SynthesisConfig,
        WindowKind,
    },
    corpus::CorpusSourceFile,
    index::CorpusIndex,
    matching::MatchSequence,
    micro_adaptation::{
        CarrierEnvelopeProfile, MicroAdaptationPlan, adapt_grain_gain_in_place,
        apply_carrier_envelope_in_place,
    },
    target::TargetAnalysis,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SynthesisPlan {
    pub window: WindowKind,
    pub output_hop_ms: u32,
    pub overlap_schedule: OverlapScheduleMode,
    pub irregularity_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SynthesisFrameSpec {
    pub sample_rate: u32,
    pub output_hop_frames: usize,
    pub irregularity_frames: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledGrain {
    pub match_step_index: usize,
    pub source_index: usize,
    pub source_start_frame: usize,
    pub len_frames: usize,
    pub output_start_frame: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SynthesisOutput {
    pub audio: MonoBuffer,
    pub scheduled_grains: Vec<ScheduledGrain>,
}

impl From<&SynthesisConfig> for SynthesisPlan {
    fn from(config: &SynthesisConfig) -> Self {
        Self {
            window: config.window,
            output_hop_ms: config.output_hop_ms,
            overlap_schedule: config.overlap_schedule,
            irregularity_ms: config.irregularity_ms,
        }
    }
}

impl SynthesisPlan {
    pub fn schedule(
        &self,
        sample_rate: u32,
        corpus_index: &CorpusIndex,
        sequence: &MatchSequence,
    ) -> Result<Vec<ScheduledGrain>, String> {
        let spec = SynthesisFrameSpec::from_plan(self, sample_rate)?;
        let mut scheduled = Vec::with_capacity(sequence.steps.len());
        let mut output_start_frame = 0;

        for (step_index, step) in sequence.steps.iter().enumerate() {
            let grain = corpus_index
                .grain(step.selected_grain_index)
                .ok_or_else(|| {
                    format!(
                        "match step {step_index} references invalid grain index {}",
                        step.selected_grain_index
                    )
                })?;

            scheduled.push(ScheduledGrain {
                match_step_index: step_index,
                source_index: grain.source_index,
                source_start_frame: grain.start_frame,
                len_frames: grain.len_frames,
                output_start_frame,
            });

            output_start_frame += scheduled_hop_frames(self.overlap_schedule, spec, step_index);
        }

        Ok(scheduled)
    }

    pub fn synthesize(
        &self,
        corpus_sources: &[CorpusSourceFile],
        corpus_index: &CorpusIndex,
        sequence: &MatchSequence,
    ) -> Result<SynthesisOutput, String> {
        let empty_target = TargetAnalysis {
            sample_rate: 0,
            original_channels: 0,
            total_frames: 0,
            frame_size_frames: 0,
            hop_size_frames: 0,
            frames: Vec::new(),
        };
        let micro_adaptation = MicroAdaptationPlan {
            gain: GainAdaptationMode::Off,
            envelope: EnvelopeAdaptationMode::Off,
        };

        self.synthesize_with_micro_adaptation(
            corpus_sources,
            corpus_index,
            sequence,
            &micro_adaptation,
            &empty_target,
        )
    }

    pub fn synthesize_with_micro_adaptation(
        &self,
        corpus_sources: &[CorpusSourceFile],
        corpus_index: &CorpusIndex,
        sequence: &MatchSequence,
        micro_adaptation: &MicroAdaptationPlan,
        target_analysis: &TargetAnalysis,
    ) -> Result<SynthesisOutput, String> {
        let sample_rate = resolve_synthesis_sample_rate(corpus_sources, corpus_index, sequence)?;
        let scheduled_grains = self.schedule(sample_rate, corpus_index, sequence)?;
        let output_frames = scheduled_grains
            .iter()
            .map(|grain| grain.output_start_frame + grain.len_frames)
            .max()
            .unwrap_or(0);
        let mut output_samples = vec![0.0; output_frames];
        let mut window_cache_len = 0;
        let mut window_cache = Vec::new();
        let mut grain_scratch = Vec::new();

        for grain in &scheduled_grains {
            let source = corpus_sources.get(grain.source_index).ok_or_else(|| {
                format!(
                    "scheduled grain {} references invalid source index {}",
                    grain.match_step_index, grain.source_index
                )
            })?;
            let source_end = grain.source_start_frame + grain.len_frames;
            if source_end > source.audio.samples.len() {
                return Err(format!(
                    "scheduled grain {} exceeds source {} length: end {} > {}",
                    grain.match_step_index,
                    grain.source_index,
                    source_end,
                    source.audio.samples.len()
                ));
            }

            if window_cache_len != grain.len_frames {
                window_cache = build_window(self.window, grain.len_frames);
                window_cache_len = grain.len_frames;
            }

            let input = &source.audio.samples[grain.source_start_frame..source_end];
            let grain_samples = if matches!(micro_adaptation.gain, GainAdaptationMode::Off) {
                input
            } else {
                let target_frame = target_analysis
                    .frames
                    .get(grain.match_step_index)
                    .ok_or_else(|| {
                        format!(
                            "micro adaptation requires target frame {} for scheduled grain {}",
                            grain.match_step_index, grain.match_step_index
                        )
                    })?;

                grain_scratch.clear();
                grain_scratch.extend_from_slice(input);
                adapt_grain_gain_in_place(
                    &mut grain_scratch,
                    micro_adaptation.gain,
                    target_frame.rms,
                );
                &grain_scratch
            };

            for frame_index in 0..grain.len_frames {
                output_samples[grain.output_start_frame + frame_index] +=
                    grain_samples[frame_index] * window_cache[frame_index];
            }
        }

        if !matches!(micro_adaptation.envelope, EnvelopeAdaptationMode::Off) {
            let profile = CarrierEnvelopeProfile::from_target_analysis(target_analysis);
            apply_carrier_envelope_in_place(
                &mut output_samples,
                micro_adaptation.envelope,
                &profile,
            )?;
        }

        Ok(SynthesisOutput {
            audio: MonoBuffer::new(sample_rate, output_samples)?,
            scheduled_grains,
        })
    }
}

impl SynthesisFrameSpec {
    pub fn from_plan(plan: &SynthesisPlan, sample_rate: u32) -> Result<Self, String> {
        if sample_rate == 0 {
            return Err("synthesis sample_rate must be > 0".to_string());
        }

        let output_hop_frames = ms_to_frames(sample_rate, plan.output_hop_ms);
        let irregularity_frames = ms_to_optional_frames(sample_rate, plan.irregularity_ms);

        if plan.overlap_schedule == OverlapScheduleMode::Fixed && irregularity_frames != 0 {
            return Err("fixed synthesis scheduling requires 0 irregularity frames".to_string());
        }

        if plan.overlap_schedule == OverlapScheduleMode::Alternating {
            if irregularity_frames == 0 {
                return Err(
                    "alternating synthesis scheduling requires irregularity frames".to_string(),
                );
            }
            if irregularity_frames >= output_hop_frames {
                return Err(
                    "alternating synthesis irregularity must be smaller than output hop"
                        .to_string(),
                );
            }
        }

        Ok(Self {
            sample_rate,
            output_hop_frames,
            irregularity_frames,
        })
    }
}

pub fn build_window(kind: WindowKind, frame_size: usize) -> Vec<f32> {
    match kind {
        WindowKind::Hann => build_hann_window(frame_size),
    }
}

fn resolve_synthesis_sample_rate(
    corpus_sources: &[CorpusSourceFile],
    corpus_index: &CorpusIndex,
    sequence: &MatchSequence,
) -> Result<u32, String> {
    if corpus_sources.len() != corpus_index.sources.len() {
        return Err(format!(
            "synthesis requires {} corpus sources, found {}",
            corpus_index.sources.len(),
            corpus_sources.len()
        ));
    }

    let Some(first_source) = corpus_sources.first() else {
        return Err("synthesis requires at least one corpus source".to_string());
    };

    for (source_index, source) in corpus_sources.iter().enumerate() {
        let expected_sample_rate = corpus_index.sources[source_index].sample_rate;
        if source.audio.sample_rate != expected_sample_rate {
            return Err(format!(
                "corpus source {} sample_rate mismatch: expected {}, found {}",
                source_index, expected_sample_rate, source.audio.sample_rate
            ));
        }
    }

    if sequence.steps.is_empty() {
        return Ok(first_source.audio.sample_rate);
    }

    let first_grain = corpus_index
        .grain(sequence.steps[0].selected_grain_index)
        .ok_or_else(|| {
            format!(
                "match step 0 references invalid grain index {}",
                sequence.steps[0].selected_grain_index
            )
        })?;
    let sample_rate = corpus_sources[first_grain.source_index].audio.sample_rate;

    for (step_index, step) in sequence.steps.iter().enumerate() {
        let grain = corpus_index
            .grain(step.selected_grain_index)
            .ok_or_else(|| {
                format!(
                    "match step {step_index} references invalid grain index {}",
                    step.selected_grain_index
                )
            })?;
        let source = corpus_sources.get(grain.source_index).ok_or_else(|| {
            format!(
                "match step {step_index} references invalid source index {}",
                grain.source_index
            )
        })?;

        if source.audio.sample_rate != sample_rate {
            return Err(format!(
                "synthesis requires a single sample_rate, found {} at step 0 and {} at step {}",
                sample_rate, source.audio.sample_rate, step_index
            ));
        }
    }

    Ok(sample_rate)
}

fn scheduled_hop_frames(
    overlap_schedule: OverlapScheduleMode,
    spec: SynthesisFrameSpec,
    step_index: usize,
) -> usize {
    match overlap_schedule {
        OverlapScheduleMode::Fixed => spec.output_hop_frames,
        OverlapScheduleMode::Alternating => {
            if step_index % 2 == 0 {
                spec.output_hop_frames - spec.irregularity_frames
            } else {
                spec.output_hop_frames + spec.irregularity_frames
            }
        }
    }
}

fn build_hann_window(frame_size: usize) -> Vec<f32> {
    use std::f32::consts::PI;

    if frame_size == 0 {
        return Vec::new();
    }
    if frame_size == 1 {
        return vec![1.0];
    }

    let scale = 2.0 * PI / (frame_size - 1) as f32;
    let mut window = Vec::with_capacity(frame_size);

    for index in 0..frame_size {
        window.push(0.5 - 0.5 * (scale * index as f32).cos());
    }

    window
}

fn ms_to_frames(sample_rate: u32, milliseconds: u32) -> usize {
    let rounded = ((sample_rate as u64 * milliseconds as u64) + 500) / 1000;
    rounded.max(1) as usize
}

fn ms_to_optional_frames(sample_rate: u32, milliseconds: u32) -> usize {
    if milliseconds == 0 {
        return 0;
    }

    ms_to_frames(sample_rate, milliseconds)
}

#[cfg(test)]
mod tests {
    use super::{ScheduledGrain, SynthesisFrameSpec, SynthesisPlan, build_window};
    use crate::{
        audio::MonoBuffer,
        config::{EnvelopeAdaptationMode, GainAdaptationMode, OverlapScheduleMode, WindowKind},
        corpus::CorpusSourceFile,
        descriptor::{DescriptorNormalization, DescriptorVector},
        index::{CorpusGrainEntry, CorpusIndex, CorpusSourceInfo},
        matching::{MatchCost, MatchSequence, MatchStep},
        micro_adaptation::MicroAdaptationPlan,
        target::{TargetAnalysis, TargetAnalysisFrame},
    };
    use std::path::PathBuf;

    #[test]
    fn synthesis_frame_spec_converts_plan_to_frames() {
        let plan = SynthesisPlan {
            window: WindowKind::Hann,
            output_hop_ms: 4,
            overlap_schedule: OverlapScheduleMode::Alternating,
            irregularity_ms: 1,
        };

        let spec = SynthesisFrameSpec::from_plan(&plan, 1_000).expect("frame spec");

        assert_eq!(spec.output_hop_frames, 4);
        assert_eq!(spec.irregularity_frames, 1);
    }

    #[test]
    fn build_hann_window_has_zero_edges() {
        let window = build_window(WindowKind::Hann, 4);

        assert_eq!(window.len(), 4);
        assert!(window[0].abs() < 1.0e-6);
        assert!((window[1] - 0.75).abs() < 1.0e-6);
        assert!((window[2] - 0.75).abs() < 1.0e-6);
        assert!(window[3].abs() < 1.0e-6);
    }

    #[test]
    fn fixed_schedule_uses_constant_output_hop() {
        let plan = SynthesisPlan {
            window: WindowKind::Hann,
            output_hop_ms: 2,
            overlap_schedule: OverlapScheduleMode::Fixed,
            irregularity_ms: 0,
        };
        let sequence = test_match_sequence(vec![0, 1, 0]);
        let corpus_index = test_corpus_index();

        let scheduled = plan
            .schedule(1_000, &corpus_index, &sequence)
            .expect("schedule");

        assert_eq!(
            scheduled,
            vec![
                ScheduledGrain {
                    match_step_index: 0,
                    source_index: 0,
                    source_start_frame: 0,
                    len_frames: 4,
                    output_start_frame: 0,
                },
                ScheduledGrain {
                    match_step_index: 1,
                    source_index: 0,
                    source_start_frame: 2,
                    len_frames: 4,
                    output_start_frame: 2,
                },
                ScheduledGrain {
                    match_step_index: 2,
                    source_index: 0,
                    source_start_frame: 0,
                    len_frames: 4,
                    output_start_frame: 4,
                },
            ]
        );
    }

    #[test]
    fn alternating_schedule_varies_hop_deterministically() {
        let plan = SynthesisPlan {
            window: WindowKind::Hann,
            output_hop_ms: 4,
            overlap_schedule: OverlapScheduleMode::Alternating,
            irregularity_ms: 1,
        };
        let sequence = test_match_sequence(vec![0, 1, 0, 1]);
        let corpus_index = test_corpus_index();

        let scheduled = plan
            .schedule(1_000, &corpus_index, &sequence)
            .expect("schedule");

        let output_starts: Vec<_> = scheduled
            .iter()
            .map(|grain| grain.output_start_frame)
            .collect();

        assert_eq!(output_starts, vec![0, 3, 8, 11]);
    }

    #[test]
    fn synthesize_match_sequence_overlap_adds_windowed_grains() {
        let plan = SynthesisPlan {
            window: WindowKind::Hann,
            output_hop_ms: 2,
            overlap_schedule: OverlapScheduleMode::Fixed,
            irregularity_ms: 0,
        };
        let corpus_sources = vec![CorpusSourceFile {
            path: PathBuf::from("source.wav"),
            audio: MonoBuffer::new(1_000, vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0]).expect("source"),
        }];
        let corpus_index = test_corpus_index();
        let sequence = test_match_sequence(vec![0, 1]);

        let output = plan
            .synthesize(&corpus_sources, &corpus_index, &sequence)
            .expect("synthesis");

        assert_eq!(output.scheduled_grains.len(), 2);
        assert_eq!(output.audio.sample_rate, 1_000);
        assert_eq!(output.audio.samples.len(), 6);

        let expected = [0.0, 0.75, 0.75, 0.75, 0.75, 0.0];
        for (actual, expected) in output.audio.samples.iter().zip(expected) {
            assert!((actual - expected).abs() < 1.0e-6);
        }
    }

    #[test]
    fn synthesize_match_sequence_rejects_mixed_sample_rates() {
        let plan = SynthesisPlan {
            window: WindowKind::Hann,
            output_hop_ms: 2,
            overlap_schedule: OverlapScheduleMode::Fixed,
            irregularity_ms: 0,
        };
        let corpus_sources = vec![
            CorpusSourceFile {
                path: PathBuf::from("a.wav"),
                audio: MonoBuffer::new(1_000, vec![0.0; 6]).expect("source"),
            },
            CorpusSourceFile {
                path: PathBuf::from("b.wav"),
                audio: MonoBuffer::new(2_000, vec![0.0; 6]).expect("source"),
            },
        ];
        let corpus_index = CorpusIndex {
            sources: vec![
                CorpusSourceInfo {
                    path: PathBuf::from("a.wav"),
                    sample_rate: 1_000,
                    total_frames: 6,
                },
                CorpusSourceInfo {
                    path: PathBuf::from("b.wav"),
                    sample_rate: 2_000,
                    total_frames: 6,
                },
            ],
            grains: vec![
                CorpusGrainEntry {
                    source_index: 0,
                    start_frame: 0,
                    len_frames: 4,
                },
                CorpusGrainEntry {
                    source_index: 1,
                    start_frame: 0,
                    len_frames: 4,
                },
            ],
            raw_descriptors: vec![
                DescriptorVector::new([0.0, 0.0, 0.0, 0.0, 0.0]),
                DescriptorVector::new([1.0, 0.0, 0.0, 0.0, 0.0]),
            ],
            normalized_descriptors: vec![
                DescriptorVector::new([0.0, 0.0, 0.0, 0.0, 0.0]),
                DescriptorVector::new([1.0, 0.0, 0.0, 0.0, 0.0]),
            ],
            normalization: DescriptorNormalization {
                mean: [0.0; 5],
                scale: [1.0; 5],
            },
        };
        let sequence = test_match_sequence(vec![0, 1]);

        let error = plan
            .synthesize(&corpus_sources, &corpus_index, &sequence)
            .expect_err("mixed sample rates should fail");

        assert_eq!(
            error,
            "synthesis requires a single sample_rate, found 1000 at step 0 and 2000 at step 1"
        );
    }

    #[test]
    fn synthesize_with_micro_adaptation_matches_target_frame_rms_before_windowing() {
        let plan = SynthesisPlan {
            window: WindowKind::Hann,
            output_hop_ms: 4,
            overlap_schedule: OverlapScheduleMode::Fixed,
            irregularity_ms: 0,
        };
        let corpus_sources = vec![CorpusSourceFile {
            path: PathBuf::from("source.wav"),
            audio: MonoBuffer::new(1_000, vec![0.25, -0.25, 0.25, -0.25]).expect("source"),
        }];
        let corpus_index = CorpusIndex {
            sources: vec![CorpusSourceInfo {
                path: PathBuf::from("source.wav"),
                sample_rate: 1_000,
                total_frames: 4,
            }],
            grains: vec![CorpusGrainEntry {
                source_index: 0,
                start_frame: 0,
                len_frames: 4,
            }],
            raw_descriptors: vec![DescriptorVector::new([0.0; 5])],
            normalized_descriptors: vec![DescriptorVector::new([0.0; 5])],
            normalization: DescriptorNormalization {
                mean: [0.0; 5],
                scale: [1.0; 5],
            },
        };
        let sequence = test_match_sequence(vec![0]);
        let target = test_target_analysis(vec![0.5], 4);
        let micro_plan = MicroAdaptationPlan {
            gain: GainAdaptationMode::MatchTargetRms,
            envelope: EnvelopeAdaptationMode::Off,
        };

        let output = plan
            .synthesize_with_micro_adaptation(
                &corpus_sources,
                &corpus_index,
                &sequence,
                &micro_plan,
                &target,
            )
            .expect("synthesis");

        let expected = [0.0, -0.375, 0.375, 0.0];
        for (actual, expected) in output.audio.samples.iter().zip(expected) {
            assert!((actual - expected).abs() < 1.0e-6);
        }
    }

    #[test]
    fn synthesize_with_micro_adaptation_applies_target_envelope_after_overlap_add() {
        let plan = SynthesisPlan {
            window: WindowKind::Hann,
            output_hop_ms: 4,
            overlap_schedule: OverlapScheduleMode::Fixed,
            irregularity_ms: 0,
        };
        let corpus_sources = vec![CorpusSourceFile {
            path: PathBuf::from("source.wav"),
            audio: MonoBuffer::new(1_000, vec![1.0, 1.0, 1.0, 1.0]).expect("source"),
        }];
        let corpus_index = CorpusIndex {
            sources: vec![CorpusSourceInfo {
                path: PathBuf::from("source.wav"),
                sample_rate: 1_000,
                total_frames: 4,
            }],
            grains: vec![CorpusGrainEntry {
                source_index: 0,
                start_frame: 0,
                len_frames: 4,
            }],
            raw_descriptors: vec![DescriptorVector::new([0.0; 5])],
            normalized_descriptors: vec![DescriptorVector::new([0.0; 5])],
            normalization: DescriptorNormalization {
                mean: [0.0; 5],
                scale: [1.0; 5],
            },
        };
        let sequence = test_match_sequence(vec![0]);
        let target = test_target_analysis(vec![0.25, 0.5], 2);
        let micro_plan = MicroAdaptationPlan {
            gain: GainAdaptationMode::Off,
            envelope: EnvelopeAdaptationMode::InheritCarrierRms,
        };

        let output = plan
            .synthesize_with_micro_adaptation(
                &corpus_sources,
                &corpus_index,
                &sequence,
                &micro_plan,
                &target,
            )
            .expect("synthesis");

        let first_segment_rms = ((output.audio.samples[0] * output.audio.samples[0]
            + output.audio.samples[1] * output.audio.samples[1])
            / 2.0)
            .sqrt();
        let second_segment_rms = ((output.audio.samples[2] * output.audio.samples[2]
            + output.audio.samples[3] * output.audio.samples[3])
            / 2.0)
            .sqrt();

        assert!((first_segment_rms - 0.25).abs() < 1.0e-6);
        assert!((second_segment_rms - 0.5).abs() < 1.0e-6);
    }

    fn test_match_sequence(selected_grain_indices: Vec<usize>) -> MatchSequence {
        MatchSequence {
            total_cost: 0.0,
            steps: selected_grain_indices
                .into_iter()
                .enumerate()
                .map(|(target_frame_index, selected_grain_index)| MatchStep {
                    target_frame_index,
                    selected_grain_index,
                    cost: MatchCost {
                        target_distance: 0.0,
                        transition_cost: 0.0,
                        transition_descriptor_distance: 0.0,
                        transition_seek_distance: 0.0,
                        source_switch_cost: 0.0,
                        total_cost: 0.0,
                    },
                })
                .collect(),
        }
    }

    fn test_corpus_index() -> CorpusIndex {
        CorpusIndex {
            sources: vec![CorpusSourceInfo {
                path: PathBuf::from("source.wav"),
                sample_rate: 1_000,
                total_frames: 6,
            }],
            grains: vec![
                CorpusGrainEntry {
                    source_index: 0,
                    start_frame: 0,
                    len_frames: 4,
                },
                CorpusGrainEntry {
                    source_index: 0,
                    start_frame: 2,
                    len_frames: 4,
                },
            ],
            raw_descriptors: vec![
                DescriptorVector::new([0.0, 0.0, 0.0, 0.0, 0.0]),
                DescriptorVector::new([1.0, 0.0, 0.0, 0.0, 0.0]),
            ],
            normalized_descriptors: vec![
                DescriptorVector::new([0.0, 0.0, 0.0, 0.0, 0.0]),
                DescriptorVector::new([1.0, 0.0, 0.0, 0.0, 0.0]),
            ],
            normalization: DescriptorNormalization {
                mean: [0.0; 5],
                scale: [1.0; 5],
            },
        }
    }

    fn test_target_analysis(frame_rms: Vec<f32>, hop_size_frames: usize) -> TargetAnalysis {
        TargetAnalysis {
            sample_rate: 1_000,
            original_channels: 1,
            total_frames: frame_rms.len() * hop_size_frames,
            frame_size_frames: hop_size_frames,
            hop_size_frames,
            frames: frame_rms
                .into_iter()
                .enumerate()
                .map(|(index, rms)| TargetAnalysisFrame {
                    start_frame: index * hop_size_frames,
                    len_frames: hop_size_frames,
                    rms,
                    raw_descriptor: DescriptorVector::new([0.0; 5]),
                    normalized_descriptor: DescriptorVector::new([0.0; 5]),
                })
                .collect(),
        }
    }
}
