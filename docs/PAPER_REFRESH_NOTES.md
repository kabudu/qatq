# Paper Refresh Notes

## Evidence State

The refreshed paper should lead with QATQ's generated public fixture corpus so
the project is independently reproducible. External runtime evidence can be
presented as optional validation, not as a dependency.

Current source files:

- Public fixture audit: `docs/PUBLIC_FIXTURE_AUDIT.md`
- Public benchmark table: `docs/PUBLIC_BENCHMARKS.md`
- Public paper table inputs: `docs/PUBLIC_PAPER_TABLES.md`
- Public comparative baselines: `docs/PUBLIC_COMPARATIVE_BASELINES.md`
- Public production KV gate: `docs/PUBLIC_BENCHMARK_GATE.md`
- Optional external validation: archived runtime-integration handoff records

## Claims Supported By Current Evidence

The following claims are supported by the current QATQ repository evidence:

- QATQ can generate, verify, benchmark, and gate its own public fixture corpus.
- QATQ phase 2 reconstructs every generated public fixture bit-for-bit.
- Compression-positive generated bfloat16-like KV fixtures compress to about
  half of raw f32 size while preserving exact f32 reconstruction.
- Compression-negative generated float32/stress fixtures are correctly passed
  through rather than being counted as failed compression.
- The public production KV throughput gate passes on the generated corpus.

The following claims are not yet supported and should not appear as conclusions:

- QATQ is universally superior to all TurboQuant variants.
- QATQ improves model quality or perplexity for lossy inference workloads.
- QATQ beats zstd, lz4, hardware FP8, or production runtime-specific codecs.
- The current `QATC` container is a random-access or streaming service format.

## Current Result Summary

Generated from `docs/BENCHMARKS.md` after the gate-policy split:

| Metric | Result |
| --- | ---: |
| Generated public fixtures | 4 |
| Compression-positive fixtures | 2 |
| Pass-through/exactness fixtures | 2 |
| Average direct ratio on compressed fixtures | 0.5009 |
| Average direct size reduction on compressed fixtures | 49.91% |
| Maximum direct decode throughput on compressed fixtures | 2.0411 ns/value |
| Maximum QATC decode throughput on compressed fixtures | 2.1015 ns/value |

The public production gate uses intentionally portable `50.00 ns/value` direct
and QATC decode ceilings so CI can run on shared hosts. Research reports may
include tighter local hardware numbers separately.

## Suggested Paper Edits

Replace synthetic-only result language with a real-data section:

```text
QATQ generates a deterministic public tensor corpus that exercises both
compression-positive bfloat16-like KV structure and compression-negative
float32/stress payloads. On this corpus, QATQ phase 2 reconstructs every row
bit-for-bit, compresses the bfloat16-like fixtures at an average 0.5009 direct
ratio versus raw f32, and correctly selects raw f32le pass-through for the
compression-negative fixtures.
```

Add a limitations paragraph:

```text
These public results establish reproducible exact transport behavior, not lossy
model-quality superiority. External runtime captures are validation appendices,
not project dependencies. Comparisons against standard
TurboQuant, zstd/lz4, hardware FP8, and runtime-native KV representations
remain future work. The current QATC artifact is sequential and should not be
described as a random-access service container.
```

## Table Plan

Use `docs/PUBLIC_PAPER_TABLES.md` as the raw table source and derive four compact
tables for the refreshed paper:

- fixture inventory by generated pattern and dtype;
- raw/fp8/lossy/phase1/phase2 comparative baseline rows;
- phase-2 compressed versus pass-through decision counts;
- lossless size ratio and decode throughput for compressed real KV tensors;
- optional external runtime evidence for compressed and pass-through paths.

Keep the full 50-row benchmark table in the companion white-paper or appendix
rather than in the main paper body.
