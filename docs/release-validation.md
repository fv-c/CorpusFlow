# Release Validation

Validation pass executed on 2026-04-19.

## Commands
```text
cargo test
cargo run -- show-config
cargo run -- validate-config <exported-default-config>
cargo bench --bench descriptor_extraction
cargo bench --bench candidate_scoring
cargo bench --bench greedy_matching
cargo bench --bench overlap_add_synthesis
cargo bench --bench offline_render_pipeline
```

## Results
- `cargo test`: passed `75` tests total (`70` unit tests, `5` integration tests).
- `show-config`: emitted a valid pretty-printed JSON config with kebab-case enum values.
- `validate-config`: accepted the exported default config and returned the expected summary string.
- Criterion benches completed for all five documented hot paths.

## Release gate status
- Config export: pass
- Config parse and validation: pass
- Unit and integration tests: pass
- Hot-path benchmark snapshot: pass
