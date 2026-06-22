# Public Compression Summary

This table is the short release-facing view of the public fixture results. It
uses the generated public corpus and reports size ratio versus raw little-endian
f32 bytes. Lower is smaller.

The strongest current QATQ claim is conservative: `phase2-lossless` provides
bit-identical f32 reconstruction and, on the public corpus, beats the
general-purpose `zstd-raw-f32le` and `lz4-raw-f32le` baselines by selecting the
smallest exact Phase 2 candidate, including tensor-aware byte-plane entropy
coding and reversible quaternion-chain residual coding.

## Exact Compression At A Glance

| dataset | values | QATQ exact strategy | QATQ exact ratio | QATQ reduction vs raw | QATC ratio | zstd raw ratio | lz4 raw ratio | exact? |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | --- |
| bf16-kv-ramp-64x8x16 | 8,192 | byte-plane-zstd | 0.3817 | 61.83% | 0.3828 | 0.4665 | 0.6901 | yes |
| bf16-kv-wave-128x8x16 | 16,384 | quaternion-chain-zstd | 0.1153 | 88.47% | 0.1158 | 0.2900 | 0.4693 | yes |
| f32-noisy-pass-through-64x12x16 | 12,288 | byte-plane-zstd | 0.6532 | 34.68% | 0.6540 | 0.9061 | 1.0040 | yes |
| stress-signed-zero-nan-inf | 4,096 | quaternion-chain-zstd | 0.0121 | 98.79% | 0.0143 | 0.0413 | 0.0673 | yes |

## What This Shows

| claim | supported by public fixtures? | evidence |
| --- | --- | --- |
| QATQ Phase 2 is lossless on the public corpus. | yes | All `phase2-lossless` and `phase2-lossless-container` rows report exact bit reconstruction. |
| QATQ Phase 2 compresses bfloat16-like KV fixtures below zstd/lz4 raw-f32 baselines. | yes | Ratios `0.3817` and `0.1153`, both below the best zstd/lz4 raw-f32 baseline for those rows. |
| Reversible quaternion-chain coding helps exact compression on public fixtures. | yes | `quaternion-chain-zstd` wins on the wave and exactness-stress fixtures while preserving f32 bits. |
| QATQ Phase 2 exact compression is competitive across all public fixtures. | yes | The competitive compression gate passes every public row where Phase 2 selects a compression strategy. |
| QATQ avoids loss of f32 bit identity while improving size. | yes | All rows are exact, including signed-zero/NaN/Inf stress values. |
| QATQ beats lossy quantizers on size. | not the Phase 2 claim | `turboquant-q4`, `phase1-q4`, FP8, and int4 rows are lossy comparator context, not lossless QATQ evidence. |

## Source Tables

- Full all-codec rows: [PUBLIC_COMPARATIVE_BASELINES.md](PUBLIC_COMPARATIVE_BASELINES.md)
- Paper-style tables: [PUBLIC_COMPARATIVE_TABLES.md](PUBLIC_COMPARATIVE_TABLES.md)
- Phase 2 focused rows: [PUBLIC_BENCHMARKS.md](PUBLIC_BENCHMARKS.md)
- Production gate: [PUBLIC_BENCHMARK_GATE.md](PUBLIC_BENCHMARK_GATE.md)
- Competitive compression gate: [PUBLIC_COMPETITIVE_COMPRESSION_GATE.md](PUBLIC_COMPETITIVE_COMPRESSION_GATE.md)
