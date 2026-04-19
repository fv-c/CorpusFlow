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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrainSpec {
    pub sample_rate: u32,
    pub grain_size_frames: usize,
    pub grain_hop_frames: usize,
}

impl GrainSpec {
    pub fn from_plan(plan: &CorpusPlan, sample_rate: u32) -> Result<Self, String> {
        if sample_rate == 0 {
            return Err("grain planning sample_rate must be > 0".to_string());
        }

        Ok(Self {
            sample_rate,
            grain_size_frames: ms_to_frames(sample_rate, plan.grain_size_ms),
            grain_hop_frames: ms_to_frames(sample_rate, plan.grain_hop_ms),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrainSpan {
    pub start_frame: usize,
    pub len_frames: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrainGrid {
    pub total_frames: usize,
    pub grains: Vec<GrainSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CorpusSourceFile {
    pub path: PathBuf,
    pub audio: MonoBuffer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusSourceSegmentation {
    pub source_index: usize,
    pub sample_rate: u32,
    pub total_frames: usize,
    pub grain_size_frames: usize,
    pub grain_hop_frames: usize,
    pub grains: Vec<GrainSpan>,
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

pub fn build_grain_grid(total_frames: usize, spec: GrainSpec) -> GrainGrid {
    if total_frames < spec.grain_size_frames {
        return GrainGrid {
            total_frames,
            grains: Vec::new(),
        };
    }

    let grain_count = 1 + (total_frames - spec.grain_size_frames) / spec.grain_hop_frames;
    let mut grains = Vec::with_capacity(grain_count);
    let mut start_frame = 0;

    while start_frame + spec.grain_size_frames <= total_frames {
        grains.push(GrainSpan {
            start_frame,
            len_frames: spec.grain_size_frames,
        });
        start_frame += spec.grain_hop_frames;
    }

    GrainGrid {
        total_frames,
        grains,
    }
}

pub fn segment_corpus_sources(
    plan: &CorpusPlan,
    sources: &[CorpusSourceFile],
) -> Result<Vec<CorpusSourceSegmentation>, String> {
    let mut segmented = Vec::with_capacity(sources.len());

    for (source_index, source) in sources.iter().enumerate() {
        let spec = GrainSpec::from_plan(plan, source.audio.sample_rate)?;
        let grid = build_grain_grid(source.audio.frame_count(), spec);

        segmented.push(CorpusSourceSegmentation {
            source_index,
            sample_rate: spec.sample_rate,
            total_frames: grid.total_frames,
            grain_size_frames: spec.grain_size_frames,
            grain_hop_frames: spec.grain_hop_frames,
            grains: grid.grains,
        });
    }

    Ok(segmented)
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

fn ms_to_frames(sample_rate: u32, milliseconds: u32) -> usize {
    let rounded = ((sample_rate as u64 * milliseconds as u64) + 500) / 1000;
    rounded.max(1) as usize
}

#[cfg(test)]
mod tests {
    use super::{
        GrainGrid, GrainSpan, GrainSpec, build_grain_grid, ms_to_frames, segment_corpus_sources,
    };
    use crate::audio::MonoBuffer;
    use crate::corpus::{CorpusPlan, CorpusSourceFile};
    use std::path::PathBuf;

    #[test]
    fn converts_milliseconds_to_frames_with_rounding() {
        assert_eq!(ms_to_frames(48_000, 100), 4_800);
        assert_eq!(ms_to_frames(44_100, 7), 309);
    }

    #[test]
    fn grain_grid_emits_full_length_spans_only() {
        let spec = GrainSpec {
            sample_rate: 48_000,
            grain_size_frames: 4,
            grain_hop_frames: 2,
        };

        let grid = build_grain_grid(10, spec);

        assert_eq!(
            grid,
            GrainGrid {
                total_frames: 10,
                grains: vec![
                    GrainSpan {
                        start_frame: 0,
                        len_frames: 4
                    },
                    GrainSpan {
                        start_frame: 2,
                        len_frames: 4
                    },
                    GrainSpan {
                        start_frame: 4,
                        len_frames: 4
                    },
                    GrainSpan {
                        start_frame: 6,
                        len_frames: 4
                    },
                ],
            }
        );
    }

    #[test]
    fn grain_grid_is_empty_when_source_is_shorter_than_grain() {
        let spec = GrainSpec {
            sample_rate: 48_000,
            grain_size_frames: 8,
            grain_hop_frames: 4,
        };

        let grid = build_grain_grid(7, spec);

        assert_eq!(
            grid,
            GrainGrid {
                total_frames: 7,
                grains: Vec::new(),
            }
        );
    }

    #[test]
    fn segment_corpus_sources_keeps_one_grid_per_file() {
        let plan = CorpusPlan {
            grain_size_ms: 100,
            grain_hop_ms: 50,
            mono_only: true,
        };
        let sources = vec![
            CorpusSourceFile {
                path: PathBuf::from("a.wav"),
                audio: MonoBuffer::new(1_000, vec![0.0; 250]).expect("mono buffer"),
            },
            CorpusSourceFile {
                path: PathBuf::from("b.wav"),
                audio: MonoBuffer::new(1_000, vec![0.0; 90]).expect("mono buffer"),
            },
        ];

        let segmented = segment_corpus_sources(&plan, &sources).expect("segmentation should work");

        assert_eq!(segmented.len(), 2);
        assert_eq!(segmented[0].grain_size_frames, 100);
        assert_eq!(segmented[0].grain_hop_frames, 50);
        assert_eq!(
            segmented[0].grains,
            vec![
                GrainSpan {
                    start_frame: 0,
                    len_frames: 100
                },
                GrainSpan {
                    start_frame: 50,
                    len_frames: 100
                },
                GrainSpan {
                    start_frame: 100,
                    len_frames: 100
                },
                GrainSpan {
                    start_frame: 150,
                    len_frames: 100
                },
            ]
        );
        assert!(segmented[1].grains.is_empty());
    }
}
