# PermeantOS QATQ Evidence Handoff - 2026-06-21

## Status

`analysis-only`

Real PermeantOS tensor captures were exported, registered, audited, benchmarked,
and spot-checked through the current QATQ exact paths. Every measured external
fixture reported `exact_bits=true`, and both required byte-for-byte `cmp` spot
checks passed. The original absolute-latency readiness gate still fails on the
two largest Phi captures, not on compression ratio, encode latency, or
reconstruction integrity. A throughput-normalized decode gate passes all 8 real
captures.

This is therefore useful codec-development evidence, but it is not a readiness
claim for production QATQ integration.

## Identifiers

- QATQ branch: `master`
- QATQ commit: `407fdf5bf96f0d3a5129716492fd2330d5a7bf8d`
- PermeantOS branch: `codex/model-family-validation-matrix`
- PermeantOS commit: `654d5039b5a73de21f7ddcffa845b0ca87796f2c`
- Capture date: `2026-06-21`
- Source runtime: local Apple Silicon MLX source exporter
- Destination runtime: AWS NVIDIA vLLM target for the Qwen/TinyLlama migration captures; local MLX tensor extraction for the Phi stress captures
- Target runtime version recorded in PermeantOS docs: vLLM `0.23.0`
- Raw capture storage: local ignored directory `captures/permeantos-20260621/`
- Raw capture git policy: not committed; stable private references are the local paths plus audit fingerprints and SHA-256 hashes below

## Host And Toolchain

- OS: `Darwin Mac.lan 25.5.0 Darwin Kernel Version 25.5.0: Mon Apr 27 20:41:26 PDT 2026; root:xnu-12377.121.6~2/RELEASE_ARM64_T8132 arm64`
- CPU: `Apple M4`
- GPU: `Apple M4`, 10 cores, Metal 4
- Memory: `25769803776` bytes
- Rust: `rustc 1.94.0 (4a4ef493e 2026-03-02)`
- Cargo: `cargo 1.94.0 (85eff7c80 2026-01-15)`

## Captures

The extractor payloads serialize tensors as JSON numeric arrays with `name`,
`shape`, and `data` fields. They do not serialize the original runtime tensor
dtype, so fixture notes record `source_payload_dtype=json-number-array` and
`runtime_tensor_dtype=not-serialized-in-extractor-payload`. The handoff export
format is raw little-endian IEEE-754 f32 with no header.

| fixture | model | tensor | shape | values | bytes | sha256 |
| --- | --- | --- | --- | ---: | ---: | --- |
| `qwen25-05b-long-1920-layer0-key` | `Qwen/Qwen2.5-0.5B-Instruct` | `layer.0.key` | `[1920, 2, 64]` | `245760` | `983040` | `7371c81877edd34cb94c9d029bbbf5ba7b42aa0a2732068ac609514b1dd03b6b` |
| `qwen25-05b-long-1920-layer23-value` | `Qwen/Qwen2.5-0.5B-Instruct` | `layer.23.value` | `[1920, 2, 64]` | `245760` | `983040` | `f6e4ff4bd0a3cba7f27b3d8aeacfa017296fa4346e516c14945687eadc37883c` |
| `tinyllama-11b-1984-layer0-key` | `TinyLlama/TinyLlama-1.1B-Chat-v1.0` | `layer.0.key` | `[1984, 4, 64]` | `507904` | `2031616` | `e34bc6f4aa0d19edda4bb21eb25bb9fa7a8bb18a97e6b00db8c67a3026bfd497` |
| `tinyllama-11b-1984-layer21-value` | `TinyLlama/TinyLlama-1.1B-Chat-v1.0` | `layer.21.value` | `[1984, 4, 64]` | `507904` | `2031616` | `7c2440639e95caf1fa0b76b77d6ee57cfb4bccf2f95ae95d949bc356568e178b` |
| `qwen25-15b-1984-layer0-key` | `Qwen/Qwen2.5-1.5B-Instruct` | `layer.0.key` | `[1984, 2, 128]` | `507904` | `2031616` | `73677b57d68c4f80e7e0d5a27dcc00d983d48b33b62a3235922a4244dddbe1f3` |
| `qwen25-15b-1984-layer27-value` | `Qwen/Qwen2.5-1.5B-Instruct` | `layer.27.value` | `[1984, 2, 128]` | `507904` | `2031616` | `1269871639a33545ad9f33d590773328367b2a508f15e7a67ffa0eeb23abcfda` |
| `phi35-mini-256-layer0-key` | `microsoft/Phi-3.5-mini-instruct` | `layer.0.key` | `[256, 32, 96]` | `786432` | `3145728` | `8b1bd5d62f904efca2976a8b5da74dfaaad06e79855a9ba032d5792728ae00cf` |
| `phi35-mini-256-layer31-value` | `microsoft/Phi-3.5-mini-instruct` | `layer.31.value` | `[256, 32, 96]` | `786432` | `3145728` | `d56cf00477dbfbb6027a25d7b5c4092b810f1ef37cfc40467141a4269b773378` |

## Generated Reports

- `fixtures/permeantos.manifest`
- `docs/FIXTURE_AUDIT.md`
- `docs/BENCHMARKS.md`
- `docs/PAPER_TABLES.md`
- `docs/BENCHMARK_GATE.md`
- `docs/BENCHMARK_GATE_THROUGHPUT.md`
- `handoff/permeantos/capture-metadata.json`
- `handoff/permeantos/commands.log`
- `handoff/permeantos/manifest-add-commands.txt`

## Validation Summary

- `cargo fmt --check`: passed
- `cargo check`: passed
- `cargo test`: passed in the original handoff run; rerun after QATQ-side
  optimization is recorded in `docs/VALIDATION.md`
- Fixture audit: passed for 8 external PermeantOS fixtures, 4,096,000 f32 values, 16,384,000 raw bytes
- Benchmark report: generated for synthetic controls plus 8 external fixtures
- Absolute readiness gate: failed; every failing row had `exact_bits=true`, but
  fixed decode latency thresholds were exceeded on the two 786,432-value Phi
  captures
- Throughput-normalized gate: passed with max direct decode `2.10ns/value` and
  max QATC decode `2.20ns/value`
- Direct `phase2-lossless` spot check: `cmp` passed for `qwen25-05b-long-1920-layer0-key`
- QATC spot check: `cmp` passed for `phi35-mini-256-layer0-key`

## Gate Interpretation

The current `phase2-lossless` path selects `byte-plane-blocks` for all 8 real
PermeantOS KV captures. It compresses them to `0.5000` of raw f32 for direct
payloads and about `0.5002` including QATC container overhead. That is roughly
49.98-50.00% lossless size reduction versus raw f32 on these fixtures.

The absolute gate failed because its fixed decode thresholds are not normalized
by tensor size:

- 245,760-value and 507,904-value captures pass direct and QATC checks.
- 786,432-value Phi captures pass exactness, ratio, and encode checks, but
  decode at roughly `1.45-1.51ms`, above the fixed `1000us` direct and `1200us`
  QATC thresholds.
- Normalized decode throughput is stable across the fixture set, approximately
  `1.82-1.89ns/value`, and passes the throughput-normalized gate.

This gives QATQ a clearer next development target: keep bit-exact
reconstruction and the real-KV compression ratio while deciding whether
PermeantOS readiness should use fixed per-capture latency caps, normalized
throughput caps, or a runtime-specific service-level budget.

## Limitations

- No QATQ runtime integration was performed in PermeantOS during this handoff.
- Raw capture files are local ignored artifacts, not committed to git.
- The source extractor payloads do not serialize original runtime dtype, only
  JSON numeric tensor data and shapes.
- The Phi fixtures are local MLX extraction stress captures, not completed AWS
  MLX-to-vLLM migrations.
- The readiness status remains `analysis-only` until QATQ and PermeantOS accept
  the readiness policy. The original absolute-latency gate still fails; the
  throughput-normalized gate passes.
