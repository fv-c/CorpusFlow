# Performance Skill

- Identify the hot loop before optimizing.
- Remove repeated allocations first.
- Favor contiguous numeric data and reusable scratch buffers.
- Add a benchmark before a non-trivial optimization pass.
- Parallelize only after single-thread timing data exists.
