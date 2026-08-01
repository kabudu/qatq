# API and CLI Freeze

The QATQ v0.1.0 API and CLI surface is accepted as frozen.
Do not rename the surfaces below before crates.io publication without opening a
new freeze record and documenting the compatibility impact.

## Stable Product Surface For v0.1.0

| surface | status | notes |
| --- | --- | --- |
| `qatq-exact` | primary | Default exact QATQ product mode. Lossless claims are scoped here. |
| `QATC` v2 | primary | Sequential large-tensor container for exact QATQ chunks. |
| native `f32`, `f16`, `bf16` exact tensor bytes | primary | `f16`/`bf16` are stored natively, not widened to f32. |
| `qatq encode [--dtype f32|f16|bf16] [--stride-elements N]` | primary | Single-payload exact tensor encode; `qatq-exact` is the default mode and the optional stride hint is valid for native f16/bf16. |
| `qatq encode-chunked --max-values-per-chunk N [--dtype f32|f16|bf16]` | primary | Streaming file encode into QATC. |
| `qatq decode` | primary | Decodes QATQ single payloads and QATC containers. |
| `qatq fixture generate/add/verify` | support | Public fixture and release reproducibility tooling. |
| `qatq-bench` | support | Benchmark, gate, and paper-table generation. |
| `qatq-kv-bench` | support | Direct typed KV tensor benchmark against zstd/lz4. |

## Comparator / Research Surface

| surface | status | notes |
| --- | --- | --- |
| `turboquant-q4` | comparator | Local TurboQuant-style lossy reference, not Google's implementation. |
| `phase1-q4` | comparator | Lossy quaternion predictor lineage path, not the product default. |
| `lossy-i4` | seed baseline | Retained for historical comparison. |
| `lossless-f32` | control | Exact f32 envelope/control mode. |

## Naming Decisions

- The product name remains QATQ because the exact codec includes reversible
  quaternion-chain candidates and keeps quaternion-backed compression in the
  strategy search.
- Public docs should say `QATQ exact`, `QATC`, and `qatq-exact`, and avoid
  former internal implementation names.
- Lower-level code may retain implementation-specific internal names only when
  they are private and not part of the API/CLI contract.

## Accepted Freeze Gate

Accepted on 2026-06-22:

- external runtime integration feedback was incorporated into
  `docs/EXTERNAL_RUNTIME_EVIDENCE.md`;
- the stable CLI command and mode names above are accepted for v0.1.0;
- stable functions intended for external users are documented in `README.md` or
  companion docs;
- docs state `qatq-exact` and `QATC` as the default exact product surface;
- `cargo package --allow-dirty` succeeds from the release branch;
- coverage and supply-chain checks are wired into CI;
- `docs/PRODUCTION_READINESS.md` is updated with current evidence and open
  blockers.

## Post-Freeze Change Policy

Additive exact-container APIs may be introduced without changing the frozen
CLI or QATC v2 wire format. The opaque-word and bounded-inspection APIs added
after v0.1.1 follow that rule: existing symbols and encoded bytes are unchanged.

QATQ 0.2.1 makes the existing `--mode qatq-exact` argument optional for
`qatq encode`. Omitting `--mode` selects `qatq-exact`; every explicit mode
continues to behave as before. This is a backward-compatible CLI addition and
does not change encoded bytes or the QATQ/QATC wire formats.

QATQ 0.2.0 adds `QatqExactStrategy::AdjacentXorBytePlaneZstd` and exact strategy
identifier `9` for native f16/bf16 payloads. New decoders remain backward
compatible with all version-1 QATQ envelopes and QATC v2 containers. Older
decoders reject payloads that select the new identifier rather than silently
misdecoding them. Because adding a public Rust enum variant can break exhaustive
downstream matches, this is released as 0.2.0 rather than a 0.1.x patch.

QATQ 0.3.0 adds
`try_encode_qatq_exact_tensor_le_with_stride_hint`,
CLI `qatq encode --stride-elements`, error variant
`QatqError::InvalidPredictorStride`,
`QatqExactStrategy::StridedXorBytePlaneZstd`, and exact strategy identifier
`10`. Existing no-hint encoding and previously encoded QATQ/QATC payloads remain
compatible. Older decoders reject identifier `10` instead of misdecoding it.
The public enum additions require a semver-minor release because exhaustive
downstream matches may need updating.

QATQ 0.4.0 additively introduces the optional `qatq::oracle` API and separate
`qatq-oracle` executable. Existing `qatq` commands, codec APIs, QATQ payloads,
and QATC v2 containers are unchanged. Oracle JSON and certificate schemas begin
at version 1 and reject unknown critical fields.

Before crates.io publication, any API or CLI rename must include:

- an explicit changelog entry;
- a compatibility note in this file;
- regenerated package checks;
- a decision on whether the freeze acceptance must be renewed.
