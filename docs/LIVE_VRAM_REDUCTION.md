# Live VRAM Reduction Implementation Plan

This document tracks the experimental roadmap work needed to make QATQ reduce
live GPU VRAM usage during LLM inference. It is deliberately separate from the
current product claim: QATQ is already useful for storage and transfer of
exported KV/tensor state, but live VRAM reduction requires active participation
in a runtime's KV allocator, page scheduler, and attention path.

Do not market live VRAM reduction as a shipped QATQ capability until every
release gate in this document is complete and independently reproducible.

## Current Implementation Status

QATQ now includes the codec-side foundation needed by a live VRAM reduction
adapter:

- `KvPageDescriptor` and `KvPageSnapshot` describe typed KV pages with runtime,
  model, sequence, layer, layout, token-range, shape, byte length, and checksum
  metadata.
- `LiveVramLimits` validates untrusted page metadata before allocation or
  decode.
- `LiveVramSchedulerPolicy`, `LiveVramSchedulerState`, and
  `schedule_live_vram_page` make conservative offload/keep-resident decisions
  for unknown, hot, queued, or CPU-budgeted pages.
- `try_encode_live_vram_page` can represent QATQ-compressed pages or typed raw
  pass-through pages, but strict live VRAM evidence schedules
  compression-negative pages as resident instead of counting raw pass-through as
  a successful VRAM-saving offload.
- `restore_live_vram_page` decodes or restores the stored page, verifies dtype,
  length, strategy, and checksum, and fails closed on mismatch.
- `seal_live_vram_page` and `verify_live_vram_page_seal` provide a keyed
  BLAKE3 metadata envelope for runtime/export boundaries. The seal binds the
  caller context, page descriptor, storage label, strategy, stored length, and
  stored payload bytes, and verification fails closed on metadata, payload,
  context, key, or seal-version mismatch.
- `simulate_live_vram_reduction` exercises offline page offload/restore
  decisions and reports compressed, pass-through, resident, budget-skipped, and
  verified-restore counts.
- `LiveVramRuntimeAdapter` defines the runtime-facing contract for identity,
  snapshot, commit, restore, residency, and metrics export.
- `build_live_vram_evidence_report` compares the same page stream across QATQ,
  raw bytes, zstd, and lz4, verifies every QATQ restore, and emits a portable
  JSON evidence summary through `LiveVramEvidenceReport::to_json`.
  `qatq-kv-bench --live-vram-require-page-seals` now requires a
  `--live-vram-page-seal-key-hex` key, builds the report through the sealed
  path, and emits a `metadata_seal` for every offloaded page. The full
  llama.cpp evidence runner enables this sealed path for both runtime-reclaim
  fallback and strict live-paging evidence.
- `LiveVramOffloadStore` provides a bounded in-memory offload table for runtime
  adapters: it rejects duplicates and budget overflow, verifies restore before
  commit, restores by page key, and can remove pages after successful GPU
  re-import. `LiveVramOffloadStore::with_page_seal_policy` stores a keyed
  metadata seal with each committed page and verifies it before decode or
  runtime restore.
- `LiveVramOffloadStore::new_with_shadow_validation` enables debug validation
  mode that keeps original page bytes as a shadow copy, checks restored bytes
  against that shadow, and accounts shadow memory separately.
- `try_offload_live_vram_page` and `try_restore_live_vram_page_from_store`
  provide the QATQ-side controller order: schedule, snapshot, verified store
  commit, runtime offload commit, then verified restore, runtime import, and
  store cleanup.
- `qatq-kv-bench --live-vram-max-queued-pages <n>` exposes the scheduler's
  page-count cap to evidence runs. This is the latency-shaped control for
  long-context active decode: once `n` pages have been scheduled for offload,
  later eligible pages stay resident instead of adding unbounded restore or
  staging pressure.
- `qatq-kv-bench --live-vram-prefetch-window-tokens <n>` exposes the
  scheduler's restore lead-time window. Pages whose next use is inside the hot
  window plus the prefetch window stay resident instead of being offloaded, so
  strict native evidence cannot hide late restore pressure behind a passing
  restore-before-attention trace.
- `try_offload_live_vram_page_with_reclaim_check` wraps the offload path with
  runtime metrics before and after `commit_offload`. If the adapter does not
  prove the configured GPU-byte reduction, QATQ restores the page and removes
  the store entry before returning an error.
- `try_restore_live_vram_page_from_store_with_observed_latency` records
  restore-stall budget violations from runtime-measured restore latency.
- Runtime restore errors, resource-limit rejections, and failed post-restore
  residency checks are counted as restore failures while preserving the CPU
  offload entry for retry instead of feeding missing or corrupt pages to
  attention. Restore attempts for missing/offload-raced store entries are also
  counted as restore failures so request-abort races are visible in operator
  metrics.
- `cancel_live_vram_offload` gives runtime adapters safe cancellation semantics:
  before runtime commit it drops only the pending store entry; after runtime
  commit it restores the page through the runtime before removing the store
  entry. The proof trace now distinguishes
  `cancelled-before-runtime-commit` from `cancelled-after-runtime-commit`; the
  after-commit path must include the restored page checksum and is rejected if
  no offload was active.
- `live_vram_streaming_attention_reference`,
  `live_vram_materialized_attention_reference`, and
  `compare_live_vram_streaming_attention_reference` give runtime adapters an
  executable reference for page-bounded attention: the comparison report
  consumes K/V pages incrementally, validates dimensions and finite values,
  matches materialised softmax attention within test tolerance, reports max
  absolute/relative error, and records peak page KV values versus materialised
  KV values. `decode_tensor_le_bytes_to_f32` and
  `compare_live_vram_typed_streaming_attention_reference` extend that reference
  to native little-endian f32, f16, and bf16 runtime page bytes.
- `LiveVramPageEvent`, `evaluate_live_vram_event_trace`, and
  `evaluate_live_vram_live_paging_proof_gate` provide a proof-grade event trace
  contract for future token-page runtime adapters: an offloaded page cannot be
  consumed by attention before restore, restore checksums must match, tokens must
  be monotonic, and unfinished offloads fail the combined live-paging proof.
- The patched llama.cpp export path can now emit a
  `qatq-live-vram-event-trace-v1` file with `--qatq-event-trace`. QATQ accepts
  that export-time trace, but the strict live-paging gate still rejects the
  current adapter because it does not prove token-page GPU reclaim inside the
  attention loop.
- The patched llama.cpp runtime can also emit
  `qatq-live-vram-attention-trace-v1` JSONL records from
  `llama_kv_cache_context::get_k/get_v` with `--qatq-attention-trace`. This
  proves real attention-path K/V reads during generation, but it is not yet
  page eviction or per-page GPU reclaim.
- The patched llama.cpp runtime can emit QATQ lifecycle events from the actual
  `get_k/get_v` attention path with `--qatq-attention-event-trace`. QATQ
  validates this append-only JSONL stream with
  `qatq-kv-bench --live-vram-event-trace-only`, proving restore-before-attention
  ordering and checksum consistency at the attention-read hook. It is still not
  allocator-backed per-page GPU reclaim.
- The patched llama.cpp runtime can emit a real computed query fixture with
  `--qatq-attention-fixture-dir`. `scripts/llama_cpp_attention_fixture_gate.py`
  slices matching K/V heads from the exported token pages and runs
  `qatq-kv-bench --attention-equivalence-gate`, proving QATQ's page-bounded
  attention reference against real llama.cpp Q/K/V artefacts without writing
  prompt text, query payloads, attention outputs, or KV payloads to the report.
- The patched llama.cpp runtime can run `--qatq-live-page-self-test <n>`, which
  snapshots one active key page from real backend KV tensor storage, evicts it
  through a page-keyed logical residency table, verifies the page is recorded
  as non-resident, restores the original bytes, and verifies the restored
  checksum. This proves adapter-visible page residency and
  snapshot/evict/restore mechanics, but it is not yet allocator-backed GPU page
  reclaim because llama.cpp still owns a persistent whole-cache backend buffer.
- The patched llama.cpp runtime can run
  `--qatq-live-persistent-page-pool-self-test <n>` with
  `--qatq-live-persistent-page-pool-trace <path>`, allocating, exact-byte
  verifying, and retaining a bounded pool of independent non-host K/V page
  tensors until `llama_context` teardown. This proves sustained page-resident
  backend storage mechanics, but it still coexists with the default whole-layer
  KV allocation.
- The patched llama.cpp runtime can run
  `--qatq-live-restore-slot-pressure-self-test <n>` with
  `--qatq-live-restore-slot-pressure-max-bytes <bytes>`, finding a real active
  accelerator key page and rejecting an oversized restore before allocation
  when the configured slot budget is too small. A focused Metal/MLX proof at
  `/private/tmp/qatq-live-vram-restore-slot-pressure-20260624-r2` rejected a
  262,144-byte `MTL0` page against a one-byte limit while still passing the
  strict native live-paging, QATQ restore, codec, attention, and MLX gates.
- `LiveVramOperatorMetrics` emits dependency-free operator counters with the
  documented metric names while excluding sequence identifiers, model names,
  prompts, and raw tensor contents.

This is not yet live GPU VRAM reduction by itself. A runtime adapter must still
wire these APIs into actual KV page snapshot, GPU-free, prefetch, restore, and
attention-safety hooks.

## Latest Real GPU Export Evidence

The strongest current evidence is
[`docs/LLAMA_CPP_LIVE_VRAM_GPU_EVIDENCE.md`](LLAMA_CPP_LIVE_VRAM_GPU_EVIDENCE.md).
Treat that report as historical evidence plus a work log, not as an unlimited
production claim. A fresh structural audit of the pinned llama.cpp adapter
patch at `7992aa7c8e21ea2eb7a5e4802da56eec7b376036` reports
`export_ready: true`, `page_staging_ready: true`, and `live_paging_ready:
true` under the structural native live-paging gate. The current adapter exposes
`--qatq-native-page-streaming-attention-backend-op`, routes bounded K/V page
segments into the conservative masked `GGML_OP_QATQ_SEGMENTED_KQV` Metal
backend op through a retained multi-page pool, and accepts f16 or f32 attention
masks. The route
is versioned as `qatq-segmented-kqv-backend-contract-v1`, carries explicit
resource bounds of 4,096 segments, 4,096 tokens per page, and 8,192 total
K/V tokens, and fails closed for unsupported softcap, ALiBi, multi-stream,
transposed-V, offload, or dtype cases instead of returning approximate
attention.

The current cleaned adapter routes bounded K/V page segments through retained
tiled page-table tensors before `GGML_OP_QATQ_SEGMENTED_KQV`, preserving graph
dependencies for active-token K/V writes without constructing a logical K/V
arena and without building a second per-graph page-pool copy. A Qwen2.5 1.5B
Metal smoke at `/private/tmp/qatq-live-vram-retained-table-p64` matched the
full-GPU 8-token baseline with `[264, 943, 315, 56287, 429, 5711, 279,
56287]`, reached 31.21 tok/s, reserved 1,575 graph nodes with 58 splits at
64-token pages, used a 4.10 MiB Metal compute buffer, and wrote 784
page-segment trace records. This latest route carries explicit token-to-page
and token-to-local tables into the Metal backend op.

A follow-up 32-token Qwen2.5 1.5B Metal smoke at
`/private/tmp/qatq-live-vram-retained-table-n32` matched the full-GPU output
manifest exactly and restored graph reuse to 30 reused graphs. Reducing the
small-window Metal launch to one SIMD-width threadgroup improved the retained
page-table eval slice to 22.19 ms/token, 45.06 tok/s. The full-GPU baseline is
still much faster at 11.37 ms/token, 87.98 tok/s, so this is a correctness and
incremental latency milestone rather than a production-performance result. A
page-table upload cache experiment was explicitly rejected because it produced
divergent generated tokens; future dirty-page tracking must be tied to executed
state, not graph-build assumptions.

A later page-aligned reserve run in the same directory kept the exact same
32 generated tokens as the full-GPU baseline while reducing graph shape and
staging fan-out. The retained path staged `n_kv: 64` with `segment_count: 1`
per K/V/layer, wrote 448 page-segment records, reused 30 graphs, reduced graph
nodes from 1,575 to 1,071, lowered the CPU compute buffer to 0.0985 MiB, and
reached 21.43 ms/token, 46.65 tok/s. The output comparison report at
`/private/tmp/qatq-live-vram-retained-table-n32/output-comparison-pad64reserve.json`
passed against the full-GPU manifest with identical generated token IDs and
text hash. This is a structural improvement to the retained backend route, not
a solution to the remaining split/kernel overhead.

A tighter `LLAMA_QATQ_N_KV_PAD_TOKENS=48` smoke, exposed through the evidence
runner as `--n-kv-pad-tokens 48`, reduced the staged attention window to
`n_kv: 48` for the same 38-token workload. It preserved the full-GPU output
manifest, wrote 560 page-segment records because the graph rebuilt more often,
reused 28 graphs, lowered CPU compute buffer use to 0.0946 MiB, and reached
21.11 ms/token, 47.36 tok/s. The comparison report at
`/private/tmp/qatq-live-vram-retained-table-n32/output-comparison-pad48reserve.json`
passed with the same generated text hash. This knob is safe only as a rounding
quantum above the active K/V high-water mark; it should be tuned per evidence
profile, not promoted blindly as the production default.

2026-06-25 follow-up diagnostics fixed two correctness/attestation issues in
the retained page-table route:

- the segmented graph bridge now consumes the retained page-table `sync_tensor`
  when present, so the graph has a real dependency on the `ggml_cpy` that fills
  the retained page view from live K/V bytes;
- the Metal backend-op dispatch is temporarily pinned to the conservative
  32-thread path because the larger 256-thread path produced divergent tokens
  on a 705-token Qwen2.5 1.5B prompt;
- manifest accounting now includes retained tiled page-table pools in
  `gpu_page_staging_bytes`, preventing the strict gate from passing on hidden
  Metal allocation overhead.

Those fixes made the medium Qwen2.5 1.5B backend-op diagnostic preserve the
full-GPU generated tokens exactly, but they also exposed the current production
blocker: retained page-table pools are allocated per layer and per K/V kind,
which is too expensive on Metal. A 705-token, 512-token-page backend-op run
reported 56 offload and 56 restore events with exact output, but the strict
live-paging gate correctly rejected `44,040,192` staged GPU bytes against a
`22,020,096` byte total K/V context. A longer 3.5k-token diagnostic likewise
rejected `293,601,280` staged bytes against `102,760,448` total K/V context.
The next implementation target is pooled or compact retained page-table
allocation that stages only the bounded hot/restore window with allocation
overhead below the full K/V context. Until that passes, live VRAM reduction
remains experimental rather than production-ready.

2026-06-25 compact-residency follow-up: the tiled allocator now plans
offloaded versus retained pages before allocating the retained page table. Cold
pages can stay CPU-backed in the graph-native segmented route, while the
bounded hot/restore page window occupies a compact retained GPU pool. The real
Metal run at
`/private/tmp/qatq-live-vram-native-ggml-compact-residency-p9` passed strict
live-paging and latency gates on Qwen2.5 1.5B with a 705-token prompt:

- full-GPU output preserved for the 9-token continuation;
- `112/112` exact restores;
- `56/56` offloaded compressed pages and `56` resident pages;
- zero pass-through pages;
- QATQ aggregate storage beat raw, zstd, and lz4;
- persistent GPU K/V residency fell from `22,020,096` bytes to `14,680,064`
  bytes;
- generated-token p95/p99 latency stayed within the configured gate.

The same compact residency model now also has a strict backend-op proof. The
follow-up run at `/private/tmp/qatq-live-vram-backend-op-compact-native-gate`
used `--native-page-streaming-attention-backend-op` and passed
`--require-native-page-streaming`: full-GPU output preserved, `112/112` exact
restores, `56/56` compressed offloads, zero pass-through pages, backend
consumer `backend_scheduled_segmented_attention`, MLX GPU equivalence, and
persistent GPU K/V residency reduced from `22,020,096` bytes to `14,680,064`
bytes. Mixed-residency backend scheduling is now implemented for the focused
case by combining compact retained GPU pages with graph-local transient page
pools for CPU-backed cold segments.

2026-06-25 bootstrapped restore-slot pressure follow-up: the compact
backend-op shape was rerun from a clean pinned llama.cpp bootstrap at
`/private/tmp/qatq-llama-bootstrap-proof`. The proof directory
`/private/tmp/qatq-live-vram-restore-slot-pressure-bootstrap-20260625-r4`
passed strict live-paging, strict native page-streaming,
`backend_scheduled_flattened_flash_attention`, MLX GPU streaming-attention
verification, aggregate codec gates, and the one-byte restore-slot pressure
self-test. The run selected 4 KV GPU layers, restored `168/168` pages exactly,
offloaded `112/112` compressed pages with zero pass-through, reduced persistent
GPU K/V from `35.00 MiB` to `12.00 MiB`, stored `25.00 MiB` through QATQ versus
`28.65 MiB` zstd and `31.12 MiB` lz4, and rejected a real `262144` byte `MTL0`
key page before allocation against the one-byte restore-slot limit. A wider
14-layer backend-op attempt failed the live-paging gate first because staged
page bytes exceeded total KV context bytes; that failure is expected and keeps
the proof honest.

That is still an experimental adapter milestone rather than a production-ready
runtime feature. The next production target is to harden and optimise the
transient staging path under long contexts, pressure, broader model families,
more dtypes, more page sizes, and sustained decode workloads without rebuilding
full K/V tensors or losing exactness.

The transient graph-local page pool is now fail-closed on bytes rather than
only segment count. `--native-page-streaming-transient-pool-max-bytes` sets the
per-attention-window K/V transient budget; the runtime rejects a zero or
oversized request before completing the graph. The hostile Metal run at
`/private/tmp/qatq-live-vram-backend-op-transient-budget-fail-clean` reached
the backend-op path and cleanly rejected a one-byte budget with exit code `1`
and message `QATQ backend-op transient page pool byte budget exceeded`. The
normal strict run at
`/private/tmp/qatq-live-vram-backend-op-transient-budget-pass` passed the same
backend-op/MLX gate under the default `256 MiB` ceiling.

The structural live-paging gate now passes
`live.native_backend_op_transient_pool_byte_budget`,
`live.native_backend_op_avoids_staged_arena`, and
`live.native_backend_op_descriptor_path`, proving that direct retained page
tables are used when possible and that mixed-residency fallback staging is
bounded. This is still an experimental adapter milestone rather than a
production-complete claim across models, context lengths, runtimes, pressure
profiles, and latency tails.

`scripts/llama_cpp_live_vram_adapter_audit.py` now reports required gate
failures separately from legacy diagnostic compatibility failures. Current
patch-file and applied-source audits pass `--require-live-paging` with
`required_live_paging_failures: []` and `required_page_staging_failures: []`.
Older concat-composed and persistent-source diagnostic checks remain visible
under `non_required_failures`; they are not the retained page-table backend-op
product route.

The backend-op route now also has a selective resident fast path. Page-segment
metadata marks whether a segment is actually live-offloaded; the graph keeps
all-resident layers on llama.cpp's stock attention path and only routes layers
with cold/offloaded K/V pages through `backend_scheduled_segmented_attention`.
The structural audit allows `get_k`/`get_v` inside the native entry point only
behind this `live_offloaded` guard, and the page-segment verifier now requires
cold segments to be native and consumed while allowing resident segments to be
reported as fast-path fallback. This removed most of the long-context backend
cliff. The custom segmented backend-op still misses the p95 gate, but the
newer retained flattened Flash Attention route with cold-slot reuse closes the
scoped four-layer long-context p95/p99 gate for eligible layouts.

The latest focused Metal optimisation reshapes the backend-op kernel from one
single-thread group per output element to one 256-thread group per query/head.
It computes the page-streamed logits and softmax denominator once in
threadgroup memory, then writes output dimensions from the shared weights. The
V accumulation now walks each page range directly instead of performing a
linear page lookup for every token and output dimension, and explicit
token-to-page/token-to-local tables remove the remaining per-token page search
inside the Metal kernel. The retained tiled page-table fill fixes the
stale-host-pack failure mode by running page copies inside ggml after
current-token K/V writes exist, then the backend op consumes the retained table
directly. The graph and Metal backend are now aligned to a conservative
8,192-token fast window, which covers the current 5.9k-token Coder diagnostic
without the earlier Metal assertion; longer contexts still need true tiling,
lower-overhead dirty-page tracking, or a policy fallback.

A strict rebuilt-adapter breadth matrix passed at
`/private/tmp/qatq-live-vram-breadth-backend-op-20260624` using real Qwen2.5
1.5B, Qwen2.5 3B, and Phi 3.5 mini generation on Apple Metal plus external MLX
GPU validation. The matrix ran with the backend-op route explicitly enabled,
512 MiB host-memory pressure, strict live-paging, strict native page-streaming,
aggregate raw/zstd/lz4 codec gates, and bulk artifact pruning. It restored
576/576 pages exactly across the three cases, offloaded 456/456 pages through
QATQ, used zero pass-through pages, and passed event traces totalling 2,064
events. Reclaimable GPU K/V was 28.00 MiB, 111.00 MiB, and 480.00 MiB for the
three cases. QATQ stored 27.53 MiB, 90.73 MiB, and 420.46 MiB versus zstd
29.25 MiB, 98.51 MiB, and 455.42 MiB, and lz4 31.12 MiB, 106.95 MiB, and
493.38 MiB. The native status for every case reported
`backend_scheduled_segmented_attention: true`,
`accelerated_runtime_attention_graph: true`,
`page_bounded_attention_equivalence_passed: true`, and
`external_mlx_streaming_attention_passed: true`; MLX checked 28/336,
36/576, and 32/1024 layer/head coverage respectively.

The current evidence supports an experimental strict native backend-op
live-VRAM proof across two model families, three real GGUF models, exact
restore, MLX equivalence, aggregate codec gates, and 512 MiB host-memory
pressure, plus the broader historical storage/transfer and page-staging
evidence below. It still does not support a production-complete claim across
arbitrary runtimes, models, prompts, dtypes, page sizes, sequence mixes,
pressure profiles, and long-context latency tails.
Before live VRAM reduction graduates from experimental, the backend-op route
must pass the broader model, dtype, page-size, latency-tail, long-context,
memory-pressure, restore-slot, and soak gates from the rebuilt adapter.

The in-process server cancellation path has since been hardened beyond the
original Qwen2.5 1.5B 1 GiB pressure soaks. The probe now runs repeated
stream-cancel/follow-up cycles against one `llama-server` process, records
server RSS, applies touched host-memory pressure, and fails on per-iteration or
follow-up latency ceilings. It starts `llama-server` in its own process group,
shuts down that group with SIGTERM/SIGKILL escalation if needed, and records
the `shutdown_cleanup` signal/escalation metadata in `summary.json`. It also
exposes strict native multi-stream trace
gates: `--require-flattened-flash-consumer` requires attention-consumed trace
rows to use `backend_scheduled_flattened_flash_attention`, and
`--require-live-offloaded-stream-count <n>` requires live-offloaded page
segments across at least `n` stream indices. Qwen2.5 1.5B passed 50-cycle
non-unified and
unified runs under 2 GiB host pressure at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-pressure2g-latency-soak50-20260625`
and
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-kvu-pressure2g-latency-soak50-20260625`.
Qwen2.5 3B then passed 20-cycle non-unified and unified 1 GiB pressure runs at
`/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-pressure1g-latency-soak20-20260625`
and
`/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-kvu-pressure1g-latency-soak20-20260625`.
The Qwen2.5 3B unified run recorded p95/p99 iteration latency 7.73s/7.73s,
p95/p99 follow-up latency 4.39s/4.51s, 24,568 live-offloaded segments, and
46.41 MiB RSS growth. A follow-up adapter change then implemented scoped
native multi-stream retained page tables for non-unified two-slot flattened
Flash Attention by splitting retained page-table views per stream, preserving
per-stream masks, and emitting explicit `stream_index` trace metadata. The
Qwen2.5 1.5B native multi-stream pressure run at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-multistream-pressure1g-soak20-20260625`
passed 20/20 cancellation/follow-up iterations under 1 GiB host pressure with
10,696 attention-page-segment events, 1,528 attention-consumed events, 16,008
live-offloaded segments, p95/p99 iteration latency 4.74s/4.76s, p95/p99
follow-up latency 2.61s/2.63s, and 46.63 MiB RSS growth. The trace records
`stream_index: 0` and `stream_index: 1` segments with shape `[128,2,64,1]`
under `backend_scheduled_flattened_flash_attention`. A broader Qwen2.5 3B
native multi-stream rerun at
`/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-native-multistream-pressure1g-soak20-ctx8192-20260625`
then passed 20/20 non-unified two-slot cancellation/follow-up iterations under
1 GiB host pressure with an 8192-token server context, 18,072
attention-page-segment events, 2,008 attention-consumed events, 33,928
live-offloaded segments, p95/p99 iteration latency 11.19s/11.21s, p95/p99
follow-up latency 4.34s/4.35s, and 111.81 MiB RSS growth. Its consumed
attention rows all used `backend_scheduled_flattened_flash_attention` and
included both `stream_index: 0` and `stream_index: 1` retained-table segments.
A strict-gated integrated Qwen2.5 1.5B rerun at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-strictgates-soak10-ctx8192-20260625`
passed 10/10 with `--require-flattened-flash-consumer` and
`--require-live-offloaded-stream-count 2`, so the server probe now proves the
flattened Flash consumer and both live-offloaded stream indices through its own
fail-closed result path. It recorded 1,048 flattened Flash consumer rows,
17,368 live-offloaded segments, p95/p99 iteration latency 6.44s/6.44s,
p95/p99 follow-up latency 2.62s/2.62s, and 106.73 MiB RSS growth under
1 GiB host pressure.
A second-model strict-gated integrated rerun at
`/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-native-strictgates-soak5-ctx8192-20260625`
passed 5/5 with the same strict native trace gates on Qwen2.5 3B. It recorded
568 flattened Flash consumer rows, 8,768 live-offloaded segments, both
live-offloaded stream indices, p95/p99 iteration latency 11.11s/11.11s,
p95/p99 follow-up latency 4.34s/4.34s, and 111.53 MiB RSS growth under
1 GiB host pressure.
A Phi 3.5 mini model-family run at
`/private/tmp/qatq-live-vram-server-cancel-phi35mini-p64-nonunified-native-multistream-pressure1g-soak20-ctx8192-20260625`
then passed 20/20 with the same non-unified two-slot native route under 1 GiB
host pressure. It recorded 16,064 attention-page-segment events, 2,008
attention-consumed events, 33,768 live-offloaded segments, p95/p99 iteration
latency 15.81s/15.82s, p95/p99 follow-up latency 4.77s/4.77s, and 836.23 MiB
RSS growth after the server probe was hardened to include per-iteration RSS
peaks in the memory gate. Its consumed rows all used
`backend_scheduled_flattened_flash_attention` and traced both stream indices
with shape `[96,32,64,1]`. A strict-gated Phi 3.5 mini rerun at
`/private/tmp/qatq-live-vram-server-cancel-phi35mini-p64-nonunified-native-strictgates-soak3-ctx8192-20260625`
passed 3/3 with `--require-flattened-flash-consumer` and
`--require-live-offloaded-stream-count 2`, recording 376 flattened Flash
consumer rows, 5,328 live-offloaded segments, both live-offloaded stream
indices, p95/p99 iteration latency 15.99s/15.99s, p95/p99 follow-up latency
4.99s/4.99s, and 836.47 MiB RSS growth. This closes the first three-model-family
two-stream retained-table blocker under both raw trace inspection and the
reusable strict server-probe gates. The strict server-probe path is now also
captured by `scripts/llama_cpp_live_vram_server_cancel_matrix.py` with
`adapters/llama-cpp/live-vram-server-strict.local.example.json`, so the
Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini probes can be rerun as one
sequential, fail-closed matrix that writes per-case artifacts plus aggregate
JSON and Markdown summaries. The first real matrix run at
`/private/tmp/qatq-live-vram-server-cancel-strict-matrix-20260625` passed all
three cases: Qwen2.5 1.5B 10/10 with p95 iteration/follow-up latency
6.45s/2.66s and 17,368 live-offloaded segments; Qwen2.5 3B 5/5 with
11.13s/4.31s and 8,768 live-offloaded segments; and Phi 3.5 mini 3/3 with
16.39s/5.07s and 5,328 live-offloaded segments. Every case kept the
flattened-Flash consumer and two live-offloaded stream-index gates enabled.
A fresh current-binary rerun at
`/private/tmp/qatq-live-vram-server-strict-current-20260625` deliberately kept
the same strict gates and exposed a production-shaped resource bound: Qwen2.5
1.5B and Qwen2.5 3B passed, but Phi 3.5 mini aborted when the retained tiled
page-pool hit the old 1 GiB default while the probe allowed 1.5 GiB RSS growth.
The server probe now exports
`LLAMA_QATQ_ATTENTION_PERSISTENT_PAGE_SOURCE_MAX_RETAINED_BYTES`, deriving the
default retained-pool ceiling as `max(1024 MiB, max_server_rss_growth_mib)`.
The focused Phi rerun at
`/private/tmp/qatq-live-vram-server-phi35-strict-retained-budget-20260625`
passed 3/3 with 5,328 live-offloaded segments, 376 flattened Flash consumers,
predicted-token throughput p50 33.21 tok/s, p95/p99 iteration latency
23.84s/23.84s, p95/p99 follow-up latency 5.47s/5.47s, and 1,046.11 MiB RSS
growth under the 1.5 GiB gate. The full retained-budget matrix at
`/private/tmp/qatq-live-vram-server-strict-retained-budget-current-20260625`
then passed all three strict cases: Qwen2.5 1.5B 10/10 with p50 predicted
throughput 77.84 tok/s and 14,808 live-offloaded segments, Qwen2.5 3B 5/5 with
43.98 tok/s and 7,488 live-offloaded segments, and Phi 3.5 mini 3/3 with
33.13 tok/s and 5,328 live-offloaded segments.
A longer Qwen2.5 1.5B native multi-stream
burn-in at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-multistream-pressure1g-soak100-ctx8192-20260625`
then passed 100/100 cancellation/follow-up iterations under 1 GiB host
pressure with the corrected RSS gate, 67,816 attention-page-segment events,
9,688 attention-consumed events, 168,968 live-offloaded segments, p95/p99
iteration latency 6.56s/6.58s, p95/p99 follow-up latency 2.64s/2.65s, and
108.19 MiB RSS growth across 202 RSS samples. Its consumed rows all used
`backend_scheduled_flattened_flash_attention` and traced both stream indices.
A harsher Qwen2.5 1.5B native multi-stream pressure run at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-multistream-pressure2g-soak50-ctx8192-20260625`
then passed 50/50 under 2 GiB host pressure with the corrected RSS gate, 34,216
attention-page-segment events, 4,888 attention-consumed events, 84,568
live-offloaded segments, p95/p99 iteration latency 6.60s/6.61s, p95/p99
follow-up latency 2.66s/2.66s, and 106.48 MiB RSS growth across 102 RSS
samples. Its consumed rows all used `backend_scheduled_flattened_flash_attention`
and traced both stream indices. Broader native multi-stream burn-in across more
page sizes, contexts, pressure profiles, and multi-model long-duration runs
remains open. A Qwen2.5 3B native multi-stream long soak at
`/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-native-multistream-pressure1g-soak50-ctx8192-20260625`
passed 50/50 under 1 GiB host pressure with the corrected RSS gate, 43,992
attention-page-segment events, 4,888 attention-consumed events, 84,568
live-offloaded segments, p95/p99 iteration latency 11.22s/12.19s, p95/p99
follow-up latency 4.36s/4.91s, and 114.61 MiB RSS growth across 102 RSS
samples. Its consumed rows all used `backend_scheduled_flattened_flash_attention`
and traced both stream indices.
A harsher Qwen2.5 3B 2 GiB pressure rerun at
`/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-native-multistream-pressure2g-soak50-ctx8192-20260625`
then passed 50/50 with the corrected RSS gate, 43,992 attention-page-segment
events, 4,888 attention-consumed events, 84,568 live-offloaded segments,
p95/p99 iteration latency 11.03s/12.17s, p95/p99 follow-up latency
4.25s/5.48s, and 115.25 MiB RSS growth across 102 RSS samples. Its consumed
rows all used `backend_scheduled_flattened_flash_attention`, traced both stream
indices, and carried shape `[128,2,64,1]`.
A Phi 3.5 mini native multi-stream long soak at
`/private/tmp/qatq-live-vram-server-cancel-phi35mini-p64-nonunified-native-multistream-pressure1g-soak50-ctx8192-20260625`
then passed 50/50 under 1 GiB host pressure with the corrected RSS gate, 39,104
attention-page-segment events, 4,888 attention-consumed events, 84,168
live-offloaded segments, p95/p99 iteration latency 16.40s/18.13s, p95/p99
follow-up latency 5.05s/6.71s, and 832.47 MiB RSS growth across 102 RSS
samples. Its consumed rows all used `backend_scheduled_flattened_flash_attention`,
traced both stream indices, and covered the four configured cold K/V layers
with shape `[96,32,64,1]`.
A focused Phi 3.5 mini page-size variant at
`/private/tmp/qatq-live-vram-server-cancel-phi35mini-p128-nonunified-native-multistream-pressure1g-soak20-ctx8192-20260625`
then passed 20/20 under the same 1 GiB host-pressure gate with 128-token pages,
14,784 attention-page-segment events, 1,848 attention-consumed events, 16,648
live-offloaded segments, p95/p99 iteration latency 16.16s/16.16s, p95/p99
follow-up latency 5.00s/5.04s, and 822.64 MiB RSS growth. Its consumed rows all
used `backend_scheduled_flattened_flash_attention`, traced both stream indices,
and carried shape `[96,32,128,1]`.
A Qwen2.5 1.5B 16,384-context stress rerun at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-multistream-pressure1g-soak20-ctx16384-repeat160-20260625`
then reproduced and fixed a real long-context crash boundary. The first run
failed closed after one iteration with `QATQ segmented KQV backend contract
exceeded max total tokens`; the flattened Flash route was still applying an
aggregate single-stream segmented-backend total-token cap before the
stream-local retained table boundary. After the patch and rebuild, the same
profile passed 20/20 cancellation/follow-up iterations under 1 GiB host
pressure, recording 19,656 attention-page-segment events, 2,808
attention-consumed events, 60,168 live-offloaded segments, p95/p99 iteration
latency 9.96s/9.96s, p95/p99 follow-up latency 2.66s/2.68s, and 243.30 MiB RSS
growth. This is a specific multi-stream long-context crash fix, not unlimited
context-length coverage.
The extended strict server matrix now covers that long-context class and a
Phi page-size variant through the repeatable wrapper. The run at
`/private/tmp/qatq-live-vram-server-cancel-extended-matrix-20260625` passed
Qwen2.5 1.5B at 16,384 context for 10/10 iterations with p95
iteration/follow-up latency 11.06s/2.68s, 35,288 live-offloaded segments, and
317,424 KiB RSS growth. It also passed Phi 3.5 mini with 128-token pages for
5/5 iterations with p95 iteration/follow-up latency 19.13s/5.16s, 7,288
live-offloaded segments, and 1,071,296 KiB RSS growth. Both cases kept the
strict flattened-Flash consumer and two live-offloaded stream-index gates
enabled.
The current retained-budget rerun at
`/private/tmp/qatq-live-vram-server-extended-retained-budget-current-20260625`
passed the same extended wrapper with current binaries: Qwen2.5 1.5B 16,384
context passed 10/10 with p50 predicted throughput 77.37 tok/s, p95/p99
iteration latency 13.86s/13.86s, p95/p99 follow-up latency 2.56s/2.56s,
32,728 live-offloaded segments, 1,528 flattened Flash consumers, and
445,904 KiB RSS growth; Phi 3.5 mini with 128-token pages passed 5/5 with
33.95 tok/s p50 predicted throughput, p95/p99 iteration latency 24.58s/24.58s,
p95/p99 follow-up latency 5.14s/5.14s, 7,288 live-offloaded segments, 568
flattened Flash consumers, and 969,904 KiB RSS growth.
A three-model native-server comparison matrix at
`/private/tmp/qatq-live-vram-server-cancel-baseline-multimodel-20260625`
passed all six native/QATQ cases across Qwen2.5 1.5B, Qwen2.5 3B, and
Phi 3.5 mini. Native baseline cases disabled all QATQ live-VRAM environment
variables; paired QATQ cases kept strict flattened-Flash and two
live-offloaded stream-index gates enabled. QATQ/native p95 iteration ratios
were 1.246x, 1.163x, and 1.150x respectively. Follow-up p95 ratios were
1.680x, 1.494x, and 1.370x. QATQ RSS-growth ratios were 4.169x, 3.615x, and
9.302x in this host-RSS probe. This is the first multi-model shared-server
cancellation latency/RSS baseline; it is not yet a peak-VRAM, tokens/sec, FP8,
CPU-offload, zstd, lz4, or multi-runtime comparison.
The current retained-budget baseline rerun at
`/private/tmp/qatq-live-vram-server-baseline-retained-budget-current-20260625`
passed all six native/QATQ cases and added predicted-token throughput ratios.
QATQ/native predicted-token p50 ratios were 0.903x for Qwen2.5 1.5B, 0.938x
for Qwen2.5 3B, and 0.880x for Phi 3.5 mini. Iteration p95 ratios were 1.153x,
1.105x, and 1.727x; follow-up p95 ratios were 1.217x, 1.132x, and 1.607x. RSS
growth ratios were 4.547x, 4.675x, and 12.786x respectively. The Qwen cases are
now close enough for further experimental optimisation, while Phi remains the
current memory-overhead outlier.
The follow-up Phi policy sweep at
`/private/tmp/qatq-live-vram-server-phi-policy-current-20260625` tested the
large-KV Phi shape with fewer QATQ-routed layers and 128-token pages. The best
strict candidate, `phi35-mini-qatq-l1-p128-strict`, passed 5/5 with p50
predicted throughput 37.12 tok/s, 1,822 live-offloaded segments, 142 flattened
Flash consumers, a QATQ/native predicted-token p50 ratio of 0.949x, iteration
p95 ratio 1.469x, follow-up p95 ratio 1.355x, and RSS-growth ratio 4.161x. The
checked-in strict matrix now uses that conservative Phi policy. The
threshold-gated backend-memory rerun at
`/private/tmp/qatq-live-vram-server-strict-backend-memory-current-20260625`
passed all three model cases and records llama.cpp/Metal allocation diagnostics
in the summary. Qwen2.5 1.5B passed 10/10 at 78.30 tok/s p50 with 14,808
live-offloaded segments, 968 flattened Flash consumers, 1,426 MiB backend self,
192 MiB backend KV, and 299 MiB backend compute. Qwen2.5 3B passed 5/5 at
43.86 tok/s p50 with 7,488 live-offloaded segments, 528 flattened Flash
consumers, 2,391 MiB backend self, 256 MiB backend KV, and 300 MiB backend
compute. Phi 3.5 mini passed 3/3 at 36.33 tok/s p50 with 882 live-offloaded
segments, 88 flattened Flash consumers, 5,339 MiB backend self, 2,976 MiB
backend KV, and 134 MiB backend compute. Phi is no longer the same severe
outlier under the recommended policy, but it still needs longer soaks and
hardware-counter peak-VRAM validation.
The server-cancellation probe now also extracts timing fields from
`llama-server` non-streaming `/completion` responses when they are present and
reports predicted-token throughput in matrix summaries. A focused Qwen2.5 1.5B
timing smoke at `/private/tmp/qatq-live-vram-server-timing-smoke-20260625`
passed the native/QATQ pair with strict QATQ trace gates enabled. Native
predicted-token p50 was 89.31 tok/s; QATQ predicted-token p50 was 51.43 tok/s.
The resulting QATQ/native ratios were 0.576x for predicted-token p50, 0.601x
for predicted-token p95, 1.235x for iteration p95, 1.731x for follow-up p95,
and 4.119x for RSS growth. This proves the server harness can now collect real
throughput metrics, while also confirming that the current shared-server live
VRAM route still needs substantial performance and RSS optimisation.
That first timing smoke exposed a policy bug rather than only a kernel
bottleneck: when QATQ page staging was enabled, non-selected KV layers were
left as canonical CPU KV instead of staying on llama.cpp's native GPU KV path.
The patched adapter now keeps non-selected layers on native GPU KV and routes
only selected layers through canonical CPU KV plus accelerator page staging.
The strict trace-enabled rerun at
`/private/tmp/qatq-live-vram-server-timing-smoke-gpufallback-20260625` passed
with native predicted-token p50 88.02 tok/s and QATQ predicted-token p50
77.97 tok/s. QATQ/native ratios improved to 0.886x for predicted-token p50,
0.882x for predicted-token p95, 1.153x for iteration p95, and 1.227x for
follow-up p95 while still recording 7,488 live-offloaded segments and 528
flattened-Flash consumers. This is the current trace-enabled shared-server
throughput datapoint.
A queue-depth tuning matrix at
`/private/tmp/qatq-live-vram-server-queue-depth-20260625` then compared
Qwen2.5 1.5B native against QATQ `max_queued_pages` 8, 16, and 32. All QATQ
cases passed strict flattened-Flash and two-stream live-offload gates. q8
recorded 3,328 live-offloaded segments and the best QATQ iteration-p95 ratio
at 1.247x, with follow-up p95 1.710x and RSS-growth ratio 4.218x. q32 recorded
10,048 live-offloaded segments, similar iteration/follow-up ratios of
1.256x/1.714x, and the lowest RSS-growth ratio at 4.156x. q16 was worse on
this workload, with 1.593x iteration-p95 and 2.891x follow-up-p95 ratios. This
suggests q8 is the better low-latency candidate for this specific server
cancellation shape, while q32 remains competitive on host RSS growth.
The timing-enabled rerun at
`/private/tmp/qatq-live-vram-server-queue-depth-timing-20260625` passed the
same four-case matrix and showed that queue depth is not the primary
throughput lever: native predicted-token p50 was 87.04 tok/s, while q8, q16,
and q32 recorded 50.37 tok/s, 49.86 tok/s, and 50.59 tok/s. QATQ/native
predicted-token p50 ratios were 0.579x, 0.573x, and 0.581x, and p95 ratios
were 0.578x, 0.570x, and 0.594x. q32 was slightly best on throughput and
follow-up p95 in this run; q8 remained close with fewer live-offloaded
segments.
A no-trace layer-count sweep after the native-GPU fallback fix was rerun from
the clean bootstrapped `llama-server` with explicit backend-memory and
projected device-memory ceilings at
`/private/tmp/qatq-live-vram-server-layer-sweep-bootstrap-notrace-projected-gated-20260625`.
It passed native plus QATQ l0/l1/l2/l4, five cancellation/follow-up iterations
per case, 1 GiB host-memory pressure, and empty `gate_failures`.
QATQ/native predicted-token p50 ratios were 0.956x, 0.923x, 0.884x, and
0.858x respectively. The l4 case reduced backend KV memory from 224 MiB to
192 MiB and projected device memory from 1458 MiB to 1426 MiB, with
iteration/follow-up p95 ratios of 1.052x and 1.159x. RSS-growth ratios were
still 1.002x, 1.892x, 2.720x, and 4.152x, so host RSS and direct peak-VRAM
measurement remain blockers.
That broad sweep is exploratory, not the accepted policy. A stricter
comparison-gated repeat at
`/private/tmp/qatq-live-vram-server-layer-sweep-bootstrap-notrace-comparison-gated-20260625`
rejected l2/l4 candidates on latency, throughput, and host RSS. The accepted
no-trace policy is now the l1-only config at
`adapters/llama-cpp/live-vram-server-layer-policy-notrace.local.example.json`.
It runs one required warmup cancellation/follow-up cycle so first-use lazy page
allocation is reported separately, then gates five measured steady-state
cycles. The bootstrapped proof at
`/private/tmp/qatq-live-vram-server-layer-policy-bootstrap-notrace-warmup-gated-20260625`
passed native plus QATQ l1 under 1 GiB host-memory pressure. QATQ/native p50
and p95 throughput ratios were 0.956x and 0.965x, iteration/follow-up p95
ratios were 1.014x and 1.028x, steady-state RSS-growth ratio was 1.279x,
backend KV memory dropped from 224 MiB to 216 MiB, and projected device memory
dropped from 1458 MiB to 1450 MiB. The warmup cost is still explicit: QATQ
grew 59.44 MiB during lazy initialisation versus 6.16 MiB during the measured
steady-state window.
The accepted no-trace policy was then broadened to the three local model
families with
`adapters/llama-cpp/live-vram-server-family-policy-notrace.local.example.json`.
The bootstrapped run at
`/private/tmp/qatq-live-vram-server-family-policy-bootstrap-notrace-warmup-gated-20260625`
passed all six native/QATQ cases with backend-memory diagnostics, one warmup
cycle, five measured steady-state cycles, 1 GiB host-memory pressure, and
global comparison gates. Qwen2.5 1.5B QATQ/native p50 and p95 throughput
ratios were 0.972x and 0.970x, iteration/follow-up p95 ratios were 1.041x and
1.043x, and steady-state RSS-growth ratio was 0.940x. Qwen2.5 3B recorded
0.980x/0.992x throughput ratios, 1.003x/1.017x iteration/follow-up p95 ratios,
and 0.955x steady-state RSS ratio. Phi 3.5 mini with the l1, 128-token-page
policy recorded 0.983x/0.975x throughput ratios, 1.011x/1.219x
iteration/follow-up p95 ratios, and 1.447x steady-state RSS ratio. Backend KV
memory dropped from 224 to 216 MiB for Qwen2.5 1.5B, 288 to 280 MiB for
Qwen2.5 3B, and 3072 to 2976 MiB for Phi. Phi still has the largest host
pressure shape: its QATQ warmup grew 257.56 MiB and measured steady-state RSS
grew 83.83 MiB, so longer Phi soaks and peak-VRAM hardware counters remain
production gates.
The first 10-cycle family-policy soak initially rejected the default Qwen2.5
3B QATQ q32 queue policy after one tail-latency spike and a small-denominator
steady-state RSS ratio over the gate. A targeted q8 Qwen2.5 3B probe then
passed 10/10 with 0.943x p50 throughput versus the native soak baseline,
follow-up p95 inside the native baseline, and only 5.66 MiB measured
steady-state RSS growth. The accepted family configs initially moved to q8 for
Qwen2.5 3B QATQ. The corrected full soak at
`/private/tmp/qatq-live-vram-server-family-policy-soak-bootstrap-notrace-warmup-gated-20260625`
passed all six native/QATQ cases with 10 measured cycles each. Qwen2.5 1.5B
recorded QATQ/native p50/p95 throughput ratios of 0.981x/0.984x, p95
iteration/follow-up ratios of 1.008x/1.028x, RSS-growth ratio 0.941x, and
backend KV 224->216 MiB. Qwen2.5 3B with q8 recorded 0.943x/0.958x throughput
ratios, 1.068x/1.078x latency ratios, RSS ratio 1.637x, and backend KV
288->280 MiB. Phi 3.5 mini recorded 1.003x/0.999x throughput ratios,
0.889x/0.747x latency ratios, RSS ratio 1.372x, and backend KV 3072->2976
MiB. This is now the strongest accepted no-trace shared-server policy soak,
but it still does not replace overnight burn-in or direct peak-VRAM
instrumentation.
A longer three-repeat burn-in attempt at
`/private/tmp/qatq-live-vram-server-family-policy-soak-burnin3-taildelta-security-gated-20260625`
failed closed under slower host conditions: Qwen2.5 3B q8 missed the accepted
p50 throughput ratio at 0.815x, and Phi native exceeded the steady RSS tail
gate before its QATQ pair could run. Focused Qwen2.5 3B probes then tested
more conservative queue/page policies. q4 with 64-token pages fixed p50 at
0.951x but still missed p95 at 0.787x; q2 with 64-token pages passed in
isolation at `/private/tmp/qatq-live-vram-server-qwen3-q2-policy-soak-20260625`
but failed the full-family q2 burn-in at
`/private/tmp/qatq-live-vram-server-family-policy-soak-burnin2-q2-taildelta-security-gated-20260625`
on the second repeat with Qwen2.5 3B p50/p95 throughput ratios 0.630x/0.678x.
128-token pages with q4 passed in isolation but failed the corrected
full-family p05/p50 rerun. The strongest focused candidate is now 256-token
pages with q4:
`/private/tmp/qatq-live-vram-server-qwen3-p256-q4-focused-p05-20260626`, which
passed with p05/p50/p95 throughput ratios 1.595x/1.075x/0.980x, p95
iteration/follow-up ratios 0.773x/0.628x, backend K/V 288->280 MiB, projected
device memory 2423->2415 MiB, and zero RSS tail gate growth. The checked-in
family configs now carry that candidate. The full-family three-repeat burn-in at
`/private/tmp/qatq-live-vram-server-family-policy-soak-burnin3-p256q4-p05-tailgate-20260626`
then passed 3/3 repeats, 18 real server cases total, with empty comparison and
aggregate gate failures. Backend K/V and projected-device jitter ratios were
1.0 for every native and QATQ case. Qwen2.5 3B QATQ/native p05/p50/p95
throughput ratios stayed inside policy in every repeat; the weakest repeat
recorded 0.929x/0.945x/0.959x, with backend K/V 288->280 MiB and projected
device memory 2423->2415 MiB. This supersedes the earlier two-repeat evidence
at
`/private/tmp/qatq-live-vram-server-family-policy-soak-burnin2-p256q4-p05-tailgate-20260626`.
The superseded p128/q4 full-family rerun at
`/private/tmp/qatq-live-vram-server-family-policy-soak-burnin2-p128q4-p05-tailgate-20260626`
failed because Qwen2.5 3B missed p05/p50 at 0.832x/0.775x.
The same accepted soak now also runs with a steady-state RSS tail-growth gate:
`adapters/llama-cpp/live-vram-server-family-policy-soak-notrace.local.example.json`
sets `max_rss_tail_growth_kib: 8192` over the last four measured iterations.
The bootstrapped tail-gated run at
`/private/tmp/qatq-live-vram-server-family-policy-soak-bootstrap-notrace-tail-gated-20260625`
passed all six cases. Qwen2.5 1.5B recorded QATQ/native p50/p95 throughput
ratios of 0.977x/0.956x, p95 iteration/follow-up ratios of 1.012x/1.022x,
RSS-growth ratio 1.345x, backend KV 224->216 MiB, and 0 KiB tail RSS growth.
Qwen2.5 3B q8 recorded 1.012x/1.004x throughput ratios, 0.978x/0.801x
latency ratios, RSS ratio 0.594x, backend KV 288->280 MiB, and 0 KiB tail RSS
growth. Phi 3.5 mini recorded 0.989x/0.980x throughput ratios,
0.995x/1.027x latency ratios, RSS ratio 1.461x, backend KV 3072->2976 MiB,
and 0 KiB tail RSS growth. This adds a real no-slow-drift proof for the scoped
10-cycle policy, but still does not replace overnight burn-in or direct
hardware-counter peak-VRAM instrumentation.
The policy comparison gate was then tightened again so QATQ must reduce
backend K/V memory versus the native baseline (`max_backend_accelerator_context_ratio:
0.99`) and must not regress projected device memory
(`max_projected_device_memory_ratio: 1.0`). The device-memory-gated run at
`/private/tmp/qatq-live-vram-server-family-policy-soak-bootstrap-notrace-device-gated-20260625`
passed all six cases with empty comparison gate failures. Backend K/V ratios
were 0.964x for Qwen2.5 1.5B, 0.972x for Qwen2.5 3B, and 0.969x for Phi 3.5
mini. Projected device-memory ratios were 0.995x, 0.997x, and 0.982x. QATQ
still met the existing throughput, latency, RSS-growth, and RSS-tail gates;
the highest RSS-growth ratio was 1.902x for Qwen2.5 1.5B, still below the
2.0x policy ceiling. This proves the scoped accepted policy is both
steady-state stable and memory-reducing under llama.cpp backend diagnostics,
but direct hardware peak-VRAM counters remain a separate production gate.
`scripts/llama_cpp_live_vram_server_burnin.py` now repeats a configured server
matrix as a bounded burn-in gate. It fails on the first failed matrix run and
can enforce aggregate jitter ceilings for RSS growth, backend K/V memory, and
projected device memory across repeated runs. The first layer-policy burn-in
at `/private/tmp/qatq-live-vram-server-layer-policy-burnin2-device-jitter-20260625`
failed correctly on the existing QATQ/native RSS-growth ratio gate
(`2.401x > 2.0x`), even though backend K/V and projected device memory still
improved. The scoped accepted-family Qwen2.5 1.5B burn-in at
`/private/tmp/qatq-live-vram-server-family-qwen15b-policy-burnin2-device-jitter-20260625`
passed two native/QATQ matrix repeats with empty comparison and aggregate gate
failures. Backend K/V and projected-device jitter ratios were 1.0 for both
native and QATQ cases; QATQ RSS-growth jitter was 1.010x, and QATQ RSS tail
range stayed at or below 64 KiB. This is a meaningful burn-in increment, but
still not an overnight burn-in claim.
The bounded burn-in was then broadened to two Qwen model families at
`/private/tmp/qatq-live-vram-server-family-qwen15b-qwen3b-policy-burnin2-device-jitter-20260625`.
That run repeated the accepted family policy twice over Qwen2.5 1.5B and
Qwen2.5 3B native/QATQ pairs, for eight real matrix cases total. Both repeats
passed with empty comparison and aggregate gate failures. Backend K/V and
projected-device jitter ratios were 1.0 for all four cases. Qwen2.5 1.5B
QATQ/native p50 throughput ratios were 0.952x and 0.978x across the two
repeats, while backend K/V ratio stayed 0.964x and projected device ratio
stayed 0.995x. Qwen2.5 3B QATQ/native p50 throughput ratios were 0.996x and
0.988x, backend K/V ratio stayed 0.972x, and projected device ratio stayed
0.997x. This was superseded by the full-family burn-in at
`/private/tmp/qatq-live-vram-server-family-policy-soak-burnin2-taildelta-security-gated-20260625`,
which repeated Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini together with empty
comparison and aggregate gate failures. The accepted comparison gate now caps
positive QATQ RSS tail-growth delta over native at 2048 KiB, avoiding the
undefined zero-denominator ratio case when native tail growth is exactly flat.
The largest observed QATQ positive tail delta was 1712 KiB, while backend K/V
and projected-device jitter ratios stayed 1.0 for every native and QATQ case.
Overnight burn-in remains open.
Direct hardware peak-VRAM counter evidence is now a machine-checked blocker
rather than a prose-only caveat. `scripts/llama_cpp_live_vram_hardware_counters.py`
inspected the latest accepted burn-in matrix summary and local counter tooling at
`/private/tmp/qatq-live-vram-server-family-policy-soak-burnin3-p256q4-p05-tailgate-20260626/hardware-counters.json`.
It confirmed that all six matrix cases had llama.cpp backend projected-device
memory and accelerator-breakdown diagnostics, but direct peak-VRAM counters were
unavailable on this Apple Metal host. The helper now has an explicit NVIDIA
sampling path: with `--sample-pid`, `--sample-seconds`, and
`--require-direct-peak-vram`, it only passes when `nvidia-smi` returns
per-process `pid,used_memory` samples for the target runtime. Detecting
`nvidia-smi` on `PATH` is not enough. The live `llama-server` cancellation
probe also supports integrated sampling with `--sample-direct-peak-vram`,
`--require-direct-peak-vram-counter`, and
`--direct-peak-vram-sample-interval-ms`, storing the resulting
`direct_peak_vram_counter` object in `summary.json`. Matrix configs can carry
the same policy as `sample_direct_peak_vram`,
`require_direct_peak_vram_counter`, and `direct_peak_vram_sample_interval_ms`,
so burn-in runs on suitable hosts can fail closed on missing direct counters.
`direct_peak_vram_retain_samples` bounds the retained JSON samples for long
runs while preserving complete `sample_count` and `peak_memory_mib`.
The server probe now also bounds JSONL evidence ingestion with
`--max-trace-bytes` and `--max-trace-line-bytes`. Trace files are parsed
streamingly, and oversized files, malformed JSONL rows, or pathological
individual lines fail the evidence gate rather than being read into memory or
silently ignored. Matrix configs can forward the same policy as
`max_trace_bytes` and `max_trace_line_bytes`.
For native/QATQ comparison groups, `comparison_gates` can now require direct
hardware counters with `require_direct_peak_vram_counters: 1` and cap QATQ's
direct peak-VRAM ratio with `max_direct_peak_vram_ratio`.
Repeated burn-ins can also reject unstable direct counter measurements with
`--max-direct-peak-vram-jitter-ratio`.
On this host, `nvidia-smi` is absent;
`powermetrics` is present but requires superuser and documents per-process GPU
time rather than per-process peak GPU memory; `vmmap` is present but reports
virtual memory regions, not direct peak GPU memory. Backend projected memory,
backend K/V ratio gates, and RSS gates therefore remain strong engineering
evidence, but they are not presented as direct hardware peak-VRAM proof.

The first repeated direct selected-layer memory-accounting follow-up is now
green. The older broad 14-layer shape failed correctly under stricter
accounting because it staged 44,040,192 bytes against a 36,700,160-byte total
K/V context. The smaller Qwen2.5 1.5B selected-layer proof at
`/private/tmp/qatq-live-vram-native-layer-memory-gpufallback-pressure1g-repeat2-20260625`
passed twice under 1 GiB touched host-memory pressure with four QATQ-routed
layers, 168/168 exact restores per iteration, 112/112 QATQ offloaded pages per
iteration, zero pass-through, 56 resident pages, and stable 23.00 MiB
persistent GPU K/V reclaim. QATQ stored 25.00 MiB for the same block set in
both iterations versus 28.65 MiB with zstd and 31.12 MiB with lz4, while the
MLX gate checked 28 layers and 336 heads with streaming/materialised ratios of
1.768 and 1.776. This is meaningful direct residency evidence for the fixed
selected-layer path, but it is only one model/profile. Production graduation
still requires broader model/context/dtype/page-size coverage, longer burn-in,
and host/RSS optimisation.

The selected-layer direct-residency proof was then broadened across the local
three-model runtime set. The matrix at
`/private/tmp/qatq-live-vram-native-layer-memory-breadth-pressure1g-repeat2-20260625`
passed 6/6 cases under 1 GiB touched host-memory pressure: Qwen2.5 1.5B,
Qwen2.5 3B, and Phi 3.5 mini, each repeated twice. Every case selected four
QATQ-routed layers, restored all pages exactly, used zero pass-through pages,
passed strict live-paging and native page-streaming gates, and passed MLX
streaming-attention validation. Stable reclaim and storage numbers were:
23.00 MiB reclaim with QATQ 25.00 MiB versus zstd 28.65 MiB and lz4 31.12 MiB
for Qwen2.5 1.5B; 52.00 MiB reclaim with QATQ 49.62 MiB versus zstd 57.42 MiB
and lz4 62.43 MiB for Qwen2.5 3B; and 432.00 MiB reclaim with QATQ 391.57 MiB
versus zstd 448.79 MiB and lz4 493.38 MiB for Phi 3.5 mini. This turns the
direct selected-layer proof from a single-profile result into a small
three-model breadth result. It still does not close long-context latency-tail,
server burn-in, broader dtype/page-size, or runtime-adapter maintenance gates.

The remainder of this section preserves the hardening trail from earlier local
Metal runs. Older paragraphs that describe the backend-op route as missing are
superseded by the strict smoke above; older negative long-context latency
evidence remains relevant until a low-regression native long-context pass
replaces it.

The same installed runtime was also probed beyond f16. Real Apple Metal runs
accepted bf16 and f32 K/V cache settings, and strict native Qwen2.5 1.5B
evidence passed for bf16 at `/private/tmp/qatq-live-vram-dtype-bf16-20260624`
and f32 at `/private/tmp/qatq-live-vram-dtype-f32-20260624`. Both dtype runs
restored 168/168 pages exactly, offloaded 112/112 pages through QATQ with zero
pass-through pages, consumed pages through `ggml_segmented_kqv`, checked MLX
across 28 layers and 336 heads, and passed bounded restore-slot rejection.
The reproducible dtype matrix
`adapters/llama-cpp/live-vram-native-dtype.local.example.json` then passed 4/4
strict native cases at `/private/tmp/qatq-live-vram-native-dtype-matrix-20260624`
for Qwen2.5 Coder 3B bf16/f32 and Qwen2.5 3B bf16/f32, again with exact
restore, zero pass-through pages, native segmented attention, MLX checks across
36 layers and 576 heads, and bounded restore-slot rejection. The expanded dtype
matrix at `/private/tmp/qatq-live-vram-native-dtype-heavy-matrix-20260624` then
passed 8/8 strict native cases by adding Qwen2.5 Coder 7B bf16/f32 and
Phi 3.5 mini bf16/f32. This proves the native path is not f16-only on the
tested Apple M4/Qwen/Phi local model set. The same eight-case dtype matrix
then passed 16/16 runs at
`/private/tmp/qatq-live-vram-native-dtype-heavy-soak-2x-1g-20260624` with
stable reclaimable GPU bytes and stable raw/QATQ/zstd/lz4 byte totals under
1 GiB host-memory pressure. It still needs broader context, page-size, and
longer-duration burn-in coverage.

The matrix runner now records token-tail latency from each case's generated
`tokens.csv` file and can fail closed on insufficient token samples or excessive
mixed-KV p95/p99 regression. A focused real Metal/MLX latency-tail run at
`/private/tmp/qatq-live-vram-real-gpu-latency-tail-20260624` used 32 short
decode-token samples with strict live-paging, native `ggml_segmented_kqv`, GPU
page staging, aggregate codec gates, stable reclaim/codec-byte gates, and 512
MiB host-memory pressure. It passed with Qwen2.5 1.5B, restored 168/168 pages,
offloaded 112/112 pages through QATQ with zero pass-through pages, and reported
mixed-KV p95 `31,026 us` versus full-GPU p95 `31,467 us` plus mixed-KV p99
`74,207 us` versus full-GPU p99 `75,977 us`. This is a focused latency-tail
proof, not a replacement for 1-hour or overnight burn-in.

The same latency gate was then promoted to the sustained Coder matrix using
`--override-short-predict 32`. The real Metal/MLX run at
`/private/tmp/qatq-live-vram-real-gpu-sustained-latency-32tok-20260624` passed
4/4 Qwen2.5 Coder 3B/7B cases with 32 token samples per case, strict
live-paging, native `ggml_segmented_kqv`, GPU page staging, aggregate codec
gates, stable reclaim/codec-byte gates, and 512 MiB host-memory pressure.
Every case restored exactly, used zero pass-through pages, passed the MLX
streaming-attention gate, and stayed inside the configured p95/p99 latency
regression budget. The observed mixed-KV p95 regressions ranged from
`-4.30%` to `-0.17%`; mixed-KV p99 ranged from `-5.58%` to `+0.87%`.

The p95/p99 gate was then widened across the current breadth, dtype, and
page-size matrices, again with `--override-short-predict 32`, strict
live-paging, native `ggml_segmented_kqv`, GPU page staging, aggregate codec
gates, stable reclaim/codec-byte gates, MLX checks, and 512 MiB host-memory
pressure:

| matrix | work dir | result | largest mixed p95 regression | largest mixed p99 regression |
| --- | --- | ---: | ---: | ---: |
| breadth | `/private/tmp/qatq-live-vram-real-gpu-breadth-latency-32tok-20260624` | 3/3 pass | `+2.78%` | `+5.80%` |
| dtype | `/private/tmp/qatq-live-vram-real-gpu-dtype-latency-32tok-20260624` | 8/8 pass | `+5.21%` | `+2.80%` |
| page size | `/private/tmp/qatq-live-vram-real-gpu-page-size-latency-32tok-20260624` | 5/5 pass | `+3.21%` | `+0.02%` |

Across these 16 additional real GPU cases, all pages restored exactly, every
offloaded page used QATQ, pass-through pages stayed at zero, and every case
passed strict native live-paging plus MLX equivalence. This closes the current
local breadth/dtype/page-size p95/p99 gate.

A repeated sustained-Coder latency soak then ran the Qwen2.5 Coder 3B/7B review
and memory matrix twice at
`/private/tmp/qatq-live-vram-real-gpu-sustained-latency-soak-2x-20260624`. It
passed 8/8 real Metal/MLX cases with strict live-paging, native
`ggml_segmented_kqv`, GPU page staging, aggregate codec gates, stable
reclaim/codec-byte gates, a 35% elapsed-jitter gate, 512 MiB host-memory
pressure, and 32 decode-token latency samples per case. Every repeated run
restored all pages exactly, used zero pass-through pages, kept reclaimable GPU
bytes and QATQ/zstd/lz4 byte totals stable, and stayed inside the configured
p95/p99 regression budget. The largest observed mixed-KV p95 regression was
`+10.86%`; the largest p99 regression was `+1.58%`. This closes repeated local
sustained-Coder latency stability for the current matrix; long-context and
long-duration burn-in still remain open.

The first long-context active-decode tuning pass on 2026-06-24 deliberately
failed the deep p95/p99 gate and became the main live-VRAM production blocker.
The same profile has since been narrowed to the current scoped pass shape: four
selected cold KV layers with the flattened Flash Attention route enabled. The diagnostic
profile
`adapters/llama-cpp/live-vram-native-long-context-latency.local.example.json`
uses Qwen2.5 Coder 3B, a 6.8k-token prompt, 1024-token pages,
`cold-after-hot`, `max_queued_pages: 96`, and a deep full-GPU baseline.
Historical capped-offload and retained-page reuse runs preserved correctness,
exact restore, QATQ compression wins, MLX equivalence, and bounded artefact
retention, but did not make active long-context decode latency safe:

| run | result | reclaimable GPU KV | offloaded pages | deep p95 regression | deep p99 regression |
| --- | --- | ---: | ---: | ---: | ---: |
| 18/36 KV layers, native segmented, cap 96 | fail latency gate | 58.50 MiB | 96 | `+434%` | `+64.7%` |
| 18/36 KV layers, native segmented, cap 16 with retained-page reuse | fail latency gate | 58.50 MiB | 16 | `+442%` | `+71.9%` |
| 18/36 KV layers, non-native fallback, cap 16 | fail latency gate | 58.50 MiB | 16 | `+269%` | `+32.7%` |
| 24/36 KV layers, non-native fallback, cap 16 | fail allocator gate | over-staged | - | - | - |
| 23/36 KV layers, non-native fallback, cap 16, 2% reclaim diagnostic | fail latency gate | 7.25 MiB | 16 | `+627%` | `+20.6%` |
| 36/36 KV layers, native segmented, cap 96, page-segment trace disabled | fail latency gate | 126.00 MiB | 96 | `+98.4%` | `+57.6%` |
| 36/36 KV layers, persistent page-source, cap 96, page-segment trace disabled | fail latency gate | 126.00 MiB | 96 | `+103.3%` | `+92.9%` |
| 18/36 KV layers, backend-op native page streaming, cap 16, 1 GiB host pressure | fail timeout before deep mixed-KV timings | not reached | not reached | no samples | no samples |
| 1/36 KV layers, flattened Flash Attention route, cap 16, earlier single matrix with 1 GiB host pressure | pass scoped strict native gate, superseded for latency-tail evidence | 237.00 MiB | 16 | `-7.5%` | `+13.4%` |
| 1/36 KV layers, retained flattened Flash Attention table, cap 16, 2-iteration matrix with 1 GiB host pressure | pass scoped strict native gate | 223.25 MiB | 16 | worst `+3.1%` | worst `+4.2%` |
| 2/36 KV layers, retained flattened Flash Attention table, cap 16, 1 GiB host pressure | pass scoped strict native gate | 194.50 MiB | 16 | `+7.3%` | `-2.5%` |
| 3/36 KV layers, retained flattened Flash Attention table, cap 8, 2-iteration matrix with 1 GiB host pressure | pass scoped strict native gate | 165.75 MiB | 8 | worst `+8.6%` | better than baseline |
| 4/36 KV layers, retained flattened Flash Attention table, cap 8, 2-iteration matrix with 1 GiB host pressure | fail strict p95 boundary | 137.00 MiB | 8 | worst `+18.4%` | `+11.3%` |
| 4/36 KV layers, retained flattened Flash Attention table, cap 4, before cold-slot reuse | fail strict p95 boundary | 137.00 MiB | 4 | worst `+19.2%` | better than baseline |
| 4/36 KV layers, retained flattened Flash Attention table, cap 4, cold-slot reuse, 2-iteration matrix with 1 GiB host pressure | pass scoped strict native gate | 137.00 MiB | 4 | worst `+8.9%` | better than baseline |
| 4/36 KV layers, retained flattened Flash Attention table, cap 8, cold-slot reuse, 2-iteration matrix with 1 GiB host pressure | pass scoped strict native gate | 137.00 MiB | 8 | worst `+1.7%` | `+0.9%` |
| 4/36 KV layers, retained flattened Flash Attention table, cap 16, cold-slot reuse, 2-iteration matrix with 1 GiB host pressure | pass scoped strict native gate | 137.00 MiB | 16 | better than baseline | better than baseline |
| 4/36 KV layers, retained flattened Flash Attention table, cap 32, cold-slot reuse, 2-iteration matrix with 1 GiB host pressure | pass scoped strict native gate | 137.00 MiB | 32 | worst `+7.7%` | better than baseline |
| 4/36 KV layers, retained flattened Flash Attention table, cap 96, cold-slot reuse, 2-iteration matrix with 1 GiB host pressure | pass scoped strict native gate | 137.00 MiB | 96 | worst `+8.4%` | `+6.4%` |

The same fixed scheduler-aware native page-segment allocation passed a strict
short native proof at
`/private/tmp/qatq-live-vram-long-context-native-proof-all-staged-cap96-v3-20260624`.
That run used all 36 Qwen2.5 Coder 3B KV layers, cap 96, 1024-token pages,
`--require-live-paging`, `--require-native-page-streaming`, and the MLX
streaming-attention gate. It reclaimed 126.00 MiB, restored 432/432 pages
exactly, stored 96/96 offloaded pages through QATQ with zero pass-through pages,
proved `ggml_segmented_kqv` non-concat runtime attention consumption, and had
MLX check 36 layers and 576 heads. It is correctness and allocator evidence,
not a p95/p99 latency pass, because it generated only 8 tokens.

The sealed latency-budget runtime-reclaim fallback then passed a longer
128-token proof at
`/private/tmp/qatq-live-vram-long-context-runtime-reclaim-sealed-fallback-128tok-r4-20260624`.
That run kept the same Qwen2.5 Coder 3B long-context class and cap 96 policy,
restored 432/432 pages exactly, stored 96/96 offloaded pages through QATQ with
96/96 metadata seals, used zero pass-through pages, beat raw/zstd/lz4 on
432/432 page boundaries, and reduced persistent GPU K/V residency from
207.00 MiB to 0.00 MiB. Its 127-token deep timing comparison stayed inside the
15% p95 / 20% p99 latency budget: mixed-KV p50 was `+0.32%`, p95 was `+0.43%`,
and p99 was `+13.02%` versus the full-GPU baseline. This is the first
production-shaped long-context latency result with both sealed page metadata
and a fail-closed deep latency gate, but its claim is deliberately narrower than
strict native live paging: it proves a host-backed runtime-reclaim policy with
exact restore and preserved output, not transparent native page
eviction/restoration inside llama.cpp attention.

A fresh strict config-gated backend-op rerun at
`/private/tmp/qatq-live-vram-long-context-config-gated-20260624` reinforced
that boundary. It used the long-context local example's top-level matrix gates:
stable reclaim, stable raw/QATQ/zstd/lz4 bytes, 32 short/deep token samples,
15% p95 and 20% p99 limits, a deep full-GPU baseline, and 1 GiB of
page-touched host memory. The run reached the deep mixed-KV backend-op stage
after the short full-GPU, short mixed-KV, and deep full-GPU stages, then timed
out after 2,400 seconds before deep mixed-KV `token-timings.csv` or
`output-manifest.json` existed. The partial native trace contained 1,224
page-segment records and 576 attention-fixture files. Treat this as
fail-closed production evidence: strict native long-context active decode
still needs a runtime optimisation or policy fallback before it can be claimed.

A 2026-06-25 follow-up found and fixed a broader routing issue: the native
backend-op flag was forcing all 36 layers through segmented attention, even
when a layer had no cold/offloaded pages. The adapter now uses the native
backend-op only for layers whose page segments contain `live_offloaded: true`
and leaves all-resident layers on the stock llama.cpp attention path. This
turned the long-context backend-op result from a cliff into a smaller, bounded
overhead. The custom segmented K/Q/V Metal path still misses the strict
15% p95 / 20% p99 production gate for the best segmented backend-op profile, but an
eligible flattened route that feeds bounded page tables into llama.cpp's
backend-scheduled Flash Attention path now clears that scoped gate:

| run | result | cold backend-op events | resident fast-path events | deep p95 regression | deep p99 regression |
| --- | --- | ---: | ---: | ---: | ---: |
| `/private/tmp/qatq-live-vram-long-context-backend-op-l4-fast8192-20260625` | fail latency before selective fast path | all layers routed through backend-op | 0 | `+756%` | `+479%` |
| `/private/tmp/qatq-live-vram-long-context-backend-op-l4-selective-r2-20260625` | fail latency after selective fast path | 152 | 1,216 | `+73.9%` | `+58.3%` |
| `/private/tmp/qatq-live-vram-long-context-backend-op-l1-selective-20260625` | fail p95 only, shared retained pool | 38 | 1,330 | `+20.1%` | `+14.9%` |
| `/private/tmp/qatq-live-vram-long-context-backend-op-l1-default-privatepool-20260625` | fail p95 only, per-layer retained pool default | 38 | 1,330 | `+19.6%` | `+19.4%` |
| `/private/tmp/qatq-live-vram-long-context-flattened-flash-l1-strict-20260625` | pass, eligible flattened Flash Attention route | 38 | 1,330 | `+12.6%` | `+4.3%` |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-flash-20260625-r2` | pass, earlier single matrix with 1 GiB host pressure, superseded for latency-tail evidence | 16 compressed cold pages | 488 resident pages | `-7.5%` | `+13.4%` |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-129tok-2x-20260625` | pass, retained flattened table, 2 iterations, 1 GiB host pressure | 16 compressed cold pages | 488 resident pages | worst `+3.1%` | worst `+4.2%` |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l2-129tok-20260625` | pass, retained flattened table, 1 GiB host pressure | 16 compressed cold pages | 488 resident pages | `+7.3%` | `-2.5%` |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l3-q8-129tok-2x-20260625` | pass, retained flattened table, cap 8, 2 iterations, 1 GiB host pressure | 8 compressed cold pages | 496 resident pages | worst `+8.6%` | better than baseline |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l4-q8-129tok-2x-20260625` | fail strict p95, retained flattened table, cap 8, 2 iterations, 1 GiB host pressure | 8 compressed cold pages | 496 resident pages | worst `+18.4%` | `+11.3%` |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l4-q4-coldreuse-129tok-2x-20260625` | pass, retained flattened table, cap 4, cold-slot reuse, 2 iterations, 1 GiB host pressure | 4 compressed cold pages | 500 resident pages | worst `+8.9%` | better than baseline |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l5-q4-coldreuse-129tok-2x-20260625` | pass, five selected-layer breadth, retained flattened table, cap 4, cold-slot reuse, 2 iterations, 1 GiB host pressure | 4 compressed cold pages | 500 resident pages | worst `+10.7%` | `+15.0%` |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l6-q4-coldreuse-129tok-2x-20260625` | pass, six selected-layer breadth, retained flattened table, cap 4, cold-slot reuse, 2 iterations, 1 GiB host pressure | 4 compressed cold pages | 500 resident pages | worst `+8.1%` | better than baseline |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l4-q8-coldreuse-129tok-2x-20260625` | pass, retained flattened table, cap 8, cold-slot reuse, 2 iterations, 1 GiB host pressure | 8 compressed cold pages | 496 resident pages | worst `+1.7%` | `+0.9%` |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l4-q16-coldreuse-129tok-2x-20260625` | pass, retained flattened table, cap 16, cold-slot reuse, 2 iterations, 1 GiB host pressure | 16 compressed cold pages | 488 resident pages | better than baseline | better than baseline |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l4-q32-coldreuse-129tok-2x-20260625` | pass, retained flattened table, cap 32, cold-slot reuse, 2 iterations, 1 GiB host pressure | 32 compressed cold pages | 472 resident pages | worst `+7.7%` | better than baseline |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l4-q96-coldreuse-129tok-2x-20260625` | pass, retained flattened table, cap 96, cold-slot reuse, 2 iterations, 1 GiB host pressure | 96 compressed cold pages | 408 resident pages | worst `+8.4%` | `+6.4%` |
| `/private/tmp/qatq-live-vram-long-context-prefetch-active-20260625` | fail deep latency, explicit prefetch 32, cap 96, 2 iterations, 1 GiB host pressure | 96 compressed cold pages | 408 resident pages | iter 2 `+36.1%` | iter 2 `+40.7%` |
| `/private/tmp/qatq-live-vram-long-context-prefetch-active-q32-20260625` | pass, explicit prefetch 32, cap 32, 2 iterations, 1 GiB host pressure | 32 compressed cold pages | 472 resident pages | worst `+7.6%` | better than baseline |
| `/private/tmp/qatq-live-vram-long-context-latency-repeat2-current-20260625` | pass, current binaries, checked-in config, explicit prefetch 32, cap 32, 2 iterations, 1 GiB host pressure | 32 compressed cold pages | 472 resident pages | better than baseline in both iterations | better than baseline in both iterations |
| `/private/tmp/qatq-live-vram-long-context-latency-bootstrap-repeat2-20260625` | pass, clean bootstrapped pinned source/binary, checked-in config, explicit prefetch 32, cap 32, 2 iterations, 1 GiB host pressure | 32 compressed cold pages | 472 resident pages | better than baseline in both iterations | better than baseline in both iterations |
| `/private/tmp/qatq-live-vram-long-context-backend-op-l1-p2048-selective-20260625` | fail p95 with 2,048-token pages | 38 | 1,330 | `+41.0%` | `+16.5%` |

The best current checked-in strict native long-context case is therefore the
conservative prefetch-active matrix profile at
`/private/tmp/qatq-live-vram-long-context-latency-bootstrap-repeat2-20260625`:
four selected KV layers with 1,024-token pages, the per-layer retained pool default,
explicit `prefetch_window_tokens: 32`, and `--native-page-streaming-flatten-flash` /
`--qatq-native-page-streaming-flatten-flash` enabled. Output comparison passed,
native page-streaming status passed, `504/504` pages restored exactly,
`32/32` cold pages compressed through QATQ, zero pass-through pages, cold rows
consumed by `backend_scheduled_flattened_flash_attention`, and the checked-in
deep p95/p99 gate passed across two iterations with both deep p95 and deep p99
better than the full-GPU baseline under 1 GiB host-memory pressure. The
bootstrapped repeat-2 run kept stable reclaim (`137.00 MiB`) and stable codec
bytes (QATQ `179.00 MiB`, zstd `218.30 MiB`, lz4 `237.57 MiB`) across both
iterations. Its deep p95 regressions were `-30.9%` and `-31.1%`; its deep p99
regressions were `-50.6%` and `-27.9%`. The retained flattened table avoids
rebuilding the full transient page table every decode token. Cold-slot reuse
also avoids repeated syncs for immutable live-offloaded rows after their
retained table slot is populated; first use and mutable tail rows still sync
before attention. It reclaimed
137.00 MiB of persistent GPU K/V while keeping the strict latency budget. QATQ
stored 179.00 MiB versus zstd 218.30 MiB and lz4 237.57 MiB on the same page
boundaries. The bootstrapped rerun kept 128 short and 128 deep generated-token
samples per iteration. Earlier four-layer cap-8 and cap-4 reruns preserved exactness,
compression, stable reclaim, and p99, but missed the deep p95 budget before
cold-slot reuse; after the fix, cap 8, 16, and 32 pass in repeated local
matrices. A fresh cap-96 rerun with explicit prefetch preserved correctness and
compression but failed deep p95/p99 latency, so aggressive reclaim remains a
tuning frontier rather than the checked-in default. Five/six selected-layer
breadth runs also pass, but with lower reclaim. The next frontier is broader
runtime/model coverage, more aggressive reclaim-policy coverage, and longer
soaks rather than this queue-width path alone. This is
meaningful but intentionally scoped performance evidence: it covers an eligible
one-stream Flash Attention layout and a four-layer policy, not every runtime
layout or broader policy.

This means the current strict native page-staged llama.cpp adapter has a
credible low-regression long-context path, but it is still not
production-complete. The follow-up low-overhead work also fixed one real
allocator bug: native page-segment staging now honours the same scheduler
residency predicate as the event trace, so all-layer page staging with cap 96
reclaimed 126.00 MiB instead of over-staging the full context. The remaining
native implementation step is broader optimisation and validation: extend the
cold-slot reuse and flattened-table strategy beyond the eligible Flash
Attention layout, improve the native segmented attention consumer, or teach
the native page path to fall back before page staging creates a p95 cliff.
Lowering the offload cap alone was insufficient before cold-slot reuse because
page-staged attention cost still dominated once the prompt reached the 6.8k
token range.

The native path has also passed a focused page-size breadth matrix. The
configuration
`adapters/llama-cpp/live-vram-native-page-size.local.example.json` now runs
from the clean pinned llama.cpp bootstrap with real Apple Metal, MLX
`Device(gpu, 0)`, native flattened Flash page streaming, GPU page staging,
aggregate codec gates, strict live-paging gates, strict native page-streaming
gates, restore-slot pressure checks, and 1 GiB host-memory pressure. The fresh
bootstrapped run at
`/private/tmp/qatq-live-vram-page-size-bootstrap-20260625` passed 5/5
Qwen/Qwen-Coder cases across 256, 1024, and 2048-token pages, with exact
restore, zero raw pass-through pages, MLX all-layer/head page-streaming gates,
and QATQ beating raw, zstd, and lz4 in aggregate for every case. The passing
matrix uses compact `[1, 2, 4]` selected-layer frontiers for the 256-token
pages to avoid staging more GPU memory than they reclaim, and broader
page-segment traces for 2048-token pages so the strict verifier observes real
multi-segment streaming. The matching Phi-specific matrix at
`/private/tmp/qatq-live-vram-phi-page-size-bootstrap-20260625` passed 3/3
Phi 3.5 mini cases across 256, 512, and 1024-token pages with the same strict
native, MLX, restore-slot, aggregate codec, and 1 GiB pressure gates. Longer
contexts and longer-duration burn-in remain open.

Qwen2.5 1.5B and Phi 3.5 mini started as hardening/stress fixtures and exposed
graph-budget, telemetry-size, performance, and compression-policy limits in
older concat-composed paths. They have since been rerun through the native
`ggml_segmented_kqv`/`GGML_OP_QATQ_SEGMENTED_KQV` backend-op path and remain
important breadth fixtures. The project should still avoid a production-complete
live VRAM claim until the native route is burned in across more models, dtypes,
context lengths, sequence mixes, latency-tail runs, and adverse memory-pressure
profiles.

On 2026-06-23, a patched Metal-backed llama.cpp build exported real internal
f16 KV tensors from Qwen2.5 1.5B, Qwen2.5 Coder 3B, and Phi 3.5 mini local GGUF
models. QATQ replayed those manifests through `qatq-kv-bench
--live-vram-export-dir`, verified exact restore for every page, and compared
the same page boundaries against raw bytes, zstd, and lz4.

Summary:

| model / workload | prompt tokens | raw active KV | QATQ stored | zstd | lz4 | exact restores |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Qwen2.5 1.5B Instruct | 4,224 | 115.50 MiB | 85.81 MiB | 104.77 MiB | 113.87 MiB | 56 / 56 |
| Qwen2.5 Coder 3B Instruct | 3,600 | 126.56 MiB | 95.90 MiB | 115.18 MiB | 125.29 MiB | 72 / 72 |
| Phi 3.5 mini Instruct | 3,072 | 1,152.00 MiB | 884.37 MiB | 1,047.21 MiB | 1,155.02 MiB | 64 / 64 |

This proves real GPU-runtime KV export ingestion and long-context exact
compression-positive replay. The current patched llama.cpp adapter also proves
coarse runtime GPU KV reduction when KV layers are split between Metal and host
memory at context construction time. It does not prove GPU token-page eviction,
live restore inside the attention loop, or transparent end-to-end live VRAM
reduction during generation.

A 2026-06-24 rerun with the attention-read hook enabled passed the four-profile
Metal matrix at `/private/tmp/qatq-live-vram-matrix-attention-20260624`. The
matrix covered Qwen2.5 1.5B daily-driver, Phi 3.5 operations/security,
Qwen2.5 Coder 3B workhorse, and Qwen2.5 Coder 7B powerhouse profiles. It
passed 4/4 runtime-reclaim cases, restored every exported page exactly, beat
raw/zstd/lz4 on the same page boundaries, and captured real attention-path
K/V read telemetry for every case: 672, 896, 936, and 784 attention-read events
respectively. That historical runtime-reclaim audit failed by design because
the runtime had no page allocator, no per-page manifest attestation, and
attention still read whole-cache tensor views. The current retained page-table
backend-op path described above supersedes that limitation for the pinned
patched adapter.

The patched llama.cpp exporter now supports `--qatq-page-tokens <n>` for
token-range KV page export. A Qwen2.5 1.5B Metal run with `--page-tokens 1024`
at `/private/tmp/qatq-live-vram-page-evidence-1024-20260624` exported 224
token pages, restored 224/224 exactly, stored 224/224 through QATQ with zero
pass-through pages, and beat zstd/lz4 on 224/224 page boundaries. An earlier
512-token export-only run restored successfully but failed the compression gate
on one tail page, so 1024 tokens was the first safer export-only page size for
that model/prompt shape. The strict live-paging gate still fails closed for
export-only token-page captures because token-page export is not
allocator-backed token-page eviction.

`scripts/llama_cpp_live_vram_page_size_sweep.py` now makes that page-size
selection reproducible. A Qwen2.5 1.5B sweep at
`/private/tmp/qatq-live-vram-page-size-sweep-20260624` tested 512, 1024, and
2048 token pages. The 512-token run failed the all-pages compression gate,
while 1024 and 2048 passed; 1024 remains the recommended first experimental
size because it is the smallest passing size.

A follow-up Qwen2.5 1.5B Metal run at
`/private/tmp/qatq-live-vram-page-end-hot-window-20260624` used
`--page-tokens 1024`, `--hot-window-tokens 1024`, and
`--next-required page-end`. The replay kept 56 current hot-window token pages
resident and offloaded 168 colder pages through QATQ, while still restoring
224/224 pages exactly and beating zstd/lz4 on 224/224 page boundaries. This
proves page-aware scheduling over a real llama.cpp KV export; it does not prove
transparent live token-page eviction because the strict live-paging gate still
fails closed on whole-tensor allocation granularity.

The strict verifier now also rejects traces that claim to offload pages the
scheduler kept resident. Replaying the hot-window capture through
`--live-vram-live-paging-gate` fails with the expected allocator limitations
and `live VRAM trace offloaded a page that evidence kept resident (x56)`. A
future live adapter must emit attention-loop events that match the scheduler's
actual page residency decisions.

The patched llama.cpp exporter now has scheduler-aligned trace flags:
`--qatq-trace-current-token`, `--qatq-trace-hot-window-tokens`, and
`--qatq-trace-next-required`. A fresh Qwen2.5 1.5B Metal run at
`/private/tmp/qatq-live-vram-page-end-aligned-trace-20260624` used those flags
with the same page-end policy. Its trace emitted 224 snapshots, 168 offloads,
168 restores, and 224 attention-use events, matching the evidence split of 56
resident hot-window pages and 168 offloaded pages. The strict live-paging gate
then failed only on allocator proof: whole-tensor granularity, zero reclaimable
page bytes, and zero GPU saved ratio.

The patched llama.cpp runner now also supports a backend page-operation
self-test. A direct Apple M4 Metal smoke and a reproducible runner pass at
`/private/tmp/qatq-live-vram-self-test-runner-20260624` proved that one active
key page can be read from runtime KV tensor storage, zeroed through the backend,
verified as changed, restored, and checksum-verified. The same run preserved
the deterministic continuation, restored 224/224 exported pages exactly, kept
56 hot-window pages resident, stored 168 colder pages through QATQ, used zero
pass-through pages, beat zstd/lz4 on 224/224 pages, and retained the same
allocator limitation: 14.00 MiB of coarse mixed-layer GPU KV reclaim, but no
per-page Metal allocation reclaim.

A real Qwen2.5 1.5B Apple Metal attention-fixture run then exported a computed
llama.cpp query vector plus six f16 K/V token pages at
`/private/tmp/qatq-real-attention-fixture-pages-20260624`. QATQ sliced the real
K/V pages for layer 0, head 0 and passed
`qatq-kv-bench --attention-equivalence-gate` with 23 tokens, 128-dimensional
heads, zero max absolute error, zero max relative error, and a peak
page/materialised KV working-set ratio of 0.173913043. This proves
page-bounded attention equivalence over real llama.cpp Q/K/V artefacts. It
still does not prove transparent VRAM reduction because llama.cpp is not yet
consuming those QATQ pages inside a native paged attention loop.

The same gate is now integrated into `scripts/llama_cpp_live_vram_evidence.py`.
A fresh Qwen2.5 1.5B Apple Metal run at
`/private/tmp/qatq-live-vram-integrated-attention-20260624-rerun` passed with
24/28 KV GPU layers, 224/224 exact restores, 224/224 QATQ pages beating
zstd/lz4, 728 real attention-path K/V read events, and attention equivalence
over four f16 K/V pages and 3,521 tokens with zero max absolute/relative error.
The peak page/materialised KV working-set ratio was 0.290826470.

The evidence runner now also enables the actual attention-path lifecycle trace
by default. A Qwen2.5 1.5B Apple Metal run at
`/private/tmp/qatq-live-vram-attention-events-20260624` passed with 24/28 KV GPU
layers, preserved the 8-token deterministic continuation, restored 224/224
pages exactly, stored 224/224 offloaded pages through QATQ, beat zstd/lz4 on
224/224 page boundaries, and reduced coarse Metal KV allocation from 98.00 MiB
to 84.00 MiB. The deep run captured 728 attention-read telemetry events and
8,960 attention-path lifecycle events from `get_k/get_v`: 2,240 snapshots,
2,240 offload commits, 2,240 restore commits, 2,240 attention uses, zero
unfinished offloads, and a passing QATQ event-trace gate. This proves the
attention hook emits restore-before-attention lifecycle evidence; it still does
not prove that Metal pages were actually freed and later reallocated.

The same runner now writes replayable evidence tables alongside `summary.md`:
`pages.csv` contains one row per exported page with runtime/model/page metadata,
schedule decision, storage strategy, raw/QATQ/zstd/lz4 byte counts, and restore
verification. `tokens.csv` contains run-level proof summary rows plus
per-`llama_decode` timing rows from patched llama.cpp for the full-GPU,
mixed-KV, CPU-KV, and deep mixed-KV paths. A fresh Apple Metal run at
`/private/tmp/qatq-live-vram-gpu-real-20260624` produced 25 per-decode timing
rows plus the four summary rows while preserving the deterministic continuation,
restoring 224/224 pages exactly, and passing the attention-equivalence and
attention-lifecycle gates. It is still not policy-grade p50/p95/p99 latency
telemetry with restore-stall attribution.

The patched llama.cpp adapter then gained an explicit logical page residency
primitive: `qatq_live_evict_page`, `qatq_live_restore_page`, and
`qatq_live_page_resident`. A fresh integrated Apple Metal run at
`/private/tmp/qatq-live-vram-logical-residency-20260624` passed with
`--live-page-self-test-tokens 16`; the deep run restored 224/224 exported pages,
beat zstd/lz4 on every page boundary, passed the attention lifecycle and
attention-equivalence gates, and the self-test restored layer 0 stream 0 key
page bytes for 16 active tokens, 8,192 bytes. The manifest records
`live_page_residency_granularity: "per-page"` while still reporting
`gpu_allocation_granularity: "whole-tensor"`, keeping the physical allocation
claim deliberately scoped.

The QATQ CLI now enforces that boundary. `--live-vram-live-paging-gate`
requires the manifest to attest `live_page_residency_granularity: "per-page"`
and still requires physical `gpu_allocation_granularity: "per-page"` before it
will report page-level GPU reclaim. A fresh Apple Metal Qwen2.5 1.5B run at
`/private/tmp/qatq-live-vram-real-gpu-20260624-gated` passed the
runtime-reclaim evidence path and the logical page self-test, then correctly
failed the stricter live-paging gate because the current llama.cpp allocation
granularity remains `whole-tensor`.

The adapter then gained `--qatq-live-physical-page-alloc-self-test <n>`, a
separate physical page tensor smoke. It allocates one page-sized non-host
backend tensor on the same backend as an active key page, round-trips real KV
bytes through that tensor, and frees it. A fresh integrated Apple Metal Qwen2.5
1.5B run at `/private/tmp/qatq-live-vram-real-gpu-20260624-physical-page-tensor`
passed this test with an `MTL0` page tensor: 16 active tokens, 8,192 requested
bytes, and 8,192 allocated bytes. This is a real physical page-tensor
allocation and byte round-trip primitive, but it is still not integrated with
the attention loop and therefore does not complete live paging.

The next adapter step moved that tensor materialisation proof into the actual
attention read path. `--qatq-attention-page-tensor-self-test <path>` makes
patched llama.cpp materialise bounded K/V page bytes observed in `get_k/get_v`
into separate non-host backend page tensors, round-trip those bytes exactly,
write JSONL evidence, and free the temporary tensors. A fresh integrated Apple
Metal Qwen2.5 1.5B run at
`/private/tmp/qatq-live-vram-real-gpu-20260624-attention-page-tensor` passed
this gate with 16 `MTL0` attention-path page tensor events, 7,340,032 requested
bytes, and 7,340,032 allocated bytes while also preserving deterministic
continuation, restoring 224/224 exported pages exactly, beating zstd/lz4 on
224/224 page boundaries, passing attention equivalence, passing attention
lifecycle verification, and passing the logical and physical page self-tests.
This proves real attention-path page tensor materialisation mechanics. It still
does not prove transparent live paging because llama.cpp's attention graph does
not yet consume those page tensors as its K/V source.

The following adapter increment added
`--qatq-attention-materialized-source-trace <path>`. In that mode `get_k/get_v`
wraps the attention source in `ggml_cont` before returning it to llama.cpp's
attention graph and emits a JSONL trace for each materialised K/V source. A
paired Apple Metal Qwen2.5 1.5B smoke at
`/private/tmp/qatq-materialized-source-smoke` first compared the native full-GPU
attention source against this materialised-source path and preserved the
generated text hash exactly. The stronger integrated run at
`/private/tmp/qatq-live-vram-real-gpu-20260624-materialized-source-integrated`
then folded this gate into the full evidence bundle: 448 short-run
materialised-source events, 728 deep-run materialised-source events, identical
native/materialised generated text hash `16975742283144246935`, 224/224 exact
restores, 224/224 QATQ wins over zstd/lz4, attention equivalence with zero
error, and the attention-path page tensor self-test on `MTL0`. This proves
llama.cpp can consume a materialised K/V source without output drift. It is
still an intermediate proof: the materialised tensor is copied from the
persistent KV cache, so it does not release that persistent allocation or
complete live VRAM reduction.

The adapter then moved from whole-source materialisation to bounded
page-composed attention sources with
`--qatq-attention-page-composed-source-trace <path>`. This mode splits the
`get_k/get_v` attention source into token pages, materialises each page, then
uses `ggml_concat` to compose the page stream before llama.cpp attention
consumes it. A standalone Apple Metal smoke at
`/private/tmp/qatq-page-composed-source-smoke` passed with 448 page-composed
source events, 64-token pages, four pages per early K/V source, and identical
generated output versus the native attention source. The integrated run at
`/private/tmp/qatq-live-vram-real-gpu-20260624-page-composed-source-integrated`
then passed the full evidence bundle: 448 short-run page-composed events, 728
deep-run page-composed events, max deep page count 4, identical native and
page-composed generated hash `16975742283144246935`, 224/224 exact restores,
224/224 QATQ wins over zstd/lz4, attention equivalence with zero error, and
the `MTL0` attention-path page tensor self-test. This proves attention can
consume a page-composed K/V source without output drift. It still does not
complete live VRAM reduction because the pages are composed from the persistent
KV cache rather than replacing that allocation with QATQ-backed page residency.

The adapter then moved the attention source one step closer to native page
residency with `--qatq-attention-persistent-page-source-trace <path>`. This
mode splits the `get_k/get_v` attention source into token pages, allocates
retained page tensors on the same non-host backend, uses graph-native
`ggml_cpy` operations to fill those retained page tensors during execution, and
then composes the copied page tensors with `ggml_concat` before attention
consumes them. A direct Apple Metal smoke at
`/private/tmp/qatq-persistent-page-source-smoke` passed on backend `MTL0` with
112 persistent page-source events, 64-token pages, four pages per early K/V
source, retained backend page tensors, and identical generated output versus
the native full-GPU attention source. This is stronger than the page-composed
source proof because attention consumes independently allocated backend page
tensors rather than only materialised page views. It still does not complete
live VRAM reduction because the graph copies are sourced from the default
persistent KV cache and the default allocator has not yet released page-level
GPU memory.

The integrated run at
`/private/tmp/qatq-live-vram-real-gpu-20260624-persistent-page-source-integrated`
then passed the full evidence bundle on Apple Metal: 112 short-run persistent
page-source events, 192 deep-run persistent page-source events, backend `MTL0`,
max deep page count 2 with page-token values 512 and 1024, 288 max retained
page tensors, identical native/persistent-source generated hash
`13136292135900150276`, 112/112 exact restores, 112/112 QATQ wins over
zstd/lz4, 4,032 attention lifecycle events, attention equivalence over 1,761
tokens with zero max absolute/relative error, and 16 retained persistent
page-pool buffers. Runtime-reclaim evidence reported 7.00 MiB reclaimable GPU
KV at whole-tensor/layer granularity. The same capture intentionally failed
`--live-vram-live-paging-gate` with `allocation granularity whole-tensor cannot
prove page-level GPU reclaim`, which remains the correct production boundary.

The adapter now also has an explicit `--qatq-gpu-page-staging` mode. In this
mode the canonical llama.cpp KV tensors are allocated off GPU, while
`get_k/get_v` stages attention pages into retained accelerator page tensors via
the persistent page-source path. A paired Apple Metal smoke at
`/private/tmp/qatq-gpu-page-staging-smoke` preserved generated output against
the native full-GPU baseline and produced `gpu_allocation_granularity:
"per-page"` plus `gpu_page_staging_bytes` in the manifest. QATQ deliberately
rejects this as strict live-paging evidence when the staged GPU bytes are not
below the full KV context size; that early smoke failed closed with
`page-staged GPU bytes below total KV context bytes, got 7340032 >= 7340032`.
This prevents a page-staging adapter that materialises every attention page at
once from being marketed as live VRAM reduction. The current retained
page-table backend-op path is the successor proof path: it keeps canonical KV
off GPU, stages bounded pages into retained accelerator page-table tensors, and
feeds those tables directly to segmented attention. The remaining runtime work
is broader proof that this path holds up across longer contexts and pressure
profiles while keeping only a bounded subset of pages resident at a time.

QATQ now has a bounded-attention gate for that next runtime shape. The
`qatq-kv-bench --attention-*` path accepts
`--attention-max-peak-page-kv-ratio <r>`, and
`scripts/llama_cpp_attention_fixture_gate.py` forwards
`--max-peak-page-kv-ratio <r>` for real llama.cpp Q/K/V fixtures. Running this
against the real Metal fixture at
`/private/tmp/qatq-live-vram-real-gpu-20260624-persistent-page-source-integrated`
passed with zero max absolute/relative error and a peak page/materialised KV
ratio of `0.581487791` under a `0.75` gate. This proves the streaming softmax
recurrence can preserve attention output while needing less peak K/V residency
than materialising the full attention context. The remaining task is moving
that recurrence into the llama.cpp runtime graph/kernel so the measured GPU
staging bytes follow the same bound during generation.

QATQ core now also carries the page-summary form of that recurrence through
`live_vram_segment_summary_attention_reference` and
`compare_live_vram_segment_summary_attention_reference`. This is the executable
oracle for native multi-segment attention: each page produces a local max,
denominator, and unnormalised output, then the reducer combines page summaries
with a stable online softmax update. `qatq-kv-bench --attention-*` emits the
same check as `segment_summary_reduction: "online-page-summary"`. That proves
the exact reduction the runtime needs, and the llama.cpp/Metal adapter now wires
the f32 subset through a backend-schedulable masked `GGML_OP_QATQ_SEGMENTED_KQV`
path. The current llama.cpp patch also has a compile-tested multi-segment graph
bridge that avoids K/V concatenation by concatenating only per-page logits for
one global softmax, then slicing the probabilities back across V pages. That
bridge is useful for correctness and adapter evolution, but the adapter audit
now requires a descriptor/page-table backend that avoids staged arenas and
per-graph page-pool copies,
page-bounded equivalence evidence, and strict GPU latency/memory validation
before production live-VRAM claims; a graph-only bridge or compile-only backend
proof is not sufficient.

The next adapter increment added a retained backend page-pool proof with
`--qatq-live-persistent-page-pool-self-test <n>` and
`--qatq-live-persistent-page-pool-trace <path>`. A direct Apple Metal smoke at
`/private/tmp/qatq-persistent-page-pool-smoke` retained 8 verified `MTL0` page
buffers until context teardown. The integrated evidence run at
`/private/tmp/qatq-live-vram-real-gpu-20260624-persistent-page-pool-integrated`
then required 16 retained page buffers and passed: 16 persistent page-pool
events, backend `MTL0`, key and value pages, 8,388,608 requested bytes and
8,388,608 allocated bytes. The same run preserved deterministic output,
restored 112/112 exported pages exactly, stored 112/112 offloaded pages through
QATQ, beat zstd/lz4 on 112/112 pages, passed attention lifecycle verification,
passed page-composed attention source verification, and passed attention
equivalence over 1,761 tokens with zero max absolute/relative error. Replaying
the same capture through `--live-vram-live-paging-gate` still fails with
`allocation granularity whole-tensor cannot prove page-level GPU reclaim`,
which is the correct remaining boundary: the retained page pool is real
page-resident storage, but the main KV allocator still allocates whole-layer
buffers.

A later deterministic Apple M4 check paired a full-GPU KV run with a mixed
`--qatq-kv-gpu-layers 16` run on the same Phi 3.5 mini model, prompt, greedy
sampler, f16 K/V cache, and 32-token continuation. The mixed run reduced Metal
KV allocation from 96 MiB to 48 MiB plus 48 MiB host KV. The generated token
arrays and generated text hashes matched exactly. QATQ replay over the mixed
export stored 9.87 MiB versus 10.88 MiB raw, 10.09 MiB zstd, and 10.92 MiB lz4,
with 64/64 exact restores and 64/64 pages beating the best general codec.

A fresh Qwen2.5 Coder 3B Apple M4 check then paired full-GPU KV with
`--qatq-kv-gpu-layers 18`. The 32-token continuation gate preserved the
generated text hash exactly while reducing the small-run Metal KV allocation
from 9.00 MiB to 4.50 MiB plus 4.50 MiB host KV. A deeper 3,521-token prefill
export reported 132,120,576 total KV context bytes, 66,060,288 GPU KV bytes,
and 36/72 GPU-resident tensors. QATQ replay verified 72/72 restores and stored
96,784,543 bytes versus 129,798,144 raw bytes, 118,057,317 zstd bytes, and
128,483,967 lz4 bytes, beating the best general codec on 72/72 pages.

This check is now reproducible with
`scripts/llama_cpp_live_vram_evidence.py`. The runner fails closed unless the
paired full-GPU and mixed-KV output manifests match, the all-CPU-KV native
baseline also preserves output, mixed-KV decode stays within the configured
full-GPU regression ceiling, mixed-KV decode is faster than all-CPU-KV, the deep
mixed-KV export carries runtime allocator attestation, the export-time event
trace passes QATQ verification, the runtime reclaim gate passes, every page
restores exactly, restore-deadline estimates have zero misses, and every
offloaded page is QATQ-compressed and beats zstd/lz4. With
`--sweep-kv-gpu-layers`, the runner
evaluates multiple mixed-KV layer counts and selects the fastest candidate that
still passes those memory, output, and performance gates.

For long-context latency-budget evidence, enable the runner's deep token gate:

```sh
python3 scripts/llama_cpp_live_vram_evidence.py \
  --llama-simple /path/to/patched/llama-simple \
  --model /path/to/model.gguf \
  --model-id <stable-model-id> \
  --sweep-kv-gpu-layers 36 \
  --page-tokens 1024 \
  --deep-latency-baseline \
  --min-deep-token-latency-samples 64 \
  --max-deep-mixed-token-p95-regression-ratio 0.15 \
  --max-deep-mixed-token-p99-regression-ratio 0.20 \
  --work-dir /tmp/qatq-live-vram-runtime-reclaim-latency
```

That gate compares generated-token rows from the deep full-GPU and deep mixed
KV runs, writes `deep-latency-gate.json`, and fails before the run can be
treated as production-shaped evidence if the sample count, p95, or p99 budget
is missed. This is now the preferred way to reproduce the long-context
runtime-reclaim fallback result; strict native runs should use the same gate
once the page-streaming attention path is fast enough.

For matrix runs, put those production-shaped gates in the config itself under
top-level `matrix_gates` so the evidence is reproducible from the JSON file and
not from operator memory. The checked-in
`adapters/llama-cpp/live-vram-native-long-context-latency.local.example.json`
now requires 128 short/deep generated-token samples, 15% p95 and 20% p99 decode
regression ceilings, stable reclaim/codec bytes, a deep full-GPU baseline, and
1 GiB of bounded host-memory pressure unless the operator deliberately passes
`--ignore-config-gates` for diagnostics.

The same runner now separates the scoped live-paging gate from the future
production native-attention gate:

```sh
python3 scripts/llama_cpp_live_vram_evidence.py \
  --llama-simple /path/to/patched/llama-simple \
  --model /path/to/model.gguf \
  --model-id <stable-model-id> \
  --sweep-kv-gpu-layers 18,24,30 \
  --work-dir /tmp/qatq-live-vram-evidence \
  --require-live-paging \
  --gpu-page-staging
```

`--require-live-paging` switches from the coarse runtime-reclaim gate to
`qatq-kv-bench --live-vram-live-paging-gate`. The current page-staged llama.cpp
adapter can pass this scoped gate with `--gpu-page-staging`: it proves
page-granular runtime reclaim, restore-before-attention ordering, exact QATQ
restore, and deterministic continuation preservation.

The future production gate is stricter:

```sh
python3 scripts/llama_cpp_live_vram_evidence.py \
  --llama-simple /path/to/patched/llama-simple \
  --model /path/to/model.gguf \
  --model-id <stable-model-id> \
  --sweep-kv-gpu-layers 18,24,30 \
  --work-dir /tmp/qatq-live-vram-evidence \
  --require-live-paging \
  --require-native-page-streaming \
  --native-page-streaming-attention-backend-op \
  --native-page-streaming-flatten-flash \
  --gpu-page-staging \
  --mlx-streaming-attention-gate
```

`--require-native-page-streaming` fails unless the runtime attention graph
consumes K/V pages through a non-concat page-streaming path. A fresh structural
audit of the current pinned adapter patch at
`7992aa7c8e21ea2eb7a5e4802da56eec7b376036` reports `export_ready: true` and
`page_staging_ready: true`, and `live_paging_ready: true` under the structural
native live-paging gate: the applied source exposes page-segment telemetry,
GPU page staging, a compile-tested multi-segment `ggml_segmented_kqv` graph
bridge, f32/f16/bf16 Metal `kernel_qatq_segmented_kqv_*` source targets, and a
retained tiled page-table backend path. The latest real Qwen2.5 1.5B Metal
smoke matched the full-GPU 8-token baseline
`[264, 943, 315, 56287, 429, 5711, 279, 56287]`, consumed 784 page-segment
records, reached 31.21 tok/s, and used a 4.10 MiB Metal compute buffer. It
still lacks broad real-model runtime equivalence, latency-tail, and memory
pressure evidence. The
evidence runner now preflights this structural audit whenever
`--require-native-page-streaming` is requested, so strict native runs can fail
before expensive GPU execution if the adapter source shape regresses.

The page-source trace contract is explicit. `page-composed-source` and
`persistent-page-source` events must include `composition` and
`native_page_streaming`. Older compatibility runs that report
`composition: "ggml_concat"` and `native_page_streaming: false` are rejected by
the native production gate, with failing sources recorded in
`native-page-streaming-status.json` and the `evidence-summary` row of
`tokens.csv`. The status now separates `segmented_graph_bridge`,
`backend_scheduled_segmented_attention`, and
`backend_scheduled_flattened_flash_attention`: the first proves the runtime
graph can consume segmented K/V without rebuilding the full K/V source, while
the latter two prove that the segmented or flattened reduction is implemented
as an accelerator-scheduled runtime attention path. Production live-VRAM
evidence also requires `accelerated_runtime_attention_graph: true` and
`page_bounded_attention_equivalence_passed: true`. A CPU custom op, graph-only
bridge, or compile-only backend proof can prove useful pieces, but fails the
production native gate until runtime equivalence and latency evidence are
covered.

The patched llama.cpp adapter also exposes a bounded page-segment trace from
the actual `get_k/get_v` attention read path. `attention-page-segments` rows
report `composition: "none"` and enumerate the K/V page tensors before
page-source composition. That trace is necessary but not sufficient by itself:
strict native evidence also requires a real native consumer such as
`backend_scheduled_segmented_attention` or
`backend_scheduled_flattened_flash_attention`.
The evidence runner now also validates segment structure as a native-kernel
contract: K/V rows must be paired by sequence, layer, `n_kv`, token ranges,
native-streaming status, attention-consumed status, and consumer. Unpaired or
mismatched K/V segment ranges fail closed before the run can contribute to
native live-VRAM evidence.

The multi-model matrix runner exposes the same switch:

```sh
python3 scripts/llama_cpp_live_vram_matrix.py \
  --config adapters/llama-cpp/live-vram-matrix.local.example.json \
  --llama-simple /path/to/patched/llama-simple \
  --work-dir /tmp/qatq-live-vram-matrix \
  --require-live-paging \
  --require-native-page-streaming \
  --native-page-streaming-attention-backend-op \
  --native-page-streaming-flatten-flash \
  --gpu-page-staging
```

Use `--require-native-page-streaming` when validating the production-shaped
native route. Compatibility matrices that omit the native flags may still be
useful for diagnostics, but concat-composed source paths must not be treated as
production-complete.

Add `--prune-bulk-artifacts` to broad local matrix runs when the aggregate
`summary.md`, `pages.csv`, `tokens.csv`, JSON reports, and logs are enough for
the audit trail. The per-case runner then removes bulky generated export
directories after each case writes its aggregate evidence, which keeps repeated
GPU stress tests from exhausting `/private/tmp`.
Use `max_queued_pages` in long-context matrix cases to tune the latency/reclaim
frontier. An unbounded `cold-after-hot` run is useful as a stress diagnostic,
but it can create a restore/staging cliff by offloading every old page needed
by active decode.
Use `--skip-attention-page-segments-trace` only for low-overhead latency
diagnostics where the native page-segment proof itself would contaminate token
timing. The strict native page-streaming gate rejects that flag because the
page-segment trace is the machine-checkable evidence that attention consumed
segmented pages instead of falling back to concat materialisation.

The pinned llama.cpp patch now also exposes a stricter executable contract via
`--qatq-native-page-streaming-contract`. The flag runs the compile-tested
`qatq_validate_segmented_kqv_contract` hook in `src/llama-graph.cpp`: it
collects K/V page segments at the future native consumer boundary, validates
segment pairing, query/key head dimensions, K/V dtype consistency, token
extents, and unsupported KQ-bias/sink/MLA feature combinations, and emits
`ggml_segmented_kqv_contract` trace rows for valid inputs. This is a
lower-level contract probe, not production evidence. The strict native gate now
uses the accelerator-schedulable `GGML_OP_QATQ_SEGMENTED_KQV` backend route via
`--qatq-native-page-streaming-attention-backend-op`.

The evidence runner can exercise that boundary without running a full evidence
bundle:

```sh
python3 scripts/llama_cpp_live_vram_evidence.py \
  --llama-simple /path/to/patched/llama-simple \
  --llama-cpp-source /path/to/patched/llama.cpp \
  --model /path/to/model.gguf \
  --work-dir /tmp/qatq-native-contract-probe \
  --native-page-streaming-contract-probe
```

This writes `native-page-streaming-contract-probe.json` and is useful for
debugging validation-boundary regressions. It is not a substitute for the
backend-op evidence route. The matrix runner accepts
`--native-page-streaming-contract-probe` for that narrow diagnostic, while
`--require-native-page-streaming` defaults to
`--native-page-streaming-attention-backend-op` for strict evidence runs.

The structural adapter audit makes the remaining runtime work explicit:

```sh
python3 scripts/llama_cpp_live_vram_adapter_audit.py \
  --llama-cpp /private/tmp/qatq-llama.cpp \
  --require-live-paging \
  --require-runtime-security \
  --output /private/tmp/qatq-live-vram-adapter-audit-20260624.json
```

For quick repo-local checks that avoid cloning/building llama.cpp, audit the
pinned patch directly:

```sh
python3 scripts/llama_cpp_live_vram_adapter_audit.py \
  --patch-file adapters/llama-cpp/qatq-kv-export-7992aa7c8.patch \
  --require-live-paging \
  --require-runtime-security \
  --output /private/tmp/qatq-live-vram-adapter-patch-audit.json
```

Patch-file mode reports `audit_scope: "patch-snippet"` and
`authoritative_for_native_release: false`. Use it as a cheap fail-closed smoke
check for the repository patch. Before production native claims, still run the
applied-source audit against a real patched llama.cpp checkout and rebuild.
`--require-live-paging` checks the production-shaped attention and allocator
surface. `--require-runtime-security` is a separate fail-closed gate for
lifecycle traceability, page-staging, backend page self-tests, restore-slot
rejection, physical per-page allocation attestation, and unsupported backend-op
rejection.

On the current patched llama.cpp checkout, the audit reports `export_ready:
true`, `page_staging_ready: true`, `live_paging_ready: true`,
`runtime_security_ready: true`, and empty required failure arrays when both
strict gates are enabled. Some diagnostic compatibility paths still expose
whole-cache views or concat-composed page sources, but the native backend-op
route now consumes a retained tiled K/V page table for the tested non-SWA Qwen
path. The adapter has page residency metadata, explicit `evict_page` and
`restore_page` operations, attention-path lifecycle events, page-staging
self-tests, restore-slot pressure checks, and a page-segment API. That is enough
to run page-staging and diagnostic native evidence, but not enough by itself to
claim production transparent live-VRAM reduction.

On 2026-06-24, the patched llama.cpp checkout was rebuilt at commit
`7992aa7c8e21ea2eb7a5e4802da56eec7b376036` and MLX was independently smoke
tested on `Device(gpu, 0)`. The frontier runner then passed on two local coder
models:

| model | frontier sweep | selected | full-GPU decode | selected decode | CPU-KV decode | reclaimable GPU KV | QATQ vs zstd/lz4 | exact restores |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- | ---: |
| Qwen2.5 Coder 3B Q4_K_M | 18, 24, 30 | 30 / 36 layers | 1,049,015 us | 1,058,758 us | 1,440,476 us | 21.00 MiB | pass | 72 / 72 |
| Qwen2.5 Coder 7B Q4_K_M | 14, 21, 24 | 24 / 28 layers | 2,458,666 us | 2,056,568 us | 2,482,412 us | 28.00 MiB | pass | 56 / 56 |
| Qwen2.5 1.5B Q4_K_M daily-driver prompt | 14, 21, 24 | 24 / 28 layers | 666,373 us | 769,716 us | 1,719,015 us | 11.00 MiB | pass | 56 / 56 |
| Phi 3.5 mini Q4_K_M operations/security prompt | 16, 24, 28 | 28 / 32 layers | 1,348,196 us | 1,403,393 us | 2,078,105 us | 192.00 MiB | pass | 64 / 64 |

The 1.5B daily-driver frontier rejected two more aggressive GPU-saving points
because their decode regressions exceeded the configured ceiling. The Phi
operations/security run selected the fastest passing point and stored 29/29
offloaded pages through QATQ with zero pass-through pages while leaving 35
scheduler-resident pages in runtime memory.

Those prompt-profile cases are now reproducible as a single matrix:

```sh
python3 scripts/llama_cpp_live_vram_matrix.py \
  --config adapters/llama-cpp/live-vram-matrix.local.example.json \
  --llama-simple /private/tmp/qatq-llama.cpp/build/bin/llama-simple \
  --work-dir /private/tmp/qatq-live-vram-matrix-20260624 \
  --timeout 1200 \
  --iterations 2
```

The initial matrix passed 2/2 real Metal cases and wrote an aggregate summary at
`/private/tmp/qatq-live-vram-matrix-20260624/summary.md`. A repeated soak matrix
then passed 4/4 gated runs with `--iterations 2`, writing
`/private/tmp/qatq-live-vram-matrix-soak-20260624/summary.md`. It preserves the
per-case fail-closed gates and exits non-zero if any configured case or
iteration fails.

Soak stability:

| case | runs | failures | elapsed min/max | reclaimable GPU min/max | QATQ min/max |
| --- | ---: | ---: | ---: | ---: | ---: |
| Qwen2.5 1.5B daily-driver | 2 | 0 | 10.73 / 11.31 s | 11.00 / 11.00 MiB | 52.55 / 52.55 MiB |
| Phi 3.5 mini operations/security | 2 | 0 | 54.08 / 56.21 s | 192.00 / 192.00 MiB | 1,166.43 / 1,166.43 MiB |

The matrix was then expanded to four real local profiles, including 3B and 7B
coder workloads, and run against the patched Metal llama.cpp binary. The run
wrote `/private/tmp/qatq-live-vram-matrix-4case-20260624/summary.md` and passed
4/4 cases:

| case | selected | exact restores | offloaded QATQ pages | pass-through | reclaimable GPU KV | QATQ | zstd | lz4 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Qwen2.5 1.5B daily-driver | 24 / 28 layers | 56 / 56 | 56 / 56 | 0 | 11.00 MiB | 52.55 MiB | 63.48 MiB | 69.03 MiB |
| Phi 3.5 mini operations/security | 28 / 32 layers | 64 / 64 | 29 / 29 | 0 | 192.00 MiB | 1,166.43 MiB | 1,389.85 MiB | 1,533.57 MiB |
| Qwen2.5 Coder 3B workhorse | 30 / 36 layers | 72 / 72 | 72 / 72 | 0 | 19.50 MiB | 86.41 MiB | 104.89 MiB | 114.18 MiB |
| Qwen2.5 Coder 7B powerhouse | 24 / 28 layers | 56 / 56 | 56 / 56 | 0 | 32.00 MiB | 161.13 MiB | 190.68 MiB | 207.09 MiB |

This is the current strongest local proof for the adapter-level feature: real
Metal inference, deterministic continuations, strict latency gates, exact QATQ
restore, compression wins over zstd/lz4, and lower GPU KV allocation across
daily-driver, operations/security, and software-engineering prompt shapes. It
still proves whole-tensor/layer placement rather than transparent token-page
eviction inside the attention loop.

The four-profile matrix was rerun fresh at
`/private/tmp/qatq-live-vram-matrix-fresh-20260624` after adding the trace
verifier and patch refresh. It again passed 4/4. A separate Qwen2.5 1.5B Metal
smoke produced an export-time trace with 224 lifecycle events over 56 tensors;
QATQ accepted the trace, and `--live-vram-live-paging-gate` correctly rejected
it as insufficient for token-page live paging.

After the evidence runner was wired to emit event traces by default, a fresh
Qwen2.5 1.5B single-frontier run at
`/private/tmp/qatq-live-vram-runner-trace-20260624` passed the runtime-reclaim
gate with trace verification enabled. The same runner with
`--require-live-paging` at
`/private/tmp/qatq-live-vram-runner-strict-20260624` failed closed because
`whole-tensor` allocation granularity cannot prove page-level GPU reclaim.

The same patched runner now records a native llama.cpp memory control through
`--memory-breakdown` and `--no-kv-offload`. On the Phi 3.5 mini 3,072-token
control, default Metal KV allocated 1,152 MiB of Metal KV context. The
`--no-kv-offload` run moved that 1,152 MiB context to host memory and reduced
Metal self memory from 3,461 MiB to 2,328 MiB, with prompt eval moving from
8,082.30 ms to 8,597.08 ms for 3,072 tokens. This is the native CPU-KV baseline
the QATQ live adapter must beat or complement.

`qatq-kv-bench --live-vram-export-dir` can now include allocator-aware residency
estimates and restore-deadline estimates:

```sh
qatq-kv-bench \
  --live-vram-export-dir captures/llama-kv \
  --live-vram-runtime-commit 7992aa7c8 \
  --live-vram-adapter-version qatq-kv-export-7992aa7c8 \
  --live-vram-model-id <model-id> \
  --live-vram-gpu-context-bytes <runtime-kv-context-bytes> \
  --live-vram-allocation-granularity whole-context \
  --live-vram-restore-bytes-per-token <measured-restore-budget> \
  --output evidence.json
```

For current llama.cpp Metal KV captures, `whole-context` is intentionally
conservative: QATQ reports compression and exact restore, but reports zero
reclaimable GPU bytes because exported page slices do not release the runtime's
already allocated Metal KV buffer. A future adapter must report `per-page` only
after the allocator can really reclaim cold page memory.

The optional `--live-vram-restore-bytes-per-token` flag emits a
`restore_deadline_report` block. It estimates whether scheduled pages can be
decoded and copied back before their `next_required_token`. The value should be
derived from runtime-specific restore throughput, not guessed as a product
claim.

Use the strict proof gate when testing an adapter that claims real live VRAM
reduction:

```sh
qatq-kv-bench \
  --live-vram-export-dir captures/llama-kv \
  --live-vram-runtime-commit 7992aa7c8 \
  --live-vram-adapter-version qatq-kv-export-7992aa7c8 \
  --live-vram-model-id <model-id> \
  --live-vram-gpu-context-bytes <runtime-kv-context-bytes> \
  --live-vram-allocation-granularity per-page \
  --live-vram-restore-bytes-per-token <measured-restore-budget> \
  --live-vram-proof-gate \
  --live-vram-min-gpu-saved-ratio 0.10 \
  --output evidence.json
```

The proof gate intentionally fails current llama.cpp `whole-context` exports. A
passing report requires page-granular reclaim, non-zero reclaimable GPU bytes,
GPU saved ratio at or above the threshold, exact restore coverage for every
page, zero restore-deadline misses by default, and every offloaded page being
QATQ-compressed and better than the best general-codec baseline. Codec-negative
pages are kept resident in strict evidence rather than counted as VRAM-saving
offloads. llama.cpp mixed KV-layer placement can produce `whole-tensor`
allocator evidence and real lower GPU KV bytes, but that is still not enough
for this page-level proof gate.

For a true token-page live-paging claim, include an attention-path event trace
and use the combined live-paging gate:

```sh
qatq-kv-bench \
  --live-vram-export-dir captures/llama-kv \
  --live-vram-runtime-commit 7992aa7c8 \
  --live-vram-adapter-version qatq-kv-export-7992aa7c8 \
  --live-vram-model-id <model-id> \
  --live-vram-restore-bytes-per-token <measured-restore-budget> \
  --live-vram-event-trace trace.json \
  --live-vram-live-paging-gate \
  --live-vram-page-seal-key-hex <64-hex-secret> \
  --live-vram-min-gpu-saved-ratio 0.10 \
  --output evidence.json
```

The trace must be emitted by the runtime while generation is executing. The gate
fails if attention consumes an offloaded page before restore, if restore
checksums do not match, if token order goes backwards, or if the trace ends with
pages still offloaded.

For mixed KV-layer placement, use the runtime reclaim gate instead:

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

This gate requires runtime-attested `total_context_bytes`, `gpu_context_bytes`,
and `gpu_allocation_granularity`; non-zero reclaimed GPU bytes; a GPU saved
ratio at or above the threshold; exact restore coverage; restore-deadline
compliance; and every offloaded page being QATQ-compressed and better than the
best general-codec baseline. It allows `whole-tensor` evidence because that is
the current llama.cpp mixed-layer adapter granularity.

Use the typed attention-equivalence gate before claiming that an adapter's
attention path can operate over bounded KV pages:

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

This report records only dimensions, dtypes, page/token counts, max error, and
peak page KV working-set ratio. It intentionally omits query vectors, attention
outputs, and KV payloads. A passing report proves numerical equivalence between
page-streamed softmax attention and materialised softmax attention for the
provided page files, and now also records the page-summary reduction result
used as the QATQ-side oracle for native multi-segment attention; it does not
prove page-granular GPU reclaim by itself.
For llama.cpp captures, prefer the fixture gate:

```sh
python3 scripts/llama_cpp_attention_fixture_gate.py \
  --export-dir captures/llama-kv \
  --attention-fixture-dir captures/attention-fixture \
  --qatq-kv-bench target/release/qatq-kv-bench \
  --output attention-equivalence.json
```

The full evidence runner now invokes this gate by default unless
`--skip-attention-equivalence` is provided. The runner generates a fresh
per-run page-seal key for runtime-reclaim fallback and strict live-paging
evidence, requires `--live-vram-require-page-seals`, and does not write the
secret into the evidence bundle.

When `--require-native-page-streaming` is used, this equivalence report becomes
part of the native evidence gate rather than optional supporting material. The
native gate fails unless the runtime consumes bounded K/V segments through the
accelerator-schedulable `ggml_segmented_kqv` path, the segment trace pairs K/V
rows exactly, the MLX page-streaming reference passes, and the QATQ
page-bounded attention-equivalence report is present and passing.
The structural audit also requires both an actual backend-schedulable segmented
K/Q/V op surface for the online page-summary reduction and runtime graph
integration that calls it with bounded page tensors, because a CPU custom op,
graph-only bridge, or uncalled backend op cannot prove performant live VRAM
reduction.

In proof-gate mode, allocator evidence must come from the runtime manifest
itself. For llama.cpp exports this means `manifest.json` must include
`gpu_allocation_granularity` and `gpu_context_bytes`. Manual
`--live-vram-allocation-granularity` / `--live-vram-gpu-context-bytes` values
remain available for exploratory replay estimates, but they are ignored for
proof-grade gating.

For `--live-vram-live-paging-gate`, `manifest.json` must also include
`live_page_residency_granularity: "per-page"`. That field proves the runtime
has an explicit page residency table and page-level evict/restore primitive. It
does not replace the physical allocation check: a runtime that can logically
evict pages from a persistent whole-tensor GPU buffer still fails the live
paging gate until it also proves page-granular GPU allocation reclaim.

The physical page tensor self-test is deliberately not accepted as
`gpu_allocation_granularity: "per-page"` by itself. It proves that a page-sized
backend tensor can be allocated, populated exactly, read back exactly, and freed
on the accelerator; the remaining runtime work is to make attention consume
page tensors instead of whole-cache views and to attest sustained per-page GPU
residency in the manifest.

## Current Runtime Adapter Cut

The maintained llama.cpp adapter patch now includes
`llama-simple --qatq-kv-gpu-layers <n>`. The flag keeps the first `n` KV layers
on the accelerator and places the remaining KV tensors on host memory during
context construction. This is a real allocator-level reduction, not a replay
estimate: on the Apple M4 Phi 3.5 mini smoke run, `--qatq-kv-gpu-layers 16`
split the 96 MiB f16 KV cache into 48 MiB Metal KV and 48 MiB host KV while
generation completed and QATQ replay verified 64/64 exported tensors. The
patched runner can also emit `--qatq-output-manifest <path>` so paired
full-GPU and mixed-placement runs can prove generated-token identity. Run
`qatq-kv-bench --compare-output-baseline ... --compare-output-candidate ...
--compare-output-gate` to make that behaviour check fail closed in automation.

This is still coarse-grained. It reduces GPU KV allocation at layer granularity
from the start of the run; it does not yet evict cold token pages after prefill,
compress them in QATQ while resident on CPU, and restore them just before
attention needs them.

## Goal

Build a runtime integration that can move cold KV-cache pages out of GPU VRAM,
store them through QATQ/QATC or raw pass-through decisions, restore them before
attention needs them, and prove a net reduction in peak VRAM without unacceptable
generation latency, throughput loss, correctness drift, or operational risk.

The target outcome is a measured statement like:

> With runtime `R`, model `M`, context length `C`, dtype `D`, and policy `P`,
> QATQ live KV paging reduced peak GPU VRAM by `X%` versus the runtime native KV
> cache and by `Y%` versus native CPU offload, while keeping p95 per-token
> latency within `Z%`, preserving bit-identical restored KV pages, and preserving
> task/output behaviour across the benchmark suite.

Anything weaker remains research evidence, not a production claim.

## Non-Goals

- Do not replace the runtime's attention implementation.
- Do not claim transparent support for arbitrary runtimes.
- Do not compress active pages that are needed by the current attention window.
- Do not persist compression-negative QATQ envelopes when raw pass-through is
  the correct storage decision.
- Do not hide latency by excluding restore stalls from benchmark accounting.
- Do not evaluate only synthetic public fixtures; live runtime traffic is
  required before any release claim.

## System Boundary

Live VRAM reduction has four ownership boundaries:

| boundary | owner | responsibility |
| --- | --- | --- |
| QATQ codec | `qatq` crate | encode/decode typed tensor bytes, resource limits, checksums, pass-through decisions, QATC chunking |
| Runtime adapter | runtime-specific crate/patch/plugin | expose KV pages, page lifecycle hooks, dtype/layout metadata, restore/import APIs |
| Page scheduler | adapter or runtime extension | decide hot/cold pages, trigger offload/prefetch/evict/restore, enforce latency budgets |
| Benchmark harness | standalone reproducibility tooling | collect VRAM, latency, throughput, exactness, task quality, and baseline comparisons |

The runtime adapter should live outside the core QATQ crate unless it is a small
dependency-free example. QATQ should define contracts, fixtures, and test
expectations; runtime-specific implementation belongs with the runtime or an
adapter repository.

## Runtime Target Strategy

### First Target: llama.cpp Adapter

Start with llama.cpp because QATQ already has a version-pinned KV export patch
and direct KV ingestion evidence. Extend that work from export-only capture to
page-level offload/restore.

Required runtime properties:

- identifiable K/V cache tensors per layer and sequence;
- stable page/block ownership metadata;
- explicit points before attention where a page can be restored;
- instrumentation for GPU VRAM, CPU memory, transfer time, and token latency;
- ability to run comparable baselines with native KV cache, CPU offload, and
  available KV quantization modes.

### Second Target: vLLM or Another Paged KV Runtime

After llama.cpp proves the mechanics, evaluate a runtime with a native paged KV
cache. A paged runtime is likely to expose cleaner page lifecycle hooks and more
realistic pressure scenarios.

Do not begin a second runtime until the first target has:

- bit-identical page restore tests;
- measurable VRAM reduction;
- task/output preservation;
- latency accounting that includes restore stalls;
- a documented adapter contract that can be reused.

## Technical Design

### KV Page Model

A QATQ live page is a typed, bounded, restorable unit of KV cache state.

Minimum metadata:

| field | purpose |
| --- | --- |
| `runtime_id` | runtime and adapter identity |
| `runtime_commit` | exact runtime source/version |
| `adapter_version` | QATQ adapter contract version |
| `model_id` | model family/name/path digest where possible |
| `seq_id` | runtime sequence/request identifier |
| `layer_id` | transformer layer |
| `kv_kind` | key or value |
| `dtype` | f32, f16, bf16, or runtime-native representation |
| `shape` | logical tensor shape |
| `layout` | contiguous, transposed, blocked, paged, or runtime-specific layout |
| `token_range` | token span covered by the page |
| `raw_len` | decoded byte length |
| `checksum` | checksum of native page bytes before compression |
| `storage_label` | QATQ compressed label or raw pass-through label |
| `qatq_strategy` | selected QATQ strategy when compressed |
| `compressed_len` | stored payload byte length |
| `metadata_seal` | keyed seal over descriptor, storage decision, payload bytes, and caller context when crossing a runtime or process boundary |

Invariants:

- page metadata must be committed atomically with the payload;
- page metadata that crosses a runtime or process boundary must carry a keyed
  seal; the shared Rust restore helper routes sealed stores through
  `LiveVramRuntimeAdapter::restore_sealed_committed_page`, and adapters must
  reject pages whose seal does not verify before restore or GPU upload;
- restored page bytes must match the original checksum before the runtime uses
  the page;
- a peak-VRAM reduction claim must not require materialising the full logical
  KV set in GPU memory before attention; the runtime needs either page-bounded
  attention or equally strong allocator/attention evidence;
- the scheduler must never evict the only valid copy of a page until the
  offloaded payload is durably available and verified;
- compression-negative pages must stay resident in strict live VRAM proof
  reports rather than expanding into a larger QATQ envelope or being counted as
  compressed savings;
- adapter metadata must be sufficient to restore the page without relying on
  process-local pointers.

### Storage Tiers

The implementation should support these tiers in order:

1. **GPU resident**: hot pages used by current or imminent attention.
2. **CPU uncompressed**: baseline offload tier and fallback for pages that are
   not compression-positive.
3. **CPU QATQ compressed**: first QATQ live target; lower memory at the cost of
   decode latency.
4. **Disk/NVMe QATQ compressed**: later stress tier for very long context or
   low-priority sessions.
5. **Remote QATQ compressed**: out of scope for first implementation; relevant
   only after local paging is proven.

The first production-shaped experiment should compare GPU resident, runtime
native CPU offload, zstd CPU compressed, lz4 CPU compressed, and QATQ CPU
compressed tiers.

### Scheduler Policy

The first scheduler should be simple and measurable:

- never offload pages inside the active attention horizon;
- rank pages by distance from the current decode position and sequence activity;
- keep a minimum hot-resident window per layer;
- prefetch pages before they are needed using a fixed lookahead;
- bound concurrent encode/decode jobs;
- bound total compressed CPU memory;
- fall back to native CPU offload or resident pages when compression stalls.

Later policies can use token reuse, conversation branches, speculative decode,
or retrieval activity, but not before the simple policy has baseline evidence.

### Encode Path

1. Runtime marks a page as cold and evictable.
2. Adapter snapshots the page bytes into a stable CPU buffer.
3. Adapter computes checksum and metadata.
4. QATQ production decision chooses compressed QATQ payload or raw pass-through.
5. Adapter stores payload and metadata in the offload table.
6. Adapter verifies the stored payload can restore before freeing GPU memory in
   debug/validation mode.
7. Runtime frees or reuses the GPU page only after the offload entry is valid.

Failure handling:

- snapshot failure: page remains resident;
- encode failure: page remains resident and error counter increments;
- compression-negative decision: use raw CPU offload or keep resident depending
  on policy;
- metadata mismatch: reject offload and keep resident;
- memory pressure during encode: abort offload before freeing GPU page.

### Restore Path

1. Scheduler predicts a page will be needed.
2. Adapter reserves or locates a GPU page slot.
3. QATQ restores the payload into a CPU staging buffer.
4. Adapter verifies checksum before GPU upload.
5. Adapter uploads bytes to the runtime layout expected by attention.
6. Runtime marks page resident before attention reaches it.

Failure handling:

- checksum mismatch: abort generation or fall back to a known-good resident copy
  if one exists;
- decode resource-limit rejection: abort the page restore and mark the adapter
  unhealthy;
- GPU allocation failure: fall back to runtime native behaviour or fail the
  request explicitly;
- prefetch miss: record restore stall in latency metrics; never hide it from
  benchmark reports.

### Concurrency and Resource Bounds

Initial hard limits:

- maximum encode workers: `min(physical_cores / 2, 4)`;
- maximum decode workers: `min(physical_cores / 2, 4)`;
- maximum queued pages: bounded by configured CPU memory budget;
- maximum page decoded bytes: adapter-defined and checked before allocation;
- maximum QATC/QATQ payload bytes: adapter-defined and checked before decode;
- maximum prefetch lookahead: fixed token/page window, not unbounded by context.

Metrics must expose dropped/offload-skipped pages so a benchmark cannot report
VRAM savings by silently failing offloads.

## API and Contract Work

- [x] Define `KvPageDescriptor` with dtype, shape, layout, layer, sequence, and
      token-range metadata.
- [x] Define live page storage semantics using existing QATQ exact typed tensor
      payloads: compressed QATQ payload or typed raw pass-through.
- [x] Define `LiveVramRestoreStatus` with success, checksum failure,
      resource-limit rejection, missing payload, and metadata mismatch states.
- [x] Define bounded page encode/restore APIs for runtime adapters:
      `try_encode_live_vram_page` and `restore_live_vram_page`.
- [x] Define a conservative scheduler API with hot-window, queue, and CPU-memory
      budget controls.
- [x] Define a runtime adapter trait for snapshot, offload commit, restore, page
      residency query, and metrics export.
- [x] Define a scheduler trait with hot/cold decision support and a fixed-window
      implementation backed by hot-window, queue, and CPU-budget policy.
- [x] Define a bounded offload commit store that verifies pages before a runtime
      is allowed to free the GPU resident copy.
- [x] Define controller helpers that enforce offload/restore operation ordering
      across the adapter, scheduler, and offload store.
- [x] Define a portable JSON evidence summary for benchmark evidence bundles.
- [x] Define versioned adapter compatibility fields so old benchmark artefacts
      cannot be mistaken for current runtime proof.
- [x] Document which API names are experimental and keep them out of stable
      product claims until the feature graduates. The crate exposes
      `LIVE_VRAM_API_STATUS = "experimental"` and docs keep runtime VRAM
      reduction out of production-ready claims.

## Implementation Phases

### Phase 0 - Design Lock

- [x] Choose first runtime target and exact runtime commit: patched llama.cpp
      export replay, target commit `7992aa7c8`.
- [x] Record baseline runtime build flags, GPU type, driver/runtime versions, and
      model files.
- [x] Decide page size and chunking policy for first llama.cpp export
      experiment. Initial evidence uses 1024-token pages after a 512-token run
      missed the all-pages compression gate by one tail page.
- [x] Add a repeatable llama.cpp page-size sweep runner for real GPU evidence:
      `scripts/llama_cpp_live_vram_page_size_sweep.py`.
- [ ] Decide minimum acceptable VRAM reduction and maximum acceptable latency
      regression.
- [ ] Define benchmark prompt suite and task suite before running experiments.
- [x] Add an explicit experimental status marker. Runtime integrations should
      surface `LIVE_VRAM_API_STATUS` and stay opt-in until release gates pass.

Exit criteria:

- signed-off adapter contract;
- reproducible environment document;
- no public claim beyond experiment status.

### Phase 1 - Offline Page Simulator

- [x] Build a simulator that consumes exported KV captures and replays page
      offload/restore decisions without runtime integration.
- [x] Validate page descriptors, storage decisions, checksums, and restore
      ordering.
- [x] Compare QATQ, raw CPU offload, zstd, and lz4 per page.
- [x] Add a llama.cpp export-manifest replay path for patched exporter
      directories through `qatq-kv-bench --live-vram-export-dir`.
- [x] Inject restore deadlines and measure simulated prefetch misses with
      `evaluate_live_vram_prefetch_deadlines`.
- [x] Generate JSON evidence summaries with contract version, runtime metadata,
      page metadata, per-page baseline sizes, and verified restore status.
- [x] Add a reproducible fail-closed Metal evidence runner for current
      whole-tensor/layer allocator evidence:
      `scripts/llama_cpp_live_vram_evidence.py`.

Exit criteria:

- simulator proves byte-identical restore for every page;
- simulator reports compression-positive and pass-through pages separately;
- simulator catches intentionally corrupted, missing, duplicated, and reordered
  pages.

### Phase 2 - Runtime Snapshot and Restore

- [x] Add runtime adapter hooks to snapshot page bytes.
- [x] Add llama.cpp export adapter support for bounded token-range page files
      with `--qatq-page-tokens`; this snapshots exported token pages after
      generation, not live allocator pages during attention.
- [x] Add an experimental llama.cpp backend page self-test with
      `--qatq-live-page-self-test`; this now routes through the adapter-visible
      page residency primitive and proves real runtime key-page
      snapshot/evict/restore mechanics without claiming per-page GPU reclaim.
- [x] Add a retained non-host backend page-pool self-test with
      `--qatq-live-persistent-page-pool-self-test` and
      `--qatq-live-persistent-page-pool-trace`; this proves independent
      page-sized K/V tensors can persist on the runtime backend, but it still
      does not replace llama.cpp's whole-layer KV allocation.
- [x] Add QATQ-side offload commit and restore primitives for a page without
      freeing live GPU memory yet.
- [x] Round-trip active pages through CPU buffers and compare checksum in the
      QATQ offload store.
- [x] Add QATQ-side controller helpers that keep hot pages resident, commit
      offloads in order, and restore through the runtime adapter before store
      cleanup.
- [x] Add QATQ-side debug-mode page verification after restore with optional
      shadow copies.
- [x] Keep feature disabled for normal generation. QATQ exposes only explicit
      library calls and `qatq-kv-bench --live-vram-export-dir`; no normal encode,
      decode, or generation path enables live VRAM reduction implicitly.

Exit criteria:

- page round trips work inside the runtime process;
- no change to generation output when verification mode is enabled;
- adapter rejects mismatched dtype, shape, layout, layer, or token range.

### Phase 3 - Cold Page Offload

- [x] Add QATQ-side page-bounded attention references and
      `compare_live_vram_streaming_attention_reference`, with tests for
      materialised equivalence, page-split invariance, large-score numerical
      stability, invalid-page rejection, and lower peak KV working set.
- [x] Add typed f32/f16/bf16 little-endian page decoding and
      `compare_live_vram_typed_streaming_attention_reference`, with tests for
      native f16/bf16 runtime page bytes and malformed/non-finite page
      rejection.
- [x] Expose the typed page-bounded attention equivalence check through
      `qatq-kv-bench --attention-*` so runtime adapters can run a privacy-safe
      pass/fail gate over exported query, key-page, and value-page files before
      native paged attention is claimed.
- [x] Add the page-summary segmented attention reducer
      `compare_live_vram_segment_summary_attention_reference`, expose it in the
      `qatq-kv-bench --attention-*` report, and fuzz it against the
      materialised and token-streaming references as the oracle for native
      multi-segment attention.
- [x] Add a llama.cpp attention-fixture exporter plus
      `scripts/llama_cpp_attention_fixture_gate.py`, and verify page-bounded
      attention equivalence over real Apple Metal Q/K/V artefacts.
- [x] Enable offload only for pages outside the configured hot plus prefetch
      windows through `schedule_live_vram_page` and
      `FixedWindowLiveVramScheduler`; the llama.cpp trace adapter mirrors this
      with `--qatq-trace-prefetch-window-tokens`.
- [x] Keep a shadow resident copy in QATQ validation mode.
- [x] Provide a QATQ-side commit primitive that lets adapters free GPU memory
      only after offload commit succeeds.
- [x] Provide a QATQ-side controller that orders schedule, snapshot, verified
      store commit, runtime offload commit, runtime restore, and store cleanup.
- [ ] Restore pages before attention needs them.
- [ ] Replace whole-cache attention consumption in a runtime adapter with a
      page-bounded attention path or an equivalent native paged-attention path
      that does not materialise all logical KV pages in GPU memory at once.
- [x] Add a QATQ-side event-trace verifier that rejects attention use of an
      offloaded page before a matching restore event. Runtime adapters still
      need to emit this trace from the actual attention loop.
- [x] Add `qatq-kv-bench --live-vram-event-trace`,
      `--live-vram-live-paging-gate`, and
      `--live-vram-require-page-seals` so release tooling can enforce
      restore-before-attention evidence from runtime traces and keyed metadata
      seals on every offloaded page.
- [x] Make the strict live-paging gate reject event traces that offload pages
      the scheduler kept resident.
- [x] Add scheduler-aligned export trace flags to the patched llama.cpp runner
      so export-time traces can match QATQ's resident/offloaded page decisions.
- [x] Provide QATQ-side restore-stall accounting from observed runtime restore
      latency.
- [ ] Record every restore stall in runtime per-token latency metrics.

Exit criteria:

- measured peak VRAM decreases on at least one long-context workload;
- output/task behaviour matches baseline within the selected deterministic or
  task-level acceptance policy;
- latency report includes restore stalls and queue delays.

### Phase 4 - Baseline Competition

- [ ] Compare against native runtime KV cache.
- [ ] Compare against native CPU offload.
- [ ] Compare against runtime-native KV quantization where available.
- [ ] Compare against zstd and lz4 with the same page boundaries.
- [ ] Compare against raw CPU page offload with no compression.
- [x] Compare multiple QATQ page sizes for the first Qwen2.5 1.5B Metal
      export profile: 512 failed, 1024 passed, and 2048 passed.
- [x] Run an initial hot-window replay over a real Qwen2.5 1.5B Metal token-page
      export: 1024 hot-window tokens kept 56 pages resident and offloaded 168
      through QATQ.
- [ ] Compare hot-window sizes under a runtime adapter with actual page eviction
      and prefetch.

Exit criteria:

- QATQ beats at least one simpler offload baseline on peak VRAM while staying
  inside latency bounds;
- QATQ is not declared successful if raw CPU offload or native runtime paging
  provides a better memory/latency trade-off.

### Phase 5 - Production Hardening Candidate

- [ ] Remove validation-only shadow copies and remeasure.
- [x] Add QATQ-side corruption, resource-limit, and cancellation handling.
      Runtime timeout/OOM hooks still need adapter-specific implementation.
- [ ] Add sustained multi-request tests. QATQ-side sealed multi-sequence store
      interleaving and runtime-adapter controller interleaving now have
      deterministic stress coverage; real llama.cpp multi-request residency
      still needs longer adapter-level runs.
- [x] Add QATQ-side operator metrics suitable for adapter export.
- [ ] Define safe defaults and documented unsafe knobs.
- [ ] Run release-scale fuzzing and stress tests. Scheduled fuzzing now includes
      the QATQ-side live page, live lifecycle/event-sequence, and segmented
      attention-equivalence surfaces; release-scale duration and runtime stress
      still remain.

Exit criteria:

- feature remains experimental but can be tested by external users with clear
  safety warnings;
- all known correctness and resource-limit gates pass.

## Testing Plan

### Unit Tests

- [x] Page descriptor validates dtype, shape, layer, sequence, and token
      range invariants.
- [x] Storage decision preserves raw length, storage label, and strategy metadata.
- [x] Restore rejects dtype and checksum mismatches.
- [x] Scheduler never evicts pages inside the hot window.
- [x] Scheduler enforces queue and memory budgets.
- [x] Scheduler records skipped pages for unknown-use, hot-window, queue, and
      CPU-budget decisions.
- [x] Pass-through pages are treated as successful storage decisions.
- [x] Evidence report verifies QATQ restore for every page candidate before
      recording page-level baseline comparisons.
- [x] Adapter contract can snapshot, commit, restore, query residency, and export
      metrics through a fake runtime test adapter.
- [x] Offload store rejects duplicate pages, CPU budget exhaustion, and corrupt
      payloads before commit.
- [x] Offload store restores and removes committed pages while updating pending
      page and CPU-byte metrics.
- [x] Restore rejects missing metadata at adapter/manifest level.
- [x] Restore rejects unknown storage labels at adapter/manifest level.
- [x] Controller records restore stalls with observed runtime timing data.
- [x] Event-trace proof rejects attention-before-restore, non-monotonic token
      order, unfinished offloads, and restore checksum tampering.
- [x] CLI live-paging gate accepts a valid event trace and rejects
      attention-before-restore without writing an output evidence file.
- [x] Shared Rust runtime restore helper uses the sealed adapter boundary for
      sealed stores. `try_restore_live_vram_page_from_store` now routes
      `LiveVramOffloadStore::with_page_seal_policy` entries through
      `LiveVramRuntimeAdapter::restore_sealed_committed_page`, and focused
      unit tests prove sealed stores do not fall back to the legacy raw
      metadata/bytes restore method.

### Integration Tests

- [x] Offline simulator round-trips exported KV page bundles.
- [ ] Runtime adapter snapshots pages without mutating generation state.
- [ ] Runtime adapter restores pages into a scratch slot and verifies bytes.
- [ ] Runtime adapter offloads cold pages while keeping validation shadow copies.
- [ ] Runtime adapter restores pages before attention consumes them.
- [x] Runtime adapter emits attention-loop page events from the actual llama.cpp
      `get_k/get_v` path and passes `evaluate_live_vram_event_trace` through
      `qatq-kv-bench --live-vram-event-trace-only`.
- [ ] Runtime adapter emits attention-loop page events that pass the combined
      live-paging proof gate with allocator-backed per-page reclaim.
- [x] Runtime adapter emits an export-time `qatq-live-vram-event-trace-v1` file
      that passes QATQ event-trace verification against real llama.cpp tensor
      metadata.
- [x] Runtime adapter emits actual llama.cpp attention-path key/value read
      telemetry from `llama_kv_cache_context::get_k/get_v` and the evidence
      runner validates that trace in real Metal matrix runs.
- [x] Runtime adapter can make llama.cpp attention consume materialised K/V
      source tensors through an opt-in `ggml_cont` path, and a real Apple Metal
      smoke verifies generated-token preservation against the native path.
- [x] Runtime adapter can make llama.cpp attention consume K/V sources composed
      from bounded materialised token pages through `ggml_concat`, and the
      integrated Apple Metal evidence runner verifies multi-page composition
      without generated-token drift.
- [x] Runtime adapter materialises actual llama.cpp attention-path K/V page
      bytes into non-host backend page tensors and the evidence runner validates
      the resulting JSONL against real Apple Metal execution.
- [x] Runtime adapter emits a real llama.cpp query fixture and the evidence
      runner validates QATQ page-bounded attention equivalence against sliced
      exported K/V pages.
- [x] Reproducible llama.cpp evidence runner includes actual attention-path
      lifecycle event-trace verification by default.
- [x] Reproducible llama.cpp evidence runner includes export-time trace
      verification by default.
- [x] Reproducible llama.cpp evidence runner exposes `--require-live-paging` as
      a fail-closed scoped gate for page-granular runtime reclaim.
- [x] Reproducible llama.cpp matrix runner exposes `--require-live-paging` and
      refuses to claim strict live-paging support when any case fails.
- [x] Reproducible llama.cpp evidence and matrix runners expose
      `--require-native-page-streaming` as the future production gate for a
      non-concat native attention path.
- [x] Reproducible llama.cpp matrix runner exposes fail-closed repeated-run
      stability gates for reclaimable GPU bytes, codec byte totals, elapsed
      jitter, and bounded host-memory pressure.
- [x] Reproducible llama.cpp parallel stress runner shards matrix cases into
      concurrent one-case jobs, captures per-job logs, and fails closed when
      any real-runtime child matrix fails or exceeds the outer `--job-timeout`
      wall-clock bound. `--job-timeout 0` derives `--timeout + 120`, so the
      wrapper cannot hang indefinitely if a child process stalls before its own
      matrix timeout reports. This covers process-level patched runtime
      contention; in-process request cancellation remains separate. A
      bounded 2026-06-25 Qwen2.5 1.5B smoke passed 2/2, and the broader
      16-token breadth stress at
      `/private/tmp/qatq-live-vram-parallel-stress-breadth-3case-16tok-20260625`
      passed 3/3 concurrent Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini child
      matrices with strict live-paging, native page-streaming, aggregate codec,
      GPU page-staging, MLX equivalence, page seals, and pruned artifacts
      enabled.
- [x] Structural llama.cpp adapter audit distinguishes export readiness from
      page-staged evidence and production native page-streaming readiness.
- [x] Runtime adapter emits an attention-loop `qatq-live-vram-event-trace-v1`
      file that passes `qatq-kv-bench --live-vram-live-paging-gate` in the
      scoped page-staged evidence path.
- [x] Cancellation cleans up QATQ-side queued/offloaded work before runtime
      commit and restores committed pages before cleanup after runtime commit.
- [x] Failed offload leaves page resident on the QATQ side: if
      `commit_offload` fails, QATQ removes the pending CPU store entry and the
      adapter contract requires the runtime page to remain resident.
- [x] Failed restore fails closed and does not feed corrupt KV into attention on
      the QATQ side: restore helpers keep the CPU offload entry unless the
      runtime restore succeeds and `is_page_resident` confirms the page is
      resident again. Runtime restore errors, resource-limit rejections, and
      residency proof failures now increment restore-failure metrics.
      Runtime-specific attention-path tests still remain.

### Fuzz Tests

- [x] Fuzz page metadata parser.
- [x] Fuzz page manifest parser. `fuzz/fuzz_targets/live_vram_manifest.rs`
      covers raw and synthesised llama.cpp KV manifests, including duplicate
      tensor names/files, mismatched tensor counts, unsafe file paths, zero-size
      contexts, dtype/row-byte invariants, explicit token ranges, out-of-range
      streams, overlapping pages, and cross-layer/kind/stream/range file-name
      mismatches.
- [x] Fuzz QATQ/QATC payload restore through adapter-level limits.
- [x] Fuzz reordered, duplicated, truncated, and cross-layer page manifests.
      The parser now accepts reordered non-overlapping ranges but rejects
      duplicate or overlapping logical pages and file/name mismatches; a local
      `cargo +nightly fuzz run live_vram_manifest -- -runs=4096` smoke passed
      after tightening the manifest contract.
- [x] Fuzz scheduler event sequences: offload, cancel, restore, evict, reuse.
      `fuzz/fuzz_targets/live_vram_lifecycle.rs` exercises reordered and
      duplicate lifecycle operations against the public runtime adapter
      contract.
- [x] Fuzz segmented attention equivalence. `fuzz/fuzz_targets/live_vram_attention.rs`
      generates bounded f32/f16/bf16 Q/K/V pages, arbitrary non-empty token
      splits, and checks page-bounded streaming attention against the
      materialised reference. A local `cargo +nightly fuzz run
      live_vram_attention -- -runs=4096` smoke passed after adding the target.
- [x] Fuzz mixed compressed/pass-through page bundles.
- [x] Fuzz resource-limit boundaries for page count, byte length, and token
      ranges.

### Stress Tests

- [x] Thousands of synthetic KV page layouts across layers, heads, dimensions,
      dtypes, and token spans.
- [ ] Long-context prompts from 4k, 8k, 16k, 32k, and runtime maximum supported
      context. Current evidence covers 3,072, 3,521, 3,600, and 4,224-token
      Metal-backed llama.cpp exports, but not the full required matrix.
- [x] Dtype breadth across the current local model families. Qwen2.5 1.5B,
      Qwen2.5 3B, Qwen2.5 Coder 3B/7B, and Phi 3.5 mini now have strict
      native Apple Metal/MLX passes covering f16 plus bf16/f32 K/V caches. The
      full local bf16/f32 matrix passed a 2x stability-gated soak under 1 GiB
      host-memory pressure, and the fresh 2026-06-25 dtype matrix passed 8/8
      cases with cold-slot reuse plus the minimum-tail guard. Remaining dtype
      production work is longer prompt lengths, longer-duration burn-in, more
      page sizes, more runtimes, and harsher memory pressure.
- [x] Native page-size breadth across at least three page sizes for the current
      local Qwen/Qwen-Coder/Phi proof set. The fresh bootstrapped strict native
      matrix at `/private/tmp/qatq-live-vram-page-size-bootstrap-20260625`
      passed 5/5 real Metal/MLX cases across 256, 1024, and 2048-token pages
      from the clean pinned llama.cpp checkout, under 1 GiB host-memory
      pressure, with exact restores, zero pass-through pages, aggregate
      QATQ > zstd/lz4/raw gates, native flattened Flash page streaming, and
      restore-slot pressure checks. The matching Phi matrix at
      `/private/tmp/qatq-live-vram-phi-page-size-bootstrap-20260625` passed
      3/3 real Metal/MLX Phi 3.5 mini cases across 256, 512, and 1024-token
      pages under the same gate family. Remaining page-size work is longer
      contexts, longer-duration burn-in, and more runtime families.
- [x] QATQ-side sealed multi-sequence store interleaving across thousands of
      pages, forged neighbouring keys, restores, and cancellation paths.
- [x] QATQ-side runtime-adapter controller concurrency with independent page
      residency. The ignored stress test
      `cargo test --locked --test kv_stress live_vram_runtime_adapter_concurrency_stress -- --ignored --nocapture`
      covered 2,048 pages over 8 workers, with 1,638 offloads, 819 restores,
      410 before-commit cancellations, 819 after-commit cancellations, 409
      duplicate-offload rejections, 2,863 forged-restore rejections, and 409
      recorded restore stalls while preserving the resident-or-pending
      invariant for every page.
- [x] QATQ-side shared-runtime restore/cancel race stress. The ignored stress
      test
      `cargo test --locked --test kv_stress live_vram_runtime_restore_cancel_race_stress -- --ignored --nocapture`
      covered 1,024 committed pages where restore, after-commit cancellation,
      and a forged restore attempt raced from separate worker threads. It
      observed both restore and cancellation winners on rerun, with 885
      restore winners and 139 cancellation winners, rejected all 1,024 forged
      restore attempts, rejected all 139 losing restore paths, rejected all 885
      losing cancellation paths, and left every page resident with the CPU
      offload store drained to zero pending pages.
- [ ] Real runtime multi-sequence concurrency with independent page residency.
- [x] Process-abort fail-closed evidence for the currently wired patched
      `llama-simple` adapter path. The reusable probe
      `scripts/llama_cpp_live_vram_abort_probe.py` waits for a real QATQ KV
      export marker, interrupts generation, and fails if normal completion
      artifacts appear after abort.
      `/private/tmp/qatq-live-vram-abort-probe-qwen15b-20260625` passed on
      Qwen2.5 1.5B with SIGINT return code `-2`, 57 exported files, a
      49,921-byte event trace, a 218,674-byte page-segment trace, and no output
      manifest or token timings after abort.
- [x] Scoped in-process patched `llama-server` request-cancellation proof. The
      reusable probe `scripts/llama_cpp_live_vram_server_cancel_probe.py` starts
      `llama-server` with QATQ GPU page staging enabled through environment
      variables, opens a streaming `/completion`, closes the client connection
      mid-stream, verifies `/health`, and sends a follow-up completion to the
      same server. The Qwen2.5 1.5B conservative page-size run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-releasepages-20260625`
      passed with cancellation after 256 streamed bytes, healthy follow-up
      serving, 896 page-segment events, 128 attention-consumed events, and 128
      live-offloaded segments.
- [x] Smaller-page server cancellation budget hardening. The server probe now
      derives page-segment and graph-node budgets from the configured
      `ctx_size / page_tokens` policy instead of inheriting the conservative
      1024-token-page budget. The previous 64-token page run failed closed at
      `QATQ persistent attention segment count exceeds safe graph object
      budget`; the budgeted rerun at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-budgeted-20260625`
      passed with cancellation after 256 streamed bytes, healthy follow-up
      serving, 952 page-segment events, 136 attention-consumed events, and 584
      live-offloaded segments.
- [x] Scoped multi-request server cancellation proof for the unified-KV path.
      The two-slot run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-concurrent-kvu-20260625`
      started a follow-up completion before stream cancellation, cancelled at
      monotonic timestamp `1071535.400833875`, completed the follow-up at
      `1071538.179702583`, recovered health, returned 2,560 follow-up bytes,
      and recorded 1,120 page-segment events with 1,312 live-offloaded
      segments.
- [x] Scoped non-unified two-slot server cancellation proof. The retained tiled
      page-pool path now returns no QATQ live segments for unsupported
      multi-stream reserve shapes instead of aborting, while supported
      one-stream slot work can still use live page staging. The run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-concurrent-nonunified-20260625`
      started a follow-up completion before stream cancellation, cancelled at
      `1071885.968636041`, completed the follow-up at `1071888.515890166`,
      recovered health, returned 2,550 follow-up bytes, and recorded 1,120
      page-segment events with 544 live-offloaded segments.
- [x] Bounded repeated shared-server cancellation soak. The reusable server
      probe now accepts `--iterations <n>` and fails if any repeated
      cancellation/follow-up cycle misses stream bytes, health recovery,
      follow-up completion, or QATQ page-segment/live-offload traces. The
      non-unified Qwen2.5 1.5B run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-soak20-20260625`
      passed 20/20 iterations in one `llama-server` process with 5,120 total
      streamed bytes before cancellation, 10,696 page-segment events, 1,288
      attention-consumed events, and 10,728 live-offloaded segments. The matching
      unified-KV run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-kvu-soak20-20260625`
      passed 20/20 iterations with 5,120 total streamed bytes before
      cancellation, 9,632 page-segment events, 1,376 attention-consumed events,
      and 24,568 live-offloaded segments.
- [x] Bounded host-pressure shared-server cancellation soak. The server probe
      can now allocate touched host-memory pressure and fail if server RSS
      grows beyond `--max-server-rss-growth-mib` between post-readiness and
      final sampling. The 1 GiB host-pressure non-unified run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-pressure1g-soak20-20260625`
      passed 20/20 iterations with 10,728 live-offloaded segments and
      59.3 MiB RSS growth. The matching unified-KV run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-kvu-pressure1g-soak20-20260625`
      passed 20/20 iterations with 24,568 live-offloaded segments and
      44.8 MiB RSS growth.
- [x] Bounded shared-server cancellation latency gates. The server probe now
      records per-iteration duration and follow-up completion duration, and can
      fail with `--max-iteration-seconds` or `--max-followup-seconds`. The
      latency-gated 1 GiB pressure non-unified run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-pressure1g-latency-soak100-20260625`
      passed 100/100 iterations with 25,600 total streamed bytes before
      cancellation, 53,608 live-offloaded segments, 61.25 MiB RSS growth, p95
      iteration latency 4.80s, p99 iteration latency 4.84s, p95 follow-up
      latency 2.61s, and p99 follow-up latency 2.64s. The matching unified-KV
      run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-kvu-pressure1g-latency-soak100-20260625`
      passed 100/100 iterations with 122,488 live-offloaded segments, 44.8 MiB
      RSS growth, p95 iteration latency 4.83s, p99 iteration latency 4.85s,
      p95 follow-up latency 2.68s, and p99 follow-up latency 2.70s.
- [x] Scoped true native multi-stream retained page tables for the two-stream
      non-unified flattened Flash Attention path. The retained page-table
      planner now stores per-stream page slots, validates contiguity per
      `stream_index`, preserves stream-local masks, and traces explicit
      stream indices. The Qwen2.5 1.5B 20-cycle server cancellation run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-multistream-pressure1g-soak20-20260625`
      passed under 1 GiB host pressure. The Qwen2.5 3B follow-up at
      `/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-native-multistream-pressure1g-soak20-ctx8192-20260625`
      also passed 20/20 under 1 GiB host pressure with the same stream-split
      flattened Flash Attention route.
- [x] Longer scoped native multi-stream server burn-in. The Qwen2.5 1.5B
      100-cycle run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-multistream-pressure1g-soak100-ctx8192-20260625`
      passed with 168,968 live-offloaded segments, p95/p99 iteration latency
      6.56s/6.58s, p95/p99 follow-up latency 2.64s/2.65s, and 108.19 MiB RSS
      growth under the corrected per-iteration RSS peak gate.
- [x] Harsher native multi-stream server pressure. The Qwen2.5 1.5B 2 GiB
      host-pressure run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-multistream-pressure2g-soak50-ctx8192-20260625`
      passed 50/50 with 84,568 live-offloaded segments, p95/p99 iteration
      latency 6.60s/6.61s, p95/p99 follow-up latency 2.66s/2.66s, and
      106.48 MiB RSS growth under the corrected per-iteration RSS peak gate.
- [x] Add a second-model native multi-stream long soak. The Qwen2.5 3B run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-native-multistream-pressure1g-soak50-ctx8192-20260625`
      passed 50/50 with p95/p99 iteration latency 11.22s/12.19s, p95/p99
      follow-up latency 4.36s/4.91s, 84,568 live-offloaded segments, and
      114.61 MiB RSS growth under the corrected per-iteration RSS peak gate.
- [x] Add second-model harsher-pressure native multi-stream server coverage.
      The Qwen2.5 3B 2 GiB host-pressure run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-native-multistream-pressure2g-soak50-ctx8192-20260625`
      passed 50/50 with p95/p99 iteration latency 11.03s/12.17s, p95/p99
      follow-up latency 4.25s/5.48s, 84,568 live-offloaded segments, and
      115.25 MiB RSS growth under the corrected per-iteration RSS peak gate.
- [x] Add Phi 3.5 mini native multi-stream long-soak coverage. The Phi 3.5
      mini run at
      `/private/tmp/qatq-live-vram-server-cancel-phi35mini-p64-nonunified-native-multistream-pressure1g-soak50-ctx8192-20260625`
      passed 50/50 with p95/p99 iteration latency 16.40s/18.13s, p95/p99
      follow-up latency 5.05s/6.71s, 84,168 live-offloaded segments, and
      832.47 MiB RSS growth under the corrected per-iteration RSS peak gate.
- [x] Add focused Phi page-size variation for in-process native server
      cancellation. The 128-token page run at
      `/private/tmp/qatq-live-vram-server-cancel-phi35mini-p128-nonunified-native-multistream-pressure1g-soak20-ctx8192-20260625`
      passed 20/20 with p95/p99 iteration latency 16.16s/16.16s, p95/p99
      follow-up latency 5.00s/5.04s, 16,648 live-offloaded segments, and
      822.64 MiB RSS growth. Its consumed rows carried shape `[96,32,128,1]`.
- [ ] Broader native multi-stream retained page-table pressure variation and
      model/runtime burn-in. The current proof now covers Qwen2.5 1.5B,
      Qwen2.5 3B, and Phi 3.5 mini two-stream, 64-token-page, flattened Flash
      Attention runs, with long-soak coverage for all three model families and
      Qwen2.5 1.5B/Qwen2.5 3B 2 GiB pressure passes plus one focused Phi
      128-token page-size server pass, but still needs additional pressure
      levels and more page/context/runtime variation.
- [x] Add scoped mixed-prompt shared-server coverage. The Qwen2.5 1.5B
      mixed-prompt matrix at
      `/private/tmp/qatq-live-vram-server-mixed-prompts-current-20260625`
      passed daily-driver, software-engineering, and retrieval/incident prompt
      classes for 3/3 iterations each with strict flattened Flash, two-stream
      live-offload, backend-memory diagnostics, backend-memory ceilings, and
      empty `gate_failures`.
- [x] Add scoped mixed-model prompt shared-server coverage. The mixed-model
      prompt matrix at
      `/private/tmp/qatq-live-vram-server-mixed-model-prompts-current-20260625`
      passed Qwen2.5 1.5B daily-driver, Qwen2.5 3B software-engineering, and
      Phi 3.5 mini operations-incident prompt classes for 3/3 iterations each
      with strict flattened Flash, two-stream live-offload, backend-memory
      diagnostics, backend-memory ceilings, and empty `gate_failures`.
- [x] Add a longer scoped mixed-model prompt soak. The mixed-model soak matrix
      at
      `/private/tmp/qatq-live-vram-server-mixed-model-soak-current-20260625`
      passed the same Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini prompt
      classes for 10/10 iterations each with strict flattened Flash,
      two-stream live-offload, backend-memory diagnostics, backend-memory
      ceilings, and empty `gate_failures`.
- [x] QATQ-side sealed CPU compressed-tier budget pressure. The ignored stress
      test
      `cargo test --locked --test kv_stress live_vram_sealed_cpu_tier_budget_pressure_stress -- --ignored --nocapture`
      attempted 2,048 pages, accepted 1,046 pages up to a 2,638,637-byte CPU
      tier budget, rejected 1,002 pages fail-closed without changing store
      metrics, restored all accepted pages, and drained the sealed shadow store
      to zero. Runtime memory pressure and long burn-in still remain separate
      production gates.
- [ ] Runtime memory pressure with CPU compressed tier near configured budget.
      Bounded 512 MiB host-memory pressure passes for the current native
      breadth and sustained Coder matrices, but this is not yet a full
      runtime-level memory-pressure burn-in.
- [x] GPU pressure with bounded restore slot allocation failure. QATQ-side
      controller tests cover runtime restore allocation failure and
      resource-limit rejection semantics, and the real Metal/MLX proof at
      `/private/tmp/qatq-live-vram-restore-slot-pressure-20260624-r2` rejected
      a 262,144-byte `MTL0` key page against a one-byte restore-slot limit
      before allocation. This does not replace unbounded OOM-adjacent adverse
      testing.
- [x] Corruption injection at 0.01%, 0.1%, and 1% page payload rates. The
      ignored deterministic stress test
      `cargo test --locked --test kv_stress live_vram_corruption_injection_rates_fail_closed -- --ignored --nocapture`
      covered 2,048 live-VRAM pages, 1,548 QATQ-compressed pages, 500 raw typed
      pass-through pages, and 62,954 injected stored-payload byte mutations.
      The first run exposed an accepted mutation in the QATQ exact header scale
      field for non-predictor strategies; the parser now rejects non-canonical
      scale metadata and the stress test passes.
- [x] Cross-request key isolation for the QATQ-side offload store. The ignored
      deterministic stress test
      `cargo test --locked --test kv_stress live_vram_cross_request_key_isolation_stress -- --ignored --nocapture`
      covered 2,048 committed live-VRAM pages and rejected 14,336 forged
      restore attempts across neighbouring runtime, model, sequence, layer,
      K/V kind, token-start, and token-end keys. Legitimate keys still restored
      exactly after the forged attempts.
- [ ] Scheduler thrash cases where attention repeatedly approaches cold pages.
- [ ] Sustained generation for at least 1 hour under mixed prompt lengths.
- [ ] Overnight soak with metrics export and no unbounded memory growth.

### Performance Tests

Collect:

- peak GPU VRAM;
- steady-state GPU VRAM;
- CPU memory;
- compressed CPU bytes;
- raw CPU bytes avoided;
- encode ns/byte and ns/value;
- decode ns/byte and ns/value;
- GPU upload time;
- prefetch lead time;
- restore stalls per 1k tokens;
- time-to-first-token;
- tokens/sec;
- p50/p95/p99 per-token latency;
- queue depth;
- offload skip count;
- pass-through count;
- checksum failure count.

Minimum benchmark matrix:

| axis | required values |
| --- | --- |
| model families | at least two: small daily-driver and coding/long-context class |
| context lengths | 4k, 8k, 16k, and one longer limit where runtime/model supports it |
| dtypes | f16 and bf16 where runtime supports both; f32 if available |
| page sizes | at least 3 sizes; current Qwen/Qwen-Coder native proof covers 256, 1024, and 2048-token pages, and current Phi native proof covers 256, 512, and 1024-token pages |
| hot windows | at least 3 window sizes |
| prompt classes | chat, summarisation, code generation, retrieval-heavy context |
| baselines | native, CPU offload, runtime KV quantization, raw page offload, zstd, lz4 |

## Validation Gates

### Correctness Gate

- [ ] Every restored page checksum matches the original page checksum.
- [ ] No corrupted page can reach attention in validation mode.
- [ ] Runtime event trace proves every offloaded page is restored before
      attention consumes it.
- [ ] Deterministic prompts match baseline token output where the runtime is
      deterministic.
- [ ] Non-deterministic/task prompts preserve task-level decisions against the
      baseline acceptance rubric.
- [ ] Adapter metadata is sufficient to reproduce a failed run.

### Memory Gate

- [ ] Peak GPU VRAM is lower than native runtime KV cache by the configured
      threshold.
- [ ] Peak GPU VRAM is lower than native CPU offload or another simpler baseline
      for at least one documented workload before any strong claim is made.
- [ ] CPU memory growth remains bounded by configured budget.
- [ ] Compressed tier reports actual resident bytes, not just payload bytes.

### Latency Gate

- [x] Focused p95 per-token latency regression gate for a real Metal/MLX
      Qwen2.5 1.5B run. The matrix runner now supports
      `--min-token-latency-samples`,
      `--max-mixed-token-p95-regression-ratio`, and
      `--max-mixed-token-p99-regression-ratio`; the 32-token latency-tail proof
      at `/private/tmp/qatq-live-vram-real-gpu-latency-tail-20260624` passed
      with mixed-KV p95 slightly faster than full-GPU p95.
- [x] Focused p99 per-token latency gate for the same 32-token real Metal/MLX
      Qwen2.5 1.5B proof. Broader p99 outlier attribution across long soaks
      remains open.
- [x] Sustained Coder p95/p99 latency matrix across Qwen2.5 Coder 3B/7B review
      and memory profiles. The real Metal/MLX run at
      `/private/tmp/qatq-live-vram-real-gpu-sustained-latency-32tok-20260624`
      passed 4/4 cases with 32 token samples, strict live-paging, native
      `ggml_segmented_kqv`, zero pass-through pages, MLX checks, and p95/p99
      regression gates.
- [x] p95/p99 latency gates across the current breadth, dtype, and page-size
      matrices. Fresh real Metal/MLX runs at
      `/private/tmp/qatq-live-vram-real-gpu-breadth-latency-32tok-20260624`,
      `/private/tmp/qatq-live-vram-real-gpu-dtype-latency-32tok-20260624`, and
      `/private/tmp/qatq-live-vram-real-gpu-page-size-latency-32tok-20260624`
      passed 16/16 cases with strict native live-paging and 32 token samples
      per case.
- [x] Repeated sustained-Coder p95/p99 latency soak for the current local
      Qwen2.5 Coder 3B/7B matrix. The two-iteration real Metal/MLX run at
      `/private/tmp/qatq-live-vram-real-gpu-sustained-latency-soak-2x-20260624`
      passed 8/8 cases with stable reclaim/codec bytes, a 35% elapsed-jitter
      gate, 512 MiB host-memory pressure, and the same p95/p99 thresholds.
- [x] Single-run deep p95/p99 latency gate for long-context fallback evidence.
      `scripts/llama_cpp_live_vram_evidence.py` now supports
      `--min-deep-token-latency-samples`,
      `--max-deep-mixed-token-p95-regression-ratio`, and
      `--max-deep-mixed-token-p99-regression-ratio`, writes
      `deep-latency-gate.json`, and fails closed when the runtime-reclaim
      fallback exceeds the configured generated-token latency budget.
- [ ] p95/p99 latency gates across longer contexts and long-duration latency
      soaks.
- [ ] p99 latency outliers are explained by restore stalls or eliminated in
      long-duration burn-in.
- [ ] Restore stalls are included in end-to-end token timing.
- [ ] Time-to-first-token regression is within threshold.
- [ ] Tokens/sec remains within threshold for long-context generation.

### Security and Robustness Gate

- [x] Malformed payloads are rejected before allocation above configured limits
      in QATQ restore paths.
- [x] Corrupt payloads fail closed in QATQ restore paths.
- [x] Cross-request page reuse is impossible inside QATQ keys without matching
      sequence metadata.
- [x] QATQ-side cancellation cannot drop a committed offload entry without first
      restoring it through the runtime adapter.
- [x] QATQ-side live-paging proof traces reject cancellation stage mismatches:
      before-runtime-commit cancellation after offload, after-runtime-commit
      cancellation without offload, and after-runtime-commit cancellation with
      missing or mismatched restored checksums all fail closed.
- [x] QATQ-side metrics do not leak prompt text or raw tensor contents by
      default.
- [x] QATQ-side metrics avoid high-cardinality request IDs by default.

### Reproducibility Gate

- [x] Benchmark bundle records runtime commit, QATQ commit, adapter commit,
      compiler, GPU, driver, model identifier, prompt suite, dtype, page size,
      hot window, and policy.
- [ ] Scripts can replay the benchmark from a clean checkout.
- [ ] Reports include raw data and summarised tables.
- [x] Reports distinguish compressed pages from pass-through pages.
- [x] Reports include failed/skipped offload counts.
- [x] Reports include the live page event trace or a privacy-safe hashable trace
      summary for attention-safety audit.
- [x] Evidence runner writes `pages.csv` with per-page compression, schedule,
      and restore-verification rows.
- [x] Evidence runner writes `tokens.csv` with run-level proof-summary rows and
      per-`llama_decode` timing rows from patched llama.cpp. Full p50/p95/p99
      latency policy and restore-stall attribution remain open under the
      latency gate.

## Evidence Bundle Format

Each benchmark run should emit:

- `manifest.json`: runtime, model, hardware, commits, policy, prompts.
- `pages.csv`: one row per page with raw bytes, stored bytes, storage label,
  strategy, schedule, general-codec baselines, and restore result. Per-page
  encode/decode timing is still a future extension.
- `tokens.csv`: current runner output records run-level proof summary rows plus
  one row per `llama_decode` call. Future live adapter output must extend it to
  p50/p95/p99 latency policy, restore stalls, queue depth, VRAM, and CPU memory.
- `summary.md`: human-readable result table and claim scope.
- `failures.jsonl`: structured errors, cancellations, rejected pages, corruptions.
- `fixtures/`: optional sampled page payloads where licensing/privacy allows.

Private prompts, model weights, customer data, and raw production KV tensors must
not be committed to the QATQ repository. Public evidence must use reproducible
public prompts and models or publish only aggregate metrics plus checksums.

## Operational Safety

Default operator posture:

- disabled unless explicitly enabled;
- experimental warning at startup;
- hard memory budgets required;
- fail closed on restore errors;
- keep native runtime path available;
- expose kill switch;
- expose per-request opt-out;
- disable on unknown runtime/adapter version mismatch.

QATQ-side metrics currently emitted by `LiveVramOperatorMetrics`:

- `qatq_live_pages_resident_gpu`
- `qatq_live_pages_offloaded_cpu_raw`
- `qatq_live_pages_offloaded_qatq`
- `qatq_live_offload_bytes_raw`
- `qatq_live_offload_bytes_stored`
- `qatq_live_restore_failures_total`
- `qatq_live_checksum_failures_total`
- `qatq_live_offload_skipped_total`
- `qatq_live_pass_through_total`
- `qatq_live_restore_stalls_total`
- `qatq_live_restore_stall_nanoseconds_total`
- `qatq_live_shadow_validation_bytes`

Runtime adapters must add timing and GPU-side metrics that QATQ cannot observe
directly:

- `qatq_live_encode_seconds`
- `qatq_live_decode_seconds`
- `qatq_live_restore_stall_seconds`
- GPU allocation/free failures;
- GPU upload/download transfer time;
- per-token latency and queue depth.

## Documentation Requirements

- [x] Adapter contract document.
- [ ] Runtime-specific setup guide.
- [ ] Benchmark reproduction guide.
- [ ] Known limitations and unsupported runtimes.
- [ ] Operator safety guide.
- [x] Evidence report for each accepted exported-KV runtime/model matrix.
- [ ] Claim-scope statement for the website and README once the feature is
      proven.

## Release Checklist

The feature remains experimental until all items below are complete:

- [x] First runtime adapter implemented behind an experimental flag.
- [x] Offline simulator tests pass.
- [x] Runtime backend page snapshot/mutate/restore self-test passes for the
      patched llama.cpp runner.
- [x] Runtime snapshot/restore tests pass inside a real live page allocator.
- [x] Cold page offload tests pass for the scoped llama.cpp page-staged adapter.
- [x] Fuzz tests run in CI or scheduled workflow.
- [ ] Stress suite passes at the agreed duration. Current bounded 3x
      host-memory-pressure stress matrices pass, and the full accepted-family
      run at
      `/private/tmp/qatq-live-vram-server-family-policy-soak-burnin2-taildelta-security-gated-20260625`
      passed two complete Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini
      native/QATQ repeats with backend K/V and projected-device jitter ratios
      at 1.0 and a 2048 KiB positive RSS tail-delta comparison gate. The
      three-repeat follow-up at
      `/private/tmp/qatq-live-vram-server-family-policy-soak-burnin3-taildelta-security-gated-20260625`
      failed closed under slower host conditions, and the current accepted
      config now uses the focused passing Qwen2.5 3B q2 policy. The agreed
      overnight or longer-duration suite is still open.
- [ ] Benchmark matrix includes native, CPU offload, zstd, lz4, and available
      runtime-native KV compression baselines.
- [x] Peak VRAM reduction beats the agreed threshold in the scoped llama.cpp
      page-staged evidence runs.
- [x] Shared-server strict matrix enforces llama.cpp/Metal backend memory
      ceilings for projected device memory plus accelerator self, KV/context,
      and compute allocations. The threshold-gated rerun at
      `/private/tmp/qatq-live-vram-server-strict-backend-memory-current-20260625`
      passed Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini with empty
      `gate_failures`.
- [x] Latency regression stays within the agreed threshold in the scoped
      llama.cpp page-staged evidence runs.
- [x] Task/output preservation passes for the scoped llama.cpp page-staged
      evidence runs.
- [x] Evidence runner no longer requires a manually maintained patched
      llama.cpp checkout. `scripts/llama_cpp_adapter_bootstrap.py` clones the
      pinned upstream commit, applies the checked-in QATQ patch, runs the
      applied-source live-paging adapter audit, and builds `llama-simple` plus
      `llama-server`. The clean proof at
      `/private/tmp/qatq-llama-bootstrap-proof/bootstrap-report.json` completed
      every command with exit code `0`, and the audit report at
      `/private/tmp/qatq-llama-bootstrap-proof/qatq-adapter-audit.json` has
      empty required live-paging/page-staging failures. The strict
      three-model server matrix then passed from the freshly bootstrapped
      `llama-server` at
      `/private/tmp/qatq-live-vram-server-strict-bootstrap-proof-20260625`
      with empty `gate_failures`. Reproducing the full evidence matrices still
      requires local GGUF models and suitable GPU/MLX runtime availability.
- [ ] Security review covers corrupt payloads, cross-request isolation,
      cancellation, OOM, and metadata spoofing. The current ignored QATQ-side
      stress family
      `cargo test --locked --test kv_stress live_vram_ -- --ignored --nocapture`
      passes corrupt payload injection, cross-request key isolation, sealed
      multi-sequence interleaving, sealed CPU-tier budget pressure,
      runtime-adapter controller concurrency, and shared-runtime restore/cancel
      races. The latest aggregate run covered 2,048 corruption pages with
      62,954 injected mutations, 14,336 forged cross-request restore attempts,
      2,048 sealed multi-sequence pages across 8 workers, 1,046 accepted and
      1,002 rejected CPU-tier budget commits, 2,048 runtime-adapter concurrency
      pages, and 1,024 restore/cancel race pages with 885 restore winners and
      139 cancellation winners. QATQ also exposes a keyed BLAKE3 page-seal
      primitive for metadata/payload/context attestation, and strict
      `qatq-kv-bench` live-paging evidence now requires and emits
      per-offloaded-page seals.
      `LiveVramOffloadStore::with_page_seal_policy` enforces seals before
      QATQ-store-backed runtime restore. `LiveVramSealedRestoreRequest`,
      `LiveVramOffloadStore::sealed_restore_request`, and
      `LiveVramRuntimeAdapter::restore_sealed_committed_page` give adapters a
      verified restore object for direct restore/GPU-upload paths; the shared
      restore helper now uses that sealed method for sealed stores. Runtime-
      specific OOM review and wiring that guard into each adapter's GPU upload
      boundary remain open.
- [x] Docs clearly state the exact runtime/model/context/dtype scope.
- [ ] Website claim is updated only after evidence supports it.

## Open Decisions

- [x] First runtime target and commit.
- [ ] Page size defaults.
- [ ] Hot window defaults.
- [ ] CPU memory budget defaults.
- [x] Acceptable fallback p95/p99 latency regression for current long-context
      evidence: 15% p95 and 20% p99 generated-token regression. Strict native
      page-streaming latency thresholds still need a broader burn-in decision.
- [ ] Minimum VRAM reduction threshold for a public claim.
- [ ] Whether the adapter lives in the QATQ repository, a runtime fork, or a
      separate integration repository.
- [ ] Whether compressed pages use the current QATC sequential container or a
      new random-access page container for live service use.

## Current Status

Status: **experimental native llama.cpp backend-op compact proof implemented;
production live-VRAM adapter not yet complete**.

QATQ has strong storage/transfer evidence, direct Metal-backed llama.cpp KV
ingestion evidence across multiple local GGUF models, and native page-staged
llama.cpp proofs behind `--qatq-gpu-page-staging`. In that scoped runtime path,
canonical K/V tensors stay off GPU while scheduler-resident pages are staged
onto `MTL0`, QATQ restores are verified, and the strict live-paging gate sees
lower runtime-attested GPU K/V residency. The current strongest path consumes
bounded K/V page segments through the native
`GGML_OP_QATQ_SEGMENTED_KQV` backend-op route rather than treating legacy
concat-composed diagnostic paths as production evidence.
`/private/tmp/qatq-live-vram-backend-op-compact-native-gate` preserved the
full-GPU output for a 705-token Qwen2.5 1.5B prompt, restored `112/112` pages,
stored `56/56` offloaded pages through QATQ, had zero pass-through pages,
passed MLX GPU streaming-attention equivalence, and reduced persistent GPU K/V
residency from `22,020,096` bytes to `14,680,064` bytes. It is not
production-ready yet because broader runtime coverage, longer burn-in,
long-context latency-tail proof, pressure testing, and adapter security review
remain open.

The accepted llama-server backend-memory policy now also has bounded
three-repeat evidence across the local Qwen and Phi family set in one run.
`/private/tmp/qatq-live-vram-server-family-policy-soak-burnin3-p256q4-p05-tailgate-20260626`
passed three complete native/QATQ repeats over Qwen2.5 1.5B, Qwen2.5 3B, and
Phi 3.5 mini, for eighteen real matrix cases total. QATQ kept backend K/V
below native at 224->216 MiB, 288->280 MiB, and 3072->2976 MiB; projected
device memory stayed lower at 1458->1450 MiB, 2423->2415 MiB, and
5403->5304 MiB.
Backend K/V and projected-device jitter ratios were 1.0 for every native and
QATQ case. The accepted comparison gate now checks positive QATQ RSS
tail-growth delta over native instead of a raw ratio, preserving leak detection
when native tail growth is exactly 0 KiB. The largest observed positive QATQ
tail delta was 1712 KiB against the 2048 KiB gate in the earlier two-repeat
run; the current three-repeat policy also passed with empty comparison and
aggregate gate failures. This closes the bounded accepted-policy full-family
repeatability gap; it does not close overnight burn-in, direct hardware
peak-VRAM counters, or non-llama.cpp runtime coverage.
The first three-repeat full-family attempt then failed closed under slower host
conditions: Qwen2.5 3B q8 missed the p50 throughput gate and Phi native failed
the steady RSS tail gate. Follow-up Qwen2.5 3B probes showed that 64-token q2
could pass in isolation but failed the full-family repeat, while 128-token q4
also failed the corrected full-family p05/p50 run. The checked-in family config
now uses 256-token pages with q4 for Qwen2.5 3B. The full-family burn-in at
`/private/tmp/qatq-live-vram-server-family-policy-soak-burnin3-p256q4-p05-tailgate-20260626`
passed three repeats across Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini with
empty comparison and aggregate gate failures. This re-closes the bounded
accepted-policy repeatability gap, but it does not close overnight burn-in,
direct peak-VRAM hardware counters, or non-llama.cpp runtime coverage.

The compact backend-op route keeps hot pages in a compact persistent GPU pool
and leaves cold/offloaded pages CPU-backed. When an attention window mixes
retained GPU pages with CPU-backed restored pages, the adapter builds a
graph-local transient page pool for that window before scheduling
`GGML_OP_QATQ_SEGMENTED_KQV`. That is a valid proof of compact persistent K/V
residency. The transient pool is now guarded by
`--native-page-streaming-transient-pool-max-bytes`; longer-context optimisation
and pressure testing remain open before public live-VRAM reduction claims.

A fresh 2026-06-24 local verification pass rebuilt and ran the patched
llama.cpp Metal adapter against Qwen2.5 Coder 3B and Qwen2.5 Coder 7B GGUF
models. The current binary matrix passed 2/2 cases, QATQ beat zstd/lz4 on every
offloaded page, and MLX executed real GPU tensor workloads on `Device(gpu, 0)`.
Qwen2.5 1.5B and Phi 3.5 mini were also run as stress fixtures; they exposed
known hardening boundaries around tiny pages, long concat-composed diagnostic
paths, optional trace size, and codec-negative pages. The historical
whole-tensor/layer adapter still fails closed when asked to prove token-page
allocator reclaim. The native page-staged mode is now the stricter proof path
for that claim, with the remaining production caveat above.

The roadmap now has a real MLX-backed bounded-attention primitive:
`scripts/mlx_live_vram_streaming_attention.py` consumes llama.cpp-exported K/V
pages and an attention query fixture, runs materialised attention and
page-streaming online-softmax attention on MLX `Device(gpu, 0)`, and gates on
output drift plus peak page/materialised K/V ratio. It passed on Qwen2.5 Coder
3B and Qwen2.5 Coder 7B exports with max absolute error `4.470348e-08` and
peak page/materialised ratios `0.290826470` and `0.726756565`. This proves the
bounded-resident attention recurrence can run on a real local GPU over real
runtime pages, but it is still a primitive rather than a transparent llama.cpp
generation-loop adapter.

The same primitive now supports `--stream-from-qatq-store`. In that mode it
uses the release `qatq` binary to encode each selected f16/bf16/f32 K/V page,
decodes every page into an isolated restore buffer, verifies the decoded bytes
against the raw llama.cpp export, and streams only the restored page through
MLX GPU attention. Qwen2.5 Coder 3B and Qwen2.5 Coder 7B layer/head proofs
passed with QATQ storage ratios `0.367659279` and `0.366466687`, while
preserving the same attention-output error bound. This is still not transparent
llama.cpp live paging, but it proves the core compressed-page dataflow needed
by the final native adapter.

The proof has now been widened from one query head to all exported query heads
for one real layer. The patched llama.cpp attention fixture exports every query
head for the captured layer, and `scripts/mlx_live_vram_streaming_attention.py
--head -1 --stream-from-qatq-store` gates every head using grouped-query
head-to-KV-head mapping. Fresh Qwen2.5 Coder 3B and Qwen2.5 Coder 7B runs
passed across 16/16 and 28/28 query heads respectively, with QATQ storage ratios
`0.366478231` and `0.366466687`, max absolute error `2.384186e-07`, and peak
page/materialised ratios `0.581487791` and `0.726756565`.

The proof has also been widened across multiple captured layers. On
2026-06-24, Qwen2.5 Coder 3B ran a 4,800-token Metal prefill and passed the
QATQ-compressed MLX streaming gate for layers 0-3, all 64 exported query-head
checks, five pages per layer, max absolute error `1.549721e-06`, aggregate
QATQ ratio `0.590034129`, and peak page/materialised ratio `0.213333333`.
Qwen2.5 Coder 7B ran a 4,512-token Metal prefill and passed the same gate for
layers 0-1, all 56 exported query-head checks, five pages per layer, max
absolute error `5.960464e-07`, aggregate QATQ ratio `0.513008605`, and peak
page/materialised ratio `0.226950355`.

The native llama.cpp adapter now has a strict live-paging pass with native
segmented attention. With `--qatq-gpu-page-staging`, canonical K/V tensors stay
off GPU, selected page tensors are staged onto `MTL0`, and the attention graph
consumes bounded page segments through `ggml_segmented_kqv` instead of
concat-composed K/V sources. A Qwen2.5-Coder 3B run at
`/private/tmp/qatq-live-vram-native-ggml-strict-20260624` used 1024-token
pages, `current-token=512`, `next-required=page-end`, and
`--native-page-streaming-attention-ggml`. The runtime manifest attested
`live_page_residency_granularity: "per-page"` and
`gpu_allocation_granularity: "per-page"`. The strict
`qatq-kv-bench --live-vram-live-paging-gate` passed: GPU K/V before
`66,060,288` bytes, GPU K/V after `51,904,512` bytes, QATQ stored
`50,197,010` bytes versus `59,148,957` zstd and `64,291,680` lz4, 144/144
verified restores, and 144/144 pages beating the best general codec. MLX ran on
`Device(gpu, 0)` and checked all 36 captured layers and 576 heads from the
restored QATQ page store with max absolute error `0.000002861`.

This moves live VRAM reduction from an offline/MLX primitive into a native
llama.cpp generation-path proof with an accelerator-schedulable segmented
attention graph. It is still experimental: broader model/context coverage,
sustained latency tuning, and broader compression-policy burn-in remain open
before calling the feature production-complete. The later 2026-06-25 minimum
offload-tail guard addresses the specific one-token codec-negative tail case by
keeping pages smaller than `LLAMA_QATQ_TRACE_MIN_OFFLOAD_PAGE_TOKENS`
resident; broader policy defaults still need more runtime evidence.

A sustained native matrix then ran from the installed patched
`/private/tmp/qatq-llama.cpp/build/bin/llama-simple` binary at
`/private/tmp/qatq-live-vram-native-sustained-matrix-20260624-r2`. It passed
4/4 real GPU cases over Qwen2.5 Coder 3B and Qwen2.5 Coder 7B profiles with
native `ggml_segmented_kqv`, strict live-paging gates, GPU page staging, and
MLX `Device(gpu, 0)` streaming-attention gates:

| case | exact restores | offloaded QATQ pages | resident pages | MLX coverage | reclaimable GPU | QATQ / zstd / lz4 |
| --- | ---: | ---: | ---: | --- | ---: | --- |
| Qwen2.5 Coder 3B review, 512-token pages | 216 / 216 | 144 / 144 | 72 | 36 layers, 576 heads | 22.50 MiB | 34.92 / 40.03 / 43.51 MiB |
| Qwen2.5 Coder 3B memory, 1024-token pages | 144 / 144 | 144 / 144 | 0 | 36 layers, 576 heads | 18.00 MiB | 51.52 / 60.28 / 65.49 MiB |
| Qwen2.5 Coder 7B review, 512-token pages | 112 / 112 | 56 / 56 | 56 | 28 layers, 784 heads | 32.00 MiB | 38.17 / 43.04 / 46.68 MiB |
| Qwen2.5 Coder 7B memory, 512-token pages | 168 / 168 | 112 / 112 | 56 | 28 layers, 784 heads | 40.00 MiB | 52.27 / 58.97 / 63.98 MiB |

This is the current strongest live-VRAM proof: real models, real Metal
runtime, exact QATQ page restore, no raw pass-through, native non-concat
attention consumption, and external MLX verification. It is still an
experimental adapter proof rather than a production feature until the same
gates are broadened across more model families, dtypes, longer contexts,
repeated latency soaks, and a maintained/version-pinned llama.cpp adapter.

A breadth matrix then widened the strict native proof beyond Coder-only
fixtures. The run at
`/private/tmp/qatq-live-vram-native-breadth-matrix-20260624-r7` passed 3/3
additional real Metal/MLX cases:

| case | exact restores | offloaded QATQ pages | resident pages | MLX coverage | reclaimable GPU | QATQ / zstd / lz4 |
| --- | ---: | ---: | ---: | --- | ---: | --- |
| Qwen2.5 1.5B daily-driver, 512-token pages | 168 / 168 | 112 / 112 | 56 | 28 layers, 336 heads | 17.50 MiB | 25.03 / 28.65 / 31.12 MiB |
| Qwen2.5 3B general, 1025-token pages | 216 / 216 | 216 / 216 | 0 | 36 layers, 576 heads | 24.07 MiB | 82.22 / 98.22 / 106.95 MiB |
| Phi 3.5 mini operations, 512-token pages | 192 / 192 | 128 / 128 | 64 | 32 layers, 1024 heads | 288.00 MiB | 391.64 / 448.77 / 493.38 MiB |

The Phi run exposed and then verified a concrete adapter hardening fix:
`llama-simple` now applies bounded extra ggml graph metadata reserve for native
segmented attention even when the older persistent-page-source trace is not
enabled. Before that fix, the Phi graph reserve path aborted with a ggml memory
pool assertion.

The same breadth work found and resolved a Qwen2.5 3B general-instruct frontier
failure. At 18 staged KV layers, tested 1025/1537-token page shapes staged more
GPU page bytes than the original KV context, so the strict live-paging gate
rejected them. Reducing the staged frontier to 12 KV layers passed the strict
gate while still reclaiming 24.07 MiB and preserving output. That makes
page-staging byte pressure part of the frontier selection policy for this
adapter, not a codec correctness failure.

The same strict native matrices were then repeated with `--iterations 2`. The
breadth soak at
`/private/tmp/qatq-live-vram-native-breadth-soak-2x-20260624` passed 6/6 runs
across Qwen2.5 1.5B, Qwen2.5 3B general, and Phi 3.5 mini. The Coder sustained
soak at `/private/tmp/qatq-live-vram-native-sustained-soak-2x-20260624` passed
8/8 runs across Qwen2.5 Coder 3B/7B review and memory profiles. The stability
tables showed zero failures, stable reclaimable GPU MiB, stable QATQ byte
counts, zero pass-through pages, strict live-paging gate passes, native
`ggml_segmented_kqv` attention consumption, and MLX GPU page-streaming
equivalence in every repeated run.

The matrix runner now also exposes bounded stress gates for live-VRAM burn-in:
`--require-stable-reclaim`, `--require-stable-qc-bytes`,
`--max-elapsed-jitter-ratio`, and `--host-memory-pressure-mib`. A stricter
2026-06-24 pass ran the breadth matrix three times under 512 MiB of page-touched
host-memory pressure at
`/private/tmp/qatq-live-vram-native-breadth-stress-3x-512m-20260624`. It passed
9/9 real Metal/MLX runs with stable reclaimable GPU bytes and stable raw/QATQ,
zstd, and lz4 byte totals. The sustained Coder matrix then ran three times
under the same stress gate at
`/private/tmp/qatq-live-vram-native-sustained-stress-3x-512m-20260624` and
passed 12/12 runs. These pressure runs prove deterministic stability for the
current local fixtures; they do not replace longer burn-in, higher-pressure
adverse runs, or OOM-adjacent restore-slot failure tests.

A fresh 2026-06-25 breadth/dtype verification then reran the current local
matrix through cold-slot reuse plus the default minimum-tail guard. The breadth
run at `/private/tmp/qatq-live-vram-native-breadth-coldreuse-min16tail-20260625`
passed 3/3 real Metal/MLX cases across Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5
mini with strict live-paging, native page-streaming, aggregate codec,
GPU page-staging, MLX equivalence, and 1 GiB host-memory-pressure gates. The
dtype run at `/private/tmp/qatq-live-vram-native-dtype-coldreuse-min16tail-20260625`
passed 8/8 bf16/f32 cases across Qwen2.5 Coder 3B, Qwen2.5 3B, Qwen2.5 Coder
7B, and Phi 3.5 mini with the same gate shape. This upgrades current dtype
breadth from open proof work to scoped experimental evidence, but longer
prompts, longer soaks, more page sizes, more runtimes, and harsher pressure
profiles remain production gates.

The reproducible bootstrap path now has matching compact breadth evidence. The
matrix runner accepts `--llama-cpp-source`, which lets strict native verification
audit a bootstrapped checkout even when binaries are built under `build-qatq`.
`/private/tmp/qatq-live-vram-layer-memory-breadth-bootstrap-20260625` passed
3/3 real Metal/MLX Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini cases from the
clean pinned checkout at `/private/tmp/qatq-llama-bootstrap-proof`, under 1 GiB
host-memory pressure, with strict live-paging, strict native page-streaming,
backend-scheduled flattened Flash Attention, restore-slot pressure checks,
MLX verification, and pruned artifacts. The selected frontier was 4 KV GPU
layers for all three cases, reclaiming 23.00 MiB, 52.00 MiB, and 432.00 MiB of
persistent GPU K/V respectively while QATQ beat zstd and lz4 in every case.

A bounded process-level stress smoke first exercised the parallel wrapper
against the installed patched llama.cpp binary:
`/private/tmp/qatq-live-vram-parallel-stress-qwen15b-2x-20260625`. It ran two
concurrent Qwen2.5 1.5B `qwen25-15b-daily-native-512p-aggregate` child matrices
and passed 2/2. A broader follow-up at
`/private/tmp/qatq-live-vram-parallel-stress-breadth-3case-16tok-20260625`
then ran three concurrent 16-token child matrices across Qwen2.5 1.5B,
Qwen2.5 3B, and Phi 3.5 mini, and passed 3/3. The Qwen2.5 1.5B case restored
`168/168` pages exactly, compressed `112/112` offloaded pages with zero
pass-through, reclaimed 28.00 MiB of persistent GPU K/V, and stored 25.99 MiB
with QATQ versus 28.64 MiB with zstd and 31.12 MiB with lz4. The Qwen2.5 3B
case restored `504/504` pages exactly, compressed `360/360` offloaded pages,
reclaimed 111.00 MiB, and stored 80.79 MiB with QATQ versus 98.34 MiB with
zstd and 106.99 MiB with lz4. The Phi 3.5 mini case restored `192/192` pages
exactly, compressed `128/128` offloaded pages, reclaimed 480.00 MiB, and
stored 402.61 MiB with QATQ versus 448.82 MiB with zstd and 493.38 MiB with
lz4. A two-token cold-start breadth diagnostic failed the Qwen2.5 3B
performance gate while the same case passed serially; the 16-token pass is the
current process-level breadth evidence because it reduces one-off Metal
pipeline timing noise without relaxing correctness, native-page, MLX, or codec
gates. This proves reproducible process-level patched-runtime contention and
log isolation for the current native route; it still does not prove in-process
multi-request cancellation inside one llama.cpp server.
The wrapper now also records `timed_out` and `job_timeout_seconds` in
`summary.json`, writes partial child stdout/stderr on timeout, and marks the
aggregate failed. Timed-out child matrices run in their own process group; the
wrapper sends group `SIGTERM`, escalates to group `SIGKILL` if needed, and
records the cleanup signal in JSON and Markdown summaries. This prevents
unattended process-level stress jobs from stalling or leaving live child
processes behind without evidence.

A fresh installed-runtime verification on 2026-06-24 reran that proof from
`/private/tmp/qatq-llama.cpp/build/bin/llama-simple` and widened it across the
available local Qwen models:

| model | prompt tokens | pages | resident pages | offloaded pages | GPU K/V before | GPU K/V after | QATQ | zstd | lz4 | gate |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Qwen2.5 1.5B Q4_K_M | 1,536 | 168 | 56 | 112 | 49.00 MiB | 12.00 MiB | 24.89 MiB | 34.95 MiB | 40.05 MiB | pass |
| Qwen2.5 Coder 3B Q4_K_M | 2,048 | 288 | 72 | 216 | 81.00 MiB | 15.00 MiB | 44.53 MiB | 60.92 MiB | 68.68 MiB | pass |
| Qwen2.5 Coder 7B Q4_K_M | 2,048 | 224 | 56 | 168 | 126.00 MiB | 24.00 MiB | 61.46 MiB | 84.70 MiB | 102.82 MiB | pass |

All three runs preserved deterministic output against the full-GPU baseline,
restored every exported page exactly, used zero pass-through pages, and beat
zstd/lz4 on every tested page boundary. The 1.5B fixture also passed MLX
`Device(gpu, 0)` streaming attention from a QATQ page store across 12 query
heads with max absolute error `4.768372e-07`, QATQ ratio `0.239797592`, and a
peak page/materialised K/V ratio of `0.333333333`.

The MLX verifier is now part of the fail-closed evidence runner through
`--mlx-streaming-attention-gate`, not only a separately invoked smoke. A fresh
integrated run at
`/private/tmp/qatq-live-vram-all-layer-mlx-perf-gated-page-staging-20260624`
used the patched installed llama.cpp binary, QATQ release binaries, strict
live-paging, and `/private/tmp/qatq-mlx-venv/bin/python` in one gated flow. It
preserved output, restored 168/168 pages, stored 112/112 offloaded pages
through QATQ, reduced GPU K/V from 51,380,224 bytes to 12,582,912 bytes, beat
zstd/lz4 on every offloaded page boundary, and required MLX on `Device(gpu, 0)` to
validate all 28 captured layers and 336 query-head checks from a compressed
QATQ page store. The run also enabled the new MLX coverage/performance gates:
minimum 28 layers, minimum 336 heads, QATQ store compression ratio below 1.0,
and streaming/materialised attention time ratio no higher than 3.0. The
observed ratio was `1.664277`. At this point MLX was still carrying part of the
page-bounded attention proof externally; later strict native backend-op runs
closed that specific concat-source gap for the scoped llama.cpp adapter path.

The same MLX coverage/performance gates are now supported by
`scripts/llama_cpp_live_vram_matrix.py` and by the local matrix example. A
two-case Coder matrix at
`/private/tmp/qatq-live-vram-coder-native-gate-status-20260624` passed the strict
live-paging gate for Qwen2.5 Coder 3B and Qwen2.5 Coder 7B.
The 3B workhorse case restored 288/288 pages, offloaded 216/216 through QATQ,
checked 36 MLX layers and 576 heads, reclaimed 66.00 MiB of GPU K/V, and
reported MLX QATQ ratio `0.618498` with streaming/materialised time ratio
`1.868`. The 7B powerhouse case restored 224/224 pages, offloaded 168/168
through QATQ, checked 28 MLX layers and 784 heads, reclaimed 112.00 MiB of GPU
K/V, and reported MLX QATQ ratio `0.548739` with time ratio `1.822`.

The persistent page-source path is now byte-budgeted as well as count-budgeted.
The patched llama.cpp runner accepts
`--qatq-attention-persistent-page-source-max-source-bytes` and
`--qatq-attention-persistent-page-source-max-retained-bytes`; the runtime
throws before graph growth or backend allocation can exceed those budgets, and
the patched `llama-simple` catches those exceptions, frees the Metal context,
and exits with code `1`. A good-path Qwen2.5 1.5B run at
`/private/tmp/qatq-live-vram-byte-budgeted-page-staging-retained-20260624`
passed with 256 MiB per-source and 1 GiB retained budgets, observing
`262,144` max requested bytes per source event and `12,582,912` retained bytes.
Two hostile probes confirmed clean fail-closed behaviour for a one-byte source
budget and a one-byte retained budget.
