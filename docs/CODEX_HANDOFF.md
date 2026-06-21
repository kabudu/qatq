# Codex Handoff

Open the next dedicated Codex session in:

```text
/Users/kabudu/projex/qatq
```

Suggested next goal:

```text
Use Lazarus mode to validate `phase2-lossless` and the `QATC` chunk container
against real KV-cache fixtures or PermeantOS migration captures, then use those
measurements to improve residual coding, performance policy, and paper tables.
```

Reference paper URL:

```text
https://pub-3600fb3390724222be9e7f11caf4e0c9.r2.dev/Quaternion-Augmented-TurboQuant-Clean.pdf
```

Important current-state notes:

- The seed `lossy-i4` path is the PermeantOS experimental migration baseline,
  not the full Quaternion-Augmented TurboQuant design.
- The `phase1-q4` path now implements quaternion grouping, deterministic
  Hamilton-product rotation, scalar q4 quantization, and a compact residual-sign
  side channel.
- The seed `lossless-f32` path is an exact control envelope, not compression.
- The `phase2-lossless` path is the current exact QATQ-family codec. It stores
  raw bits, byte-RLE, byte-plane RLE, or Phase 1 prediction plus run-coded XOR
  residuals and verifies bit-identical reconstruction.
- The `QATC` container wraps multiple Phase 2 payloads for sequential large
  tensor files. Random-access metadata and service-level streaming remain
  future work.
- Current benchmarks are synthetic and documented in `docs/BENCHMARKS.md`.
  PermeantOS integration and real KV-cache validation are still pending.
- General-purpose byte compression is out of scope unless evidence shows the
  transform improves residual entropy on non-KV tensor payloads.
