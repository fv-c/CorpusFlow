mod common;

use common::{SAMPLE_RATE, build_benchmark_frame};
use corpusflow::descriptor::BaselineDescriptorExtractor;
use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn bench_descriptor_extraction(c: &mut Criterion) {
    let frame_size = 4_800;
    let frame = build_benchmark_frame(frame_size);
    let mut extractor =
        BaselineDescriptorExtractor::new(SAMPLE_RATE, frame_size).expect("extractor");

    c.bench_function("descriptor_extract_4800_samples", |b| {
        b.iter(|| {
            black_box(
                extractor
                    .extract_frame(black_box(&frame))
                    .expect("descriptor"),
            )
        })
    });
}

criterion_group!(benches, bench_descriptor_extraction);
criterion_main!(benches);
