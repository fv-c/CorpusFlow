use std::path::Path;

use crate::{
    audio::AudioBuffer,
    config::{RenderMode, RenderingConfig},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderPlan {
    pub mode: RenderMode,
}

impl From<&RenderingConfig> for RenderPlan {
    fn from(config: &RenderingConfig) -> Self {
        Self { mode: config.mode }
    }
}

pub fn write_output_wav<P>(path: P, mode: RenderMode, buffer: &AudioBuffer) -> Result<(), String>
where
    P: AsRef<Path>,
{
    match mode {
        RenderMode::Mono if buffer.channels != 1 => Err(format!(
            "mono rendering requires 1 channel, found {}",
            buffer.channels
        )),
        RenderMode::Stereo if buffer.channels != 2 => Err(format!(
            "stereo rendering requires 2 channels, found {}",
            buffer.channels
        )),
        RenderMode::AmbisonicsReserved => {
            Err("ambisonics rendering is reserved for a later phase".to_string())
        }
        _ => crate::audio::write_wav(path, buffer),
    }
}
