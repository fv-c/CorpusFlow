use crate::config::{SynthesisConfig, WindowKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SynthesisPlan {
    pub window: WindowKind,
    pub output_hop_ms: u32,
}

impl From<&SynthesisConfig> for SynthesisPlan {
    fn from(config: &SynthesisConfig) -> Self {
        Self {
            window: config.window,
            output_hop_ms: config.output_hop_ms,
        }
    }
}
