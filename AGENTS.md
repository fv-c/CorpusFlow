# CorpusFlow Agent Guide

## Project intent
- Build an offline Rust CLI for corpus-based granular matching and audio resynthesis.
- Deliver in small milestones: preprocess/index, target analysis, matching, synthesis, rendering.

## Read order
1. `AGENTS.md`
2. `docs/INDEX.md`
3. Relevant file in `SKILLS/`
4. Relevant file in `PROMPTS/`
5. Only then read touched source files

## Architectural principles
- Keep stage boundaries explicit: corpus prep, target analysis, matching, synthesis, rendering.
- Prefer simple structs and module-local functions over trait-heavy abstraction.
- Mono corpus baseline first; stereo and ambisonics stay in rendering/output design.
- Keep configuration explicit, serializable, and deterministic.

## Dependency policy
- Prefer `std` first.
- Add one crate at a time only when a current milestone needs it.
- Record each dependency and one-line justification in `docs/architecture.md`.
- Prefer mature, stable crates with narrow scope.

## Performance policy
- Treat descriptor extraction, candidate scoring, matching, and overlap-add as hot paths.
- Avoid per-frame allocations; reuse buffers and favor contiguous `Vec` layouts.
- Add benchmarks as soon as a loop exists; optimize from measurements, not guesses.
- Parallelize only after single-thread baseline behavior is correct and measured.

## Token economy policy
- Use the phase format: Goal, Files, Code, Notes, Next.
- Keep notes compact and technical.
- Do not restate established context or propose broad speculative rewrites.

## Development workflow
- Implement one coherent milestone at a time.
- Keep patches small and scoped.
- Update docs/guidance files when project structure or policy changes.
- Run targeted tests for each touched area before stopping.
- At the end of each completed phase, create a commit.
- Commit format: `[topic] descrizione.`

## Definition of done
- Code builds on stable Rust.
- Touched modules have focused tests or a documented reason why not.
- Guidance/docs stay aligned with the implemented baseline.
- No hidden behavior: defaults and assumptions are visible in code or docs.
