# Runtime Adapter Contract

QATQ is a standalone codec project. Runtime integrations should depend on the
public QATQ API and should not require runtime-specific fixture paths, metadata,
or daemon behavior.

## Production Chunk Flow

Use QATQ's production chunk helpers:

```rust
let encoded = qatq::try_encode_production_chunk(values)?;
send(encoded.metadata.storage_label(), encoded.metadata.raw_f32le_len, encoded.bytes);
```

Restore with the metadata that traveled with the chunk:

```rust
let values = qatq::restore_production_chunk(&metadata, bytes)?;
```

Supported storage labels:

- `qatq-exact`: `bytes` is a QATQ exact payload.
- `raw-f32le-pass-through`: `bytes` is raw little-endian f32.

Adapters must preserve:

- `storage`
- `raw_f32le_len`
- optional QATQ exact `strategy`
- byte payload without transcoding

## Required Runtime Behavior

- Reject chunks that omit storage metadata.
- Reject unknown storage labels.
- Restore bytes before committing migrated state.
- Compare or checksum restored values when the runtime has an original tensor.
- Treat pass-through chunks as successful QATQ decisions, not compression
  failures.

## Adapter Examples

The `examples/` directory contains a Rust production-chunk example that can be
used as the reference for MLX, vLLM, llama.cpp, or other runtime-specific
adapters.

Runtime-specific adapters should live in their own repositories unless they are
dependency-free examples that compile as part of QATQ CI.

The experimental llama.cpp adapter patch is the current reference for
proof-grade evidence capture. It exposes export, lifecycle trace, attention
trace, materialised attention source trace, page-composed attention source
trace, persistent page-source trace, attention-equivalence fixture, backend
page self-test, attention-path page tensor self-test, GPU page-staging,
persistent page-pool self-test, and
`--qatq-token-timings` hooks. The QATQ evidence
runner folds those outputs into `pages.csv`, `tokens.csv`, and JSON verifier
reports. Those artefacts prove runtime tensor ingestion, exact restore,
attention-path ordering, attention consumption from materialised K/V source
tensors, attention consumption from K/V sources composed out of bounded token
pages, attention consumption from retained backend page tensors filled through
graph-native page copies, attention-path non-host page tensor materialisation,
GPU page-staging with canonical KV off GPU, retained non-host page-pool storage
mechanics, and real decode timing for the patched runner; they do not by
themselves prove native
token-page eviction unless the adapter also replaces persistent whole-buffer KV
allocation with page-granular residency and reclaim.

The strict live-paging gate also rejects page-staging manifests when
`gpu_page_staging_bytes` is greater than or equal to `total_context_bytes`.
This protects against a superficially per-page adapter that stages every page
at once and therefore does not reduce live GPU KV memory.

## Experimental Live VRAM Adapter Flow

Live VRAM reduction is experimental. Runtime adapters must expose it only as an
explicit opt-in path and should surface `qatq::LIVE_VRAM_API_STATUS`, currently
`experimental`, in their own startup logs or diagnostics.

The QATQ-side contract is:

```rust
let mut store = qatq::LiveVramOffloadStore::new_with_shadow_validation(
    qatq::LiveVramLimits::default(),
    1024,
    512 * 1024 * 1024,
    512 * 1024 * 1024,
);

let scheduler = qatq::FixedWindowLiveVramScheduler {
    policy: qatq::LiveVramSchedulerPolicy::default(),
};

let outcome = qatq::try_offload_live_vram_page(
    adapter,
    &mut store,
    &scheduler,
    descriptor,
    scheduler_state,
    qatq::LiveVramLimits::default(),
)?;
```

For proof-grade live VRAM reduction, adapters should use the measured reclaim
controller instead of the lower-level helper:

```rust
let measured = qatq::try_offload_live_vram_page_with_reclaim_check(
    adapter,
    &mut store,
    &scheduler,
    descriptor,
    scheduler_state,
    qatq::LiveVramLimits::default(),
    descriptor.raw_len,
)?;
```

This captures runtime metrics before and after `commit_offload`. If the adapter
does not report at least the configured GPU-byte reduction, QATQ restores the
page and removes the pending store entry before returning an error. That keeps a
failed VRAM-saving attempt from silently becoming a correctness or accounting
hazard.

The measured reclaim controller also requires the adapter to report
`LiveVramGpuAllocationGranularity::PerPage` from
`gpu_allocation_granularity()`. Whole-context, whole-tensor, and unknown
allocation granularities are rejected before snapshotting a page, because a
falling byte counter is not enough evidence that the runtime can reclaim a
logical KV page without freeing or perturbing a larger allocation.

When a runtime path bypasses `try_restore_live_vram_page_from_store` and uploads
stored page bytes directly, it must first build a sealed restore request:

```rust
let request = store.sealed_restore_request(&page_key)?;
let restored = request.restore_bytes(qatq::LiveVramLimits::default())?;
adapter.upload_restored_page(request.metadata(), &restored)?;
```

For out-of-store hand-offs, use the same guard from the seal policy:

```rust
let request = seal_policy.verify_restore_request(
    &metadata,
    stored_bytes,
    &metadata_seal,
    qatq::LiveVramLimits::default(),
)?;
let restored = request.restore_bytes(qatq::LiveVramLimits::default())?;
```

This verifies the keyed metadata seal, stored length, descriptor, storage
metadata, strategy, context, and payload before any bytes are decoded or uploaded
to a runtime/GPU restore slot. Adapters should treat a failed sealed request as a
hard restore rejection and leave the runtime page non-resident.

A runtime adapter implementing `LiveVramRuntimeAdapter` owns the runtime-specific
work:

- produce a stable CPU snapshot for a `KvPageDescriptor`;
- keep the GPU page resident until QATQ has verified and committed the offload
  entry;
- make `commit_offload` atomic from QATQ's point of view: if it returns an
  error, the runtime page must still be resident and QATQ drops the pending
  CPU-side store entry;
- free or reuse the GPU page only after `commit_offload` has accepted the
  encoded page;
- keep the page resident when QATQ cannot produce a compressed payload that is
  smaller than raw bytes and better than the configured general-codec baseline;
  production chunk pass-through is valid for storage/transfer, but strict live
  VRAM offload claims require QATQ-compressed offloaded pages;
- restore bytes into the runtime's expected KV layout before attention consumes
  them;
- verify a `LiveVramSealedRestoreRequest` before any direct restore or GPU
  upload boundary that does not flow through QATQ's store-backed restore
  controller;
- consume restored KV with a page-bounded attention path when claiming peak
  live VRAM reduction. Restoring every needed page into the original whole-cache
  GPU tensor before attention may preserve output, but it does not lower peak
  KV residency;
- pass an explicit native page-streaming proof before claiming production live
  VRAM reduction. For the llama.cpp adapter, this means
  `scripts/llama_cpp_live_vram_evidence.py --require-live-paging
  --require-native-page-streaming`, not only the scoped page-staged gate. A
  `ggml_concat`-composed page source is useful evidence, but it is still not a
  native streaming attention path. CPU custom-op attention can be used as a
  correctness bridge during development, but production evidence must use an
  accelerator-schedulable backend consumer rather than `ggml_custom_4d`.
  Runtime traces should make this explicit
  with `composition` and `native_page_streaming` fields, and production gates
  must reject `composition: "ggml_concat"` plus `native_page_streaming: false`;
- expose bounded K/V page segments from the runtime's actual attention read
  path before building a native page-streaming kernel. Segment traces should use
  `composition: "none"` and should not be treated as production proof until the
  runtime also reports that those segments were consumed by native attention.
  The evidence runner requires key and value segment rows to pair by sequence,
  layer, `n_kv`, token ranges, native-streaming status, attention-consumed
  status, and consumer. A trace that emits an accelerated consumer string but
  mismatches K/V token ranges is rejected before it can count as native
  evidence. The strict native gate also requires the page-bounded
  attention-equivalence report to be present and passing, so native segment
  consumption is only accepted when the bounded-page attention maths has been
  checked against the materialised reference;
- provide a graph-side preflight before enabling the final native consumer. The
  llama.cpp adapter exposes `--qatq-native-page-streaming-preflight`, which
  validates paired K/V segments at the attention graph-build boundary and emits
  `ggml_segmented_kqv_preflight` rows while still reporting
  `native_page_streaming: false` and `attention_consumed: false`. This is a
  compile-checked contract for the future kernel, not production evidence. The
  production gate requires an accelerated runtime consumer such as
  `backend_scheduled_segmented_attention` or
  `backend_scheduled_flattened_flash_attention`;
  `ggml_segmented_kqv_preflight` is explicitly preflight-only;
- provide an executable fail-closed contract before enabling the final native
  consumer. The llama.cpp adapter exposes
  `--qatq-native-page-streaming-contract`, which validates segmented K/Q/V
  geometry and unsupported feature boundaries at the intended backend insertion
  point and emits `ggml_segmented_kqv_contract` rows. This remains a
  contract-only probe; the strict native evidence path now uses
  `--qatq-native-page-streaming-attention-backend-op`, and may add
  `--qatq-native-page-streaming-flatten-flash` for eligible Flash Attention
  layouts;
- provide multi-segment segmented attention before claiming production native
  live VRAM reduction. The current llama.cpp patch has an executable
  multi-segment `GGML_OP_QATQ_SEGMENTED_KQV` route for bounded native
  validation, but production claims still require page-bounded runtime
  equivalence, latency-tail, and memory evidence across real models;
- report restored pages as resident through `is_page_resident`; QATQ keeps the
  CPU-side offload entry and fails closed if a restore returns `Restored` but
  the residency check fails or reports the page absent;
- report runtime-measured restore latency through
  `try_restore_live_vram_page_from_store_with_observed_latency`;
- emit page lifecycle events from the actual attention path and validate them
  with `evaluate_live_vram_event_trace`;
- report its GPU allocation granularity through
  `LiveVramRuntimeAdapter::gpu_allocation_granularity`;
- call `cancel_live_vram_offload` when a request is cancelled while an offload is
  pending or already runtime-committed.

Allocation granularity is part of the correctness contract. QATQ exposes
`LiveVramGpuAllocationGranularity` and
`estimate_live_vram_residency_after_offload` so reports do not confuse logical
offload with real GPU allocation reduction:

- `PerPage`: the runtime can reclaim GPU memory for logical pages after
  `commit_offload`.
- `WholeTensor`, `WholeContext`, or `RuntimeUnknown`: QATQ must not claim page
  offload reduced GPU bytes unless the runtime provides stronger evidence.

The current llama.cpp exporter patch falls into the conservative whole-buffer
class: it can export and compress active KV tensors, and `--no-kv-offload` can
place KV on host memory from the start, but exporting/compressing slices does
not release an already allocated Metal KV buffer.

QATQ includes `live_vram_streaming_attention_reference`,
`live_vram_segment_summary_attention_reference`,
`live_vram_materialized_attention_reference`,
`compare_live_vram_streaming_attention_reference`, and
`compare_live_vram_segment_summary_attention_reference` as executable
references for the next runtime step. The comparison reports check
page-streamed softmax attention and page-summary softmax reduction against
materialised softmax attention, report max absolute and relative error, reject
malformed or non-finite pages, and record peak page KV values versus the
materialised KV value count. The page-summary reducer is the direct QATQ-side
oracle for a native multi-segment `ggml_segmented_kqv` implementation: each
page contributes a local max, denominator, and unnormalised output, then the
runtime combines those summaries with the same stable online softmax
recurrence. For real runtime tensor pages, `decode_tensor_le_bytes_to_f32` and
`compare_live_vram_typed_streaming_attention_reference` accept little-endian
f32, f16, and bf16 page bytes before running the same equivalence gate. Runtime
adapters do not have to call this Rust reference directly, but a production
live-paging adapter should implement the same invariant in the runtime's native
attention path and expose an equivalent pass/fail report.

Adapters can also run the same invariant from files with `qatq-kv-bench`:

```sh
qatq-kv-bench \
  --attention-query f32:captures/query.f32le \
  --attention-key-page f16:captures/key-page-0000.f16le \
  --attention-key-page f16:captures/key-page-0001.f16le \
  --attention-value-page f16:captures/value-page-0000.f16le \
  --attention-value-page f16:captures/value-page-0001.f16le \
  --attention-head-dim <head-dim> \
  --attention-value-dim <value-dim> \
  --attention-tolerance 0.00001 \
  --attention-equivalence-gate \
  --output attention-equivalence.json
```

The output is privacy-safe evidence: it records dtypes, dimensions, page/token
counts, max errors, peak page KV values versus materialised KV values, and the
`segment_summary_reduction: "online-page-summary"` result, but does not write
the query, attention output, or KV payloads. Passing this gate is required
before claiming a runtime attention implementation can operate over bounded
pages. It is not sufficient by itself for a live VRAM reduction claim; that
still requires runtime allocator evidence showing page-granular GPU memory
reclaim and an attention-loop event trace that restores pages before use.

For patched llama.cpp captures, prefer the real fixture bridge:

```sh
python3 scripts/llama_cpp_attention_fixture_gate.py \
  --export-dir captures/llama-kv \
  --attention-fixture-dir captures/attention-fixture \
  --qatq-kv-bench target/release/qatq-kv-bench \
  --output attention-equivalence.json
```

That script reads llama.cpp's exported attention fixture, slices the matching
head out of real K/V token pages, handles the transposed value layout, and runs
the same privacy-safe QATQ gate.

Runtime adapters can validate attention-loop lifecycle events independently of
allocator proof while the native paged-attention implementation is still being
built:

```sh
qatq-kv-bench \
  --live-vram-event-trace-only \
  --live-vram-event-trace captures/attention-event-trace.jsonl \
  --live-vram-event-trace-gate \
  --output attention-event-trace-report.json
```

This accepts either the existing JSON trace object or append-only JSONL events.
The gate checks snapshot/offload/restore/attention ordering, restore checksums,
monotonic tokens, and unfinished offloads. Passing it proves the lifecycle
ordering contract for the emitted trace; it does not prove GPU pages were
actually freed.

When emitting `qatq-kv-bench --live-vram-export-dir` evidence, pass the runtime
KV context bytes and allocation granularity:

```sh
qatq-kv-bench \
  --live-vram-export-dir captures/llama-kv \
  --live-vram-runtime-commit <runtime-commit> \
  --live-vram-adapter-version <adapter-version> \
  --live-vram-model-id <model-id> \
  --live-vram-gpu-context-bytes <runtime-kv-context-bytes> \
  --live-vram-allocation-granularity whole-context \
  --live-vram-prefetch-window-tokens <restore-lead-tokens> \
  --live-vram-restore-bytes-per-token <measured-restore-budget> \
  --output evidence.json
```

This adds a `residency_estimate` block that separates logical offload bytes,
stored CPU bytes, and reclaimable GPU bytes. The optional restore budget adds a
`restore_deadline_report` block that flags pages whose estimated QATQ decode
plus runtime copy cost would exceed their available token lead time. The
prefetch window is also part of the scheduler: pages needed inside hot plus
prefetch stay resident, so adapters should widen it when restore or staging
latency would otherwise land too close to attention use.

Adapters that claim real live VRAM reduction should run the strict gate:

```sh
qatq-kv-bench \
  --live-vram-export-dir captures/llama-kv \
  --live-vram-runtime-commit <runtime-commit> \
  --live-vram-adapter-version <adapter-version> \
  --live-vram-model-id <model-id> \
  --live-vram-gpu-context-bytes <runtime-kv-context-bytes> \
  --live-vram-allocation-granularity per-page \
  --live-vram-restore-bytes-per-token <measured-restore-budget> \
  --live-vram-proof-gate \
  --live-vram-min-gpu-saved-ratio 0.10 \
  --output evidence.json
```

The gate fails unless the report proves page-granular GPU reclaim, non-zero
reclaimable GPU bytes, threshold-clearing GPU savings, exact restore coverage,
restore-deadline compliance, and QATQ beating the best general-codec baseline
on every page. Current llama.cpp Metal exports use `whole-context` for default
GPU KV and `whole-tensor` for mixed KV-layer placement, so they are valid codec
and allocator evidence but should fail this page-level proof gate.

Runtime adapters that implement true token-page paging should also run
`evaluate_live_vram_live_paging_proof_gate`. It combines the memory,
compression, and restore-deadline proof above with a page event trace. The trace
must be emitted from the real runtime path, not reconstructed from offline
manifests. At minimum it should record:

- `Snapshot` when the runtime captures the page bytes QATQ will store;
- `OffloadCommitted` only after the runtime has accepted the QATQ-side offload;
- `RestoreCommitted` after the bytes have been copied back and checksummed;
- `AttentionUse` whenever attention is about to consume the page;
- `Cancelled` when an in-flight or committed offload is rolled back.

The trace proof rejects non-monotonic token order, unknown pages, duplicate
offloads, restore-without-offload, restore checksum mismatch, attention use
while a page is still offloaded, and traces that end with pages still offloaded
when the strict policy is used.

The command-line equivalent is:

```sh
qatq-kv-bench \
  --live-vram-export-dir captures/llama-kv \
  --live-vram-runtime-commit <runtime-commit> \
  --live-vram-adapter-version <adapter-version> \
  --live-vram-model-id <model-id> \
  --live-vram-restore-bytes-per-token <measured-restore-budget> \
  --live-vram-event-trace trace.json \
  --live-vram-live-paging-gate \
  --live-vram-page-seal-key-hex <64-hex-secret> \
  --live-vram-min-gpu-saved-ratio 0.10 \
  --output evidence.json
```

The trace file uses `format: "qatq-live-vram-event-trace-v1"` and an `events`
array. Each event records `token`, `event`, `runtime_id`, `model_id`, `seq_id`,
`layer_id`, `kind`, `token_start`, `token_end`, and `checksum` for snapshot,
offload, and restore events. Valid event labels are `snapshot`,
`offload-committed`, `restore-committed`, `attention-use`,
`cancelled-before-runtime-commit`, and `cancelled-after-runtime-commit`.
The legacy `cancelled` label is still parsed as
`cancelled-after-runtime-commit`, but new adapters should emit the explicit
stage. After-runtime-commit cancellation is a restore path, so proof-grade
traces must include the restored page checksum on that event. Before-runtime
commit cancellation is valid only before the runtime has committed the offload;
after-runtime-commit cancellation is valid only after the page is offloaded.
When the gate passes, `evidence.json` includes an `event_trace_report` block.
Production-shaped runtime-reclaim and strict live-paging evidence should pass
`--live-vram-require-page-seals` and include a keyed `metadata_seal` on every
offloaded page; omit or rotate the seal key from published reports.

For the current mixed KV-layer llama.cpp adapter, use the coarser runtime
reclaim gate:

```sh
qatq-kv-bench \
  --live-vram-export-dir captures/llama-kv-mixed \
  --live-vram-runtime-commit 7992aa7c8 \
  --live-vram-adapter-version qatq-kv-export-7992aa7c8 \
  --live-vram-model-id <model-id> \
  --live-vram-hot-window-tokens 0 \
  --live-vram-restore-bytes-per-token <measured-restore-budget> \
  --live-vram-runtime-reclaim-gate \
  --live-vram-page-seal-key-hex <64-hex-secret> \
  --live-vram-require-page-seals \
  --live-vram-min-gpu-saved-ratio 0.10 \
  --output evidence.json
```

That gate accepts `whole-tensor` allocator evidence only when the manifest
contains runtime-attested `total_context_bytes` and `gpu_context_bytes`, the
reported GPU saving clears the threshold, every page restores exactly, restore
deadlines pass, and every offloaded page is QATQ-compressed and beats the best
general-codec baseline. It proves runtime KV allocation reduction, not
token-page live paging.

For proof-gate mode, allocator fields must be runtime-attested in the export
manifest. QATQ accepts manual allocator CLI flags only for estimates; it will
not let a caller turn replay-only evidence into proof-grade live VRAM evidence
by passing `--live-vram-allocation-granularity per-page` on the command line.

Safe cancellation is stage-dependent:

- `BeforeRuntimeCommit`: QATQ drops the pending store entry; the runtime must
  still have a resident valid GPU page.
- `AfterRuntimeCommit`: QATQ restores the page through the runtime adapter before
  removing the store entry.

Adapters can export operator counters without leaking prompt text, sequence
identifiers, model identifiers, or raw tensor bytes:

```rust
let metrics = qatq::LiveVramOperatorMetrics::from_store(&store, resident_gpu_pages);
let prometheus = metrics.to_prometheus_text();
```

Do not claim live GPU VRAM reduction until the adapter has runtime evidence for
peak VRAM reduction, p95/p99 token latency, restore stalls, exact page restore,
and task/output preservation against the required baselines in
`docs/LIVE_VRAM_REDUCTION.md`.
