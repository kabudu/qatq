# White-Paper Results Draft

## Scope

This white-paper draft records what the current standalone QATQ implementation
can claim from its generated public fixture corpus. External runtime evidence is
appendix material rather than a project dependency.

QATQ is not complete as a final public product. The exact phase-2 codec,
production decision API, public fixtures, CI, fuzz scaffold, and release
checklist now exist, but random-access/streaming container work and broader
comparative baselines remain open.

## Experimental Setup

- Repository: QATQ Rust crate and CLI.
- Data: 4 generated public fixtures exported as raw little-endian f32.
- Patterns represented: bfloat16-like KV ramp, bfloat16-like KV wave, noisy
  float32 pass-through, and signed-zero/NaN/infinity exactness stress.
- Gate input: `fixtures/public.manifest`.
- Benchmark mode: `phase2-only`, `--no-synthetic`.
- Production gate: `docs/PUBLIC_BENCHMARK_GATE.md`.
- Comparative baselines: `docs/PUBLIC_COMPARATIVE_BASELINES.md`.
- Quality-proxy experiments: `docs/PUBLIC_QUALITY_EXPERIMENTS.md`.
- Optional external validation: independently supplied runtime fixture
  manifests and result summaries.

## Production Decision Behavior

QATQ phase 2 now exposes a production decision API:

```rust
qatq::try_encode_phase2_lossless_decision_with_config(values, config)
```

The decision has two valid production outcomes:

- `Compressed`: transmit/store a QATQ phase-2 payload.
- `PassThroughRaw`: transmit/store raw f32le bytes with pass-through metadata.

The generated public corpus exercises both outcomes: bfloat16-like KV fixtures
compress, while noisy float32 and exactness-stress fixtures pass through.

## Benchmark Results

| Metric | Result |
| --- | ---: |
| Public fixtures | 4 |
| Compressed decisions | 2 |
| Raw pass-through decisions | 2 |
| Average direct compressed ratio | 0.5009 |
| Average direct compressed reduction | 49.91% |
| Maximum direct decode throughput | 2.0411 ns/value |
| Maximum QATC decode throughput | 2.1015 ns/value |

All phase-2 and QATC rows in the current evidence bundle report exact bit
reconstruction. Pass-through rows are not compression misses; they are the
expected production decision when phase-2 compression would not reduce payload
size.

`docs/PUBLIC_COMPARATIVE_BASELINES.md` contains the public all-codec comparison
against raw f32, software FP8 e4m3, seed lossy-i4, phase1-q4, phase2-lossless,
phase2 exhaustive, and QATC container rows. These are codec-level baselines, not
runtime-native hardware comparisons. The comparison now includes zstd/lz4
raw-f32le byte-compression rows and a `turboquant-q4` base reference row. The
`turboquant-q4` row is a QATQ reference comparator with QJL residual signs for
inner-product estimation, not an official Google implementation.

The QJL reference path uses bounded deterministic signed-Hadamard projections,
so QJL projection and correction are structured O(d log d) operations rather
than dense projection loops. These rows are now suitable as runtime-comparator
measurements, while still remaining a QATQ reference implementation rather than
official Google code.

`docs/PUBLIC_QUALITY_EXPERIMENTS.md` adds the first paper-facing quality proxy:
four deterministic finite inner-product probes per public fixture. It compares
the `turboquant-q4` QJL estimator with the `phase1-q4` decoded-vector inner
product. This supports discussion of codec-level dot-product preservation, but
it still does not establish model-quality or perplexity superiority.

## Gate Policy

The gate policy is now split:

- `production-kv`: production readiness for generated or external KV-like
  tensors. The public CI gate uses portable `50.00 ns/value` direct and QATC
  decode ceilings.
- `latency-budget`: fixed absolute microsecond analysis for small tensors or
  deployment-specific service envelopes. This gate currently fails on large
  tensors because fixed `1000us/1200us` decode ceilings scale poorly with
  value count.

The latency-budget failure should remain visible, but it is not the production
readiness result for large KV tensors.

## Interpretation

The strongest current result is conservative: QATQ phase 2 is an exact,
production-callable storage decision path that can compress bfloat16-like KV
tensors to roughly half of raw f32 size and can safely pass through
compression-negative float32/stress tensors.

This is enough to justify a standalone open-source QATQ release candidate and a
refreshed paper section around reproducible exact transport results. It is not
enough to declare QATQ superior to all standard TurboQuant deployments.

## Remaining Work Before A Public Claim

- Compare against runtime-native quantization baselines.
- Add end-to-end model/task quality experiments before making quality claims
  against Google's TurboQuant paper.
- Add model-quality or task-quality evaluation for lossy Phase 1.
- Expand fuzzing duration in CI and add coverage/supply-chain checks.
- Define a random-access or streaming container/service format if runtime
  paging is required.
