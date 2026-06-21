# Codex Handoff

Open the next dedicated Codex session in:

```text
/Users/kabudu/projex/qatq
```

Suggested first goal:

```text
Use Lazarus mode to implement the Phase 1 paper-faithful training-free QATQ
pipeline from docs/ROADMAP.md, using the source paper notes in
docs/PAPER_NOTES.md, with tests, benchmarks, and clear comparison against the
seed lossy-i4 baseline.
```

Reference paper URL:

```text
https://pub-3600fb3390724222be9e7f11caf4e0c9.r2.dev/Quaternion-Augmented-TurboQuant-Clean.pdf
```

Important current-state notes:

- The seed `lossy-i4` path is the PermeantOS experimental migration baseline,
  not the full Quaternion-Augmented TurboQuant design.
- The seed `lossless-f32` path is an exact control envelope, not compression.
- A real lossless QATQ-family codec needs residual coding and bit-identical
  reconstruction tests.
- General-purpose byte compression is out of scope unless evidence shows the
  transform improves residual entropy on non-KV tensor payloads.

