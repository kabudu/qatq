# PermeantOS QATQ Evidence Handoff - 2026-06-21 Expanded Run

## Status

`analysis-only`

This run produced a broader real PermeantOS tensor evidence bundle against QATQ commit
`69695c7f0cc3e14246d3bf2fda420f4597e42aed`. The expanded manifest contains 50
real KV fixtures: the previous 8 AWS/MLX-vLLM or local MLX captures plus 42 new
live local MLX captures from GPT-2, Phi-3.5 mini, and Qwen2.5 7B.

The important result is nuanced:

- fixture verification passed for all 50 captures;
- every benchmark row reported `exact_bits=true`;
- direct `phase2-lossless` byte-for-byte spot checks passed for GPT-2 and large Phi captures;
- QATC container byte-for-byte spot checks passed for large GPT-2 and large Phi captures;
- both readiness gates failed on performance/compression criteria, not integrity.

This is therefore a useful QATQ engineering handoff, not a production readiness claim.

## Identifiers

- QATQ branch: `permeantos-evidence-20260621`
- QATQ commit: `69695c7f0cc3e14246d3bf2fda420f4597e42aed`
- PermeantOS branch: `codex/pytorch-target-runtime-adapter`
- PermeantOS commit: `e36fec97c79e3ee1a4fb504c37a53f39633f39ec`
- Capture date: `2026-06-21`
- Source runtime for new captures: local Apple Silicon MLX live runtime
- Existing cross-runtime captures retained: local MLX source to AWS vLLM target
- Raw capture storage: local ignored directories `captures/permeantos-20260621/` and `captures/permeantos-20260621-expanded/`

## Host And Toolchain

- OS: `Darwin Mac.lan 25.5.0 Darwin Kernel Version 25.5.0: Mon Apr 27 20:41:26 PDT 2026; root:xnu-12377.121.6~2/RELEASE_ARM64_T8132 arm64`
- CPU: `Apple M4`
- Memory: `25769803776` bytes
- Rust: `rustc 1.94.0 (4a4ef493e 2026-03-02)`
- Cargo: `cargo 1.94.0 (85eff7c80 2026-01-15)`
- MLX-LM: `0.31.3`

## Capture Matrix

- Total fixtures: 50
- Total values: 23,461,888
- Total bytes: 93,847,552
- New capture directory size: about 74 MB
- Models/families represented:
  - Qwen2.5 0.5B, 1.5B, and 7B
  - TinyLlama 1.1B
  - Phi-3.5 mini
  - GPT-2
- Tensor roles: KV `key` and `value`
- Layer coverage in new captures: early, middle, late
- Sequence coverage in new captures: 64, 256, and 512 for GPT-2/Phi; 64 for Qwen2.5 7B
- New runtime dtype evidence:
  - GPT-2: `mlx.core.float32`
  - Phi-3.5 mini: `mlx.core.bfloat16`
  - Qwen2.5 7B: `mlx.core.bfloat16`
- Export dtype for all captures: raw little-endian IEEE-754 f32, no header

## Blocked Or Bounded Capture Attempts

- `google/gemma-2-2b-it`: blocked by Hugging Face gated repo 403 in this environment.
- `mlx-community/gemma-4-12B-it-4bit`: blocked because installed `mlx_lm 0.31.3` does not support `gemma4_unified`.
- `Qwen/Qwen2.5-Coder-7B-Instruct`: bounded attempt was interrupted while still fetching/resolving after roughly 2.5 minutes.

## Validation Summary

- `cargo fmt --check`: passed
- `cargo check`: passed
- `cargo test`: passed, 82 tests total across unit/integration/doc suites
- Fixture audit: passed for 50 external PermeantOS fixtures
- Phase2-only benchmark report: generated in `docs/BENCHMARKS.md`
- Phase2 paper table draft: generated in `docs/PAPER_TABLES.md`
- Full all-codec benchmark: interrupted after an extended CPU-active run; phase2-only report was generated instead because this handoff evaluates `phase2-lossless` and QATC readiness
- Fixed absolute-latency gate: failed
- Throughput-normalized gate: failed

## Exactness Spot Checks

Direct `phase2-lossless` checks:

- `gpt2-seq64-layer0-key`: encode, decode, and `cmp` passed
- `microsoft-phi-3-5-mini-instruct-seq512-layer31-value`: encode, decode, and `cmp` passed

QATC container checks:

- `microsoft-phi-3-5-mini-instruct-seq512-layer0-key`: encode-chunked, decode, and `cmp` passed
- `gpt2-seq512-layer0-key`: encode-chunked, decode, and `cmp` passed

No `exact_bits=false` rows were observed in either gate report.

## Gate Interpretation

The previous Qwen/TinyLlama/Phi bfloat16-derived KV captures and the new
Qwen2.5 7B/Phi captures generally compress to about `0.5000` direct ratio and
about `0.5002` QATC container ratio, while preserving exact bits. This is strong
evidence that the current byte-plane strategy handles many bfloat16-derived KV
captures well.

The new GPT-2 captures are different: they export from `mlx.core.float32` and
remain effectively incompressible under the current phase2 strategy, with ratios
around `1.0000-1.0003` and decode throughput around `4.0-4.3ns/value`. These
rows fail both ratio and throughput gate thresholds while still reconstructing
bit-for-bit.

The Phi 512-token captures remain compressible and exact, but direct encode time
is roughly `9ms`, exceeding the current `5000us` encode cap. QATC container
decode throughput passes for those large Phi rows in the throughput gate.

## Recommendation Back To QATQ

Keep the run as `analysis-only` and use it to guide codec engineering:

1. Preserve the current exact/QATC paths; integrity is holding across all 50 real fixtures.
2. Add strategy detection or a bypass policy for float32-style GPT-2 KV tensors where phase2 adds size and latency instead of compression.
3. Consider readiness policy as a matrix, not a single global threshold: bfloat16-derived KV can use ratio and ns/value targets; float32 KV needs either a different strategy or an explicit no-compress decision.
4. Treat fixed absolute latency as a small-tensor service budget, not the universal readiness gate for large tensors.
5. Re-run after QATQ adds a better float32 strategy, skip/bypass heuristic, or per-runtime profile thresholds.

## Handoff Files

- `fixtures/permeantos.manifest`
- `docs/FIXTURE_AUDIT.md`
- `docs/BENCHMARKS.md`
- `docs/PAPER_TABLES.md`
- `docs/BENCHMARK_GATE.md`
- `docs/BENCHMARK_GATE_THROUGHPUT.md`
- `handoff/permeantos/capture-metadata.json`
- `handoff/permeantos/commands.log`
- dated copies under `handoff/permeantos/runs/20260621-expanded-real-mlx/`
