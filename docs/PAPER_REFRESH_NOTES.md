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
- Public quality-proxy experiments: `docs/PUBLIC_QUALITY_EXPERIMENTS.md`
- Public task-quality experiments: `docs/PUBLIC_TASK_QUALITY_EXPERIMENTS.md`
- Public production KV gate: `docs/PUBLIC_BENCHMARK_GATE.md`
- Public competitive compression gate: `docs/PUBLIC_COMPETITIVE_COMPRESSION_GATE.md`
- Runtime model-output task experiment:
  `docs/RUNTIME_TASK_QUALITY_EXPERIMENTS.md`
- Optional external validation: independently supplied runtime fixture
  manifests and result summaries

## Claims Supported By Current Evidence

The following claims are supported by the current QATQ repository evidence:

- QATQ can generate, verify, benchmark, and gate its own public fixture corpus.
- QATQ exact reconstructs every generated public fixture bit-for-bit.
- Generated public fixtures compress below the best zstd/lz4 raw-f32 baseline
  for each row while preserving exact f32 reconstruction.
- The production API keeps raw f32 pass-through available for future
  compression-negative tensors.
- The public production KV throughput gate passes on the generated corpus.
- The public competitive compression gate passes on the generated corpus.
- QATQ exact preserves deterministic top-1 retrieval decisions on the public
  task-quality experiment because it reconstructs the finite fixture records
  exactly.
- QATQ exact preserves a local Ollama model-output relevance-score task:
  `phi4-mini:latest` generated score tensors are ingested as runtime fixtures,
  reconstructed bit-for-bit, and keep raw top-1 task decisions unchanged.
- `turboquant-q4` and `phase1-q4` now have deterministic codec-level
  inner-product preservation probes over the generated public corpus.

The following claims are not yet supported and should not appear as conclusions:

- QATQ is universally superior to all TurboQuant variants.
- QATQ improves model quality or perplexity for lossy inference workloads.
- QATQ's public retrieval proxy or local model-output score task is a substitute
  for language-model perplexity or direct live KV-cache evaluation.
- QATQ beats hardware FP8 or production runtime-specific codecs.
- The current `QATC` container is a random-access or streaming service format.

## Current Result Summary

Generated from `docs/BENCHMARKS.md` after the gate-policy split:

| Metric | Result |
| --- | ---: |
| Generated public fixtures | 4 |
| Compression-positive fixtures | 4 |
| Pass-through decisions | 0 |
| Average direct ratio on public fixtures | 0.2906 |
| Average direct size reduction on public fixtures | 70.94% |
| Maximum direct decode throughput on public fixtures | 10.0976 ns/value |
| Maximum QATC decode throughput on public fixtures | 13.9664 ns/value |

The public production gate uses intentionally portable `50.00 ns/value` direct
and QATC decode ceilings so CI can run on shared hosts. Research reports may
include tighter local hardware numbers separately.

## Suggested Paper Edits

Replace synthetic-only result language with a real-data section:

```text
QATQ generates a deterministic public tensor corpus that exercises both
compression-positive bfloat16-like KV structure and exactness stress payloads.
On this corpus, QATQ exact reconstructs every row bit-for-bit and compresses
all generated public fixtures at an average 0.2906 direct ratio versus raw f32,
below the best zstd/lz4 raw-f32 baseline for each row. The reversible
quaternion-chain residual candidate is selected on the public wave and
exactness-stress fixtures; byte-plane zstd remains smaller on the ramp and
noisy fixtures.
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

Use `docs/PUBLIC_PAPER_TABLES.md`, `docs/PUBLIC_QUALITY_EXPERIMENTS.md`, and
`docs/PUBLIC_TASK_QUALITY_EXPERIMENTS.md` as raw table sources and derive compact
tables for the refreshed paper:

- fixture inventory by generated pattern and dtype;
- raw/fp8/lossy/phase1/qatq_exact comparative baseline rows;
- QATQ exact compressed versus pass-through decision counts;
- lossless size ratio and decode throughput for compressed real KV tensors;
- TurboQuant QJL versus quaternion-overlay inner-product proxy error;
- QATQ exact retrieval-task agreement, with lossy comparators clearly
  separated;
- local Ollama model-output score-task agreement;
- optional external runtime evidence for compressed and pass-through paths.

Keep the full 50-row benchmark table in the companion white-paper or appendix
rather than in the main paper body.
