# Architecture Overview

## Baseline stages
1. Corpus audio I/O and normalization
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
- `target.rs`: target analysis skeleton
- `descriptor.rs`: descriptor spec skeleton
- `matching.rs`: matching model skeleton
- `synthesis.rs`: synthesis skeleton
- `rendering.rs`: output/render skeleton

## Minimal viable architecture
- Keep one crate with a library + CLI binary.
- Data moves linearly by stage; later caches/indexes stay explicit.
- Matching owns cost composition, not descriptor extraction or synthesis.
- Rendering is separate from synthesis so spatial modes can evolve independently.

## Active dependencies
- `hound`: narrow-scope WAV reader/writer for offline baseline audio I/O.
- `serde`: derive `Serialize`/`Deserialize` for explicit config types.
