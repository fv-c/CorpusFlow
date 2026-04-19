use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub corpus: CorpusConfig,
    pub target: TargetConfig,
    pub matching: MatchingConfig,
    pub synthesis: SynthesisConfig,
    pub rendering: RenderingConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            corpus: CorpusConfig::default(),
            target: TargetConfig::default(),
            matching: MatchingConfig::default(),
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
        if !self.matching.alpha.is_finite() || !self.matching.beta.is_finite() {
            return Err("matching weights must be finite".to_string());
        }
        if self.synthesis.overlap_percent > 100 {
            return Err("synthesis overlap_percent must be <= 100".to_string());
        }

        Ok(())
    }

    pub fn summary(&self) -> String {
        format!(
            "corpus(grain={}ms hop={}ms) target(frame={}ms hop={}ms) matching(alpha={}, beta={}) rendering({})",
            self.corpus.grain_size_ms,
            self.corpus.grain_hop_ms,
            self.target.frame_size_ms,
            self.target.hop_size_ms,
            self.matching.alpha,
            self.matching.beta,
            self.rendering.mode.as_str(),
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
}

impl Default for MatchingConfig {
    fn default() -> Self {
        Self {
            alpha: 1.0,
            beta: 0.25,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SynthesisConfig {
    pub window: WindowKind,
    pub overlap_percent: u8,
}

impl Default for SynthesisConfig {
    fn default() -> Self {
        Self {
            window: WindowKind::Hann,
            overlap_percent: 50,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderingConfig {
    pub mode: RenderMode,
}

impl Default for RenderingConfig {
    fn default() -> Self {
        Self {
            mode: RenderMode::Mono,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowKind {
    Hann,
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

#[cfg(test)]
mod tests {
    use super::{AppConfig, MatchingConfig};

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
        };

        let error = config.validate().expect_err("config should be invalid");
        assert_eq!(error, "matching weights must be finite");
    }
}
