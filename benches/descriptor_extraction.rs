use corpusflow::descriptor::BaselineDescriptorExtractor;
use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn bench_descriptor_extraction(c: &mut Criterion) {
    let sample_rate = 48_000;
    let frame_size = 4_800;
    let frame = build_benchmark_frame(sample_rate, frame_size);
    let mut extractor =
        BaselineDescriptorExtractor::new(sample_rate, frame_size).expect("extractor");

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

fn build_benchmark_frame(sample_rate: u32, frame_size: usize) -> Vec<f32> {
    let mut frame = Vec::with_capacity(frame_size);

    for index in 0..frame_size {
        let time = index as f32 / sample_rate as f32;
        let sample = 0.6 * (2.0 * std::f32::consts::PI * 220.0 * time).sin()
            + 0.25 * (2.0 * std::f32::consts::PI * 880.0 * time).sin()
            + 0.15 * (2.0 * std::f32::consts::PI * 1760.0 * time).sin();
        frame.push(sample.clamp(-1.0, 1.0));
    }

    frame
}

criterion_group!(benches, bench_descriptor_extraction);
criterion_main!(benches);
