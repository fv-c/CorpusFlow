# Benchmark Snapshot

Collected on 2026-04-19 with `rustc 1.94.0 (4a4ef493e 2026-03-02)` on `Darwin arm64`.

## Criterion medians
- `descriptor_extract_4800_samples`: `29.202 us`
- `matching_score_candidate_initial`: `7.2771 ns`
- `matching_score_candidate_with_transition`: `8.4511 ns`
- `greedy_matching_256_frames_512_grains`: `432.85 us`
- `overlap_add_synthesis_256_grains`: `785.24 us`
- `offline_render_pipeline_128_frames_256_grains`: `1.8210 ms`

## Notes
- Criterion reported all observed deltas as within the noise threshold or no significant change.
- `offline_render_pipeline_128_frames_256_grains` needed a longer collection window than the default 5 s estimate; Criterion completed after extending the estimated run to about 9.2 s.
- These numbers are local baseline snapshots, not cross-machine targets.

## Command set
```text
cargo bench --bench descriptor_extraction
cargo bench --bench candidate_scoring
cargo bench --bench greedy_matching
cargo bench --bench overlap_add_synthesis
cargo bench --bench offline_render_pipeline
```
