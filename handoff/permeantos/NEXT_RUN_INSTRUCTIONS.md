# PermeantOS Next Run Instructions

## Purpose

Test QATQ as a production storage decision path inside PermeantOS, not just as
an offline benchmark tool.

QATQ has now cleared the current large-tensor throughput gate on the 50-fixture
PermeantOS evidence set. The next run must verify that PermeantOS can use the
QATQ library decision API end to end:

- compress tensors that select a compression-positive Phase 2 strategy;
- pass through tensors that select `raw-bits`;
- restore the exact original tensor bytes;
- record end-to-end runtime overhead from PermeantOS, not only QATQ
  microbenchmarks.

## Required QATQ State

Use QATQ branch:

```text
permeantos-evidence-20260621
```

Use commit at or after:

```text
e2c252b Optimize phase2 byte-plane block encode
```

Before running PermeantOS integration tests, verify QATQ locally:

```sh
cd /Users/kabudu/projex/qatq
git rev-parse --abbrev-ref HEAD
git rev-parse HEAD
cargo fmt --check
cargo check
cargo test
```

Current expected QATQ validation:

- `cargo test`: 89 tests pass.
- `docs/BENCHMARK_GATE_THROUGHPUT.md`: status `pass`.
- `docs/BENCHMARK_GATE.md`: may still fail on fixed absolute decode-us
  ceilings for large tensors. Treat that report as small-tensor/service-budget
  analysis, not the large-tensor readiness gate.

## Integration Contract

PermeantOS should call:

```rust
qatq::try_encode_phase2_lossless_decision_with_config
```

Do not call the CLI for the production path. The CLI remains useful for
independent spot checks and artifact debugging.

Expected dependency options:

```toml
# Preferred for this run if PermeantOS can depend on the local checkout.
qatq = { path = "/Users/kabudu/projex/qatq" }

# Alternative if testing from GitHub.
qatq = { git = "https://github.com/kabudu/qatq", branch = "permeantos-evidence-20260621" }
```

Core storage decision:

```rust
use qatq::{
    try_encode_phase2_lossless_decision_with_config,
    Phase1Config,
    Phase2EncodeDecision,
};

let decision = try_encode_phase2_lossless_decision_with_config(
    &values_f32,
    Phase1Config::default(),
)?;

match decision {
    Phase2EncodeDecision::Compressed {
        payload,
        strategy,
        raw_f32le_len,
    } => {
        // Store/transmit `payload` as a QATQ Phase 2 artifact.
        // Record `strategy.as_str()` and `raw_f32le_len` in PermeantOS metadata.
    }
    Phase2EncodeDecision::PassThroughRaw { bytes } => {
        // Store/transmit raw little-endian f32 bytes.
        // Record this as QATQ pass-through, not as a compression failure.
    }
}
```

Restore behavior:

- For `Compressed`, call `qatq::decode(&payload)` or
  `qatq::decode_phase2_lossless(&payload)` and compare restored f32 bits with
  the original tensor.
- For `PassThroughRaw`, import the raw little-endian f32 bytes directly and
  compare byte-for-byte with the original raw export.
- If PermeantOS stores a wrapper enum or transport envelope, it must explicitly
  distinguish `qatq-phase2` from `raw-f32le-pass-through`.

Do not persist a `raw-bits` QATQ payload as the production representation.
`raw-bits` means QATQ decided compression is not useful for that tensor.

## Required Tensor Cases

Run at minimum these two classes:

1. Compression-positive bfloat16-derived KV tensors
   - Expected decision: `Compressed`
   - Expected strategy: usually `byte-plane-blocks`
   - Expected ratio: roughly `0.5000` direct, `0.5002` QATC/container-level
     when measured as a file artifact
   - Existing evidence examples:
     - `microsoft-phi-3-5-mini-instruct-seq512-layer0-key`
     - `microsoft-phi-3-5-mini-instruct-seq512-layer31-value`
     - Qwen2.5 or TinyLlama bfloat16-derived KV captures

2. Compression-negative float32 KV tensors
   - Expected decision: `PassThroughRaw`
   - Expected strategy in benchmark reports: `raw-bits`
   - Expected production action: bypass QATQ compression and store raw f32le
   - Existing evidence examples:
     - `gpt2-seq64-layer0-key`
     - `gpt2-seq512-layer0-key`

Preferred matrix:

- GPT-2 seq64 and seq512 K/V: prove pass-through works for small and larger
  float32 KV.
- Phi-3.5 mini seq64, seq256, and seq512 K/V: prove compressed path scales.
- Qwen2.5 7B seq64 K/V: prove the path is not Phi-only.
- One migration-style source/destination pair if PermeantOS can produce both.

## Measurements To Record

Record PermeantOS end-to-end timings separately from QATQ microbenchmarks:

- tensor export or view materialization time;
- QATQ decision encode time;
- storage/transmit byte count;
- restore/decode/import time;
- byte-for-byte exactness result;
- selected decision: `Compressed` or `PassThroughRaw`;
- selected Phase 2 strategy for compressed tensors;
- original dtype before f32 export/import;
- tensor shape and value count;
- model, layer, role, context length, and runtime path.

Use monotonic timing around the PermeantOS integration boundary. The goal is to
learn total integration cost, not only codec kernel cost.

## Required Validation

For every tested tensor:

- original raw f32le bytes must be recoverable;
- restored raw f32le bytes must compare byte-for-byte equal;
- decision metadata must match the stored representation;
- compression-positive tensors must not silently fall back to raw unless QATQ
  returns `PassThroughRaw`;
- pass-through tensors must not be counted as QATQ compression failures.

Also rerun QATQ offline reports against any new captures:

```sh
cd /Users/kabudu/projex/qatq
cargo run -- fixture verify \
  --manifest fixtures/permeantos.manifest \
  --output docs/FIXTURE_AUDIT.md

cargo run --release --bin qatq-bench -- \
  --phase2-only \
  --no-synthetic \
  --output docs/BENCHMARKS.md \
  --paper-output docs/PAPER_TABLES.md \
  --manifest fixtures/permeantos.manifest

cargo run --release --bin qatq-bench -- \
  --phase2-only \
  --no-synthetic \
  --manifest fixtures/permeantos.manifest \
  --gate-output docs/BENCHMARK_GATE_THROUGHPUT.md \
  --gate-require-external \
  --gate-policy production-kv \
  --max-phase2-ratio 0.95 \
  --max-phase2-encode-us 5000 \
  --max-phase2-decode-ns-per-value 3.00 \
  --max-phase2-container-ratio 0.96 \
  --max-phase2-container-decode-ns-per-value 3.00
```

Run the fixed absolute-latency gate too, but do not use it as the large-tensor
readiness decision:

```sh
cargo run --release --bin qatq-bench -- \
  --phase2-only \
  --no-synthetic \
  --manifest fixtures/permeantos.manifest \
  --gate-output docs/BENCHMARK_GATE.md \
  --gate-require-external \
  --gate-policy latency-budget \
  --max-phase2-ratio 0.95 \
  --max-phase2-encode-us 5000 \
  --max-phase2-decode-us 1000 \
  --max-phase2-container-ratio 0.96 \
  --max-phase2-container-decode-us 1200
```

Expected current interpretation:

- `BENCHMARK_GATE_THROUGHPUT.md` should pass for the known 50-fixture set.
- `BENCHMARK_GATE.md` may fail on large tensors because fixed decode-us
  ceilings do not scale with value count.

## Deliverables Back To QATQ

Return or update:

- `fixtures/permeantos.manifest`
- `docs/FIXTURE_AUDIT.md`
- `docs/BENCHMARKS.md`
- `docs/PAPER_TABLES.md`
- `docs/BENCHMARK_GATE.md`
- `docs/BENCHMARK_GATE_THROUGHPUT.md`
- `handoff/permeantos/RUN_NOTES.md`
- `handoff/permeantos/commands.log`
- PermeantOS integration patch or branch name
- end-to-end decision/timing table
- raw captures or stable private artifact references

The end-to-end table must include at least:

```text
model | tensor | values | original dtype | decision | strategy | raw bytes | stored bytes | encode/decision ms | restore ms | exact cmp | notes
```

## Status Rules

Use these statuses in `RUN_NOTES.md`:

- `passed`: throughput-normalized gate passed, end-to-end exactness passed, and
  both `Compressed` and `PassThroughRaw` production paths were exercised.
- `analysis-only`: exactness passed but integration overhead, missing metadata,
  incomplete model coverage, or fixed absolute-latency policy still needs QATQ
  analysis.
- `failed`: any exactness mismatch, corrupt restore, wrong decision metadata,
  or unreproducible capture path.

Any exactness failure is a hard failure. Do not continue performance tuning
until QATQ has the smallest reproducible fixture and command log.

## QATQ Follow-Up Expected

After PermeantOS returns results, QATQ should:

- inspect whether production pass-through matches benchmark `raw-bits` rows;
- inspect whether compressed runtime overhead tracks the offline microbenchmarks;
- update paper/white-paper evidence tables with real end-to-end timings;
- decide whether a chunked decision API is needed for tensors above
  `MAX_VALUES_PER_PAYLOAD`;
- decide whether fixed absolute-latency gate thresholds should be retired,
  scoped to small tensors, or replaced by ns/value policy.
