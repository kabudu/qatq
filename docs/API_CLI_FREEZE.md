# API and CLI Freeze

QATQ is still a source release candidate. Do not publish to crates.io until this
surface is intentionally accepted as stable.

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
- Public docs should say `QATQ exact`, `QATC`, and `qatq-exact`, not
  `phase2-lossless`.
- Lower-level code may retain implementation-specific internal names only when
  they are private and not part of the API/CLI contract.

## Pre-Publish Freeze Gate

Before crates.io publishing:

- no CLI command or mode renames without a changelog entry;
- all stable functions intended for external users have rustdoc examples or are
  documented in `README.md`;
- no docs claim a default mode that differs from the CLI/API;
- `cargo package --allow-dirty` succeeds from a clean source release candidate;
- `docs/PRODUCTION_READINESS.md` is updated with current evidence and open
  blockers.
