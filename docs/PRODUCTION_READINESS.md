# Production Readiness

QATQ is release-candidate grade, not yet declared production-complete.

## Implemented Evidence

| area | status | evidence |
| --- | --- | --- |
| Exact codec | ready for v0.1.0 | `cargo test`, public fixtures, exact checksum verification. |
| Native typed tensors | ready for v0.1.0 | f32/f16/bf16 encode/decode and QATC tests. |
| QATC container | hardened for v0.1.0 | decode limits, checksum validation, hostile count/length CLI tests. |
| Public compression gates | ready for v0.1.0 | `docs/PUBLIC_COMPETITIVE_COMPRESSION_GATE.md`. |
| Deterministic KV stress | ready for v0.1.0 | ignored stress test plus scheduled workflow. |
| Direct llama.cpp KV ingestion | proven | `docs/LLAMA_CPP_KV_COMPRESSION_REPORT.md`. |
| Broad llama.cpp KV matrix | reproducible | `scripts/llama_cpp_kv_matrix.py`; report generated when local models are present. |
| External live migration | proven for one scoped run | `docs/EXTERNAL_RUNTIME_EVIDENCE.md`. |
| API/CLI naming | accepted for v0.1.0 | `docs/API_CLI_FREEZE.md`. |
| Coverage checks | wired in CI | `.github/workflows/coverage-supply-chain.yml`; line coverage gate `75%`. |
| Supply-chain checks | wired in CI | `.github/workflows/coverage-supply-chain.yml`; RustSec audit, locked metadata, duplicate dependency check. |
| GitHub Release binaries | wired in CI | `.github/workflows/release.yml`; cargo-dist archives, installers, checksums, and signed/notarized macOS zip archives from annotated tags. |
| crates.io publication | manually gated | `.github/workflows/publish-crate.yml`; requires `crates-io` environment approval. |

## Remaining Production Gates

| gate | required before production-complete |
| --- | --- |
| Runtime breadth | Run and publish the llama.cpp KV matrix across at least two model families, three prompt classes, f16/bf16/f32, short and longer prompt contexts, and multiple packed chunk sizes. |
| External runtime breadth | Repeat live-migration evidence across more runtime pairs, model families, context lengths, dtypes, and chunk layouts. |
| Adapter maintenance | Keep the llama.cpp patch version-pinned and refresh it whenever the target llama.cpp commit changes. |
| Fuzzing | Keep scheduled fuzzing green and review crashes before releases. |
| Security review | Re-run malicious/corrupt QATC tests and fuzz targets before tagging. |
| Release publication | Configure `CARGO_REGISTRY_TOKEN`, protect the `crates-io` GitHub environment with required reviewers, and record the manual publication step. |

## Current Claim

QATQ can exactly compress and restore native tensor bytes, including real
llama.cpp f16 KV-cache captures. In a scoped external Rust live-migration run,
standalone QATQ preserved exact task behavior through 128 generated tokens and
transferred 14,004,990 bytes for streamed block artifacts that measured
50,331,648 raw bytes, 20,405,381 zstd bytes, and 28,739,217 lz4 bytes. That is
strong evidence for the codec and adapter path, not yet a universal superiority
claim across all models, prompts, dtypes, and cache layouts.

The production target for the initial release is storage and transfer of
exported KV/tensor state. Live GPU VRAM reduction remains experimental because
it requires runtime KV paging/offload hooks, cold-page scheduling, and latency
proof under generation workloads.
