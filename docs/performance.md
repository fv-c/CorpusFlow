# Performance Notes

## Hot paths
- Descriptor extraction
- Candidate scoring
- Matching loop
- Overlap-add synthesis

## Early rules
- No allocation inside repeated frame/grain loops unless measured and justified.
- Prefer packed numeric vectors over nested dynamic structures.
- Reuse temporary buffers for FFT/windowing once those stages exist.

## Benchmark order
1. Descriptor extraction microbenchmark
2. Corpus search/scoring microbenchmark
3. Matching loop microbenchmark
4. Overlap-add microbenchmark
5. End-to-end offline render benchmark

## Reproducible benchmark workflow
- Keep benchmark fixtures synthetic and committed under `benches/` so every run exercises the same frame sizes, grain counts, and render settings.
- Run individual Criterion benches from an otherwise idle machine with `cargo bench --bench descriptor_extraction`, `cargo bench --bench candidate_scoring`, `cargo bench --bench greedy_matching`, `cargo bench --bench overlap_add_synthesis`, and `cargo bench --bench offline_render_pipeline`.
- Use the default release benchmark profile for comparisons; only compare runs collected from the same hardware and Rust toolchain.
- Read the Criterion median and variance together; regressions matter more than a single fast sample.

## Bottleneck summary
- `descriptor_extraction` isolates the descriptor hot path: frame scan plus FFT-backed spectral features on a 4,800-sample window.
- `candidate_scoring` isolates the per-grain inner loop used by matching, including descriptor distance, seek distance, and source-switch penalty.
- `greedy_matching` measures the full `target_frames x corpus_grains` selection pass; this should dominate as corpus size grows.
- `overlap_add_synthesis` measures window generation reuse plus sample accumulation across scheduled grains.
- `offline_render_pipeline` exercises the current complete reconstruction path (`greedy_match -> SynthesisPlan::synthesize -> render_reconstruction`) to show whether matching, overlap-add, or post-convolution dominates total offline render time.
