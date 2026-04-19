# Architecture Skill

- Read `AGENTS.md`, `docs/INDEX.md`, and `docs/architecture.md` first.
- Preserve stage separation: preprocessing/indexing, target analysis, matching, synthesis, rendering.
- Add modules only when a concrete phase needs them.
- Prefer inspectable structs over generic framework code.
- Update docs when stage boundaries or dependencies change.
