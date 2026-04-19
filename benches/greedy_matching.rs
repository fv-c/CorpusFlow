mod common;

use common::{build_corpus_index, build_matching_model, build_target_analysis};
use corpusflow::matching::greedy_match;
use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn bench_greedy_matching(c: &mut Criterion) {
    let model = build_matching_model();
    let corpus_index = build_corpus_index(512);
    let target_analysis = build_target_analysis(256);

    c.bench_function("greedy_matching_256_frames_512_grains", |b| {
        b.iter(|| {
            black_box(
                greedy_match(
                    black_box(&model),
                    black_box(&corpus_index),
                    black_box(&target_analysis),
                )
                .expect("match"),
            )
        })
    });
}

criterion_group!(benches, bench_greedy_matching);
criterion_main!(benches);
