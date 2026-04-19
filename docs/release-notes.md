# Release Notes

## 2026-04-19 baseline
- Added canonical external JSON config export through `show-config`.
- Added `run --config PATH` so the scaffold path can execute against an explicit config file.
- Added `validate-config PATH` for release-time config checks without entering the run path.
- Locked external config parsing to reject unknown fields and normalized enum values to kebab-case for stable CLI-facing JSON.
- Recorded a fresh Criterion snapshot for descriptor extraction, candidate scoring, greedy matching, overlap-add synthesis, and the offline render pipeline.

## Current baseline scope
- Offline mono-first corpus workflow remains explicit across corpus, target, matching, synthesis, and rendering stages.
- Stereo output is available through rendering duplication of the mono reconstruction.
- Ambisonics remains intentionally reserved behind explicit positioning JSON validation and still does not render audio output.

## Known limits
- `run` still reports scaffold readiness rather than performing the full end-to-end offline reconstruction from disk inputs.
- Config validation checks structural and numeric invariants, but it does not yet require existing corpus or target paths.
- Benchmark figures are local machine snapshots and should be compared only against runs from the same hardware and toolchain.
