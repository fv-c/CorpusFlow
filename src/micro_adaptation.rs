use crate::{
    config::{EnvelopeAdaptationMode, GainAdaptationMode, MicroAdaptationConfig},
    target::TargetAnalysis,
};

const RMS_EPSILON: f32 = 1.0e-8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MicroAdaptationPlan {
    pub gain: GainAdaptationMode,
    pub envelope: EnvelopeAdaptationMode,
}

impl From<&MicroAdaptationConfig> for MicroAdaptationPlan {
    fn from(config: &MicroAdaptationConfig) -> Self {
        Self {
            gain: config.gain,
            envelope: config.envelope,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GainAdjustment {
    pub source_rms: f32,
    pub target_rms: f32,
    pub applied_gain: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CarrierEnvelopeProfile {
    pub hop_size_frames: usize,
    pub frame_rms: Vec<f32>,
}

impl CarrierEnvelopeProfile {
    pub fn from_target_analysis(analysis: &TargetAnalysis) -> Self {
        Self {
            hop_size_frames: analysis.hop_size_frames,
            frame_rms: analysis.frames.iter().map(|frame| frame.rms).collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.frame_rms.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnvelopeAdjustment {
    pub segment_gains: Vec<f32>,
}

pub fn adapt_grain_gain_in_place(
    samples: &mut [f32],
    mode: GainAdaptationMode,
    target_rms: f32,
) -> GainAdjustment {
    let source_rms = root_mean_square(samples);
    let sanitized_target_rms = target_rms.max(0.0);
    let applied_gain = match mode {
        GainAdaptationMode::Off => 1.0,
        GainAdaptationMode::MatchTargetRms if source_rms > RMS_EPSILON => {
            sanitized_target_rms / source_rms
        }
        GainAdaptationMode::MatchTargetRms => 1.0,
    };

    if applied_gain != 1.0 {
        for sample in samples {
            *sample *= applied_gain;
        }
    }

    GainAdjustment {
        source_rms,
        target_rms: sanitized_target_rms,
        applied_gain,
    }
}

pub fn apply_carrier_envelope_in_place(
    samples: &mut [f32],
    mode: EnvelopeAdaptationMode,
    profile: &CarrierEnvelopeProfile,
) -> Result<EnvelopeAdjustment, String> {
    if matches!(mode, EnvelopeAdaptationMode::Off) || profile.is_empty() || samples.is_empty() {
        return Ok(EnvelopeAdjustment {
            segment_gains: Vec::new(),
        });
    }

    if profile.hop_size_frames == 0 {
        return Err("carrier envelope hop_size_frames must be > 0".to_string());
    }

    let mut segment_starts = Vec::with_capacity(profile.frame_rms.len());
    let mut start_frame: usize = 0;

    for _ in &profile.frame_rms {
        segment_starts.push(start_frame);
        start_frame = start_frame.saturating_add(profile.hop_size_frames);
    }

    apply_carrier_envelope_segments_in_place(samples, mode, &segment_starts, &profile.frame_rms)
}

pub fn apply_carrier_envelope_segments_in_place(
    samples: &mut [f32],
    mode: EnvelopeAdaptationMode,
    segment_starts: &[usize],
    target_rms: &[f32],
) -> Result<EnvelopeAdjustment, String> {
    if matches!(mode, EnvelopeAdaptationMode::Off)
        || samples.is_empty()
        || segment_starts.is_empty()
        || target_rms.is_empty()
    {
        return Ok(EnvelopeAdjustment {
            segment_gains: Vec::new(),
        });
    }

    if segment_starts.len() != target_rms.len() {
        return Err("carrier envelope segment starts must match target rms count".to_string());
    }

    let mut segment_gains = Vec::with_capacity(target_rms.len());

    for (segment_index, (&start_frame, &segment_target_rms)) in
        segment_starts.iter().zip(target_rms.iter()).enumerate()
    {
        if segment_index > 0 && start_frame <= segment_starts[segment_index - 1] {
            return Err("carrier envelope segment starts must be strictly increasing".to_string());
        }

        if start_frame >= samples.len() {
            break;
        }

        let end_frame = segment_starts
            .get(segment_index + 1)
            .copied()
            .unwrap_or(samples.len())
            .min(samples.len());
        let gain = match_segment_rms(
            &mut samples[start_frame..end_frame],
            segment_target_rms.max(0.0),
        );
        segment_gains.push(gain);
    }

    Ok(EnvelopeAdjustment { segment_gains })
}

fn match_segment_rms(segment: &mut [f32], target_rms: f32) -> f32 {
    let current_rms = root_mean_square(segment);
    let gain = if current_rms > RMS_EPSILON {
        target_rms / current_rms
    } else {
        1.0
    };

    if gain != 1.0 {
        for sample in segment {
            *sample *= gain;
        }
    }

    gain
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
    use super::{
        CarrierEnvelopeProfile, MicroAdaptationPlan, adapt_grain_gain_in_place,
        apply_carrier_envelope_in_place, apply_carrier_envelope_segments_in_place,
        root_mean_square,
    };
    use crate::{
        config::{EnvelopeAdaptationMode, GainAdaptationMode, MicroAdaptationConfig},
        descriptor::DescriptorVector,
        target::{TargetAnalysis, TargetAnalysisFrame},
    };

    #[test]
    fn micro_adaptation_plan_reflects_config_modes() {
        let config = MicroAdaptationConfig {
            gain: GainAdaptationMode::MatchTargetRms,
            envelope: EnvelopeAdaptationMode::InheritCarrierRms,
        };

        let plan = MicroAdaptationPlan::from(&config);

        assert_eq!(plan.gain, GainAdaptationMode::MatchTargetRms);
        assert_eq!(plan.envelope, EnvelopeAdaptationMode::InheritCarrierRms);
    }

    #[test]
    fn gain_adaptation_matches_target_rms() {
        let mut samples = vec![0.25, -0.25, 0.25, -0.25];

        let adjustment =
            adapt_grain_gain_in_place(&mut samples, GainAdaptationMode::MatchTargetRms, 0.5);

        assert_eq!(adjustment.source_rms, 0.25);
        assert_eq!(adjustment.target_rms, 0.5);
        assert_eq!(adjustment.applied_gain, 2.0);
        assert_eq!(root_mean_square(&samples), 0.5);
    }

    #[test]
    fn gain_adaptation_keeps_silence_when_source_has_no_energy() {
        let mut samples = vec![0.0; 4];

        let adjustment =
            adapt_grain_gain_in_place(&mut samples, GainAdaptationMode::MatchTargetRms, 0.5);

        assert_eq!(adjustment.source_rms, 0.0);
        assert_eq!(adjustment.applied_gain, 1.0);
        assert_eq!(samples, vec![0.0; 4]);
    }

    #[test]
    fn carrier_envelope_profile_uses_target_frame_rms() {
        let analysis = test_target_analysis(vec![0.25, 0.5, 0.75], 2);

        let profile = CarrierEnvelopeProfile::from_target_analysis(&analysis);

        assert_eq!(profile.hop_size_frames, 2);
        assert_eq!(profile.frame_rms, vec![0.25, 0.5, 0.75]);
    }

    #[test]
    fn carrier_envelope_matches_each_hop_segment_rms() {
        let mut samples = vec![1.0; 6];
        let profile = CarrierEnvelopeProfile {
            hop_size_frames: 2,
            frame_rms: vec![1.0, 0.5, 0.25],
        };

        let adjustment = apply_carrier_envelope_in_place(
            &mut samples,
            EnvelopeAdaptationMode::InheritCarrierRms,
            &profile,
        )
        .expect("envelope transfer should succeed");

        assert_eq!(adjustment.segment_gains, vec![1.0, 0.5, 0.25]);
        assert_eq!(root_mean_square(&samples[0..2]), 1.0);
        assert_eq!(root_mean_square(&samples[2..4]), 0.5);
        assert_eq!(root_mean_square(&samples[4..6]), 0.25);
    }

    #[test]
    fn carrier_envelope_extends_last_level_over_tail_samples() {
        let mut samples = vec![1.0; 7];
        let profile = CarrierEnvelopeProfile {
            hop_size_frames: 2,
            frame_rms: vec![1.0, 0.5],
        };

        let adjustment = apply_carrier_envelope_in_place(
            &mut samples,
            EnvelopeAdaptationMode::InheritCarrierRms,
            &profile,
        )
        .expect("tail shaping should succeed");

        assert_eq!(adjustment.segment_gains, vec![1.0, 0.5]);
        assert_eq!(root_mean_square(&samples[4..7]), 0.5);
    }

    #[test]
    fn carrier_envelope_segments_follow_explicit_segment_starts() {
        let mut samples = vec![1.0; 12];

        let adjustment = apply_carrier_envelope_segments_in_place(
            &mut samples,
            EnvelopeAdaptationMode::InheritCarrierRms,
            &[0, 4, 8],
            &[1.0, 0.5, 0.25],
        )
        .expect("segment shaping should succeed");

        assert_eq!(adjustment.segment_gains, vec![1.0, 0.5, 0.25]);
        assert_eq!(root_mean_square(&samples[0..4]), 1.0);
        assert_eq!(root_mean_square(&samples[4..8]), 0.5);
        assert_eq!(root_mean_square(&samples[8..12]), 0.25);
    }

    fn test_target_analysis(frame_rms: Vec<f32>, hop_size_frames: usize) -> TargetAnalysis {
        TargetAnalysis {
            sample_rate: 48_000,
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
