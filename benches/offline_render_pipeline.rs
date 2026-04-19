mod common;

use common::{
    build_corpus_index, build_corpus_sources, build_matching_model, build_render_plan,
    build_synthesis_plan, build_target_analysis,
};
use corpusflow::{
    matching::greedy_match, rendering::render_reconstruction, synthesis::synthesize_match_sequence,
};
use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn bench_offline_render_pipeline(c: &mut Criterion) {
    let grain_count = 256;
    let model = build_matching_model();
    let synthesis_plan = build_synthesis_plan();
    let render_plan = build_render_plan();
    let corpus_sources = build_corpus_sources(grain_count);
    let corpus_index = build_corpus_index(grain_count);
    let target_analysis = build_target_analysis(128);

    c.bench_function("offline_render_pipeline_128_frames_256_grains", |b| {
        b.iter(|| {
            let sequence = greedy_match(
                black_box(&model),
                black_box(&corpus_index),
                black_box(&target_analysis),
            )
            .expect("match");
            let synthesis = synthesize_match_sequence(
                black_box(&synthesis_plan),
                black_box(&corpus_sources),
                black_box(&corpus_index),
                black_box(&sequence),
            )
            .expect("synthesis");

            black_box(render_reconstruction(
                black_box(&render_plan),
                black_box(&synthesis.audio),
            ))
            .expect("render")
        })
    });
}

criterion_group!(benches, bench_offline_render_pipeline);
criterion_main!(benches);
