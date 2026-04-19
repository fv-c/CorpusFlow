# Release Notes

## 2026-04-19 baseline
- Added canonical external JSON config export through `show-config`.
- Added end-to-end `run [--config PATH] --output PATH` wiring from corpus and target disk inputs through matching, synthesis, micro-adaptation, rendering, and WAV output.
- Added `validate-config PATH` for release-time config checks without entering the run path.
- Locked external config parsing to reject unknown fields and normalized enum values to kebab-case for stable CLI-facing JSON.
- Recorded a fresh Criterion snapshot for descriptor extraction, candidate scoring, greedy matching, overlap-add synthesis, and the offline render pipeline.

## Current baseline scope
- Offline mono-first corpus workflow remains explicit across corpus, target, matching, synthesis, and rendering stages.
- Stereo output is available through rendering duplication of the mono reconstruction.
- Micro-adaptation gain and carrier-envelope shaping now participate in the CLI run path when enabled in config.
- Ambisonics remains intentionally reserved behind explicit output convention fields (`order`, channel ordering, normalization) plus positioning JSON validation, and still does not render audio output.

## Known limits
- Config validation checks structural and numeric invariants, but it does not yet require existing corpus or target paths.
- Benchmark figures are local machine snapshots and should be compared only against runs from the same hardware and toolchain.
