# Codex Handoff

Open the next dedicated Codex session in:

```text
/Users/kabudu/projex/qatq
```

Suggested next goal:

```text
Use Lazarus mode to continue making QATQ a standalone open-source codec:
public fixtures, public CI/release hygiene, runtime adapter contracts,
comparative baselines, and paper/white-paper updates without weakening
bit-exactness.
```

Reference paper URL:

```text
https://pub-3600fb3390724222be9e7f11caf4e0c9.r2.dev/Quaternion-Augmented-TurboQuant-Clean.pdf
```

Important current-state notes:

- The seed `lossy-i4` path is a retained baseline, not the full
  Quaternion-Augmented TurboQuant design.
- The `phase1-q4` path now implements quaternion grouping, deterministic
  Hamilton-product rotation, scalar q4 quantization, and a compact residual-sign
  side channel.
- The seed `lossless-f32` path is an exact control envelope, not compression.
- The `phase2-lossless` path is the current exact QATQ-family codec. It stores
  raw bits, byte-RLE, byte-plane RLE, byte-plane blocks, delta-XOR byte-plane
  RLE, or Phase 1 prediction plus run-coded XOR residuals and verifies
  bit-identical reconstruction.
- Production callers should use `try_encode_phase2_lossless_decision_with_config`.
  `Compressed` means store/transmit a QATQ Phase 2 payload. `PassThroughRaw`
  means store/transmit raw f32le bytes and record QATQ pass-through metadata.
- QATQ now has generated public fixtures under `fixtures/generated/` with
  `fixtures/public.manifest` as the default public corpus.
- The `QATC` container wraps multiple Phase 2 payloads for sequential large
  tensor files. Random-access metadata and service-level streaming remain
  future work.
- Historical external runtime evidence is archived under `handoff/`; it is not
  required for public QATQ validation.
- General-purpose byte compression is out of scope unless evidence shows the
  transform improves residual entropy on non-KV tensor payloads.
