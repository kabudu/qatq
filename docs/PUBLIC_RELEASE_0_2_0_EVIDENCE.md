# QATQ 0.2.0 Release Evidence

QATQ 0.2.0 adds a sparsity-gated adjacent-XOR byte-plane Zstd strategy for
native f16 and bf16 tensors. The transform is reversible and retains exact
original bytes. A bounded 4,096-word sample across 64 distributed consecutive
windows avoids full residual allocation for unsuitable inputs, while an
independent full-stream sparsity check prevents a sampled false positive from
selecting the strategy.

This minor release adds exact strategy ID 9 and a public strategy enum variant.
New decoders remain backward-compatible with existing exact payloads. Older
decoders reject payloads that select the new strategy rather than decoding them
incorrectly.

## Required Gates

| Gate | Result |
| --- | --- |
| format and all-target check | pass |
| default test suite | pass, 130 tests |
| locked metadata and duplicate dependency check | pass; no duplicate dependencies |
| RustSec audit | pass, no known vulnerabilities |
| line coverage | pass, 85.72% against a 75% floor |
| public fixture verification | pass |
| production KV benchmark gate | pass for all exact/container rows |
| competitive compression gate | pass for all exact/container rows |
| deterministic KV stress matrix | pass, 4,096 cases and 8,499,064 values |
| pinned llama.cpp adapter matrix | pass, 4 fresh Phi-4 Mini cases |
| native typed fuzz target | pass, 679,630 executions without a crash |
| crate package and crates.io dry run | pass, 66 files and 305.6 KiB compressed |

The stress matrix restored every value exactly. Its aggregate QATQ exact ratio
was `0.1441`, with encode throughput `59.92 ns/value` and decode throughput
`7.03 ns/value`.

## Predictor Experiment

The release-mode synthetic BF16 experiment measured:

- smooth wave ratio `0.230362` to `0.200645`, a 12.9% payload reduction;
- slow ramp ratio `0.040863` to `0.011024`, a 73.0% payload reduction;
- unchanged ratios for the piecewise-KV and random-bit cases;
- no material throughput regression in the measured cases.

The dedicated native f16/bf16 fuzz target completed 679,630 executions without
a crash. Unit coverage includes strategy selection, arbitrary and exhaustive
16-bit word patterns, random fallback, corruption after residual
recompression, truncation rejection, and the bounded distributed sampler.

## Fresh Runtime Matrix

The pinned llama.cpp commit `7992aa7c8` was rebuilt with QATQ's maintained
adapter and tested using Phi-4 Mini native f16 and bf16 packed KV captures at
16- and 64-token budgets. All four cases exported and restored 2,228,224 raw
bytes exactly:

| dtype | QATQ bytes | ratio | selected strategy |
| --- | ---: | ---: | --- |
| f16 | 1,929,497 | 0.8659 | byte-plane-zstd |
| bf16 | 1,600,669 | 0.7184 | byte-plane-zstd |

The predictor did not activate on these real captures because the existing
byte-plane strategy was smaller. This is non-regression evidence for adaptive
selection, not evidence that adjacent-XOR improves every model or tensor
layout.

- matrix report SHA-256:
  `943420bba94d568f7a93d4f2add3419cc50a1051cfe091aa5a5c292a5ba812c9`

This evidence does not broaden QATQ's claims to universal compression
superiority or direct live GPU-memory reduction.
