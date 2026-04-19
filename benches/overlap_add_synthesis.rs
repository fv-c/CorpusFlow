mod common;

use common::{
    build_corpus_index, build_corpus_sources, build_match_sequence, build_synthesis_plan,
};
use corpusflow::synthesis::{SynthesisPlan, synthesize_match_sequence};
use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn bench_overlap_add_synthesis(c: &mut Criterion) {
    let grain_count = 128;
    let plan: SynthesisPlan = build_synthesis_plan();
    let corpus_sources = build_corpus_sources(grain_count);
    let corpus_index = build_corpus_index(grain_count);
    let sequence = build_match_sequence(256, grain_count);

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

criterion_group!(benches, bench_overlap_add_synthesis);
criterion_main!(benches);
