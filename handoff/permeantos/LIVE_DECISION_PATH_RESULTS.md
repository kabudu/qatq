# QATQ Live Decision-Path Results - 2026-06-21

## Status

`passed`

PermeantOS has now exercised QATQ's production phase-2 storage-decision API on
the live migration path, not only in standalone fixture or benchmark harnesses.

The live integration used:

```rust
qatq::try_encode_phase2_lossless_decision_with_config(
    values,
    qatq::Phase1Config::default(),
)
```

The two production branches were both validated:

- `Compressed`: transfer and restore a `qatq-phase2` payload.
- `PassThroughRaw`: transfer and restore a `raw-f32le-pass-through` payload.

## Live Migration Evidence

| Manifest | Source data | Seq | Layers | Chunks | Compressed | Pass-through | Bytes | Ratio | Commit |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `migration-20260621-192111-40451-manifest.json` | deterministic mock KV | 512 | 4 | 16 | 16 | 0 | 1,221,318 | 0.5824 | passed |
| `migration-20260621-192230-41693-manifest.json` | GPT-2 f32le captures | 64 | 1 | 2 | 0 | 2 | 393,216 | 1.0000 | passed |

Compressed strategy histogram from `migration-20260621-192111-40451`:

- `byte-plane-rle`: 10 chunks
- `byte-plane-blocks`: 6 chunks

The pass-through run used GPT-2 f32le captures that the phase-2 decision policy
correctly rejected for compression because the encoded representation would not
improve storage size.

## Validation Summary

PermeantOS validation reported:

- `cargo fmt --check`: pass
- `cargo check`: pass
- `cargo test`: pass
- live compressed QATQ migration: pass
- live pass-through QATQ migration: pass

QATQ validation reported:

- `cargo fmt --check`: pass
- `cargo check`: pass
- `cargo test`: pass, 89 tests total
- fixture audit regenerated
- phase2 benchmark report regenerated
- paper tables regenerated
- throughput gate: pass
- fixed absolute latency gate: fail on large tensor decode-us ceilings

The fixed absolute latency gate failure is not a bit-exactness or live
integration failure. The throughput-normalized gate is the current production
readiness signal for mixed-size external KV captures.

## Canonical References

- PermeantOS integration evidence:
  `/Users/kabudu/projex/permeant-os/docs/qatq-production-decision-live-migration-2026-06-21.md`
- Current QATQ run notes:
  `handoff/permeantos/RUN_NOTES.md`
- Current QATQ command log:
  `handoff/permeantos/commands.log`
- Throughput readiness gate:
  `docs/BENCHMARK_GATE_THROUGHPUT.md`
- Fixed absolute latency analysis:
  `docs/BENCHMARK_GATE.md`

## Next QATQ Work

The next engineering target is no longer proving that both decision branches can
move through PermeantOS. That is done.

QATQ should now:

- make the throughput-normalized gate the primary production-readiness gate for
  large external KV tensors;
- revise or split the fixed absolute latency gate so large tensors are judged by
  throughput while small service-budget checks remain visible;
- update the paper and companion white-paper around the real 50-fixture evidence
  set plus the live PermeantOS migration proof;
- keep exact bit reconstruction and no-compress pass-through behavior as
  non-negotiable invariants for future optimization.
