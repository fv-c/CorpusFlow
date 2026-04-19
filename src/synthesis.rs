use crate::config::{SynthesisConfig, WindowKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SynthesisPlan {
    pub window: WindowKind,
    pub overlap_percent: u8,
}

impl From<&SynthesisConfig> for SynthesisPlan {
    fn from(config: &SynthesisConfig) -> Self {
        Self {
            window: config.window,
            overlap_percent: config.overlap_percent,
        }
    }
}
