# PermeantOS QATQ Decision-Path Run Notes - 2026-06-21

## Status

`passed`

Per the QATQ handoff status rules, this run passes because:

- PermeantOS wired live migration directly to `try_encode_phase2_lossless_decision_with_config`.
- A real PermeantOS live migration exercised `Compressed { payload, strategy, raw_f32le_len }` chunks and committed successfully.
- A real PermeantOS live migration exercised `PassThroughRaw { bytes }` chunks from GPT-2 f32le captures and committed successfully.
- Both live migrations restored exact f32 values before daemon commit validation.
- QATQ throughput gate passed across the external PermeantOS fixture matrix.

The fixed absolute latency gate remains `fail` for large captures because several large tensors exceed fixed decode-us ceilings while preserving exact bits and staying within throughput-style ns/value limits. Treat that as a policy/threshold follow-up, not a correctness failure.

## Revisions Tested

- QATQ branch: `permeantos-evidence-20260621`
- QATQ source commit: `1521821 Add PermeantOS decision-path handoff`
- PermeantOS branch: `codex/pytorch-target-runtime-adapter`
- PermeantOS base commit before local integration changes: `f6cc4a0 Add llama.cpp live KV binding contract`

## PermeantOS Integration

PermeantOS now uses the QATQ library API on the live transfer path:

```rust
qatq::try_encode_phase2_lossless_decision_with_config(
    values,
    qatq::Phase1Config::default(),
)
```

Live payload chunks now carry explicit storage metadata:

- `qatq-phase2` for compressed QATQ payloads.
- `raw-f32le-pass-through` for QATQ no-compress raw f32le bypass.

The target daemon rejects QATQ chunks that omit storage metadata or declare an unsupported representation.

## Live Migration Evidence

| Manifest | Source data | Seq | Layers | Chunks | Compressed | Pass-through | Bytes | Ratio | Commit |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `migration-20260621-192111-40451-manifest.json` | deterministic mock KV | 512 | 4 | 16 | 16 | 0 | 1,221,318 | 0.5824 | passed |
| `migration-20260621-192230-41693-manifest.json` | GPT-2 f32le captures | 64 | 1 | 2 | 0 | 2 | 393,216 | 1.0000 | passed |

Compressed strategy histogram from `migration-20260621-192111-40451`:

- `byte-plane-rle`: 10 chunks
- `byte-plane-blocks`: 6 chunks

The GPT-2 pass-through live run used:

- `captures/permeantos-20260621-expanded/gpt2-seq64-layer0-key.f32le`
- `captures/permeantos-20260621-expanded/gpt2-seq64-layer0-value.f32le`

## QATQ Validation Summary

- `cargo fmt --check`: pass
- `cargo check`: pass
- `cargo test`: pass, 89 tests total across lib/bin/integration/doc targets
- fixture audit: regenerated `docs/FIXTURE_AUDIT.md`
- phase2 benchmark report: regenerated `docs/BENCHMARKS.md`
- paper tables: regenerated `docs/PAPER_TABLES.md`
- throughput gate: pass, regenerated `docs/BENCHMARK_GATE_THROUGHPUT.md`
- fixed absolute latency gate: fail by fixed decode-us ceilings on large tensors, regenerated `docs/BENCHMARK_GATE.md`

## PermeantOS Validation Summary

- `cargo fmt --check`: pass
- `cargo check`: pass
- `cargo test`: pass
- live QATQ compressed migration: pass
- live QATQ pass-through migration: pass

## Follow-Up Expectations For QATQ

- Keep the throughput gate as the primary production gate for mixed-size external KV captures.
- Revisit the fixed absolute decode-us gate policy; it currently penalizes large tensors even when throughput and exactness are healthy.
- Use PermeantOS live manifests above as the production integration proof for both decision branches.
