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
| `qatq encode --mode qatq-exact [--dtype f32|f16|bf16]` | primary | Single-payload exact tensor encode. |
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

Before crates.io publication, any API or CLI rename must include:

- an explicit changelog entry;
- a compatibility note in this file;
- regenerated package checks;
- a decision on whether the freeze acceptance must be renewed.
