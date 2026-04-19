use crate::config::MatchingConfig;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchingModel {
    pub alpha: f32,
    pub beta: f32,
}

impl From<&MatchingConfig> for MatchingModel {
    fn from(config: &MatchingConfig) -> Self {
        Self {
            alpha: config.alpha,
            beta: config.beta,
        }
    }
}
