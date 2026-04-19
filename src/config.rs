use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub corpus: CorpusConfig,
    pub target: TargetConfig,
    pub matching: MatchingConfig,
    pub micro_adaptation: MicroAdaptationConfig,
    pub synthesis: SynthesisConfig,
    pub rendering: RenderingConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            corpus: CorpusConfig::default(),
            target: TargetConfig::default(),
            matching: MatchingConfig::default(),
            micro_adaptation: MicroAdaptationConfig::default(),
            synthesis: SynthesisConfig::default(),
            rendering: RenderingConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.corpus.grain_size_ms == 0 {
            return Err("corpus grain_size_ms must be > 0".to_string());
        }
        if self.corpus.grain_hop_ms == 0 {
            return Err("corpus grain_hop_ms must be > 0".to_string());
        }
        if self.target.frame_size_ms == 0 || self.target.hop_size_ms == 0 {
            return Err("target frame_size_ms and hop_size_ms must be > 0".to_string());
        }
        if !self.matching.alpha.is_finite()
            || !self.matching.beta.is_finite()
            || !self.matching.transition_descriptor_weight.is_finite()
            || !self.matching.transition_seek_weight.is_finite()
            || !self.matching.source_switch_penalty.is_finite()
        {
            return Err("matching weights must be finite".to_string());
        }
        if self.synthesis.output_hop_ms == 0 {
            return Err("synthesis output_hop_ms must be > 0".to_string());
        }
        if self.synthesis.overlap_schedule == OverlapScheduleMode::Fixed
            && self.synthesis.irregularity_ms != 0
        {
            return Err(
                "fixed synthesis overlap_schedule requires irregularity_ms = 0".to_string(),
            );
        }
        if self.synthesis.overlap_schedule == OverlapScheduleMode::Alternating {
            if self.synthesis.irregularity_ms == 0 {
                return Err(
                    "alternating synthesis overlap_schedule requires irregularity_ms > 0"
                        .to_string(),
                );
            }
            if self.synthesis.irregularity_ms >= self.synthesis.output_hop_ms {
                return Err(
                    "synthesis irregularity_ms must be smaller than output_hop_ms".to_string(),
                );
            }
        }
        self.rendering.validate()?;

        Ok(())
    }

    pub fn summary(&self) -> String {
        format!(
            "corpus(grain={}ms hop={}ms) target(frame={}ms hop={}ms) matching(alpha={}, beta={}) micro(gain={}, envelope={}) synthesis(output_hop={}ms schedule={} irregularity={}ms) rendering({})",
            self.corpus.grain_size_ms,
            self.corpus.grain_hop_ms,
            self.target.frame_size_ms,
            self.target.hop_size_ms,
            self.matching.alpha,
            self.matching.beta,
            self.micro_adaptation.gain.as_str(),
            self.micro_adaptation.envelope.as_str(),
            self.synthesis.output_hop_ms,
            self.synthesis.overlap_schedule.as_str(),
            self.synthesis.irregularity_ms,
            self.rendering.summary(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpusConfig {
    pub root: String,
    pub grain_size_ms: u32,
    pub grain_hop_ms: u32,
    pub mono_only: bool,
}

impl Default for CorpusConfig {
    fn default() -> Self {
        Self {
            root: String::new(),
            grain_size_ms: 100,
            grain_hop_ms: 50,
            mono_only: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetConfig {
    pub path: String,
    pub frame_size_ms: u32,
    pub hop_size_ms: u32,
}

impl Default for TargetConfig {
    fn default() -> Self {
        Self {
            path: String::new(),
            frame_size_ms: 100,
            hop_size_ms: 50,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchingConfig {
    pub alpha: f32,
    pub beta: f32,
    pub transition_descriptor_weight: f32,
    pub transition_seek_weight: f32,
    pub source_switch_penalty: f32,
}

impl Default for MatchingConfig {
    fn default() -> Self {
        Self {
            alpha: 1.0,
            beta: 0.25,
            transition_descriptor_weight: 1.0,
            transition_seek_weight: 0.5,
            source_switch_penalty: 0.25,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MicroAdaptationConfig {
    pub gain: GainAdaptationMode,
    pub envelope: EnvelopeAdaptationMode,
}

impl Default for MicroAdaptationConfig {
    fn default() -> Self {
        Self {
            gain: GainAdaptationMode::Off,
            envelope: EnvelopeAdaptationMode::Off,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SynthesisConfig {
    pub window: WindowKind,
    pub output_hop_ms: u32,
    pub overlap_schedule: OverlapScheduleMode,
    pub irregularity_ms: u32,
}

impl Default for SynthesisConfig {
    fn default() -> Self {
        Self {
            window: WindowKind::Hann,
            output_hop_ms: 50,
            overlap_schedule: OverlapScheduleMode::Fixed,
            irregularity_ms: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderingConfig {
    pub mode: RenderMode,
    pub stereo_routing: StereoRouting,
    pub post_convolution: PostConvolutionConfig,
}

impl Default for RenderingConfig {
    fn default() -> Self {
        Self {
            mode: RenderMode::Mono,
            stereo_routing: StereoRouting::DuplicateMono,
            post_convolution: PostConvolutionConfig::default(),
        }
    }
}

impl RenderingConfig {
    pub fn validate(&self) -> Result<(), String> {
        self.post_convolution.validate()
    }

    pub fn summary(&self) -> String {
        format!(
            "mode={} stereo_routing={} convolution={}",
            self.mode.as_str(),
            self.stereo_routing.as_str(),
            self.post_convolution.summary(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostConvolutionConfig {
    pub enabled: bool,
    pub impulse_response: Vec<f32>,
    pub dry_mix: f32,
    pub wet_mix: f32,
    pub normalize_output: bool,
}

impl Default for PostConvolutionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            impulse_response: Vec::new(),
            dry_mix: 1.0,
            wet_mix: 1.0,
            normalize_output: true,
        }
    }
}

impl PostConvolutionConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !self.dry_mix.is_finite() || !self.wet_mix.is_finite() {
            return Err("rendering dry_mix and wet_mix must be finite".to_string());
        }
        if !(0.0..=1.0).contains(&self.dry_mix) || !(0.0..=1.0).contains(&self.wet_mix) {
            return Err("rendering dry_mix and wet_mix must be within 0.0..=1.0".to_string());
        }
        if self.enabled && self.impulse_response.is_empty() {
            return Err(
                "enabled post_convolution requires a non-empty impulse_response".to_string(),
            );
        }
        if self.impulse_response.iter().any(|tap| !tap.is_finite()) {
            return Err("rendering impulse_response must contain only finite taps".to_string());
        }

        Ok(())
    }

    pub fn summary(&self) -> String {
        if !self.enabled {
            return "off".to_string();
        }

        format!(
            "on(ir_len={} dry={} wet={} normalize={})",
            self.impulse_response.len(),
            self.dry_mix,
            self.wet_mix,
            self.normalize_output,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowKind {
    Hann,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverlapScheduleMode {
    Fixed,
    Alternating,
}

impl OverlapScheduleMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::Alternating => "alternating",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GainAdaptationMode {
    Off,
    MatchTargetRms,
}

impl GainAdaptationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::MatchTargetRms => "match-target-rms",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnvelopeAdaptationMode {
    Off,
    InheritCarrierRms,
}

impl EnvelopeAdaptationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::InheritCarrierRms => "inherit-carrier-rms",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderMode {
    Mono,
    Stereo,
    AmbisonicsReserved,
}

impl RenderMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mono => "mono",
            Self::Stereo => "stereo",
            Self::AmbisonicsReserved => "ambisonics-reserved",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StereoRouting {
    DuplicateMono,
}

impl StereoRouting {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateMono => "duplicate-mono",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AppConfig, EnvelopeAdaptationMode, GainAdaptationMode, MatchingConfig, OverlapScheduleMode,
        PostConvolutionConfig,
    };

    #[test]
    fn default_config_is_valid() {
        let config = AppConfig::default();
        assert_eq!(config.validate(), Ok(()));
    }

    #[test]
    fn invalid_grain_size_is_rejected() {
        let mut config = AppConfig::default();
        config.corpus.grain_size_ms = 0;

        let error = config.validate().expect_err("config should be invalid");
        assert_eq!(error, "corpus grain_size_ms must be > 0");
    }

    #[test]
    fn invalid_matching_weights_are_rejected() {
        let mut config = AppConfig::default();
        config.matching = MatchingConfig {
            alpha: f32::NAN,
            beta: 0.25,
            transition_descriptor_weight: 1.0,
            transition_seek_weight: 0.5,
            source_switch_penalty: 0.25,
        };

        let error = config.validate().expect_err("config should be invalid");
        assert_eq!(error, "matching weights must be finite");
    }

    #[test]
    fn invalid_synthesis_output_hop_is_rejected() {
        let mut config = AppConfig::default();
        config.synthesis.output_hop_ms = 0;

        let error = config.validate().expect_err("config should be invalid");
        assert_eq!(error, "synthesis output_hop_ms must be > 0");
    }

    #[test]
    fn fixed_synthesis_schedule_rejects_non_zero_irregularity() {
        let mut config = AppConfig::default();
        config.synthesis.irregularity_ms = 5;

        let error = config.validate().expect_err("config should be invalid");
        assert_eq!(
            error,
            "fixed synthesis overlap_schedule requires irregularity_ms = 0"
        );
    }

    #[test]
    fn alternating_synthesis_schedule_requires_positive_irregularity() {
        let mut config = AppConfig::default();
        config.synthesis.overlap_schedule = OverlapScheduleMode::Alternating;

        let error = config.validate().expect_err("config should be invalid");
        assert_eq!(
            error,
            "alternating synthesis overlap_schedule requires irregularity_ms > 0"
        );
    }

    #[test]
    fn alternating_synthesis_schedule_rejects_large_irregularity() {
        let mut config = AppConfig::default();
        config.synthesis.overlap_schedule = OverlapScheduleMode::Alternating;
        config.synthesis.irregularity_ms = config.synthesis.output_hop_ms;

        let error = config.validate().expect_err("config should be invalid");
        assert_eq!(
            error,
            "synthesis irregularity_ms must be smaller than output_hop_ms"
        );
    }

    #[test]
    fn summary_includes_micro_adaptation_modes() {
        let mut config = AppConfig::default();
        config.micro_adaptation.gain = GainAdaptationMode::MatchTargetRms;
        config.micro_adaptation.envelope = EnvelopeAdaptationMode::InheritCarrierRms;
        config.synthesis.overlap_schedule = OverlapScheduleMode::Alternating;
        config.synthesis.irregularity_ms = 5;
        config.rendering.post_convolution = PostConvolutionConfig {
            enabled: true,
            impulse_response: vec![1.0, 0.25],
            dry_mix: 0.5,
            wet_mix: 1.0,
            normalize_output: false,
        };

        let summary = config.summary();

        assert!(summary.contains("micro(gain=match-target-rms, envelope=inherit-carrier-rms)"));
        assert!(
            summary.contains("synthesis(output_hop=50ms schedule=alternating irregularity=5ms)")
        );
        assert!(summary.contains(
            "rendering(mode=mono stereo_routing=duplicate-mono convolution=on(ir_len=2 dry=0.5 wet=1 normalize=false))"
        ));
    }

    #[test]
    fn rendering_rejects_out_of_range_post_convolution_mix() {
        let mut config = AppConfig::default();
        config.rendering.post_convolution.dry_mix = 1.5;

        let error = config.validate().expect_err("config should be invalid");
        assert_eq!(
            error,
            "rendering dry_mix and wet_mix must be within 0.0..=1.0"
        );
    }

    #[test]
    fn rendering_rejects_enabled_post_convolution_without_impulse_response() {
        let mut config = AppConfig::default();
        config.rendering.post_convolution.enabled = true;

        let error = config.validate().expect_err("config should be invalid");
        assert_eq!(
            error,
            "enabled post_convolution requires a non-empty impulse_response"
        );
    }
}
