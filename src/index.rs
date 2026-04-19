use std::path::PathBuf;

use crate::{
    corpus::{CorpusSourceFile, CorpusSourceSegmentation, GrainSpan},
    descriptor::{BaselineDescriptorExtractor, DescriptorNormalization, DescriptorVector},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusSourceInfo {
    pub path: PathBuf,
    pub sample_rate: u32,
    pub total_frames: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorpusGrainEntry {
    pub source_index: usize,
    pub start_frame: usize,
    pub len_frames: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CorpusIndex {
    pub sources: Vec<CorpusSourceInfo>,
    pub grains: Vec<CorpusGrainEntry>,
    pub raw_descriptors: Vec<DescriptorVector>,
    pub normalized_descriptors: Vec<DescriptorVector>,
    pub normalization: DescriptorNormalization,
}

impl CorpusIndex {
    pub fn build(
        sources: &[CorpusSourceFile],
        segmentations: &[CorpusSourceSegmentation],
    ) -> Result<Self, String> {
        if sources.len() != segmentations.len() {
            return Err(format!(
                "corpus indexing requires matching source and segmentation counts, found {} sources and {} segmentations",
                sources.len(),
                segmentations.len()
            ));
        }

        let total_grains = segmentations.iter().map(|item| item.grains.len()).sum();
        if total_grains == 0 {
            return Err("corpus indexing requires at least one grain".to_string());
        }

        let mut index = Self {
            sources: Vec::with_capacity(sources.len()),
            grains: Vec::with_capacity(total_grains),
            raw_descriptors: Vec::with_capacity(total_grains),
            normalized_descriptors: Vec::new(),
            normalization: DescriptorNormalization {
                mean: [0.0; crate::descriptor::BASELINE_DESCRIPTOR_DIMENSIONS],
                scale: [1.0; crate::descriptor::BASELINE_DESCRIPTOR_DIMENSIONS],
            },
        };

        for (source_index, (source, segmentation)) in
            sources.iter().zip(segmentations.iter()).enumerate()
        {
            validate_source_segmentation_pair(source_index, source, segmentation)?;

            index.sources.push(CorpusSourceInfo {
                path: source.path.clone(),
                sample_rate: source.audio.sample_rate,
                total_frames: source.audio.frame_count(),
            });

            let mut extractor = BaselineDescriptorExtractor::new(
                segmentation.sample_rate,
                segmentation.grain_size_frames,
            )?;

            for grain in &segmentation.grains {
                let descriptor = extract_grain_descriptor(source, &mut extractor, *grain)?;
                index.grains.push(CorpusGrainEntry {
                    source_index,
                    start_frame: grain.start_frame,
                    len_frames: grain.len_frames,
                });
                index.raw_descriptors.push(descriptor);
            }
        }

        index.normalization = DescriptorNormalization::fit(&index.raw_descriptors)?;
        index.normalized_descriptors = index.raw_descriptors.clone();
        index
            .normalization
            .normalize_in_place(&mut index.normalized_descriptors);

        Ok(index)
    }

    pub fn len(&self) -> usize {
        self.grains.len()
    }

    pub fn is_empty(&self) -> bool {
        self.grains.is_empty()
    }

    pub fn source(&self, source_index: usize) -> Option<&CorpusSourceInfo> {
        self.sources.get(source_index)
    }

    pub fn grain(&self, grain_index: usize) -> Option<&CorpusGrainEntry> {
        self.grains.get(grain_index)
    }

    pub fn raw_descriptor(&self, grain_index: usize) -> Option<&DescriptorVector> {
        self.raw_descriptors.get(grain_index)
    }

    pub fn normalized_descriptor(&self, grain_index: usize) -> Option<&DescriptorVector> {
        self.normalized_descriptors.get(grain_index)
    }
}

fn validate_source_segmentation_pair(
    expected_source_index: usize,
    source: &CorpusSourceFile,
    segmentation: &CorpusSourceSegmentation,
) -> Result<(), String> {
    if segmentation.source_index != expected_source_index {
        return Err(format!(
            "corpus segmentation source_index mismatch: expected {}, found {}",
            expected_source_index, segmentation.source_index
        ));
    }

    if segmentation.sample_rate != source.audio.sample_rate {
        return Err(format!(
            "corpus segmentation sample_rate mismatch for source {}: expected {}, found {}",
            expected_source_index, source.audio.sample_rate, segmentation.sample_rate
        ));
    }

    if segmentation.total_frames != source.audio.frame_count() {
        return Err(format!(
            "corpus segmentation frame count mismatch for source {}: expected {}, found {}",
            expected_source_index,
            source.audio.frame_count(),
            segmentation.total_frames
        ));
    }

    Ok(())
}

fn extract_grain_descriptor(
    source: &CorpusSourceFile,
    extractor: &mut BaselineDescriptorExtractor,
    grain: GrainSpan,
) -> Result<DescriptorVector, String> {
    let start = grain.start_frame;
    let end = start + grain.len_frames;

    if end > source.audio.samples.len() {
        return Err(format!(
            "grain span [{start}, {end}) exceeds source length {}",
            source.audio.samples.len()
        ));
    }

    extractor.extract_frame(&source.audio.samples[start..end])
}

#[cfg(test)]
mod tests {
    use super::{CorpusGrainEntry, CorpusIndex};
    use crate::{
        audio::MonoBuffer,
        corpus::{CorpusSourceFile, CorpusSourceSegmentation, GrainSpan},
    };
    use std::path::PathBuf;

    #[test]
    fn builds_corpus_index_with_aligned_metadata_and_descriptors() {
        let sources = vec![
            CorpusSourceFile {
                path: PathBuf::from("a.wav"),
                audio: MonoBuffer::new(1_000, alternating_frame(240)).expect("mono buffer"),
            },
            CorpusSourceFile {
                path: PathBuf::from("b.wav"),
                audio: MonoBuffer::new(1_000, ramp_frame(160)).expect("mono buffer"),
            },
        ];
        let segmentations = vec![
            CorpusSourceSegmentation {
                source_index: 0,
                sample_rate: 1_000,
                total_frames: 240,
                grain_size_frames: 100,
                grain_hop_frames: 50,
                grains: vec![
                    GrainSpan {
                        start_frame: 0,
                        len_frames: 100,
                    },
                    GrainSpan {
                        start_frame: 50,
                        len_frames: 100,
                    },
                    GrainSpan {
                        start_frame: 100,
                        len_frames: 100,
                    },
                ],
            },
            CorpusSourceSegmentation {
                source_index: 1,
                sample_rate: 1_000,
                total_frames: 160,
                grain_size_frames: 100,
                grain_hop_frames: 50,
                grains: vec![
                    GrainSpan {
                        start_frame: 0,
                        len_frames: 100,
                    },
                    GrainSpan {
                        start_frame: 50,
                        len_frames: 100,
                    },
                ],
            },
        ];

        let index = CorpusIndex::build(&sources, &segmentations).expect("index should build");

        assert_eq!(index.sources.len(), 2);
        assert_eq!(index.len(), 5);
        assert_eq!(
            index.grain(3),
            Some(&CorpusGrainEntry {
                source_index: 1,
                start_frame: 0,
                len_frames: 100,
            })
        );
        assert_eq!(index.source(1).unwrap().path, PathBuf::from("b.wav"));
        assert_eq!(index.raw_descriptors.len(), 5);
        assert_eq!(index.normalized_descriptors.len(), 5);
        assert!(
            index
                .normalized_descriptor(0)
                .unwrap()
                .values
                .iter()
                .all(|value| value.is_finite())
        );
    }

    #[test]
    fn rejects_empty_grain_set() {
        let sources = vec![CorpusSourceFile {
            path: PathBuf::from("a.wav"),
            audio: MonoBuffer::new(1_000, vec![0.0; 90]).expect("mono buffer"),
        }];
        let segmentations = vec![CorpusSourceSegmentation {
            source_index: 0,
            sample_rate: 1_000,
            total_frames: 90,
            grain_size_frames: 100,
            grain_hop_frames: 50,
            grains: Vec::new(),
        }];

        let error =
            CorpusIndex::build(&sources, &segmentations).expect_err("empty grain set should fail");

        assert_eq!(error, "corpus indexing requires at least one grain");
    }

    #[test]
    fn rejects_source_and_segmentation_count_mismatch() {
        let sources = vec![CorpusSourceFile {
            path: PathBuf::from("a.wav"),
            audio: MonoBuffer::new(1_000, vec![0.0; 100]).expect("mono buffer"),
        }];

        let error = CorpusIndex::build(&sources, &[]).expect_err("count mismatch should fail");

        assert_eq!(
            error,
            "corpus indexing requires matching source and segmentation counts, found 1 sources and 0 segmentations"
        );
    }

    #[test]
    fn rejects_misaligned_segmentation_metadata() {
        let sources = vec![CorpusSourceFile {
            path: PathBuf::from("a.wav"),
            audio: MonoBuffer::new(1_000, vec![0.0; 100]).expect("mono buffer"),
        }];
        let segmentations = vec![CorpusSourceSegmentation {
            source_index: 1,
            sample_rate: 1_000,
            total_frames: 100,
            grain_size_frames: 100,
            grain_hop_frames: 50,
            grains: vec![GrainSpan {
                start_frame: 0,
                len_frames: 100,
            }],
        }];

        let error = CorpusIndex::build(&sources, &segmentations)
            .expect_err("misaligned metadata should fail");

        assert_eq!(
            error,
            "corpus segmentation source_index mismatch: expected 0, found 1"
        );
    }

    fn alternating_frame(len: usize) -> Vec<f32> {
        (0..len)
            .map(|index| if index % 2 == 0 { -0.75 } else { 0.75 })
            .collect()
    }

    fn ramp_frame(len: usize) -> Vec<f32> {
        (0..len)
            .map(|index| -1.0 + 2.0 * index as f32 / (len.saturating_sub(1).max(1)) as f32)
            .collect()
    }
}
