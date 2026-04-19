use std::path::{Path, PathBuf};

use crate::{
    audio::AudioBuffer,
    config::TargetConfig,
    corpus::CorpusPlan,
    descriptor::{BaselineDescriptorExtractor, DescriptorNormalization, DescriptorVector},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetPlan {
    pub frame_size_ms: u32,
    pub hop_size_ms: u32,
}

impl TargetPlan {
    pub fn validate_alignment(&self, corpus_plan: &CorpusPlan) -> Result<(), String> {
        if self.frame_size_ms != corpus_plan.grain_size_ms {
            return Err(format!(
                "target frame_size_ms must match corpus grain_size_ms, found {} and {}",
                self.frame_size_ms, corpus_plan.grain_size_ms
            ));
        }

        Ok(())
    }

    pub fn analyze(&self, input: &TargetInput) -> Result<TargetAnalysis, String> {
        let mono_samples = mixdown_to_mono(&input.audio);
        let spec = TargetFrameSpec::from_plan(self, input.audio.sample_rate)?;
        let grid = TargetFrameGrid::build(mono_samples.len(), spec);
        let mut extractor =
            BaselineDescriptorExtractor::new(spec.sample_rate, spec.frame_size_frames)?;
        let mut frames = Vec::with_capacity(grid.frames.len());

        for frame in &grid.frames {
            let start = frame.start_frame;
            let end = start + frame.len_frames;
            let raw_descriptor = extractor.extract_frame(&mono_samples[start..end])?;

            frames.push(TargetAnalysisFrame {
                start_frame: frame.start_frame,
                len_frames: frame.len_frames,
                rms: root_mean_square(&mono_samples[start..end]),
                raw_descriptor,
                normalized_descriptor: raw_descriptor,
            });
        }

        Ok(TargetAnalysis {
            sample_rate: spec.sample_rate,
            original_channels: input.audio.channels,
            total_frames: grid.total_frames,
            frame_size_frames: spec.frame_size_frames,
            hop_size_frames: spec.hop_size_frames,
            frames,
        })
    }

    pub fn analyze_against_corpus(
        &self,
        corpus_plan: &CorpusPlan,
        input: &TargetInput,
        normalization: &DescriptorNormalization,
    ) -> Result<TargetAnalysis, String> {
        self.validate_alignment(corpus_plan)?;

        let mut analysis = self.analyze(input)?;
        analysis.apply_normalization(normalization);
        Ok(analysis)
    }
}

impl From<&TargetConfig> for TargetPlan {
    fn from(config: &TargetConfig) -> Self {
        Self {
            frame_size_ms: config.frame_size_ms,
            hop_size_ms: config.hop_size_ms,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetFrameSpec {
    pub sample_rate: u32,
    pub frame_size_frames: usize,
    pub hop_size_frames: usize,
}

impl TargetFrameSpec {
    pub fn from_plan(plan: &TargetPlan, sample_rate: u32) -> Result<Self, String> {
        if sample_rate == 0 {
            return Err("target analysis sample_rate must be > 0".to_string());
        }

        Ok(Self {
            sample_rate,
            frame_size_frames: ms_to_frames(sample_rate, plan.frame_size_ms),
            hop_size_frames: ms_to_frames(sample_rate, plan.hop_size_ms),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetFrameSpan {
    pub start_frame: usize,
    pub len_frames: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFrameGrid {
    pub total_frames: usize,
    pub frames: Vec<TargetFrameSpan>,
}

impl TargetFrameGrid {
    pub fn build(total_frames: usize, spec: TargetFrameSpec) -> Self {
        if total_frames < spec.frame_size_frames {
            return Self {
                total_frames,
                frames: Vec::new(),
            };
        }

        let frame_count = 1 + (total_frames - spec.frame_size_frames) / spec.hop_size_frames;
        let mut frames = Vec::with_capacity(frame_count);
        let mut start_frame = 0;

        while start_frame + spec.frame_size_frames <= total_frames {
            frames.push(TargetFrameSpan {
                start_frame,
                len_frames: spec.frame_size_frames,
            });
            start_frame += spec.hop_size_frames;
        }

        Self {
            total_frames,
            frames,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TargetInput {
    pub path: PathBuf,
    pub audio: AudioBuffer,
}

impl TargetInput {
    pub fn load(config: &TargetConfig) -> Result<Self, String> {
        Self::load_from_path(&config.path)
    }

    pub fn load_from_path<P>(path: P) -> Result<Self, String>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        if path.as_os_str().is_empty() {
            return Err("target path must not be empty".to_string());
        }

        let audio = crate::audio::read_wav(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            audio,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TargetAnalysisFrame {
    pub start_frame: usize,
    pub len_frames: usize,
    pub rms: f32,
    pub raw_descriptor: DescriptorVector,
    pub normalized_descriptor: DescriptorVector,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TargetAnalysis {
    pub sample_rate: u32,
    pub original_channels: u16,
    pub total_frames: usize,
    pub frame_size_frames: usize,
    pub hop_size_frames: usize,
    pub frames: Vec<TargetAnalysisFrame>,
}

impl TargetAnalysis {
    pub fn apply_normalization(&mut self, normalization: &DescriptorNormalization) {
        for frame in &mut self.frames {
            frame.normalized_descriptor = normalization.normalize(frame.raw_descriptor);
        }
    }
}

fn mixdown_to_mono(buffer: &AudioBuffer) -> Vec<f32> {
    if buffer.channels == 1 {
        return buffer.samples.clone();
    }

    let channels = buffer.channels as usize;
    let mut mono = Vec::with_capacity(buffer.frame_count());

    for frame in buffer.samples.chunks_exact(channels) {
        let sample_sum = frame.iter().copied().sum::<f32>();
        mono.push(sample_sum / channels as f32);
    }

    mono
}

fn ms_to_frames(sample_rate: u32, milliseconds: u32) -> usize {
    let rounded = ((sample_rate as u64 * milliseconds as u64) + 500) / 1000;
    rounded.max(1) as usize
}

fn root_mean_square(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let mean_square =
        samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32;
    mean_square.sqrt()
}

#[cfg(test)]
mod tests {
    use super::{TargetFrameGrid, TargetFrameSpan, TargetFrameSpec, TargetInput, TargetPlan};
    use crate::{
        audio::AudioBuffer,
        corpus::CorpusPlan,
        descriptor::{DescriptorNormalization, DescriptorVector},
    };
    use std::path::PathBuf;

    #[test]
    fn validates_target_frame_size_against_corpus_grain_size() {
        let target_plan = TargetPlan {
            frame_size_ms: 80,
            hop_size_ms: 40,
        };
        let corpus_plan = CorpusPlan {
            grain_size_ms: 100,
            grain_hop_ms: 50,
            mono_only: true,
        };

        let error = target_plan
            .validate_alignment(&corpus_plan)
            .expect_err("misaligned target plan should fail");

        assert_eq!(
            error,
            "target frame_size_ms must match corpus grain_size_ms, found 80 and 100"
        );
    }

    #[test]
    fn builds_target_frame_grid_with_full_frames_only() {
        let spec = TargetFrameSpec {
            sample_rate: 48_000,
            frame_size_frames: 4,
            hop_size_frames: 2,
        };

        let grid = TargetFrameGrid::build(9, spec);

        assert_eq!(
            grid,
            TargetFrameGrid {
                total_frames: 9,
                frames: vec![
                    TargetFrameSpan {
                        start_frame: 0,
                        len_frames: 4,
                    },
                    TargetFrameSpan {
                        start_frame: 2,
                        len_frames: 4,
                    },
                    TargetFrameSpan {
                        start_frame: 4,
                        len_frames: 4,
                    },
                ],
            }
        );
    }

    #[test]
    fn analyze_target_input_downmixes_stereo_and_extracts_frames() {
        let plan = TargetPlan {
            frame_size_ms: 100,
            hop_size_ms: 50,
        };
        let input = TargetInput {
            path: PathBuf::from("target.wav"),
            audio: AudioBuffer::new(1_000, 2, interleave_stereo(&[0.0; 160], &[1.0; 160]))
                .expect("audio buffer"),
        };

        let analysis = plan.analyze(&input).expect("analysis should work");

        assert_eq!(analysis.original_channels, 2);
        assert_eq!(analysis.frame_size_frames, 100);
        assert_eq!(analysis.hop_size_frames, 50);
        assert_eq!(analysis.frames.len(), 2);
        assert!(analysis.frames.iter().all(|frame| frame.rms == 0.5));
        assert!(
            analysis
                .frames
                .iter()
                .flat_map(|frame| frame.raw_descriptor.values)
                .all(|value| value.is_finite())
        );
    }

    #[test]
    fn analyze_target_against_corpus_applies_normalization() {
        let target_plan = TargetPlan {
            frame_size_ms: 100,
            hop_size_ms: 50,
        };
        let corpus_plan = CorpusPlan {
            grain_size_ms: 100,
            grain_hop_ms: 50,
            mono_only: true,
        };
        let input = TargetInput {
            path: PathBuf::from("target.wav"),
            audio: AudioBuffer::new(1_000, 1, vec![0.5; 150]).expect("audio buffer"),
        };
        let normalization = DescriptorNormalization::fit(&[
            DescriptorVector::new([0.0, 0.0, 0.0, 0.0, 0.0]),
            DescriptorVector::new([2.0, 4.0, 6.0, 8.0, 10.0]),
        ])
        .expect("normalization");

        let analysis = target_plan
            .analyze_against_corpus(&corpus_plan, &input, &normalization)
            .expect("aligned analysis should work");

        assert_eq!(analysis.frames.len(), 2);
        assert_ne!(
            analysis.frames[0].raw_descriptor,
            analysis.frames[0].normalized_descriptor
        );
    }

    fn interleave_stereo(left: &[f32], right: &[f32]) -> Vec<f32> {
        left.iter()
            .copied()
            .zip(right.iter().copied())
            .flat_map(|(left, right)| [left, right])
            .collect()
    }
}
