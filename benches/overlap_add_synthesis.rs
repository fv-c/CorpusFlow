use corpusflow::{
    audio::MonoBuffer,
    config::{OverlapScheduleMode, WindowKind},
    corpus::CorpusSourceFile,
    descriptor::{DescriptorNormalization, DescriptorVector},
    index::{CorpusGrainEntry, CorpusIndex, CorpusSourceInfo},
    matching::{MatchCost, MatchSequence, MatchStep},
    synthesis::{SynthesisPlan, synthesize_match_sequence},
};
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::path::PathBuf;

fn bench_overlap_add_synthesis(c: &mut Criterion) {
    let plan = SynthesisPlan {
        window: WindowKind::Hann,
        output_hop_ms: 50,
        overlap_schedule: OverlapScheduleMode::Alternating,
        irregularity_ms: 10,
    };
    let corpus_sources = vec![CorpusSourceFile {
        path: PathBuf::from("source.wav"),
        audio: MonoBuffer::new(48_000, build_source_samples(48_000, 7)).expect("source"),
    }];
    let corpus_index = build_corpus_index();
    let sequence = build_match_sequence(256);

    c.bench_function("overlap_add_synthesis_256_grains", |b| {
        b.iter(|| {
            black_box(
                synthesize_match_sequence(
                    black_box(&plan),
                    black_box(&corpus_sources),
                    black_box(&corpus_index),
                    black_box(&sequence),
                )
                .expect("synthesis"),
            )
        })
    });
}

fn build_source_samples(sample_rate: u32, seconds: usize) -> Vec<f32> {
    let frame_count = sample_rate as usize * seconds;
    let mut samples = Vec::with_capacity(frame_count);

    for index in 0..frame_count {
        let time = index as f32 / sample_rate as f32;
        let sample = 0.7 * (2.0 * std::f32::consts::PI * 220.0 * time).sin()
            + 0.3 * (2.0 * std::f32::consts::PI * 660.0 * time).sin();
        samples.push(sample.clamp(-1.0, 1.0));
    }

    samples
}

fn build_corpus_index() -> CorpusIndex {
    let grain_count = 128;
    let grain_size_frames = 4_800;
    let grain_hop_frames = 2_400;
    let grains = (0..grain_count)
        .map(|index| CorpusGrainEntry {
            source_index: 0,
            start_frame: index * grain_hop_frames,
            len_frames: grain_size_frames,
        })
        .collect::<Vec<_>>();
    let descriptors = (0..grain_count)
        .map(|index| DescriptorVector::new([index as f32, 0.1, 0.2, 0.3, 0.4]))
        .collect::<Vec<_>>();

    CorpusIndex {
        sources: vec![CorpusSourceInfo {
            path: PathBuf::from("source.wav"),
            sample_rate: 48_000,
            total_frames: 48_000 * 7,
        }],
        grains,
        raw_descriptors: descriptors.clone(),
        normalized_descriptors: descriptors,
        normalization: DescriptorNormalization {
            mean: [0.0; 5],
            scale: [1.0; 5],
        },
    }
}

fn build_match_sequence(step_count: usize) -> MatchSequence {
    MatchSequence {
        total_cost: 0.0,
        steps: (0..step_count)
            .map(|target_frame_index| MatchStep {
                target_frame_index,
                selected_grain_index: target_frame_index % 128,
                cost: MatchCost {
                    target_distance: 0.0,
                    transition_cost: 0.0,
                    transition_descriptor_distance: 0.0,
                    transition_seek_distance: 0.0,
                    source_switch_cost: 0.0,
                    total_cost: 0.0,
                },
            })
            .collect(),
    }
}

criterion_group!(benches, bench_overlap_add_synthesis);
criterion_main!(benches);
