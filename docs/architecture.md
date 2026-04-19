# Architecture Overview

## Baseline stages
1. Corpus audio I/O, normalization, and resampling to the configured output sample rate
2. Corpus segmentation and descriptor extraction
3. Corpus indexing/storage
4. Target analysis
5. Matching with target and transition costs
6. Granular synthesis
7. Output rendering

## Initial source layout
- `app.rs`: top-level CLI dispatch
- `audio.rs`: WAV read/write and explicit sample buffer types
- `cli.rs`: minimal command parsing
- `config.rs`: serializable baseline configuration
- `corpus.rs`: corpus ingestion and fixed-grid grain planning
- `index.rs`: exact corpus grain index and descriptor storage
- `target.rs`: target loading, frame planning, mono analysis, descriptor pass
- `descriptor.rs`: descriptor spec skeleton
- `matching.rs`: greedy baseline matcher with target and transition costs
- `micro_adaptation.rs`: optional post-selection gain and carrier-envelope hooks
- `synthesis.rs`: overlap-add synthesis, explicit windowing, fixed/alternating scheduling
- `rendering.rs`: output/render skeleton

## Minimal viable architecture
- Keep one crate with a library + CLI binary.
- Data moves linearly by stage; later caches/indexes stay explicit.
- Matching owns cost composition, not descriptor extraction or synthesis.
- Rendering is separate from synthesis so spatial modes can evolve independently.
- Corpus grain hop, target analysis hop, and synthesis output hop should remain separate controls.
- Irregular overlap belongs to synthesis scheduling, not corpus segmentation or baseline matching.
- Carrier prosodic inheritance belongs to synthesis or micro-adaptation, not to corpus indexing or matching.
- Global envelope transfer should be available as a deterministic baseline adaptation layer.
- Optional convolution should remain a separate post-process after reconstruction, with dry/wet control, output safety normalization, and an explicit audio source from either the original target file or a separate WAV file.
- Ambisonics rendering now starts from an explicit FOA output convention (`order = 1`, ACN, SN3D/N3D) plus an external JSON positioning spec with waypoint time stamps, per-segment curve metadata, and separate jitter-cloud controls. Higher-order output remains a later extension.

## Active dependencies
- `criterion` (dev): stable microbenchmark harness for descriptor extraction and later hot loops.
- `hound`: narrow-scope WAV reader/writer for offline baseline audio I/O.
- `rustfft`: stable FFT implementation for spectral descriptor extraction.
- `serde`: derive `Serialize`/`Deserialize` for explicit config types.
- `serde_json`: parse explicit rendering-side JSON specs for future ambisonics trajectory and jitter input.
