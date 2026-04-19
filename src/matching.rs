use crate::{
    config::MatchingConfig,
    descriptor::{BASELINE_DESCRIPTOR_DIMENSIONS, DescriptorVector},
    index::{CorpusGrainEntry, CorpusIndex},
    target::TargetAnalysis,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchingModel {
    pub alpha: f32,
    pub beta: f32,
    pub transition_descriptor_weight: f32,
    pub transition_seek_weight: f32,
    pub source_switch_penalty: f32,
}

impl From<&MatchingConfig> for MatchingModel {
    fn from(config: &MatchingConfig) -> Self {
        Self {
            alpha: config.alpha,
            beta: config.beta,
            transition_descriptor_weight: config.transition_descriptor_weight,
            transition_seek_weight: config.transition_seek_weight,
            source_switch_penalty: config.source_switch_penalty,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchCost {
    pub target_distance: f32,
    pub transition_cost: f32,
    pub transition_descriptor_distance: f32,
    pub transition_seek_distance: f32,
    pub source_switch_cost: f32,
    pub total_cost: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchStep {
    pub target_frame_index: usize,
    pub selected_grain_index: usize,
    pub cost: MatchCost,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchSequence {
    pub steps: Vec<MatchStep>,
    pub total_cost: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionReference {
    pub descriptor: DescriptorVector,
    pub grain: CorpusGrainEntry,
}

impl MatchingModel {
    pub fn score_candidate(
        &self,
        target_descriptor: DescriptorVector,
        candidate_descriptor: DescriptorVector,
        previous: Option<TransitionReference>,
        candidate_grain: &CorpusGrainEntry,
    ) -> MatchCost {
        let target_distance = squared_euclidean_distance(target_descriptor, candidate_descriptor);
        let (
            transition_descriptor_distance,
            transition_seek_distance,
            source_switch_cost,
            transition_cost,
        ) = previous
            .map(|previous| {
                let descriptor_distance =
                    squared_euclidean_distance(previous.descriptor, candidate_descriptor);
                let seek_distance = normalized_seek_distance(previous.grain, candidate_grain);
                let source_switch_cost =
                    if previous.grain.source_index == candidate_grain.source_index {
                        0.0
                    } else {
                        self.source_switch_penalty
                    };
                let transition_cost = self.transition_descriptor_weight * descriptor_distance
                    + self.transition_seek_weight * seek_distance
                    + source_switch_cost;

                (
                    descriptor_distance,
                    seek_distance,
                    source_switch_cost,
                    transition_cost,
                )
            })
            .unwrap_or((0.0, 0.0, 0.0, 0.0));

        MatchCost {
            target_distance,
            transition_cost,
            transition_descriptor_distance,
            transition_seek_distance,
            source_switch_cost,
            total_cost: self.alpha * target_distance + self.beta * transition_cost,
        }
    }
}

pub fn greedy_match(
    model: &MatchingModel,
    corpus_index: &CorpusIndex,
    target_analysis: &TargetAnalysis,
) -> Result<MatchSequence, String> {
    if corpus_index.is_empty() {
        return Err("matching requires a non-empty corpus index".to_string());
    }

    let mut steps = Vec::with_capacity(target_analysis.frames.len());
    let mut total_cost = 0.0;
    let mut previous_grain_index = None;

    for (target_frame_index, target_frame) in target_analysis.frames.iter().enumerate() {
        let (selected_grain_index, cost) = select_best_candidate(
            model,
            corpus_index,
            target_frame.normalized_descriptor,
            previous_grain_index,
        );

        total_cost += cost.total_cost;
        steps.push(MatchStep {
            target_frame_index,
            selected_grain_index,
            cost,
        });
        previous_grain_index = Some(selected_grain_index);
    }

    Ok(MatchSequence { steps, total_cost })
}

fn select_best_candidate(
    model: &MatchingModel,
    corpus_index: &CorpusIndex,
    target_descriptor: DescriptorVector,
    previous_grain_index: Option<usize>,
) -> (usize, MatchCost) {
    let previous = previous_grain_index.map(|index| TransitionReference {
        descriptor: corpus_index.normalized_descriptors[index],
        grain: corpus_index.grains[index],
    });
    let mut best_grain_index = 0;
    let mut best_cost = model.score_candidate(
        target_descriptor,
        corpus_index.normalized_descriptors[0],
        previous,
        &corpus_index.grains[0],
    );

    for candidate_index in 1..corpus_index.normalized_descriptors.len() {
        let candidate_cost = model.score_candidate(
            target_descriptor,
            corpus_index.normalized_descriptors[candidate_index],
            previous,
            &corpus_index.grains[candidate_index],
        );

        if candidate_cost.total_cost < best_cost.total_cost {
            best_grain_index = candidate_index;
            best_cost = candidate_cost;
        }
    }

    (best_grain_index, best_cost)
}

fn squared_euclidean_distance(left: DescriptorVector, right: DescriptorVector) -> f32 {
    let mut sum = 0.0;

    for index in 0..BASELINE_DESCRIPTOR_DIMENSIONS {
        let delta = left.values[index] - right.values[index];
        sum += delta * delta;
    }

    sum
}

fn normalized_seek_distance(previous: CorpusGrainEntry, candidate: &CorpusGrainEntry) -> f32 {
    if previous.source_index != candidate.source_index {
        return 1.0;
    }

    let expected_start = previous.start_frame + previous.len_frames;
    let gap = candidate.start_frame.abs_diff(expected_start);
    gap as f32 / previous.len_frames.max(1) as f32
}

#[cfg(test)]
mod tests {
    use super::{
        MatchSequence, MatchingModel, TransitionReference, greedy_match, normalized_seek_distance,
        squared_euclidean_distance,
    };
    use crate::{
        descriptor::{DescriptorNormalization, DescriptorVector},
        index::{CorpusGrainEntry, CorpusIndex, CorpusSourceInfo},
        target::{TargetAnalysis, TargetAnalysisFrame},
    };
    use std::path::PathBuf;

    #[test]
    fn squared_distance_is_zero_for_identical_vectors() {
        let vector = DescriptorVector::new([1.0, 2.0, 3.0, 4.0, 5.0]);

        assert_eq!(squared_euclidean_distance(vector, vector), 0.0);
    }

    #[test]
    fn greedy_match_uses_target_distance_on_first_frame() {
        let model = MatchingModel {
            alpha: 1.0,
            beta: 10.0,
            transition_descriptor_weight: 1.0,
            transition_seek_weight: 1.0,
            source_switch_penalty: 1.0,
        };
        let corpus_index = test_corpus_index(
            vec![
                DescriptorVector::new([0.0, 0.0, 0.0, 0.0, 0.0]),
                DescriptorVector::new([10.0, 0.0, 0.0, 0.0, 0.0]),
            ],
            100,
        );
        let target_analysis =
            test_target_analysis(vec![DescriptorVector::new([0.5, 0.0, 0.0, 0.0, 0.0])]);

        let sequence = greedy_match(&model, &corpus_index, &target_analysis).expect("match");

        assert_eq!(sequence.steps.len(), 1);
        assert_eq!(sequence.steps[0].selected_grain_index, 0);
        assert_eq!(sequence.steps[0].cost.transition_cost, 0.0);
        assert_eq!(sequence.steps[0].cost.transition_seek_distance, 0.0);
    }

    #[test]
    fn greedy_match_applies_transition_cost_from_previous_grain() {
        let corpus_index = CorpusIndex {
            sources: vec![
                CorpusSourceInfo {
                    path: PathBuf::from("a.wav"),
                    sample_rate: 1_000,
                    total_frames: 200,
                },
                CorpusSourceInfo {
                    path: PathBuf::from("b.wav"),
                    sample_rate: 1_000,
                    total_frames: 100,
                },
            ],
            grains: vec![
                CorpusGrainEntry {
                    source_index: 0,
                    start_frame: 0,
                    len_frames: 100,
                },
                CorpusGrainEntry {
                    source_index: 1,
                    start_frame: 0,
                    len_frames: 100,
                },
                CorpusGrainEntry {
                    source_index: 0,
                    start_frame: 100,
                    len_frames: 100,
                },
            ],
            raw_descriptors: vec![
                DescriptorVector::new([0.0, 0.0, 0.0, 0.0, 0.0]),
                DescriptorVector::new([10.0, 0.0, 0.0, 0.0, 0.0]),
                DescriptorVector::new([6.0, 0.0, 0.0, 0.0, 0.0]),
            ],
            normalized_descriptors: vec![
                DescriptorVector::new([0.0, 0.0, 0.0, 0.0, 0.0]),
                DescriptorVector::new([10.0, 0.0, 0.0, 0.0, 0.0]),
                DescriptorVector::new([6.0, 0.0, 0.0, 0.0, 0.0]),
            ],
            normalization: DescriptorNormalization {
                mean: [0.0; crate::descriptor::BASELINE_DESCRIPTOR_DIMENSIONS],
                scale: [1.0; crate::descriptor::BASELINE_DESCRIPTOR_DIMENSIONS],
            },
        };
        let target_analysis = test_target_analysis(vec![
            DescriptorVector::new([0.5, 0.0, 0.0, 0.0, 0.0]),
            DescriptorVector::new([8.5, 0.0, 0.0, 0.0, 0.0]),
        ]);

        let without_transition = greedy_match(
            &MatchingModel {
                alpha: 1.0,
                beta: 0.0,
                transition_descriptor_weight: 1.0,
                transition_seek_weight: 1.0,
                source_switch_penalty: 2.0,
            },
            &corpus_index,
            &target_analysis,
        )
        .expect("match");
        let with_transition = greedy_match(
            &MatchingModel {
                alpha: 1.0,
                beta: 2.0,
                transition_descriptor_weight: 0.0,
                transition_seek_weight: 1.0,
                source_switch_penalty: 4.0,
            },
            &corpus_index,
            &target_analysis,
        )
        .expect("match");

        assert_eq!(selected_grains(&without_transition), vec![0, 1]);
        assert_eq!(selected_grains(&with_transition), vec![0, 2]);
        assert!(without_transition.steps[1].cost.transition_cost > 0.0);
        assert_eq!(with_transition.steps[1].cost.transition_seek_distance, 0.0);
        assert_eq!(with_transition.steps[1].cost.source_switch_cost, 0.0);
    }

    #[test]
    fn score_candidate_exposes_transition_components() {
        let model = MatchingModel {
            alpha: 1.0,
            beta: 0.5,
            transition_descriptor_weight: 2.0,
            transition_seek_weight: 3.0,
            source_switch_penalty: 4.0,
        };
        let previous = TransitionReference {
            descriptor: DescriptorVector::new([1.0, 0.0, 0.0, 0.0, 0.0]),
            grain: CorpusGrainEntry {
                source_index: 0,
                start_frame: 0,
                len_frames: 100,
            },
        };
        let candidate_grain = CorpusGrainEntry {
            source_index: 1,
            start_frame: 500,
            len_frames: 100,
        };

        let cost = model.score_candidate(
            DescriptorVector::new([3.0, 0.0, 0.0, 0.0, 0.0]),
            DescriptorVector::new([2.0, 0.0, 0.0, 0.0, 0.0]),
            Some(previous),
            &candidate_grain,
        );

        assert_eq!(cost.target_distance, 1.0);
        assert_eq!(cost.transition_descriptor_distance, 1.0);
        assert_eq!(cost.transition_seek_distance, 1.0);
        assert_eq!(cost.source_switch_cost, 4.0);
        assert_eq!(cost.transition_cost, 9.0);
        assert_eq!(cost.total_cost, 5.5);
    }

    #[test]
    fn normalized_seek_distance_is_zero_for_adjacent_grains_in_same_source() {
        let previous = CorpusGrainEntry {
            source_index: 0,
            start_frame: 100,
            len_frames: 50,
        };
        let candidate = CorpusGrainEntry {
            source_index: 0,
            start_frame: 150,
            len_frames: 50,
        };

        assert_eq!(normalized_seek_distance(previous, &candidate), 0.0);
    }

    #[test]
    fn normalized_seek_distance_penalizes_large_jump_and_source_switch() {
        let previous = CorpusGrainEntry {
            source_index: 0,
            start_frame: 100,
            len_frames: 50,
        };
        let distant_same_source = CorpusGrainEntry {
            source_index: 0,
            start_frame: 250,
            len_frames: 50,
        };
        let switched_source = CorpusGrainEntry {
            source_index: 1,
            start_frame: 0,
            len_frames: 50,
        };

        assert_eq!(
            normalized_seek_distance(previous, &distant_same_source),
            2.0
        );
        assert_eq!(normalized_seek_distance(previous, &switched_source), 1.0);
    }

    #[test]
    fn greedy_match_returns_empty_sequence_for_empty_target_analysis() {
        let model = MatchingModel {
            alpha: 1.0,
            beta: 0.25,
            transition_descriptor_weight: 1.0,
            transition_seek_weight: 0.5,
            source_switch_penalty: 0.25,
        };
        let corpus_index =
            test_corpus_index(vec![DescriptorVector::new([0.0, 0.0, 0.0, 0.0, 0.0])], 100);
        let target_analysis = TargetAnalysis {
            sample_rate: 1_000,
            original_channels: 1,
            total_frames: 0,
            frame_size_frames: 100,
            hop_size_frames: 50,
            frames: Vec::new(),
        };

        let sequence = greedy_match(&model, &corpus_index, &target_analysis).expect("match");

        assert!(sequence.steps.is_empty());
        assert_eq!(sequence.total_cost, 0.0);
    }

    fn test_corpus_index(
        normalized_descriptors: Vec<DescriptorVector>,
        grain_len_frames: usize,
    ) -> CorpusIndex {
        let grain_count = normalized_descriptors.len();

        CorpusIndex {
            sources: vec![CorpusSourceInfo {
                path: PathBuf::from("corpus.wav"),
                sample_rate: 1_000,
                total_frames: grain_count * grain_len_frames,
            }],
            grains: (0..grain_count)
                .map(|grain_index| CorpusGrainEntry {
                    source_index: 0,
                    start_frame: grain_index * grain_len_frames,
                    len_frames: grain_len_frames,
                })
                .collect(),
            raw_descriptors: normalized_descriptors.clone(),
            normalized_descriptors,
            normalization: DescriptorNormalization {
                mean: [0.0; crate::descriptor::BASELINE_DESCRIPTOR_DIMENSIONS],
                scale: [1.0; crate::descriptor::BASELINE_DESCRIPTOR_DIMENSIONS],
            },
        }
    }

    fn test_target_analysis(normalized_descriptors: Vec<DescriptorVector>) -> TargetAnalysis {
        TargetAnalysis {
            sample_rate: 1_000,
            original_channels: 1,
            total_frames: normalized_descriptors.len() * 100,
            frame_size_frames: 100,
            hop_size_frames: 50,
            frames: normalized_descriptors
                .into_iter()
                .enumerate()
                .map(|(index, descriptor)| TargetAnalysisFrame {
                    start_frame: index * 50,
                    len_frames: 100,
                    raw_descriptor: descriptor,
                    normalized_descriptor: descriptor,
                })
                .collect(),
        }
    }

    fn selected_grains(sequence: &MatchSequence) -> Vec<usize> {
        sequence
            .steps
            .iter()
            .map(|step| step.selected_grain_index)
            .collect()
    }
}
