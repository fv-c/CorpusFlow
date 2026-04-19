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
