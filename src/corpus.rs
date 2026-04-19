use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{audio::MonoBuffer, config::CorpusConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusPlan {
    pub grain_size_ms: u32,
    pub grain_hop_ms: u32,
    pub mono_only: bool,
}

impl CorpusPlan {
    pub fn from_config(config: &CorpusConfig) -> Self {
        Self {
            grain_size_ms: config.grain_size_ms,
            grain_hop_ms: config.grain_hop_ms,
            mono_only: config.mono_only,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CorpusSourceFile {
    pub path: PathBuf,
    pub audio: MonoBuffer,
}

pub fn load_corpus_sources(config: &CorpusConfig) -> Result<Vec<CorpusSourceFile>, String> {
    load_corpus_sources_from_path(&config.root, config.mono_only)
}

pub fn load_corpus_sources_from_path<P>(
    root: P,
    mono_only: bool,
) -> Result<Vec<CorpusSourceFile>, String>
where
    P: AsRef<Path>,
{
    if !mono_only {
        return Err("phase 01 corpus ingestion requires mono_only=true".to_string());
    }

    let root = root.as_ref();
    if root.as_os_str().is_empty() {
        return Err("corpus root path must not be empty".to_string());
    }

    let files = discover_wav_files(root)?;
    if files.is_empty() {
        return Err(format!("no WAV files found under `{}`", root.display()));
    }

    let mut sources = Vec::with_capacity(files.len());
    for path in files {
        let audio = crate::audio::read_mono_wav(&path)?;
        sources.push(CorpusSourceFile { path, audio });
    }

    Ok(sources)
}

fn discover_wav_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    if root.is_file() {
        if is_wav_file(root) {
            return Ok(vec![root.to_path_buf()]);
        }

        return Err(format!(
            "corpus input `{}` is not a WAV file",
            root.display()
        ));
    }

    if !root.is_dir() {
        return Err(format!(
            "corpus input `{}` does not exist or is not accessible",
            root.display()
        ));
    }

    let mut files = Vec::new();
    collect_wav_files(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_wav_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries = fs::read_dir(dir)
        .map_err(|error| format!("failed to read directory `{}`: {error}", dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to read directory `{}`: {error}", dir.display()))?;

    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();

        if path.is_dir() {
            collect_wav_files(&path, files)?;
        } else if is_wav_file(&path) {
            files.push(path);
        }
    }

    Ok(())
}

fn is_wav_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("wav"))
        .unwrap_or(false)
}
