# Phase 09: Granular Synthesis

- Goal: render selected grains with overlap-add and explicit windowing.
- Deliverables: synthesis loop, window functions, explicit synthesis output hop, output tests, benchmark.
- Constraints: correctness first, avoid unnecessary allocation in the overlap-add path.
- Follow-up in this phase: add an irregular-overlap scheduling path so reconstruction can avoid a rigid mechanical grain flow while preserving deterministic baseline behavior as the default mode.
