mod common;

use common::{build_corpus_index, build_matching_model, descriptor_for};
use corpusflow::matching::TransitionReference;
use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn bench_candidate_scoring(c: &mut Criterion) {
    let model = build_matching_model();
    let corpus_index = build_corpus_index(128);
    let target_descriptor = descriptor_for(17, 11);
    let candidate_index = 64;
    let candidate_descriptor = corpus_index.normalized_descriptors[candidate_index];
    let candidate_grain = &corpus_index.grains[candidate_index];
    let previous = TransitionReference {
        descriptor: corpus_index.normalized_descriptors[candidate_index - 1],
        grain: corpus_index.grains[candidate_index - 1],
    };

    c.bench_function("matching_score_candidate_initial", |b| {
        b.iter(|| {
            black_box(model.score_candidate(
                black_box(target_descriptor),
                black_box(candidate_descriptor),
                black_box(None),
                black_box(candidate_grain),
            ))
        })
    });

    c.bench_function("matching_score_candidate_with_transition", |b| {
        b.iter(|| {
            black_box(model.score_candidate(
                black_box(target_descriptor),
                black_box(candidate_descriptor),
                black_box(Some(previous)),
                black_box(candidate_grain),
            ))
        })
    });
}

criterion_group!(benches, bench_candidate_scoring);
criterion_main!(benches);
