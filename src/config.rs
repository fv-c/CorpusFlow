use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    pub fn from_json_str(json: &str) -> Result<Self, String> {
        let config = serde_json::from_str::<Self>(json)
            .map_err(|error| format!("failed to parse config JSON: {error}"))?;
        config.validate()?;
        Ok(config)
    }

    pub fn from_json_file<P>(path: P) -> Result<Self, String>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let json = fs::read_to_string(path)
            .map_err(|error| format!("failed to read config `{}`: {error}", path.display()))?;

        Self::from_json_str(&json)
            .map_err(|error| format!("invalid config `{}`: {error}", path.display()))
    }

    pub fn to_pretty_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self)
            .map_err(|error| format!("failed to serialize config JSON: {error}"))
    }

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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct RenderingConfig {
    pub output_sample_rate: u32,
    pub mode: RenderMode,
    pub stereo_routing: StereoRouting,
    pub post_convolution: PostConvolutionConfig,
    pub ambisonics: AmbisonicsConfig,
}

impl Default for RenderingConfig {
    fn default() -> Self {
        Self {
            output_sample_rate: 48_000,
            mode: RenderMode::Mono,
            stereo_routing: StereoRouting::DuplicateMono,
            post_convolution: PostConvolutionConfig::default(),
            ambisonics: AmbisonicsConfig::default(),
        }
    }
}

impl RenderingConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.output_sample_rate == 0 {
            return Err("rendering output_sample_rate must be > 0".to_string());
        }
        self.post_convolution.validate()?;
        self.ambisonics.validate_for_mode(self.mode)
    }

    pub fn summary(&self) -> String {
        format!(
            "sample_rate={} mode={} stereo_routing={} ambisonics={} convolution={}",
            self.output_sample_rate,
            self.mode.as_str(),
            self.stereo_routing.as_str(),
            self.ambisonics.summary(),
            self.post_convolution.summary(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AmbisonicsConfig {
    pub positioning_json_path: String,
}

impl Default for AmbisonicsConfig {
    fn default() -> Self {
        Self {
            positioning_json_path: String::new(),
        }
    }
}

impl AmbisonicsConfig {
    pub fn validate_for_mode(&self, mode: RenderMode) -> Result<(), String> {
        if mode != RenderMode::AmbisonicsReserved {
            return Ok(());
        }

        let path = self.positioning_json_path.trim();
        if path.is_empty() {
            return Err(
                "ambisonics rendering requires ambisonics.positioning_json_path".to_string(),
            );
        }

        let spec = load_ambisonics_positioning_spec(path)?;
        spec.validate()
    }

    pub fn summary(&self) -> String {
        if self.positioning_json_path.trim().is_empty() {
            return "off".to_string();
        }

        format!("json={}", self.positioning_json_path)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostConvolutionConfig {
    pub enabled: bool,
    pub source: PostConvolutionSource,
    pub audio_path: String,
    pub dry_mix: f32,
    pub wet_mix: f32,
    pub normalize_output: bool,
}

impl Default for PostConvolutionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            source: PostConvolutionSource::Target,
            audio_path: String::new(),
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
        if self.enabled
            && self.source == PostConvolutionSource::AudioFile
            && self.audio_path.trim().is_empty()
        {
            return Err(
                "enabled post_convolution with source=audio-file requires a non-empty audio_path"
                    .to_string(),
            );
        }

        Ok(())
    }

    pub fn summary(&self) -> String {
        if !self.enabled {
            return "off".to_string();
        }

        match self.source {
            PostConvolutionSource::Target => format!(
                "on(source=target dry={} wet={} normalize={})",
                self.dry_mix, self.wet_mix, self.normalize_output,
            ),
            PostConvolutionSource::AudioFile => format!(
                "on(source=audio-file path={} dry={} wet={} normalize={})",
                self.audio_path, self.dry_mix, self.wet_mix, self.normalize_output,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PostConvolutionSource {
    Target,
    AudioFile,
}

impl PostConvolutionSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Target => "target",
            Self::AudioFile => "audio-file",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowKind {
    Hann,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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
#[serde(rename_all = "kebab-case")]
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
#[serde(rename_all = "kebab-case")]
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
#[serde(rename_all = "kebab-case")]
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
#[serde(rename_all = "kebab-case")]
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

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct AmbisonicsPositioningSpec {
    trajectory: Vec<AmbisonicsTrajectoryWaypoint>,
    jitter: AmbisonicsPositionJitter,
}

impl AmbisonicsPositioningSpec {
    fn validate(&self) -> Result<(), String> {
        if self.trajectory.is_empty() {
            return Err(
                "ambisonics positioning trajectory must contain at least one waypoint".to_string(),
            );
        }

        let mut previous_time_ms = None;
        for waypoint in &self.trajectory {
            waypoint.validate()?;

            if let Some(previous) = previous_time_ms {
                if waypoint.time_ms <= previous {
                    return Err(
                        "ambisonics positioning trajectory time_ms must be strictly increasing"
                            .to_string(),
                    );
                }
            } else if waypoint.time_ms != 0 {
                return Err(
                    "ambisonics positioning trajectory must start at time_ms = 0".to_string(),
                );
            }

            previous_time_ms = Some(waypoint.time_ms);
        }

        self.jitter.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct AmbisonicsTrajectoryWaypoint {
    time_ms: u32,
    azimuth_deg: f32,
    elevation_deg: f32,
    distance: f32,
}

impl AmbisonicsTrajectoryWaypoint {
    fn validate(&self) -> Result<(), String> {
        if !self.azimuth_deg.is_finite()
            || !self.elevation_deg.is_finite()
            || !self.distance.is_finite()
        {
            return Err("ambisonics positioning waypoints must contain finite values".to_string());
        }
        if !(-90.0..=90.0).contains(&self.elevation_deg) {
            return Err(
                "ambisonics positioning elevation_deg must be within -90.0..=90.0".to_string(),
            );
        }
        if self.distance < 0.0 {
            return Err("ambisonics positioning distance must be >= 0.0".to_string());
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct AmbisonicsPositionJitter {
    azimuth_deg: f32,
    elevation_deg: f32,
    distance: f32,
}

impl AmbisonicsPositionJitter {
    fn validate(&self) -> Result<(), String> {
        if !self.azimuth_deg.is_finite()
            || !self.elevation_deg.is_finite()
            || !self.distance.is_finite()
        {
            return Err("ambisonics positioning jitter must contain finite values".to_string());
        }
        if self.azimuth_deg < 0.0 || self.elevation_deg < 0.0 || self.distance < 0.0 {
            return Err("ambisonics positioning jitter values must be >= 0.0".to_string());
        }

        Ok(())
    }
}

fn load_ambisonics_positioning_spec<P>(path: P) -> Result<AmbisonicsPositioningSpec, String>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    let json = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read ambisonics positioning JSON `{}`: {error}",
            path.display()
        )
    })?;

    serde_json::from_str(&json).map_err(|error| {
        format!(
            "failed to parse ambisonics positioning JSON `{}`: {error}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        AppConfig, EnvelopeAdaptationMode, GainAdaptationMode, MatchingConfig, OverlapScheduleMode,
        PostConvolutionConfig, PostConvolutionSource, RenderMode,
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn default_config_is_valid() {
        let config = AppConfig::default();
        assert_eq!(config.validate(), Ok(()));
    }

    #[test]
    fn default_config_roundtrips_through_pretty_json() {
        let config = AppConfig::default();
        let json = config.to_pretty_json().expect("config should serialize");
        let parsed = AppConfig::from_json_str(&json).expect("config should parse");

        assert_eq!(parsed, config);
        assert!(json.contains("\"window\": \"hann\""));
        assert!(json.contains("\"mode\": \"mono\""));
        assert!(json.contains("\"output_sample_rate\": 48000"));
        assert!(json.contains("\"source\": \"target\""));
    }

    #[test]
    fn config_json_rejects_unknown_fields() {
        let error = AppConfig::from_json_str(
            r#"{
  "corpus": {
    "root": "",
    "grain_size_ms": 100,
    "grain_hop_ms": 50,
    "mono_only": true
  },
  "target": {
    "path": "",
    "frame_size_ms": 100,
    "hop_size_ms": 50
  },
  "matching": {
    "alpha": 1.0,
    "beta": 0.25,
    "transition_descriptor_weight": 1.0,
    "transition_seek_weight": 0.5,
    "source_switch_penalty": 0.25
  },
  "micro_adaptation": {
    "gain": "off",
    "envelope": "off"
  },
  "synthesis": {
    "window": "hann",
    "output_hop_ms": 50,
    "overlap_schedule": "fixed",
    "irregularity_ms": 0
  },
  "rendering": {
    "output_sample_rate": 48000,
    "mode": "mono",
    "stereo_routing": "duplicate-mono",
    "post_convolution": {
      "enabled": false,
      "source": "target",
      "audio_path": "",
      "dry_mix": 1.0,
      "wet_mix": 1.0,
      "normalize_output": true
    },
    "ambisonics": {
      "positioning_json_path": ""
    }
  },
  "unexpected": true
}"#,
        )
        .expect_err("unknown fields should fail");

        assert!(error.contains("unknown field `unexpected`"));
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
            source: PostConvolutionSource::AudioFile,
            audio_path: "fixtures/ir.wav".to_string(),
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
            "rendering(sample_rate=48000 mode=mono stereo_routing=duplicate-mono ambisonics=off convolution=on(source=audio-file path=fixtures/ir.wav dry=0.5 wet=1 normalize=false))"
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
    fn rendering_rejects_zero_output_sample_rate() {
        let mut config = AppConfig::default();
        config.rendering.output_sample_rate = 0;

        let error = config.validate().expect_err("config should be invalid");
        assert_eq!(error, "rendering output_sample_rate must be > 0");
    }

    #[test]
    fn rendering_rejects_enabled_audio_file_post_convolution_without_audio_path() {
        let mut config = AppConfig::default();
        config.rendering.post_convolution.enabled = true;
        config.rendering.post_convolution.source = PostConvolutionSource::AudioFile;

        let error = config.validate().expect_err("config should be invalid");
        assert_eq!(
            error,
            "enabled post_convolution with source=audio-file requires a non-empty audio_path"
        );
    }

    #[test]
    fn rendering_accepts_enabled_target_post_convolution_without_audio_path() {
        let mut config = AppConfig::default();
        config.rendering.post_convolution.enabled = true;
        config.rendering.post_convolution.source = PostConvolutionSource::Target;

        assert_eq!(config.validate(), Ok(()));
    }

    #[test]
    fn ambisonics_requires_positioning_json_path() {
        let mut config = AppConfig::default();
        config.rendering.mode = RenderMode::AmbisonicsReserved;

        let error = config.validate().expect_err("config should be invalid");
        assert_eq!(
            error,
            "ambisonics rendering requires ambisonics.positioning_json_path"
        );
    }

    #[test]
    fn ambisonics_requires_positioning_json_with_trajectory_and_jitter() {
        let fixture = TempFixtureDir::new();
        let json_path = fixture.write_text_file(
            "positioning.json",
            r#"{
  "trajectory": [],
  "jitter": {
    "azimuth_deg": 2.0,
    "elevation_deg": 1.0,
    "distance": 0.1
  }
}"#,
        );
        let mut config = AppConfig::default();
        config.rendering.mode = RenderMode::AmbisonicsReserved;
        config.rendering.ambisonics.positioning_json_path =
            json_path.to_string_lossy().into_owned();

        let error = config.validate().expect_err("config should be invalid");
        assert_eq!(
            error,
            "ambisonics positioning trajectory must contain at least one waypoint"
        );
    }

    #[test]
    fn ambisonics_accepts_valid_positioning_json() {
        let fixture = TempFixtureDir::new();
        let json_path = fixture.write_text_file(
            "positioning.json",
            r#"{
  "trajectory": [
    {
      "time_ms": 0,
      "azimuth_deg": 0.0,
      "elevation_deg": 0.0,
      "distance": 1.0
    },
    {
      "time_ms": 250,
      "azimuth_deg": 30.0,
      "elevation_deg": 10.0,
      "distance": 1.2
    }
  ],
  "jitter": {
    "azimuth_deg": 2.0,
    "elevation_deg": 1.0,
    "distance": 0.1
  }
}"#,
        );
        let mut config = AppConfig::default();
        config.rendering.mode = RenderMode::AmbisonicsReserved;
        config.rendering.ambisonics.positioning_json_path =
            json_path.to_string_lossy().into_owned();

        assert_eq!(config.validate(), Ok(()));
    }

    struct TempFixtureDir {
        path: PathBuf,
    }

    impl TempFixtureDir {
        fn new() -> Self {
            let unique = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "corpusflow-config-{}-{}-{}",
                std::process::id(),
                nanos,
                unique
            ));

            fs::create_dir_all(&path).expect("temp fixture dir should be created");
            Self { path }
        }

        fn write_text_file(&self, relative: &str, contents: &str) -> PathBuf {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("fixture parent dir should be created");
            }

            fs::write(&path, contents).expect("fixture file should be written");
            path
        }
    }

    impl Drop for TempFixtureDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
