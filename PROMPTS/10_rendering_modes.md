# Phase 10: Rendering Modes

- Goal: support mono and stereo output paths with an extensible rendering seam.
- Deliverables: rendering mode config, channel routing baseline, tests.
- Constraints: ambisonics stays as a future-oriented interface point, with positioning defined by an external trajectory JSON that separates path curves from jitter-cloud controls.
- Follow-up in this phase: add an optional post-process convolution stage after reconstruction, with explicit dry/wet mix and output normalization controls.
