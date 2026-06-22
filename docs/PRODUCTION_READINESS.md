# Production Readiness

QATQ is release-candidate grade, not yet declared production-complete.

## Implemented Evidence

| area | status | evidence |
| --- | --- | --- |
| Exact codec | ready for RC | `cargo test`, public fixtures, exact checksum verification. |
| Native typed tensors | ready for RC | f32/f16/bf16 encode/decode and QATC tests. |
| QATC container | hardened for RC | decode limits, checksum validation, hostile count/length CLI tests. |
| Public compression gates | ready for RC | `docs/PUBLIC_COMPETITIVE_COMPRESSION_GATE.md`. |
| Deterministic KV stress | ready for RC | ignored stress test plus scheduled workflow. |
| Direct llama.cpp KV ingestion | proven | `docs/LLAMA_CPP_KV_COMPRESSION_REPORT.md`. |
| Broad llama.cpp KV matrix | reproducible | `scripts/llama_cpp_kv_matrix.py`; report generated when local models are present. |
| API/CLI naming | frozen for RC | `docs/API_CLI_FREEZE.md`. |

## Remaining Production Gates

| gate | required before production-complete |
| --- | --- |
| Runtime breadth | Run and publish the llama.cpp KV matrix across at least two model families, three prompt classes, f16/bf16/f32, short and longer prompt contexts, and multiple packed chunk sizes. |
| External runtime integration | Run at least one real live migration with QATQ exact artifacts, source/target checksum validation, rollback behavior, and task-decision preservation. |
| Adapter maintenance | Keep the llama.cpp patch version-pinned and refresh it whenever the target llama.cpp commit changes. |
| Fuzzing | Keep scheduled fuzzing green and review crashes before releases. |
| Security review | Re-run malicious/corrupt QATC tests and fuzz targets before tagging. |
| API stability | Do not publish to crates.io until `docs/API_CLI_FREEZE.md` has been accepted without pending renames. |

## Current Claim

QATQ can exactly compress and restore native tensor bytes, including real
llama.cpp f16 KV-cache captures. On the first real packed all-KV capture, QATQ
beat zstd and lz4 while preserving exact bytes. That is strong evidence for the
codec and adapter path, not yet a universal superiority claim across all models,
prompts, dtypes, and cache layouts.
