use std::path::{Path, PathBuf};

use crate::{audio::AudioBuffer, config::TargetConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetPlan {
    pub frame_size_ms: u32,
    pub hop_size_ms: u32,
}

impl From<&TargetConfig> for TargetPlan {
    fn from(config: &TargetConfig) -> Self {
        Self {
            frame_size_ms: config.frame_size_ms,
            hop_size_ms: config.hop_size_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TargetInput {
    pub path: PathBuf,
    pub audio: AudioBuffer,
}

pub fn load_target_audio(config: &TargetConfig) -> Result<TargetInput, String> {
    load_target_audio_from_path(&config.path)
}

pub fn load_target_audio_from_path<P>(path: P) -> Result<TargetInput, String>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    if path.as_os_str().is_empty() {
        return Err("target path must not be empty".to_string());
    }

    let audio = crate::audio::read_wav(path)?;
    Ok(TargetInput {
        path: path.to_path_buf(),
        audio,
    })
}
