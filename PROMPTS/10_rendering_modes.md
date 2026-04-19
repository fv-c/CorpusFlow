# Phase 10: Rendering Modes

- Goal: support mono and stereo output paths with an extensible rendering seam.
- Deliverables: rendering mode config, channel routing baseline, tests.
- Constraints: ambisonics now supports a first-order output baseline, with positioning defined by an external trajectory JSON that separates path curves from jitter-cloud controls; higher-order output remains future work.
- Follow-up in this phase: add an optional post-process convolution stage after reconstruction, with explicit dry/wet mix and output normalization controls.
