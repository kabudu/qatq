# llama.cpp Adapter

This directory tracks the QATQ-side contract for a patched llama.cpp KV-cache
exporter. The adapter target is direct internal K/V tensor capture, not
session-state serialization and not Ollama API output.

Expected exporter behavior:

- run llama.cpp with deterministic model, prompt, seed, and KV dtype settings;
- export `cache_k_l*` and `cache_v_l*` tensors as raw little-endian `.f16le`,
  `.bf16le`, or `.f32le` files;
- write a manifest with tensor names, shapes, dtypes, llama.cpp commit, model
  hash, prompt hash, capture point, `gpu_allocation_granularity`, and
  `gpu_context_bytes`;
- invoke QATQ exact with `--dtype` matching the exported tensor dtype.

Example QATQ calls:

```sh
cargo run -- encode --mode qatq-exact --dtype f16 cache_k_l0.f16le cache_k_l0.qatq
cargo run -- encode-chunked --max-values-per-chunk 65536 --dtype bf16 cache_v_all.bf16le cache_v_all.qatc
```

The full capture plan is in
[`docs/LLAMA_CPP_KV_CAPTURE.md`](../../docs/LLAMA_CPP_KV_CAPTURE.md).

## Owned llama.cpp Patch

QATQ carries the exporter hook as a source patch in this directory:

- `qatq-kv-export-7992aa7c8.patch`

It targets llama.cpp commit `7992aa7c8`, the commit reported by the local
Homebrew `llama-cli` build `8640`. Apply it to a llama.cpp source checkout and
build the patched `llama-simple` runner. The patch adds:

- `llama_qatq_export_kv_cache(ctx, export_dir, seq_id)`;
- `llama-simple --qatq-kv-export-dir <dir>`;
- `llama-simple --cache-type-k <f16|bf16|f32>`;
- `llama-simple --cache-type-v <f16|bf16|f32>`;
- `llama-simple --memory-breakdown`;
- `llama-simple --no-kv-offload`;
- `llama-simple --qatq-kv-gpu-layers <n>`;
- `llama-simple --qatq-gpu-page-staging`;
- `llama-simple --qatq-output-manifest <path>`;
- `llama-simple --qatq-token-timings <path>`;
- `llama-simple --qatq-event-trace <path>`;
- `llama-simple --qatq-attention-trace <path>`;
- `llama-simple --qatq-attention-event-trace <path>`;
- `llama-simple --qatq-attention-materialized-source-trace <path>`;
- `llama-simple --qatq-attention-page-composed-source-trace <path>`;
- `llama-simple --qatq-attention-persistent-page-source-trace <path>`;
- `llama-simple --qatq-attention-page-segments-trace <path>`;
- `llama-simple --qatq-attention-persistent-page-source-max-pages <n>`;
- `llama-simple --qatq-attention-persistent-page-source-max-source-pages <n>`;
- `llama-simple --qatq-attention-persistent-page-source-max-source-bytes <n>`;
- `llama-simple --qatq-attention-persistent-page-source-max-retained-bytes <n>`;
- `llama-simple --qatq-attention-page-tensor-self-test <path>`;
- `llama-simple --qatq-attention-page-tensor-self-test-limit <n>`;
- `llama-simple --qatq-native-page-streaming-preflight`;
- `llama-simple --qatq-native-page-streaming-contract`;
- `llama-simple --qatq-native-page-streaming-attention`;
- `llama-simple --qatq-native-page-streaming-attention-ggml`;
- `llama-simple --qatq-native-page-streaming-attention-backend-op`;
- `llama-simple --qatq-attention-fixture-dir <dir>`;
- `llama-simple --qatq-page-tokens <n>`;
- `llama-simple --qatq-trace-current-token <n>`;
- `llama-simple --qatq-trace-hot-window-tokens <n>`;
- `llama-simple --qatq-trace-prefetch-window-tokens <n>`;
- `llama-simple --qatq-trace-next-required uniform-after-hot|page-end|cold-after-hot`;

Prefer the bootstrap script over a hand-maintained checkout:

```sh
python3 scripts/llama_cpp_adapter_bootstrap.py \
  --work-dir /private/tmp/qatq-llama.cpp \
  --output /private/tmp/qatq-llama.cpp/bootstrap-report.json
```

The script clones `ggml-org/llama.cpp`, checks out
`7992aa7c8e21ea2eb7a5e4802da56eec7b376036`, applies the checked-in adapter
patch, runs `scripts/llama_cpp_live_vram_adapter_audit.py
--require-live-paging --require-runtime-security` against the applied source,
and builds `llama-simple` plus `llama-server`. Use `--dry-run` to emit the
planned commands without network, patch, or build side effects. Use `--force`
only when the target work directory is disposable.

After bootstrapping, the strict shared-server proof can be run directly against
the fresh binary:

```sh
python3 scripts/llama_cpp_live_vram_server_cancel_matrix.py \
  --config adapters/llama-cpp/live-vram-server-strict.local.example.json \
  --llama-server /private/tmp/qatq-llama.cpp/build-qatq/bin/llama-server \
  --work-dir /private/tmp/qatq-live-vram-server-strict-bootstrap-proof \
  --timeout 2400
```

The current clean-bootstrap proof at
`/private/tmp/qatq-live-vram-server-strict-bootstrap-proof-20260625` passed all
three strict model cases with empty `gate_failures`.
- `llama-simple --qatq-trace-max-queued-pages <n>`;
- `llama-simple --qatq-live-page-self-test <n>`;
- `llama-simple --qatq-live-physical-page-alloc-self-test <n>`;
- `llama-simple --qatq-live-persistent-page-pool-trace <path>`;
- `llama-simple --qatq-live-persistent-page-pool-self-test <n>`;
- `llama-simple --qatq-live-restore-slot-pressure-self-test <n>`;
- `llama-simple --qatq-live-restore-slot-pressure-max-bytes <n>`;
- `llama-simple --qatq-model-id <id>`;
- `llama_qatq_export_kv_cache_with_trace(ctx, dir, seq_id, trace_path, model_id,
  current_token)`.

The patch writes runtime allocator evidence into `manifest.json`:
`live_page_residency_granularity`, `gpu_allocation_granularity`,
`gpu_context_bytes`, `total_context_bytes`, `gpu_resident_tensors`, and
`total_tensors`.

Default Metal KV exports still report `gpu_allocation_granularity:
"whole-context"`. Runs with `--qatq-kv-gpu-layers <n>` keep only the first `n`
KV layers on the accelerator and place the remaining KV tensors on host memory;
those manifests report `gpu_allocation_granularity: "whole-tensor"` plus the
reduced GPU KV byte count. This proves coarse runtime KV placement and exact
continuation under reduced GPU KV allocation, but it is not yet token-page QATQ
live paging.

The current patch also includes a logical page residency primitive used by
`--qatq-live-page-self-test`. Those manifests report
`live_page_residency_granularity: "per-page"` when the adapter exposes
page-keyed evict/restore state. QATQ still refuses the strict
`--live-vram-live-paging-gate` unless physical
`gpu_allocation_granularity: "per-page"` is also attested.

Use `--qatq-output-manifest <path>` on paired runs to capture deterministic
generated-token evidence. The manifest records the prompt/model hashes,
generated token IDs, generated text hash, cache dtype, KV placement controls,
and elapsed decode time. This is useful for proving that a reduced-KV placement
run produced the same continuation as the full-GPU KV baseline.

Use `--qatq-token-timings <path>` when you want one CSV row per `llama_decode`
call. The first row is prompt prefill and later rows are generated-token decode
steps. The evidence runner now folds these rows into its aggregate `tokens.csv`
so latency analysis can use raw runtime timings rather than only total decode
duration.

Use `--qatq-event-trace <path>` with `--qatq-model-id <id>` when you want the
export path to emit a `qatq-live-vram-event-trace-v1` audit file. Add
`--qatq-trace-current-token <n>`, `--qatq-trace-hot-window-tokens <n>`,
`--qatq-trace-prefetch-window-tokens <n>`, and `--qatq-trace-next-required
uniform-after-hot|page-end|cold-after-hot` when the export-time trace should
mirror QATQ's page scheduler. `cold-after-hot` keeps recent pages resident and
gives older pages a future restore deadline for long-context diagnostics; the
prefetch window keeps near-future pages resident too, matching
`qatq-kv-bench --live-vram-prefetch-window-tokens`. Add
`--qatq-trace-max-queued-pages <n>` when the trace must mirror QATQ's
latency-aware offload cap; after `n` offloaded pages, later eligible pages
remain resident with the same lifecycle ordering. Resident pages emit snapshot
and attention-use
events; offloaded pages also emit offload-committed and restore-committed
events. QATQ can verify that trace with
`qatq-kv-bench --live-vram-event-trace`. This proves the trace schema and
restore-before-attention checker against real runtime tensor metadata, but it
is not yet an attention-loop live paging proof because the runtime still
exports tensors after generation rather than evicting and prefetching token
pages during attention.

Use `--qatq-attention-trace <path>` with `--qatq-model-id <id>` when you want
the patched runtime to emit JSONL telemetry from the actual llama.cpp
`llama_kv_cache_context::get_k/get_v` attention read path. This is stronger
than export-time lifecycle telemetry because it proves generation touched the
runtime K/V read sites, but it is still diagnostic evidence rather than live
page eviction. It does not prove that pages were evicted, restored, or that
Metal memory was reclaimed at token-page granularity.

Use `--qatq-attention-event-trace <path>` when you want the patched runtime to
emit QATQ lifecycle events from the actual `get_k/get_v` attention path. The
trace is append-only JSONL so generation does not need to buffer a large JSON
array. QATQ validates it with
`qatq-kv-bench --live-vram-event-trace-only --live-vram-event-trace <path>
--live-vram-event-trace-gate`. This proves restore-before-attention ordering
and checksum consistency at the attention-read hook. It still does not prove
page-granular Metal allocation reclaim.

Use `--qatq-attention-materialized-source-trace <path>` when you want the
patched runtime to wrap the `get_k/get_v` attention source in a `ggml_cont`
materialisation before returning it to llama.cpp's attention graph. The trace
records the source and materialised tensor shape and byte count for every
attention source event. The evidence runner compares generated tokens against a
native full-GPU run to prove this path preserves output. This is an intermediate
adapter proof: attention consumes a materialised K/V source tensor, but the
source is still copied from the persistent KV cache and does not release that
persistent allocation.

Use `--qatq-attention-page-composed-source-trace <path>` when you want the
patched runtime to split the `get_k/get_v` attention source into bounded token
pages, materialise each page with `ggml_cont`, compose those pages with
`ggml_concat`, and return that page-composed source to llama.cpp's attention
graph. The trace records page size, page count, shapes, and byte counts. This
is closer to a native paged attention consumer than the whole-source
materialisation proof, but it still composes pages from the persistent KV cache
and does not release that persistent allocation.

Use `--qatq-attention-persistent-page-source-trace <path>` when you want the
patched runtime to split the `get_k/get_v` attention source into bounded token
pages, allocate retained page tensors on the same non-host backend, graph-copy
each page into those retained tensors with `ggml_cpy`, and compose the copied
page tensors with `ggml_concat` before llama.cpp attention consumes them. Use
`--qatq-attention-persistent-page-source-max-pages <n>` to cap retained page
tensors and `--qatq-attention-persistent-page-source-max-source-pages <n>` to
cap the number of source pages composed into one attention source. Use
`--qatq-attention-persistent-page-source-max-source-bytes <n>` to cap bytes
staged for a single attention source and
`--qatq-attention-persistent-page-source-max-retained-bytes <n>` to cap the
total retained backend page pool. The current concat-composed implementation
fails closed for page sizes below 160 tokens when persistent page-source
tracing is enabled, because very small pages can exhaust ggml graph objects
before they provide useful production evidence.
When this trace is enabled, the patched runner also reserves additional ggml
graph metadata before context creation. The reserve is intentionally metadata
only and bounded; the persistent page-source hook is enabled after context
creation so the reserve pass does not try to page-compose the model's entire
padded context.
This is the strongest current attention-consumption proof because attention
reads from independently allocated backend page tensors. It still sources those
page tensors from the default persistent KV cache, so it does not by itself
prove page-granular allocator reclaim.

Use `--qatq-gpu-page-staging` with the persistent page-source trace when you
want the canonical llama.cpp KV tensors kept off GPU while attention stages
pages into retained accelerator tensors. The manifest records
`gpu_page_staging_bytes` and `gpu_page_staging_tensors`. This is an allocator
step toward live VRAM reduction, but it is not sufficient when the staged page
pool equals or exceeds the full KV context. QATQ's strict live-paging gate now
rejects that case explicitly.

The current staging implementation uses the same hot-window/page-end scheduler
predicate as the QATQ event trace. Scheduler-resident pages are staged on the
accelerator; colder pages remain CPU-backed page views. A Qwen2.5 1.5B
512-token-page fixture passed `qatq-kv-bench --live-vram-live-paging-gate` with
56 resident page tensors, 112 offloaded pages, and exact output versus a
full-GPU baseline. Fresh installed-runtime checks also passed on Qwen2.5 Coder
3B and Qwen2.5 Coder 7B local GGUF models. This is page-staging and
export/replay evidence, not the final native attention-loop implementation. In
the current pinned patch, `--qatq-native-page-streaming-attention` and
`--qatq-native-page-streaming-attention-ggml` now reach an executable non-concat
`ggml_segmented_kqv` graph bridge. The bridge consumes multiple K/V page
segments without rebuilding a full K/V tensor: it builds per-segment logits,
concatenates only the logits for one global softmax, then applies probability
views back to each V page and sums the page outputs. This is a correctness
bridge for segmented attention, not the final production kernel. The patch now
also includes a conservative f32 `GGML_OP_QATQ_SEGMENTED_KQV` Metal backend
surface for the online page-summary reduction. CPU custom-op attention is only
useful as a correctness bridge; it is not accepted by the production native
live-VRAM audit because it cannot prove accelerator-schedulable performance.
The patch also exposes `--qatq-native-page-streaming-attention-backend-op`,
which routes bounded segmented K/V pages into the masked
`GGML_OP_QATQ_SEGMENTED_KQV` Metal backend op. The route carries backend
contract `qatq-segmented-kqv-backend-contract-v1`, currently bounded to 4,096
segments, 4,096 tokens per page, 8,192 total K/V tokens, positive stream
count, query-head divisibility by stream count, and explicit fail-closed
rejections for unsupported softcap, ALiBi, multi-stream, transposed-V, offload,
mask, or dtype cases. The patch carries f32/f16/bf16 Metal kernel targets for
the segmented K/Q/V route. The latest kernel dispatch uses one bounded
threadgroup per query/head so logits and the softmax denominator are computed
once before output dimensions are written. A pinned Metal-enabled llama.cpp
build compiles `llama-simple` with that shader source, ggml op, Metal dispatch
path, and graph integration. That is still not a production mode until the
broader native matrix and long-context latency gates pass, but the rebuilt
Qwen2.5 1.5B backend-op smoke now proves the strict runtime status shape for
one real Metal/MLX case. The latest retained page-table smoke matched the
full-GPU 8-token baseline, reached 31.21 tok/s after replacing repeated V-side
page lookups with page-range accumulation, adding explicit token-to-page and
token-to-local tables, and consuming the retained tiled page table directly.
It reduced graph nodes to 1,575 from the previous graph-arena smoke's 1,799
nodes and now beats that graph-arena smoke's 30.74 tok/s, but still trails the
56.3 tok/s full-GPU baseline on the same rebuilt binary family. The fast Metal
path is currently capped at 8,192 staged K/V tokens, so longer-context
production use needs tiling or a fallback. The graph now marks page segments
with `live_offloaded` and keeps all-resident layers on stock llama.cpp
attention; only layers with cold/offloaded K/V pages route through
`backend_scheduled_segmented_attention`.
For bottleneck isolation only, `LLAMA_QATQ_NATIVE_PAGE_STREAMING_DIRECT_SOURCE_FALLBACK=1`
keeps the logical page schedule but reads the contiguous K/V source instead of
the staged page arena; the same smoke reached 33.5 tok/s and is not counted as
the strict page-staging proof. Strict evidence now requires a per-run
`--live-vram-page-seal-key-hex` with `--live-vram-require-page-seals` and emits
metadata seals for every offloaded page. Production native live-VRAM evidence
also requires `scripts/llama_cpp_live_vram_adapter_audit.py
--require-live-paging` plus `native-page-streaming-status.json` reporting
`backend_scheduled_segmented_attention: true`,
`accelerated_runtime_attention_graph: true`, and
`page_bounded_attention_equivalence_passed: true`. `segmented_graph_bridge` may
be false for the backend-op path.

Use `--qatq-live-restore-slot-pressure-self-test <n>` with
`--qatq-live-restore-slot-pressure-max-bytes <bytes>` when validating the
runtime's fail-closed restore-slot pressure path. The patched runtime finds a
real active key page on the accelerator page-staging backend and rejects the
restore before allocation if the requested page is larger than the configured
slot limit. This is a bounded adverse test for resource-limit handling; it is
not a request to deliberately OOM the process.

Use `--qatq-attention-page-tensor-self-test <path>` when you want the patched
runtime to materialise real K/V page bytes from the actual `get_k/get_v`
attention path into a separate page-sized non-host backend tensor, round-trip
those bytes exactly, and free the temporary tensor. The companion
`--qatq-attention-page-tensor-self-test-limit <n>` flag bounds how many events
are emitted. This is stronger than the post-generation physical page allocation
smoke because it executes from the attention read hook, but it still does not
prove that the attention graph consumes those temporary page tensors.

Use `--qatq-attention-fixture-dir <dir>` when you want the patched runtime to
export a real computed query vector fixture from llama.cpp's graph alongside
the K/V export. QATQ can combine that query with sliced K/V page files via
`scripts/llama_cpp_attention_fixture_gate.py` and require
`qatq-kv-bench --attention-equivalence-gate` to pass. This proves that the
page-bounded attention reference produces the same output as a materialised
attention reference over real llama.cpp tensors. It is still not a VRAM reclaim
claim by itself.

Add `--max-peak-page-kv-ratio <r>` to
`scripts/llama_cpp_attention_fixture_gate.py` when you want that proof to also
require bounded peak K/V residency. The wrapper forwards this to
`qatq-kv-bench --attention-max-peak-page-kv-ratio <r>`. This is the acceptance
gate for a future streaming attention runtime path: the page-bounded recurrence
must preserve attention output while using less peak K/V residency than the
materialised full-context reference.

Use `--qatq-page-tokens <n>` when you want the exporter to split each active
K/V tensor into bounded token-range files. The manifest records
`token_start`, `token_end`, and `active_cells` per exported page. This gives
QATQ real token-page compression and restore evidence, but it is still export
granularity rather than allocator-backed live paging.

Set `LLAMA_QATQ_N_KV_PAD_TOKENS=<n>` when you need to tune the graph reserve
rounding quantum for `--qatq-gpu-page-staging` or the native backend-op route.
This does not cap attention: the patched runtime still rounds upward from the
active K/V high-water mark. It only avoids inheriting llama.cpp's default
256-token graph padding, or the full export page size, when a smaller live
attention window is enough. Prefer the evidence runner's
`--n-kv-pad-tokens <n>` flag so the value is captured in matrix command files.
A Qwen2.5 1.5B 32-token Metal smoke with `--qatq-page-tokens 64` and
`--n-kv-pad-tokens 48` preserved the full-GPU output manifest, staged
`n_kv: 48` as one K/V segment per layer, lowered CPU compute buffer use to
0.0946 MiB, and reached 21.11 ms/token, 47.36 tok/s. It reused 28 graphs, so
this is a latency tuning knob rather than a universal default.

Use `--qatq-live-page-self-test <n>` when you want the patched runtime to
snapshot one active key page from real backend KV tensor storage, overwrite
that page with zeroes, verify the mutation, restore the original bytes, and
verify the restored checksum. This proves the adapter can perform page
snapshot/mutate/restore operations against llama.cpp's runtime tensors. It
does not prove per-page Metal allocation reclaim or transparent live paging.

Use `--qatq-live-physical-page-alloc-self-test <n>` when you want the patched
runtime to allocate one page-sized non-host backend tensor on the same backend
as an active key page, round-trip the active page bytes through that tensor, and
free it. This proves page-sized backend tensor storage mechanics on the runtime
backend, but it still does not prove that attention uses those page tensors or
that sustained KV pages are reclaimed from VRAM.

Use `--qatq-live-persistent-page-pool-trace <path>` with
`--qatq-live-persistent-page-pool-self-test <n>` when you want the patched
runtime to allocate, exact-byte verify, and retain a bounded pool of independent
non-host K/V page tensors until `llama_context` teardown. The JSONL trace uses
`qatq-live-vram-persistent-page-pool-v1` and records backend, layer, kind,
token range, requested bytes, allocated bytes, and retained page count. This is
stronger than the short-lived physical page allocation smoke because the page
buffers persist independently on the runtime backend. It still coexists with
the default whole-layer KV allocation and therefore is not a transparent live
VRAM reduction claim.

Compare paired output manifests with QATQ before treating a reduced-KV run as
behaviour-preserving evidence:

```sh
cargo run --bin qatq-kv-bench -- \
  --compare-output-baseline captures/full-gpu-output.json \
  --compare-output-candidate captures/mixed-kv-output.json \
  --compare-output-gate \
  --output captures/output-comparison.json
```

Then run `qatq-kv-bench` or `scripts/llama_cpp_kv_matrix.py` over the export
directory.

For the current live-VRAM evidence path, prefer the fail-closed runner:

```sh
python3 scripts/llama_cpp_live_vram_evidence.py \
  --llama-simple /path/to/patched/llama-simple \
  --model /path/to/model.gguf \
  --model-id <stable-model-id> \
  --sweep-kv-gpu-layers 18,24,30 \
  --page-tokens 1024 \
  --n-kv-pad-tokens 256 \
  --short-prompt "..." \
  --deep-prompt-seed "..." \
  --work-dir /tmp/qatq-live-vram-evidence
```

The runner executes paired full-GPU, mixed-KV, and native all-CPU-KV
continuations. With `--sweep-kv-gpu-layers`, it evaluates each mixed placement,
rejects candidates that change output, miss the GPU-saved threshold, exceed the
decode regression ceiling, or lose to all-CPU-KV, then selects the fastest
passing frontier point for the deep export. The deep export now asks the
patched runner for an export-time `qatq-live-vram-event-trace-v1` file, an
attention-read JSONL trace, an attention-path lifecycle JSONL trace, an
attention materialised-source JSONL trace, a page-composed source trace, and,
by default, a persistent page-source trace. It can also request a persistent
page-pool trace. It passes the export lifecycle
trace into `qatq-kv-bench`, validates the attention-read trace separately,
gates the attention-path lifecycle trace with
`qatq-kv-bench --live-vram-event-trace-only`, and compares the materialised
source path against a native generated-token baseline. It also asks for an
attention fixture by default and runs the QATQ page-bounded attention
equivalence gate over real llama.cpp Q/K/V artefacts. It also compares the
page-composed source path against a native generated-token baseline and requires
the short page-composed run to preserve output. The deep run prefers the
persistent page-source path, compares its short output against the native
baseline, and requires the deep persistent page-source trace to compose more
than one token page on a non-CPU backend. It also asks the adapter to
emit attention-path page tensor round-trip evidence by default and validates
that the temporary page tensors use a non-CPU backend. Use `--skip-event-trace`,
`--skip-attention-trace`, `--skip-attention-event-trace`,
`--skip-attention-materialized-source`,
`--skip-attention-page-composed-source`,
`--skip-attention-persistent-page-source`,
`--skip-attention-page-segments-trace`,
`--skip-attention-page-tensor-self-test`, or `--skip-attention-equivalence` only
when testing an older adapter that does not support those hooks, or when
running a low-overhead latency diagnostic that must not pay for optional
page-segment proof tracing. The strict native page-streaming gate deliberately
rejects `--skip-attention-page-segments-trace`. Use
`--mixed-kv-gpu-layers <n>` when a single-point smoke test is enough. The runner
fails if Metal is not exercised, output changes, exact restore coverage is
incomplete, allocator reclaim is missing, either trace is malformed, the
attention-equivalence gate fails, the attention page tensor self-test is absent
or CPU-backed, restore deadlines miss, offloaded pages fall back to raw
pass-through, or QATQ fails to beat zstd/lz4 on every offloaded page.
Add `--page-tokens <n>` to validate token-page export boundaries instead of one
exported page per whole K/V tensor. Scheduler-resident pages are allowed; they
are still checked for exact restore and compression baseline comparisons but
are not counted as offloaded compressed pages. Add
`--live-page-self-test-tokens <n>` to make the deep run also execute the runtime
backend page self-test described above.

Add `--mlx-streaming-attention-gate` when MLX is available and you want the
same evidence run to prove the external page-bounded attention primitive on the
local GPU. The runner requires an attention fixture, builds a QATQ-compressed
page store, invokes `scripts/mlx_live_vram_streaming_attention.py` with
`--layer -1 --head -1 --stream-from-qatq-store`, and fails unless MLX reports a
GPU device, exact-enough materialised-versus-streaming attention, bounded peak
page residency, and a compression-positive QATQ page store. Use
`--mlx-python` to point at the Python environment with MLX installed and
`--mlx-qatq-bin` to select the QATQ binary used for the compressed store. Use
`--attention-fixture-max-layers`, `--mlx-min-layers-checked`, and
`--mlx-min-heads-checked` together when the proof must cover more than the
first captured layer. Use `--mlx-max-streaming-slowdown <ratio>` to make the
same gate fail if page-streaming attention is too slow relative to the
materialised MLX reference.

`scripts/llama_cpp_live_vram_matrix.py` forwards the same MLX options from a
case config, including `mlx_streaming_attention_gate`, `mlx_python`,
`mlx_qatq_bin`, `mlx_min_layers_checked`, `mlx_min_heads_checked`, and
`mlx_max_streaming_slowdown`. Its matrix summary includes the MLX layer/head
coverage, QATQ store ratio, and streaming/materialised time ratio for each
case. The local example config includes stricter MLX-gated Coder 3B and Coder
7B cases using a controlled page-staging prefill shape.

When the runtime adapter grows actual token-page eviction and prefetch inside
the attention path, run the same script with:

```sh
python3 scripts/llama_cpp_live_vram_evidence.py \
  --llama-simple /path/to/patched/llama-simple \
  --model /path/to/model.gguf \
  --model-id <stable-model-id> \
  --sweep-kv-gpu-layers 18,24,30 \
  --work-dir /tmp/qatq-live-vram-evidence \
  --require-live-paging
```

That switches QATQ from the coarse `--live-vram-runtime-reclaim-gate` to
`--live-vram-live-paging-gate`. It is expected to fail for export-only or
whole-tensor adapter modes. Use `--gpu-page-staging` when validating the
experimental page-staged mode; that mode must prove page-granular GPU reclaim,
QATQ-compressed offloaded pages, and restore-before-attention ordering.

For the future production adapter, add the stricter native-attention gate:

```sh
python3 scripts/llama_cpp_live_vram_evidence.py \
  --llama-simple /path/to/patched/llama-simple \
  --model /path/to/model.gguf \
  --model-id <stable-model-id> \
  --sweep-kv-gpu-layers 18,24,30 \
  --work-dir /tmp/qatq-live-vram-evidence \
  --require-live-paging \
  --require-native-page-streaming \
  --gpu-page-staging \
  --mlx-streaming-attention-gate
```

`--require-native-page-streaming` requires the llama.cpp generation graph itself
to consume K/V through a non-concat page-streaming path. In the current pinned
adapter patch, `--qatq-native-page-streaming-attention` and
`--qatq-native-page-streaming-attention-ggml` reach a compile-tested
multi-segment `ggml_segmented_kqv` graph bridge. The bridge avoids rebuilding a
full K/V source. The backend-op route now also supports multi-page streaming:
it passes page offsets, the real attention mask, and strided f32/f16/bf16 K/V
views into `GGML_OP_QATQ_SEGMENTED_KQV` without packing a full K/V window
through `ggml_concat`. That prevents page-segment diagnostics and graph-only
bridges from being mistaken for performant native live-VRAM reduction while
still giving the runtime a backend-scheduled path to validate.
Use `--qatq-native-page-streaming-attention-backend-op` only when testing the
constrained fused-op boundary: it validates the page contract, passes the real
attention mask, and routes bounded f32/f16/bf16 page tensors through the Metal
backend op. Unsupported layouts and dtypes fail closed before generation. The route is
intentionally resource-bounded by the
`qatq-segmented-kqv-backend-contract-v1` limits above so a future kernel cannot
inherit unbounded page fan-out from the graph bridge.
Use the default per-offloaded-page codec gate when making strict compression
claims. Use `--aggregate-codec-gate` for live-VRAM runs where tiny tail pages
may lose to zstd/lz4 individually but QATQ still shrinks raw pages and beats
general codecs in aggregate.
The adapter also honours `LLAMA_QATQ_TRACE_MIN_OFFLOAD_PAGE_TOKENS`, defaulting
to `16`, before a page can be offloaded. That keeps one-token codec-negative
tail pages resident so runtime traces and codec evidence agree, while still
allowing useful partial-tail pages through the aggregate compression gate.

The page-source traces make this boundary machine-checkable. Current
`page-composed-source` and `persistent-page-source` rows emit
`composition: "ggml_concat"` and `native_page_streaming: false`. The evidence
runner copies those values into `native-page-streaming-status.json` and the
`evidence-summary` row of `tokens.csv`; `--require-native-page-streaming`
rejects the run until a backend-scheduled native page-streaming attention path
reports true. The status separates `segmented_graph_bridge`, which records the
current multi-segment non-K/V-concat graph bridge, from
`backend_scheduled_segmented_attention`, which is the production requirement
for an accelerator-schedulable segmented reduction.

The adapter now also exposes `--qatq-attention-page-segments-trace`, which
enumerates bounded K/V page tensors from the actual `get_k/get_v` attention
read path before page-source composition. In the default page-source proofs
these rows remain API plumbing. Future native evidence must make those rows
report `native_page_streaming: true`, `attention_consumed: true`, and consumer
`ggml_segmented_kqv`. QATQ also requires key and value segment rows to pair by
sequence, layer, `n_kv`, token ranges, native-streaming status,
attention-consumed status, and consumer; mismatched K/V segment ranges fail
closed. The strict native gate also requires the QATQ page-bounded
attention-equivalence report to run and pass for the same evidence bundle, so a
future segmented consumer cannot pass on tracing alone. The current pinned
patch intentionally fails before claiming the native consumer state.

Use `--qatq-native-page-streaming-preflight` when validating the graph-side
segment-pairing boundary for the future native consumer. It asks the actual
llama.cpp attention graph-build path to enumerate K/V page segments at the
consumer boundary, validate their pairing, and emit
`ggml_segmented_kqv_preflight` trace rows. Those rows still report
`native_page_streaming: false` and `attention_consumed: false`; they prove the
boundary shape, not final accelerated live-VRAM attention. The production
evidence gate requires the exact consumer string `ggml_segmented_kqv`; the
`_preflight` suffix is explicitly not accepted as a native consumer.

Use `--qatq-native-page-streaming-contract` only when debugging the lower-level
segmented K/Q/V validation boundary. It runs the geometry checks at the
intended backend insertion point and emits `ggml_segmented_kqv_contract` rows,
but it is no longer the recommended strict evidence path. For native evidence,
use `--qatq-native-page-streaming-attention-backend-op` or the QATQ evidence
runner's `--native-page-streaming-attention-backend-op` flag so the runtime
actually drives the backend-scheduled segmented attention op.

```sh
python3 scripts/llama_cpp_live_vram_evidence.py \
  --llama-simple /path/to/patched/llama-simple \
  --llama-cpp-source /path/to/patched/llama.cpp \
  --model /path/to/model.gguf \
  --work-dir /tmp/qatq-native-contract-probe \
  --native-page-streaming-contract-probe
```

The probe writes `native-page-streaming-contract-probe.json` and remains a
fail-closed readiness check, not native live-VRAM evidence. Do not use it in
place of the backend-op evidence route.

The patch also contains the compile-tested `qatq_validate_segmented_kqv_contract`
helper used by that flag. It validates K/V segment pairing, query/key head
dimensions, K/V dtype consistency, token extents, and unsupported
KQ-bias/sink/MLA combinations before the fail-closed backend boundary. Treat
this as implementation scaffolding, not native production evidence.

For a repeatable multi-model evidence pass, use the matrix runner:

```sh
python3 scripts/llama_cpp_live_vram_matrix.py \
  --config adapters/llama-cpp/live-vram-matrix.local.example.json \
  --llama-simple /path/to/patched/llama-simple \
  --work-dir /tmp/qatq-live-vram-matrix \
  --iterations 2
```

The matrix config is intentionally explicit about local model paths, prompts,
and layer frontiers. The runner executes each case through the same strict
per-case evidence runner, writes a per-case command log, and emits an aggregate
`summary.md`. The summary records the active evidence gate and event trace
coverage for each case. It returns non-zero if any case fails unless
`--allow-failures` is set for exploratory work. Use `--iterations` for repeated
soak evidence; every iteration must pass the full per-case gate. Add
`--prune-bulk-artifacts` when running broad local stress matrices to keep the
aggregate summaries, CSVs, JSON reports, and logs while deleting the bulky
per-run export directories after each case. Use
`--override-short-predict <n>`, `--min-token-latency-samples`,
`--max-mixed-token-p95-regression-ratio`, and
`--max-mixed-token-p99-regression-ratio` when you want the matrix to fail closed
unless the short full-GPU and mixed-KV runs contain enough token samples and the
mixed path stays within the configured p95/p99 per-token decode regression
budget. The single evidence runner and the matrix runner both support
`--deep-latency-baseline`, `--min-deep-token-latency-samples`,
`--max-deep-mixed-token-p95-regression-ratio`, and
`--max-deep-mixed-token-p99-regression-ratio` for the stricter long-context
gate; that path runs a deep full-GPU baseline, compares deep full-GPU output to
deep mixed-KV output, writes `deep-latency-gate.json`, and then enforces p95/p99
against generated-token `deep-mixed-kv` rows. Use case-level
`max_queued_pages` when tuning long-context profiles so the matrix can keep
restore/staging pressure bounded while still measuring reclaim.
Matrix configs may also include a top-level `matrix_gates` object. The runner
merges those gates into the CLI defaults unless `--ignore-config-gates` is
used, and rejects unknown gate names so a typo cannot silently weaken a
production-shaped validation run. The long-context example uses this to require
32 short/deep generated-token samples, p95/p99 latency budgets, stable
reclaim/codec bytes, and 1 GiB of page-touched host-memory pressure.
For process-level runtime contention, wrap any matrix config with the parallel
stress runner:

```sh
python3 scripts/llama_cpp_live_vram_parallel_stress.py \
  --config adapters/llama-cpp/live-vram-native-breadth.local.example.json \
  --llama-simple /path/to/patched/llama-simple \
  --work-dir /tmp/qatq-live-vram-parallel-stress \
  --jobs 2 \
  --iterations 2 \
  --job-timeout 1320 \
  --require-live-paging \
  --require-native-page-streaming \
  --native-page-streaming-attention-backend-op \
  --aggregate-codec-gate \
  --gpu-page-staging \
  --prune-bulk-artifacts
```

The wrapper shards the matrix into one-case jobs, runs those jobs concurrently,
captures stdout/stderr per job, and fails the aggregate if any child matrix
fails or exceeds its outer wall-clock timeout. `--job-timeout 0` derives
`--timeout + 120`, so the wrapper has its own fail-closed bound even if a child
matrix process stalls before its internal timeout handling reports. Timed-out
child matrices run in their own process group; the wrapper sends group
`SIGTERM`, escalates to group `SIGKILL` if needed, and records the cleanup
signal in JSON and Markdown summaries. Treat this as real patched-runtime
process-level pressure evidence. It does not replace broader multi-request
burn-in inside one shared server runtime.

For fail-closed process-abort evidence on the currently wired `llama-simple`
adapter path, use the abort probe:

```sh
python3 scripts/llama_cpp_live_vram_abort_probe.py \
  --llama-simple /path/to/patched/llama-simple \
  --model /path/to/model.gguf \
  --work-dir /tmp/qatq-live-vram-abort-probe \
  --n-predict 1024 \
  --kv-gpu-layers 4 \
  --page-tokens 1024 \
  --current-token 2048 \
  --hot-window-tokens 1024 \
  --prefetch-window-tokens 32 \
  --max-queued-pages 32
```

The probe waits for a real QATQ KV export marker, interrupts generation, and
fails if normal completion artifacts such as the output manifest or token
timings appear after abort. The Qwen2.5 1.5B run at
`/private/tmp/qatq-live-vram-abort-probe-qwen15b-20260625` observed the export,
interrupted the process with return code `-2`, retained 57 exported files plus
event/page-segment traces, and wrote no completion artifacts. Treat this as
process-abort fail-closed evidence, not as in-process server request
cancellation.

For scoped in-process server request-cancellation evidence, use the server
probe with conservative release-shaped page sizing:

```sh
python3 scripts/llama_cpp_live_vram_server_cancel_probe.py \
  --llama-server /path/to/patched/llama-server \
  --model /path/to/model.gguf \
  --work-dir /tmp/qatq-live-vram-server-cancel-probe \
  --n-predict 512 \
  --page-tokens 64 \
  --parallel-slots 2 \
  --kv-unified \
  --concurrent-followup-during-cancel \
  --iterations 5 \
  --host-memory-pressure-mib 1024 \
  --max-server-rss-growth-mib 1024 \
  --max-iteration-seconds 15 \
  --max-followup-seconds 10 \
  --prefetch-window-tokens 32 \
  --max-queued-pages 32 \
  --require-flattened-flash-consumer \
  --require-live-offloaded-stream-count 2
```

The probe starts `llama-server` in its own session and shuts down the full
process group. `summary.json` records `shutdown_cleanup.attempted`,
`shutdown_cleanup.signal`, `shutdown_cleanup.escalated`, and the final
`process_returncode`, so failed or timed-out probes do not silently leave child
runtime processes behind.

For repeatable three-model-family validation, run the checked-in sequential
matrix wrapper instead of maintaining hand-written probe commands:

```sh
python3 scripts/llama_cpp_live_vram_server_cancel_matrix.py \
  --config adapters/llama-cpp/live-vram-server-strict.local.example.json \
  --llama-server /path/to/patched/llama-server \
  --work-dir /tmp/qatq-live-vram-server-cancel-strict-matrix \
  --timeout 1800
```

The matrix runs cases sequentially to avoid hidden GPU-memory fan-out, preserves
each probe's artifacts under its own case directory, writes
`server-cancel-matrix-plan.json`, `summary.json`, and `summary.md`, and fails
closed on the first failing probe. Use `--dry-run` to verify the command shape
without starting `llama-server`.

The first strict matrix run at
`/private/tmp/qatq-live-vram-server-cancel-strict-matrix-20260625` passed all
three cases: Qwen2.5 1.5B 10/10, Qwen2.5 3B 5/5, and Phi 3.5 mini 3/3. It
kept the flattened Flash consumer and two live-offloaded stream-index gates
enabled for every case, recording 17,368, 8,768, and 5,328 live-offloaded
segments respectively.

The probe also accepts `--max-retained-page-pool-mib`; when left at the default
`0`, it derives the retained tiled page-pool ceiling from
`max(1024 MiB, --max-server-rss-growth-mib)` and exports
`LLAMA_QATQ_ATTENTION_PERSISTENT_PAGE_SOURCE_MAX_RETAINED_BYTES` for the patched
server. This avoids accepting a high RSS envelope while silently keeping the old
1 GiB retained-pool default. The current retained-budget strict matrix at
`/private/tmp/qatq-live-vram-server-strict-retained-budget-current-20260625`
passed all three cases: Qwen2.5 1.5B 10/10 with p50 predicted throughput
77.84 tok/s and 14,808 live-offloaded segments, Qwen2.5 3B 5/5 with
43.98 tok/s and 7,488 live-offloaded segments, and Phi 3.5 mini 3/3 with
33.13 tok/s and 5,328 live-offloaded segments.

For extended context and page-size coverage, run:

```sh
python3 scripts/llama_cpp_live_vram_server_cancel_matrix.py \
  --config adapters/llama-cpp/live-vram-server-extended.local.example.json \
  --llama-server /path/to/patched/llama-server \
  --work-dir /tmp/qatq-live-vram-server-cancel-extended-matrix \
  --timeout 1800
```

The first extended matrix at
`/private/tmp/qatq-live-vram-server-cancel-extended-matrix-20260625` passed
Qwen2.5 1.5B at 16,384 context for 10/10 iterations and Phi 3.5 mini with
128-token pages for 5/5 iterations. Both cases kept the strict flattened Flash
consumer and two live-offloaded stream-index gates enabled.

The current retained-budget rerun at
`/private/tmp/qatq-live-vram-server-extended-retained-budget-current-20260625`
passed the same extended wrapper with current binaries. Qwen2.5 1.5B at 16,384
context passed 10/10 with p50 predicted throughput 77.37 tok/s, p95/p99
iteration latency 13.86s/13.86s, p95/p99 follow-up latency 2.56s/2.56s, and
32,728 live-offloaded segments. Phi 3.5 mini with 128-token pages passed 5/5
with 33.95 tok/s p50 predicted throughput, p95/p99 iteration latency
24.58s/24.58s, p95/p99 follow-up latency 5.14s/5.14s, and 7,288
live-offloaded segments.

For native-server comparison, use the baseline matrix:

```sh
python3 scripts/llama_cpp_live_vram_server_cancel_matrix.py \
  --config adapters/llama-cpp/live-vram-server-baseline.local.example.json \
  --llama-server /path/to/patched/llama-server \
  --work-dir /tmp/qatq-live-vram-server-cancel-baseline-compare \
  --timeout 1800
```

The baseline case uses `--native-baseline`, which disables all QATQ live-VRAM
environment variables and intentionally skips page-trace gates. The paired
QATQ case keeps the strict flattened Flash and two-stream live-offload gates.
The first three-model comparison at
`/private/tmp/qatq-live-vram-server-cancel-baseline-multimodel-20260625`
passed all six cases. QATQ/native p95 iteration ratios were 1.246x for Qwen2.5
1.5B, 1.163x for Qwen2.5 3B, and 1.150x for Phi 3.5 mini. Follow-up p95
ratios were 1.680x, 1.494x, and 1.370x respectively. Every QATQ case kept the
strict flattened Flash and two-stream live-offload gates enabled with 8,768
live-offloaded segments. Treat this as a shared-server latency/RSS baseline,
not a peak-VRAM or tokens/sec benchmark.

The current retained-budget baseline rerun at
`/private/tmp/qatq-live-vram-server-baseline-retained-budget-current-20260625`
passed all six native/QATQ cases and added predicted-token throughput ratios.
QATQ/native predicted-token p50 ratios were 0.903x for Qwen2.5 1.5B, 0.938x
for Qwen2.5 3B, and 0.880x for Phi 3.5 mini. Iteration p95 ratios were 1.153x,
1.105x, and 1.727x; follow-up p95 ratios were 1.217x, 1.132x, and 1.607x. RSS
growth ratios were 4.547x, 4.675x, and 12.786x respectively, so Phi remains the
main memory-overhead target.

The follow-up Phi policy sweep at
`/private/tmp/qatq-live-vram-server-phi-policy-current-20260625` tested fewer
QATQ-routed layers with 128-token pages. The best strict candidate,
`phi35-mini-qatq-l1-p128-strict`, passed 5/5 with p50 predicted throughput
37.12 tok/s, 1,822 live-offloaded segments, 142 flattened Flash consumers, a
QATQ/native predicted-token p50 ratio of 0.949x, iteration p95 ratio 1.469x,
follow-up p95 ratio 1.355x, and RSS-growth ratio 4.161x. The checked-in strict
matrix now uses that conservative Phi policy; the rerun at
`/private/tmp/qatq-live-vram-server-strict-backend-memory-current-20260625`
passed all three model cases with per-case backend memory ceilings and records
llama.cpp/Metal backend memory in the matrix summary: Qwen2.5 1.5B passed
10/10 at 78.30 tok/s p50 with
backend self/KV/compute 1,426/192/299 MiB, Qwen2.5 3B passed 5/5 at
43.86 tok/s with 2,391/256/300 MiB, and Phi 3.5 mini passed 3/3 at
36.33 tok/s with 5,339/2,976/134 MiB. This is backend allocator diagnostic
evidence, not a hardware-counter peak-VRAM replacement.

For scoped prompt-breadth evidence, use
`live-vram-server-mixed-prompts.local.example.json`. The current run at
`/private/tmp/qatq-live-vram-server-mixed-prompts-current-20260625` passed
three Qwen2.5 1.5B prompt classes, daily-driver handover, software engineering
review, and retrieval-heavy incident memory, with the same strict flattened
Flash, two-stream, backend-memory diagnostic, and backend-memory ceiling gates.
All three cases completed 3/3 iterations with empty `gate_failures`.

For scoped model-plus-prompt breadth evidence, use
`live-vram-server-mixed-model-prompts.local.example.json`. The current run at
`/private/tmp/qatq-live-vram-server-mixed-model-prompts-current-20260625`
passed Qwen2.5 1.5B daily-driver, Qwen2.5 3B software-engineering, and
Phi 3.5 mini operations-incident prompt classes with the same strict gates. All
three cases completed 3/3 iterations with empty `gate_failures`.

For the longer scoped mixed-model prompt soak, use
`live-vram-server-mixed-model-soak.local.example.json`. The current run at
`/private/tmp/qatq-live-vram-server-mixed-model-soak-current-20260625`
passed the same Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini prompt classes for
10/10 iterations each with strict flattened Flash, two-stream,
backend-memory diagnostic, and backend-memory ceiling gates. All
`gate_failures` arrays were empty.

The probe also parses `llama-server` non-streaming `/completion` timing fields
when available and carries predicted-token throughput into the matrix summary.
A focused Qwen2.5 1.5B timing smoke at
`/private/tmp/qatq-live-vram-server-timing-smoke-20260625` passed the native
and QATQ pair: native predicted-token p50 was 89.31 tok/s, QATQ predicted-token
p50 was 51.43 tok/s, and QATQ/native predicted-token p50/p95 ratios were
0.576x/0.601x. The same run reported iteration p95 ratio 1.235x, follow-up p95
ratio 1.731x, RSS-growth ratio 4.119x, and 8,768 QATQ live-offloaded segments.
This is useful throughput instrumentation, but it is not yet an acceptable
production-performance result.

That first timing smoke exposed an allocation-policy issue: with QATQ page
staging enabled, non-selected KV layers were left as canonical CPU KV instead
of staying on llama.cpp's native GPU KV path. The adapter patch now keeps
non-selected layers on native GPU KV and routes only selected QATQ layers
through canonical CPU KV plus accelerator page staging. The strict
trace-enabled rerun at
`/private/tmp/qatq-live-vram-server-timing-smoke-gpufallback-20260625` passed
with native predicted-token p50 88.02 tok/s, QATQ predicted-token p50
77.97 tok/s, QATQ/native predicted-token p50/p95 ratios 0.886x/0.882x,
iteration p95 ratio 1.153x, follow-up p95 ratio 1.227x, and 7,488
live-offloaded segments with 528 flattened Flash consumers. This supersedes
the earlier 0.576x timing-smoke ratio as the current strict shared-server
datapoint.

For Qwen2.5 1.5B queue-depth tuning, use:

```sh
python3 scripts/llama_cpp_live_vram_server_cancel_matrix.py \
  --config adapters/llama-cpp/live-vram-server-queue-depth.local.example.json \
  --llama-server /path/to/patched/llama-server \
  --work-dir /tmp/qatq-live-vram-server-queue-depth \
  --timeout 1800
```

The first queue-depth run at
`/private/tmp/qatq-live-vram-server-queue-depth-20260625` passed native plus
QATQ q8/q16/q32 candidates. q8 had the best QATQ iteration-p95 ratio at
1.247x and the fewest live-offloaded segments at 3,328; q32 had similar
latency at 1.256x and the lowest RSS-growth ratio at 4.156x. q16 was a clear
outlier on this run with 1.593x iteration-p95 and 2.891x follow-up-p95 ratios.
The timing-enabled rerun at
`/private/tmp/qatq-live-vram-server-queue-depth-timing-20260625` passed the
same four-case matrix. Native predicted-token p50 was 87.04 tok/s; q8, q16,
and q32 recorded 50.37 tok/s, 49.86 tok/s, and 50.59 tok/s respectively.
QATQ/native predicted-token p50 ratios were 0.579x, 0.573x, and 0.581x. This
suggests queue depth alone is not the main throughput bottleneck for this
server-cancellation workload.

For an exploratory no-trace selected-layer throughput sweep, use:

```sh
python3 scripts/llama_cpp_live_vram_server_cancel_matrix.py \
  --config adapters/llama-cpp/live-vram-server-layer-sweep-notrace.local.example.json \
  --llama-server /path/to/patched/llama-server \
  --work-dir /tmp/qatq-live-vram-server-layer-sweep-notrace \
  --timeout 1800
```

After the native-GPU fallback fix, the exploratory sweep was rerun from the clean
bootstrapped `llama-server` with explicit backend-memory and projected
device-memory ceilings. The run at
`/private/tmp/qatq-live-vram-server-layer-sweep-bootstrap-notrace-projected-gated-20260625`
passed native plus QATQ l0/l1/l2/l4, five cancellation/follow-up iterations
per case, 1 GiB host-memory pressure, and empty `gate_failures`. QATQ/native
predicted-token p50 ratios were 0.956x, 0.923x, 0.884x, and 0.858x
respectively. The l4 case reduced backend KV memory from 224 MiB to 192 MiB
and projected device memory from 1458 MiB to 1426 MiB, with
iteration/follow-up p95 ratios of 1.052x and 1.159x. RSS-growth ratios were
1.002x, 1.892x, 2.720x, and 4.152x, so the selected-layer route is now much
healthier on throughput while host RSS and direct peak-VRAM proof remain open.

For the current accepted low-overhead no-trace policy, use the separate l1
policy config:

```sh
python3 scripts/llama_cpp_live_vram_server_cancel_matrix.py \
  --config adapters/llama-cpp/live-vram-server-layer-policy-notrace.local.example.json \
  --llama-server /path/to/patched/llama-server \
  --work-dir /tmp/qatq-live-vram-server-layer-policy-notrace \
  --timeout 1800
```

The stricter comparison-gated exploratory sweep at
`/private/tmp/qatq-live-vram-server-layer-sweep-bootstrap-notrace-comparison-gated-20260625`
rejected the deeper l2/l4 candidates on latency, throughput, and host-RSS
policy. The accepted l1 policy therefore runs one required warmup
cancellation/follow-up cycle, then gates five measured steady-state cycles. The
bootstrapped policy proof at
`/private/tmp/qatq-live-vram-server-layer-policy-bootstrap-notrace-warmup-gated-20260625`
passed native plus QATQ l1 under 1 GiB host-memory pressure. QATQ/native
predicted-token p50 and p95 ratios were 0.956x and 0.965x, iteration/follow-up
p95 ratios were 1.014x and 1.028x, and steady-state RSS-growth ratio was
1.279x. Backend KV memory dropped from 224 MiB to 216 MiB and projected device
memory from 1458 MiB to 1450 MiB. The first warmup cycle is still reported
separately: QATQ grew 59.44 MiB during lazy initialisation versus 6.16 MiB
steady-state growth across the measured cycles.

For the current three-model accepted-policy breadth proof, use:

```sh
python3 scripts/llama_cpp_live_vram_server_cancel_matrix.py \
  --config adapters/llama-cpp/live-vram-server-family-policy-notrace.local.example.json \
  --llama-server /path/to/patched/llama-server \
  --work-dir /tmp/qatq-live-vram-server-family-policy-notrace \
  --timeout 2400
```

The bootstrapped run at
`/private/tmp/qatq-live-vram-server-family-policy-bootstrap-notrace-warmup-gated-20260625`
passed all six native/QATQ cases across Qwen2.5 1.5B, Qwen2.5 3B, and
Phi 3.5 mini with backend-memory diagnostics, one warmup cycle, five measured
steady-state cycles, 1 GiB host-memory pressure, and global comparison gates.
Qwen2.5 1.5B recorded QATQ/native throughput ratios of 0.972x/0.970x,
latency ratios of 1.041x/1.043x, RSS-growth ratio 0.940x, and backend KV
224->216 MiB. Qwen2.5 3B recorded 0.980x/0.992x, 1.003x/1.017x, RSS 0.955x,
and backend KV 288->280 MiB. Phi 3.5 mini with l1 and 128-token pages recorded
0.983x/0.975x, 1.011x/1.219x, RSS 1.447x, and backend KV 3072->2976 MiB. Phi
still has the largest host-pressure shape, so longer Phi soaks and direct
peak-VRAM measurement remain production gates.

For the longer 10-cycle accepted-policy soak, use:

```sh
python3 scripts/llama_cpp_live_vram_server_cancel_matrix.py \
  --config adapters/llama-cpp/live-vram-server-family-policy-soak-notrace.local.example.json \
  --llama-server /path/to/patched/llama-server \
  --work-dir /tmp/qatq-live-vram-server-family-policy-soak-notrace \
  --timeout 3600
```

The first soak rejected the default Qwen2.5 3B q32 queue policy on follow-up
p95 and steady-state RSS ratio. A targeted q8 Qwen2.5 3B candidate removed the
tail spike, so the accepted family configs initially moved to q8 for Qwen2.5
3B QATQ. The
corrected bootstrapped soak at
`/private/tmp/qatq-live-vram-server-family-policy-soak-bootstrap-notrace-warmup-gated-20260625`
passed all six native/QATQ cases for 10 measured cycles each. Qwen2.5 1.5B
recorded throughput ratios 0.981x/0.984x, latency ratios 1.008x/1.028x, RSS
0.941x, and backend KV 224->216 MiB. Qwen2.5 3B q8 recorded
0.943x/0.958x, 1.068x/1.078x, RSS 1.637x, and backend KV 288->280 MiB.
Phi 3.5 mini recorded 1.003x/0.999x, 0.889x/0.747x, RSS 1.372x, and backend
KV 3072->2976 MiB.

The same accepted soak now enforces a steady-state RSS tail-growth gate:
`max_rss_tail_growth_kib: 8192` over the last four measured iterations. The
tail-gated bootstrapped run at
`/private/tmp/qatq-live-vram-server-family-policy-soak-bootstrap-notrace-tail-gated-20260625`
passed all six cases with 0 KiB tail RSS growth in every case. Qwen2.5 1.5B
recorded QATQ/native throughput ratios 0.977x/0.956x, latency ratios
1.012x/1.022x, RSS 1.345x, and backend KV 224->216 MiB. Qwen2.5 3B q8
recorded 1.012x/1.004x, 0.978x/0.801x, RSS 0.594x, and backend KV
288->280 MiB. Phi 3.5 mini recorded 0.989x/0.980x, 0.995x/1.027x,
RSS 1.461x, and backend KV 3072->2976 MiB. This proves the scoped 10-cycle
policy settles after warmup, but overnight burn-in and direct peak-VRAM
hardware counters are still open production gates.

A longer three-repeat follow-up at
`/private/tmp/qatq-live-vram-server-family-policy-soak-burnin3-taildelta-security-gated-20260625`
failed closed under slower host conditions: Qwen2.5 3B q8 missed the accepted
p50 throughput ratio, and Phi native exceeded the steady RSS tail gate before
its QATQ pair could run. Focused Qwen2.5 3B queue/page probes then showed q4
with 64-token pages fixed p50 but still missed p95, and q2 with 64-token pages
passed only in isolation. The full-family q2 burn-in at
`/private/tmp/qatq-live-vram-server-family-policy-soak-burnin2-q2-taildelta-security-gated-20260625`
failed on the second repeat with Qwen2.5 3B p50/p95 throughput ratios
0.630x/0.678x. 128-token pages with q4 then passed in isolation but failed the
corrected full-family p05/p50 rerun. The strongest focused candidate is now
256-token pages with q4:
`/private/tmp/qatq-live-vram-server-qwen3-p256-q4-focused-p05-20260626`, which
passed with p05/p50/p95 throughput ratios 1.595x/1.075x/0.980x, p95
iteration/follow-up ratios 0.773x/0.628x, backend K/V 288->280 MiB, projected
device memory 2423->2415 MiB, and zero RSS tail gate growth. The checked-in
family configs carry that candidate. The full-family three-repeat burn-in at
`/private/tmp/qatq-live-vram-server-family-policy-soak-burnin3-p256q4-p05-tailgate-20260626`
then passed 3/3 repeats, 18 real server cases total, with empty comparison and
aggregate gate failures. Backend K/V and projected-device jitter ratios were
1.0 for every native and QATQ case. Qwen2.5 3B QATQ/native p05/p50/p95
throughput ratios stayed inside policy in every repeat; the weakest repeat
recorded 0.929x/0.945x/0.959x, while backend K/V stayed 288->280 MiB and
projected device memory stayed 2423->2415 MiB. This supersedes the earlier
two-repeat evidence at
`/private/tmp/qatq-live-vram-server-family-policy-soak-burnin2-p256q4-p05-tailgate-20260626`.
The superseded p128/q4 rerun at
`/private/tmp/qatq-live-vram-server-family-policy-soak-burnin2-p128q4-p05-tailgate-20260626`
still failed because Qwen2.5 3B missed p05/p50 at 0.832x/0.775x.

The current accepted soak also fails closed unless QATQ reduces backend K/V
memory versus native (`max_backend_accelerator_context_ratio: 0.99`) and does
not regress projected device memory (`max_projected_device_memory_ratio: 1.0`).
The device-memory-gated bootstrapped run at
`/private/tmp/qatq-live-vram-server-family-policy-soak-bootstrap-notrace-device-gated-20260625`
passed all six cases with empty comparison gate failures. Backend K/V ratios
were 0.964x for Qwen2.5 1.5B, 0.972x for Qwen2.5 3B, and 0.969x for
Phi 3.5 mini. Projected device-memory ratios were 0.995x, 0.997x, and 0.982x.
This is the strongest current accepted policy proof from llama.cpp backend
diagnostics, but direct hardware peak-VRAM counters remain a separate
production gate.

For bounded repetition burn-in, wrap any accepted server matrix with:

```sh
python3 scripts/llama_cpp_live_vram_server_burnin.py \
  --config adapters/llama-cpp/live-vram-server-family-policy-notrace.local.example.json \
  --llama-server /path/to/patched/llama-server \
  --work-dir /tmp/qatq-live-vram-server-family-policy-burnin \
  --runs 2 \
  --max-cases 2 \
  --timeout 1800 \
  --run-timeout 2400 \
  --max-backend-kv-jitter-ratio 1.001 \
  --max-projected-device-jitter-ratio 1.001
```

The runner repeats the matrix, fails on the first failed run, starts each matrix
run in its own process group, and records `timed_out`, `cleanup_signal`,
`cleanup_escalated`, and `timeout_seconds` for every run. A matrix timeout
therefore tears down nested probe/server children instead of leaving live
runtime processes behind. The runner can also enforce aggregate jitter gates
across repeated case metrics. The server probe's
steady-state tail gate now fails on positive RSS tail growth rather than raw
tail range, so a process that returns memory during the tail window is reported
as volatile but is not rejected as leaking. The accepted comparison gate uses
positive QATQ RSS tail-growth delta over native, which remains defined when
native tail growth is exactly flat. The matrix and burn-in summaries still
report `rss_tail_growth_kib`, `rss_tail_range_kib`, tail ratio, and tail delta.

The initial layer-policy burn-in at
`/private/tmp/qatq-live-vram-server-layer-policy-burnin2-device-jitter-20260625`
failed correctly on the existing RSS-growth ratio gate (`2.401x > 2.0x`),
showing that the wrapper does not mask policy regressions. The scoped accepted
family-policy burn-in at
`/private/tmp/qatq-live-vram-server-family-policy-soak-burnin2-taildelta-security-gated-20260625`
then repeated the full accepted policy twice over Qwen2.5 1.5B, Qwen2.5 3B,
and Phi 3.5 mini native/QATQ pairs. It passed all twelve real matrix cases with
empty comparison and aggregate gate failures. Backend K/V and projected-device
jitter ratios were 1.0 for every native and QATQ case. QATQ/native backend K/V
ratios stayed at 0.964x for Qwen2.5 1.5B, 0.972x for Qwen2.5 3B, and 0.969x
for Phi 3.5 mini; projected-device ratios stayed at 0.995x, 0.997x, and
0.982x. QATQ/native p50 throughput ratios were 0.967x/0.973x for Qwen2.5
1.5B, 0.987x/0.986x for Qwen2.5 3B, and 0.977x/0.952x for Phi across the two
repeats. The largest positive QATQ RSS tail delta over native was 1712 KiB
against the 2048 KiB comparison gate. This is the current strongest
accepted-policy burn-in artefact, while
overnight burn-in and direct peak-VRAM hardware counters remain open production
gates.

To record whether the host can provide direct peak-VRAM hardware counters for
a completed matrix, run:

```sh
python3 scripts/llama_cpp_live_vram_hardware_counters.py \
  --matrix-summary /tmp/qatq-live-vram-server-family-policy/summary.json \
  --output /tmp/qatq-live-vram-server-family-policy/hardware-counters.json
```

On NVIDIA hosts, run the same helper while the target `llama-server` process is
alive to capture sampled per-process peak GPU memory:

```sh
python3 scripts/llama_cpp_live_vram_hardware_counters.py \
  --matrix-summary /tmp/qatq-live-vram-server-family-policy/summary.json \
  --output /tmp/qatq-live-vram-server-family-policy/hardware-counters.json \
  --sample-pid "$LLAMA_SERVER_PID" \
  --sample-seconds 30 \
  --sample-interval-ms 100 \
  --require-direct-peak-vram
```

This gate only passes when `nvidia-smi` reports `pid,used_memory` samples for
the requested process. Detecting `nvidia-smi` on `PATH` is not enough.

For live `llama-server` cancellation probes, prefer the integrated sampler so
the counter spans warmup and measured cancellation/follow-up iterations:

```sh
python3 scripts/llama_cpp_live_vram_server_cancel_probe.py \
  --llama-server /path/to/patched/llama-server \
  --model /path/to/model.gguf \
  --sample-direct-peak-vram \
  --require-direct-peak-vram-counter \
  --direct-peak-vram-sample-interval-ms 100
```

The probe stores `direct_peak_vram_counter` in `summary.json` and fails closed
when the required counter cannot be sampled.

Matrix configs can carry the same policy keys:
`sample_direct_peak_vram`, `require_direct_peak_vram_counter`, and
`direct_peak_vram_sample_interval_ms`. Use
`direct_peak_vram_retain_samples` to cap the retained JSON sample array for
long burn-ins; `sample_count` and `peak_memory_mib` remain complete. Use these
keys only for runs on hosts with suitable direct counters; the local Apple Metal
policy examples leave them disabled because `nvidia-smi` is absent.
The server probe also bounds evidence ingestion with `--max-trace-bytes` and
`--max-trace-line-bytes`; matrix configs can set `max_trace_bytes` and
`max_trace_line_bytes` so oversized, malformed, or pathological JSONL traces
fail closed instead of being read into memory or silently ignored during long
soaks.
When native and QATQ cases share a `comparison_group`, the matrix runner can
also enforce `require_direct_peak_vram_counters: 1` and
`max_direct_peak_vram_ratio` under `comparison_gates`, so a burn-in fails if
direct hardware counters are missing or QATQ does not meet the configured
native peak-VRAM ratio. The burn-in wrapper can then enforce repeated-run
stability with `--max-direct-peak-vram-jitter-ratio`.

The report is deliberately fail-closed if `--require-direct-peak-vram` is set.
On the current Apple Metal host, the report at
`/private/tmp/qatq-live-vram-server-family-policy-soak-burnin3-p256q4-p05-tailgate-20260626/hardware-counters.json`
confirms that all six cases in the latest accepted burn-in repeat have
llama.cpp backend projected-device and accelerator-breakdown diagnostics, but
direct peak-VRAM counters are not available through non-root host tooling.
`nvidia-smi` is not present on this host; `powermetrics` requires superuser and
reports per-process GPU time rather than peak GPU memory; `vmmap` reports
virtual memory maps. Do not treat backend projected memory, backend K/V ratio
gates, or RSS gates as direct hardware peak-VRAM proof.

For the current direct selected-layer memory-accounting proof, use:

```sh
python3 scripts/llama_cpp_live_vram_matrix.py \
  --config adapters/llama-cpp/live-vram-native-layer-memory.local.example.json \
  --llama-simple /path/to/patched/llama-simple \
  --qatq-kv-bench target/release/qatq-kv-bench \
  --work-dir /tmp/qatq-live-vram-native-layer-memory \
  --iterations 1 \
  --require-live-paging \
  --require-native-page-streaming \
  --aggregate-codec-gate \
  --require-stable-reclaim \
  --require-stable-qc-bytes \
  --host-memory-pressure-mib 1024 \
  --prune-bulk-artifacts \
  --timeout 1800
```

The run at
`/private/tmp/qatq-live-vram-native-layer-memory-gpufallback-pressure1g-repeat2-20260625`
passed that gate twice under 1 GiB touched host-memory pressure for Qwen2.5
1.5B with four QATQ-routed layers, 168/168 exact restores per iteration,
112/112 offloaded QATQ pages per iteration, zero pass-through, 56 resident
pages, and stable 23.00 MiB reclaimed persistent GPU K/V. QATQ stored 25.00
MiB in both iterations versus 28.65 MiB with zstd and 31.12 MiB with lz4; MLX
checked 28 layers and 336 heads. The previous broader 14-layer shape failed
correctly because strict accounting reported 44,040,192 staged bytes against a
36,700,160-byte total K/V context. Treat the four-layer result as the current
selected-layer direct residency proof, not as broad production coverage.

For the three-model selected-layer breadth proof, use:

```sh
python3 scripts/llama_cpp_live_vram_matrix.py \
  --config adapters/llama-cpp/live-vram-native-layer-memory-breadth.local.example.json \
  --llama-simple /path/to/patched/llama-simple \
  --qatq-kv-bench target/release/qatq-kv-bench \
  --work-dir /tmp/qatq-live-vram-native-layer-memory-breadth \
  --iterations 2 \
  --require-live-paging \
  --require-native-page-streaming \
  --aggregate-codec-gate \
  --require-stable-reclaim \
  --require-stable-qc-bytes \
  --host-memory-pressure-mib 1024 \
  --prune-bulk-artifacts \
  --timeout 1800
```

The run at
`/private/tmp/qatq-live-vram-native-layer-memory-breadth-pressure1g-repeat2-20260625`
passed 6/6 cases across Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini under
1 GiB touched host-memory pressure. Every case selected four QATQ-routed
layers, restored all pages exactly, used zero pass-through pages, passed the
strict live-paging/native/MLX gates, and kept stable reclaim plus QATQ byte
counts across two iterations per model. Reclaim and QATQ/zstd/lz4 storage were
23.00 MiB and 25.00/28.65/31.12 MiB for Qwen2.5 1.5B, 52.00 MiB and
49.62/57.42/62.43 MiB for Qwen2.5 3B, and 432.00 MiB and
391.57/448.79/493.38 MiB for Phi 3.5 mini.

The probe starts patched `llama-server`, enables QATQ GPU page staging through
environment variables, opens a streaming `/completion`, closes the client
connection mid-stream, checks `/health`, then sends a follow-up completion to
the same server process. For native multi-stream evidence,
`--require-flattened-flash-consumer` fails unless attention-consumed trace rows
use `backend_scheduled_flattened_flash_attention`, and
`--require-live-offloaded-stream-count 2` fails unless both stream indices carry
live-offloaded page segments. The Qwen2.5 1.5B run at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-releasepages-20260625`
passed: the client disconnected after 256 streamed bytes, the server remained
healthy, a follow-up request completed, and the page-segment trace contained
896 attention-page-segment events with 128 attention-consumed live-offloaded
segments. The probe derives `LLAMA_QATQ_ATTENTION_PAGE_SEGMENTS_MAX_PAGES`,
`LLAMA_QATQ_ATTENTION_PERSISTENT_PAGE_SOURCE_MAX_SOURCE_PAGES`, and
`LLAMA_QATQ_GRAPH_EXTRA_NODES` from the configured context/page policy so
smaller pages keep the same fail-closed bounds without inheriting the
1024-token-page budget. The budgeted 64-token page rerun at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-budgeted-20260625` also
passed: 952 attention-page-segment events, 136 attention-consumed events, 584
live-offloaded segments, healthy recovery, and a successful follow-up
completion. The two-slot unified-KV concurrent run at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-concurrent-kvu-20260625`
also passed: the follow-up request started before stream cancellation, finished
after cancellation, returned 2,560 bytes, recovered health, and recorded 1,120
page-segment events with 1,312 live-offloaded segments. The retained tiled
page-pool route now returns no live QATQ segments for unsupported multi-stream
reserve graphs instead of aborting, while supported one-stream slot work still
uses live page staging. The non-unified two-slot run at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-concurrent-nonunified-20260625`
also passed with follow-up completion after cancellation, health recovery, 1,120
page-segment events, and 544 live-offloaded segments. The probe can run bounded
soak cycles with `--iterations <n>` against the same server process. The
20-cycle non-unified run at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-soak20-20260625`
passed 20/20 iterations with 10,696 page-segment events and 10,728 live-offloaded
segments; the matching unified-KV run at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-kvu-soak20-20260625`
passed 20/20 iterations with 9,632 page-segment events and 24,568 live-offloaded
segments. Pressure-gated reruns with 1 GiB touched host-memory pressure and a
1 GiB RSS-growth ceiling also passed: the non-unified run at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-pressure1g-soak20-20260625`
completed 20/20 iterations with 59.3 MiB RSS growth, and the unified-KV run at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-kvu-pressure1g-soak20-20260625`
completed 20/20 iterations with 44.8 MiB RSS growth. Longer latency-gated
pressure burn-ins with 15s per-iteration and 10s follow-up ceilings also
passed: the non-unified run at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-pressure1g-latency-soak100-20260625`
reported 100/100 iterations, p95 iteration/follow-up latency 4.80s/2.61s,
53,608 live-offloaded segments, and 61.25 MiB RSS growth. The unified-KV run at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-kvu-pressure1g-latency-soak100-20260625`
reported 100/100 iterations, p95 iteration/follow-up latency 4.83s/2.68s,
122,488 live-offloaded segments, and 44.8 MiB RSS growth. Treat this as
bounded shared-server request-cancellation evidence. Follow-up pressure and
model-breadth runs also passed: Qwen2.5 1.5B non-unified/unified 2 GiB
pressure runs completed 50/50 iterations at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-pressure2g-latency-soak50-20260625`
and
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-kvu-pressure2g-latency-soak50-20260625`,
and Qwen2.5 3B non-unified/unified 1 GiB pressure runs completed 20/20
iterations at
`/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-pressure1g-latency-soak20-20260625`
and
`/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-kvu-pressure1g-latency-soak20-20260625`.
The Qwen2.5 3B unified run recorded p95/p99 iteration latency 7.73s/7.73s,
p95/p99 follow-up latency 4.39s/4.51s, 24,568 live-offloaded segments, and
46.41 MiB RSS growth. The adapter now also has a scoped native multi-stream
retained page-table proof for the non-unified two-slot flattened Flash
Attention path. The run at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-multistream-pressure1g-soak20-20260625`
completed 20/20 Qwen2.5 1.5B cancellation/follow-up iterations under 1 GiB
host pressure with explicit `stream_index: 0` and `stream_index: 1` trace
segments, p95/p99 iteration latency 4.74s/4.76s, p95/p99 follow-up latency
2.61s/2.63s, 16,008 live-offloaded segments, and 46.63 MiB RSS growth. A
strict-gated integrated probe rerun at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-strictgates-soak10-ctx8192-20260625`
then passed 10/10 with `--require-flattened-flash-consumer` and
`--require-live-offloaded-stream-count 2`, recording 1,048
`backend_scheduled_flattened_flash_attention` consumer rows, 17,368
live-offloaded segments across stream indices `0` and `1`, p95/p99 iteration
latency 6.44s/6.44s, p95/p99 follow-up latency 2.62s/2.62s, and 106.73 MiB RSS
growth. A
Qwen2.5 3B follow-up at
`/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-native-multistream-pressure1g-soak20-ctx8192-20260625`
also passed 20/20 non-unified two-slot iterations under 1 GiB host pressure
with an 8192-token server context, 33,928 live-offloaded segments, p95/p99
iteration latency 11.19s/11.21s, p95/p99 follow-up latency 4.34s/4.35s, and
111.81 MiB RSS growth. Its consumed rows all used
`backend_scheduled_flattened_flash_attention` and traced both stream indices.
The strict-gated Qwen2.5 3B integrated rerun at
`/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-native-strictgates-soak5-ctx8192-20260625`
passed 5/5 with the same `--require-flattened-flash-consumer` and
`--require-live-offloaded-stream-count 2` gates, recording 568 flattened Flash
consumer rows, 8,768 live-offloaded segments across stream indices `0` and `1`,
p95/p99 iteration latency 11.11s/11.11s, p95/p99 follow-up latency
4.34s/4.34s, and 111.53 MiB RSS growth.
A Phi 3.5 mini follow-up at
`/private/tmp/qatq-live-vram-server-cancel-phi35mini-p64-nonunified-native-multistream-pressure1g-soak20-ctx8192-20260625`
then passed 20/20 with the same stream-split route under 1 GiB host pressure,
33,768 live-offloaded segments, p95/p99 iteration latency 15.81s/15.82s,
p95/p99 follow-up latency 4.77s/4.77s, and 836.23 MiB RSS growth after the
probe started including per-iteration RSS peaks in the memory gate. Its
consumed rows all used `backend_scheduled_flattened_flash_attention` and traced
both stream indices with shape `[96,32,64,1]`.
The strict-gated Phi 3.5 mini rerun at
`/private/tmp/qatq-live-vram-server-cancel-phi35mini-p64-nonunified-native-strictgates-soak3-ctx8192-20260625`
passed 3/3 with the same strict gates, recording 376 flattened Flash consumer
rows, 5,328 live-offloaded segments across stream indices `0` and `1`, p95/p99
iteration latency 15.99s/15.99s, p95/p99 follow-up latency 4.99s/4.99s, and
836.47 MiB RSS growth.
A longer Qwen2.5 1.5B native multi-stream burn-in at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-multistream-pressure1g-soak100-ctx8192-20260625`
passed 100/100 under 1 GiB host pressure with 168,968 live-offloaded segments,
p95/p99 iteration latency 6.56s/6.58s, p95/p99 follow-up latency 2.64s/2.65s,
and 108.19 MiB RSS growth across 202 RSS samples. Its consumed rows all used
`backend_scheduled_flattened_flash_attention` and traced both stream indices.
A harsher Qwen2.5 1.5B native multi-stream run at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-multistream-pressure2g-soak50-ctx8192-20260625`
passed 50/50 under 2 GiB host pressure with 84,568 live-offloaded segments,
p95/p99 iteration latency 6.60s/6.61s, p95/p99 follow-up latency 2.66s/2.66s,
and 106.48 MiB RSS growth across 102 RSS samples. Its consumed rows all used
`backend_scheduled_flattened_flash_attention` and traced both stream indices.
A Qwen2.5 3B native multi-stream long soak at
`/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-native-multistream-pressure1g-soak50-ctx8192-20260625`
passed 50/50 under 1 GiB host pressure with 84,568 live-offloaded segments,
p95/p99 iteration latency 11.22s/12.19s, p95/p99 follow-up latency 4.36s/4.91s,
and 114.61 MiB RSS growth across 102 RSS samples. Its consumed rows all used
`backend_scheduled_flattened_flash_attention` and traced both stream indices.
The harsher Qwen2.5 3B 2 GiB pressure rerun at
`/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-native-multistream-pressure2g-soak50-ctx8192-20260625`
also passed 50/50 with 84,568 live-offloaded segments, p95/p99 iteration
latency 11.03s/12.17s, p95/p99 follow-up latency 4.25s/5.48s, and 115.25 MiB
RSS growth across 102 RSS samples. Its consumed rows all used
`backend_scheduled_flattened_flash_attention`, traced both stream indices, and
carried shape `[128,2,64,1]`.
A Phi 3.5 mini native multi-stream long soak at
`/private/tmp/qatq-live-vram-server-cancel-phi35mini-p64-nonunified-native-multistream-pressure1g-soak50-ctx8192-20260625`
passed 50/50 under 1 GiB host pressure with 84,168 live-offloaded segments,
p95/p99 iteration latency 16.40s/18.13s, p95/p99 follow-up latency
5.05s/6.71s, and 832.47 MiB RSS growth across 102 RSS samples. Its consumed
rows all used `backend_scheduled_flattened_flash_attention`, traced both stream
indices, and covered the four configured cold K/V layers with shape
`[96,32,64,1]`.
A focused Phi 3.5 mini page-size variant at
`/private/tmp/qatq-live-vram-server-cancel-phi35mini-p128-nonunified-native-multistream-pressure1g-soak20-ctx8192-20260625`
then passed 20/20 under the same 1 GiB pressure gate with 128-token pages,
16,648 live-offloaded segments, p95/p99 iteration latency 16.16s/16.16s,
p95/p99 follow-up latency 5.00s/5.04s, and 822.64 MiB RSS growth. Its consumed
rows all used `backend_scheduled_flattened_flash_attention`, traced both stream
indices, and carried shape `[96,32,128,1]`.
A Qwen2.5 1.5B long-context stress rerun at
`/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-multistream-pressure1g-soak20-ctx16384-repeat160-20260625`
now passes after fixing an over-strict flattened Flash validation boundary. The
first attempt failed closed with `QATQ segmented KQV backend contract exceeded
max total tokens`, because the old aggregate segmented-backend check summed
both streams against the single-stream total-token cap. The rebuilt server
passed the same 16,384-context, 160-repeat prompt profile for 20/20
cancellation/follow-up iterations under 1 GiB host pressure, with 60,168
live-offloaded segments, p95/p99 iteration latency 9.96s/9.96s, p95/p99
follow-up latency 2.66s/2.68s, and 243.30 MiB RSS growth. Treat this as a
specific long-context crash fix for the native multi-stream server path, not as
unbounded context-length coverage.
Much broader model/runtime burn-in remains a production hardening gate.

A bounded 2026-06-25 Qwen2.5 1.5B smoke at
`/private/tmp/qatq-live-vram-parallel-stress-qwen15b-2x-20260625` passed 2/2.
The broader 16-token breadth stress at
`/private/tmp/qatq-live-vram-parallel-stress-breadth-3case-16tok-20260625`
then passed 3/3 concurrent Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini child
matrices with strict live-paging, native page-streaming, aggregate codec, GPU
page-staging, MLX equivalence, page seals, and pruned artifacts enabled. A
two-token cold-start breadth diagnostic failed the Qwen2.5 3B performance gate
while the same case passed serially, so use the 16-token pass as the current
process-level breadth evidence.
The checked-in local examples now keep the proof reproducible in two
shapes:
`live-vram-native-sustained.local.example.json` covers Qwen2.5 Coder 3B/7B
software-engineering profiles, while `live-vram-native-breadth.local.example.json`
widens the native `ggml_segmented_kqv` path across Qwen2.5 1.5B, Qwen2.5 3B,
and Phi 3.5 mini profiles. `live-vram-native-dtype.local.example.json` extends
that same strict path across Qwen2.5 3B, Qwen2.5 Coder 3B/7B, and Phi 3.5
mini bf16/f32 cache settings.
`live-vram-native-page-size.local.example.json` focuses on native page-size
breadth across 256, 1024, and 2048-token pages for the current Qwen/Qwen-Coder
proof set.
`live-vram-native-long-context-latency.local.example.json` is intentionally a
scoped long-context latency profile for the current low-regression native route.
It exercises the deep full-GPU baseline and deep mixed-KV p95/p99 gates with
four selected KV layers, an explicit 32-token prefetch window, a conservative
32-page offload cap, the flattened Flash Attention route enabled, and
cold-slot reuse for immutable offloaded rows. A fresh 96-page prefetch-active
rerun preserved exactness and compression but missed the deep latency-tail gate,
so treat the 32-page profile as the checked-in low-regression frontier and the
96-page profile as aggressive tuning work.
The bootstrapped repeat at
`/private/tmp/qatq-live-vram-long-context-latency-bootstrap-repeat2-20260625`
passed that checked-in profile twice from the clean pinned checkout under
1 GiB host-memory pressure, with 128 short and 128 deep generated-token samples
per iteration. It restored 504/504 pages per iteration, compressed 32/32 cold
pages, kept zero pass-through pages, reclaimed 137.00 MiB of persistent GPU
K/V, stored QATQ 179.00 MiB versus zstd 218.30 MiB and lz4 237.57 MiB, and
recorded short/deep p95/p99 latency better than the full-GPU baseline in both
iterations. Reproduce it from the bootstrap layout with:

```sh
python3 scripts/llama_cpp_live_vram_matrix.py \
  --llama-simple /private/tmp/qatq-llama-bootstrap-proof/build-qatq/bin/llama-simple \
  --llama-cpp-source /private/tmp/qatq-llama-bootstrap-proof \
  --config adapters/llama-cpp/live-vram-native-long-context-latency.local.example.json \
  --work-dir /private/tmp/qatq-live-vram-long-context-latency-bootstrap-repeat2-20260625 \
  --iterations 2 \
  --require-live-paging \
  --require-native-page-streaming \
  --aggregate-codec-gate \
  --prune-bulk-artifacts \
  --timeout 3600
```

Five/six selected-layer policy runs also pass as breadth evidence with lower
reclaim. Broader model, dtype, page-size, runtime, and longer soak runs remain
the production-complete boundary.
Fresh 2026-06-25 breadth and dtype reruns use the same cold-slot reuse and
minimum-tail guard. `/private/tmp/qatq-live-vram-native-breadth-coldreuse-min16tail-20260625`
passed 3/3 strict real Metal/MLX breadth cases across Qwen2.5 1.5B, Qwen2.5
3B, and Phi 3.5 mini. `/private/tmp/qatq-live-vram-native-dtype-coldreuse-min16tail-20260625`
passed 8/8 bf16/f32 cases across Qwen2.5 Coder 3B, Qwen2.5 3B, Qwen2.5
Coder 7B, and Phi 3.5 mini. Both runs kept strict live-paging, native
page-streaming, aggregate codec, GPU page-staging, MLX equivalence, and 1 GiB
host-memory-pressure gates enabled.
The 2026-06-24 tuning pass showed that `max_queued_pages`, retained-page reuse,
native segmented attention, non-native fallback, and KV-layer frontier changes
do not yet clear the deep p95 latency gate for the Qwen2.5 Coder 3B 6.8k-token
profile. The evidence runner now has `--skip-attention-page-segments-trace` so
follow-up timing diagnostics can separate production token latency from the
native page-segment proof trace. A follow-up allocator fix made the native
page-segment path honour the same scheduler residency predicate as the event
trace, allowing all-layer page staging with cap 96 to reclaim 126.00 MiB, but
the long-context deep p95 regression remained near 2x. A strict 8-token proof
then passed live-paging, native `ggml_segmented_kqv`, and MLX 36-layer/576-head
checks with the same 126.00 MiB reclaim. This is correctness and allocator
evidence, not a long-context latency pass. The rebuilt backend-op route also
passed a strict backend-op breadth matrix with f16/f32 mask support,
source-view page routing, `backend_scheduled_segmented_attention: true`, exact
restore for 576/576 pages, 456/456 QATQ offloaded pages, zero pass-through
pages, MLX equivalence over Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini, and
512 MiB host-memory pressure. The passing cases reclaimed 28.00 MiB,
111.00 MiB, and 480.00 MiB of persistent GPU K/V residency, with QATQ
27.53/90.73/420.46 MiB versus zstd 29.25/98.51/455.42 MiB and lz4
31.12/106.95/493.38 MiB. A fresh strict config-gated backend-op rerun at
`/private/tmp/qatq-live-vram-long-context-config-gated-20260624` reached the
deep mixed-KV stage for the Qwen2.5 Coder 3B 6.8k-token profile, emitted 1,224
page-segment records plus 576 attention-fixture files, then timed out after
2,400 seconds before writing deep mixed-KV token timings or an output manifest.
That is a fail-closed production boundary, not a native long-context pass. The
2026-06-25 selective fast-path rerun then reduced the same class of
long-context backend-op overhead substantially: four cold KV layers improved
from p95 `+756%` before selective routing to p95 `+73.9%`, and the best
one-cold-layer case reached p95 `+19.6%` and p99 `+19.4%` with output
preserved, native status passing, 38 cold backend-op events, 1,330
resident fast-path events, and 3.00 MiB of GPU page staging. Retained page
pools now default to per-layer allocation so selective cold-layer paging does
not reserve every layer's pool unless
`LLAMA_QATQ_NATIVE_PAGE_STREAMING_SHARED_TILED_POOL=1` is explicitly set.
The newer flattened native route then lets eligible one-stream or
stream-split multi-stream page tables fall back into llama.cpp's
backend-scheduled Flash Attention path under
`--qatq-native-page-streaming-flatten-flash`. The current checked-in strict
matrix rerun at
`/private/tmp/qatq-live-vram-long-context-latency-repeat2-current-20260625`
preserved output across two iterations with explicit prefetch scheduling,
restored `504/504` pages per iteration, compressed `32/32` cold pages, kept
zero pass-through pages, consumed cold rows through
`backend_scheduled_flattened_flash_attention`, and cleared the configured
15%/20% short and deep latency gates under 1 GiB host-memory pressure with
deep p95/p99 better than baseline in both iterations. QATQ stored the exported
pages in 179.00 MiB versus zstd 218.30 MiB and lz4 237.57 MiB, while
reclaiming 137.00 MiB of persistent GPU K/V. The retained flattened table removes repeated
full transient-table rebuilds for mixed resident/offloaded windows. The
cold-slot reuse follow-up also removes repeated syncs for immutable
live-offloaded rows after their retained table slot is populated; first use and
mutable tail rows still sync before attention. Earlier four-layer cap-8 and
cap-4 reruns kept exactness, compression, reclaim, and p99 but missed deep p95
at worst `+18.4%` and `+19.2%` before cold-slot reuse; after the fix, cap 8,
16, 32, and 96 all clear the same repeated strict gate. This is a scoped
long-context native pass, not a
production-complete claim across models, dtypes, page sizes, and broader
policies. The custom Metal segmented
K/Q/V path remains a fallback and optimisation target for layouts that cannot
use backend Flash Attention.
The
sealed latency-budget fallback path did pass a longer
128-token run at
`/private/tmp/qatq-live-vram-long-context-runtime-reclaim-sealed-fallback-128tok-r4-20260624`:
432/432 exact restores, 96/96 QATQ offloaded pages, 96/96 metadata seals, zero
pass-through pages, 207.00 MiB GPU K/V before, 0.00 MiB after, deep p95
`+0.43%`, and deep p99 `+13.02%` inside the configured 15%/20% budget. Treat
that as keyed, host-backed runtime-reclaim evidence, not as a strict native
page-streaming attention claim.

For strict native page-granular adapter validation, promote the whole matrix to the
strict live-paging and native page-streaming gates:

```sh
python3 scripts/llama_cpp_live_vram_matrix.py \
  --config adapters/llama-cpp/live-vram-matrix.local.example.json \
  --llama-simple /path/to/patched/llama-simple \
  --work-dir /tmp/qatq-live-vram-matrix \
  --require-live-paging \
  --require-native-page-streaming \
  --native-page-streaming-attention-backend-op \
  --native-page-streaming-flatten-flash \
  --native-page-streaming-attention-ggml \
  --aggregate-codec-gate \
  --host-memory-pressure-mib 512 \
  --gpu-page-staging
```

The current pinned patch can pass this shape for scoped cases when paired with
the backend-op route, but the full sustained, breadth, dtype, page-size, and
long-context local example JSON files remain the production-validation target
set. The dtype example
`adapters/llama-cpp/live-vram-native-dtype.local.example.json` covers bf16/f32
runtime cache pages on Qwen2.5 3B, Qwen2.5 Coder 3B/7B, and Phi 3.5 mini.
For strict native matrix runs, use `--require-native-page-streaming`; the matrix
runner now defaults that gate to `--native-page-streaming-attention-backend-op`
for each case. Add `--native-page-streaming-flatten-flash` when validating the
current low-regression long-context route for eligible Flash Attention layouts.

The current restore-slot pressure reproduction path is the compact backend-op
shape, not the wider diagnostic layer selection. The 2026-06-25 bootstrapped
proof at
`/private/tmp/qatq-live-vram-restore-slot-pressure-bootstrap-20260625-r4` used
the clean pinned checkout at `/private/tmp/qatq-llama-bootstrap-proof`, selected
4 KV GPU layers from a `[1,2,4]` frontier, passed strict live-paging plus strict
native page-streaming, consumed pages through
`backend_scheduled_flattened_flash_attention`, and rejected a real 262,144-byte
Metal key page before allocation against a one-byte restore-slot limit. A
14-layer backend-op attempt correctly failed the staged-byte live-paging gate,
so use the compact shape when reproducing bounded allocation rejection.

When using the reproducible bootstrap script, pass the checkout path through to
the matrix runner because the binary is built under `build-qatq/bin` rather than
the evidence runner's inferred `<checkout>/build/bin` layout:

```sh
python3 scripts/llama_cpp_live_vram_matrix.py \
  --llama-simple /private/tmp/qatq-llama-bootstrap-proof/build-qatq/bin/llama-simple \
  --llama-cpp-source /private/tmp/qatq-llama-bootstrap-proof \
  --config adapters/llama-cpp/live-vram-native-layer-memory-breadth.local.example.json \
  --work-dir /private/tmp/qatq-live-vram-layer-memory-breadth-bootstrap-20260625 \
  --require-live-paging \
  --require-native-page-streaming \
  --aggregate-codec-gate \
  --host-memory-pressure-mib 1024 \
  --prune-bulk-artifacts \
  --timeout 1200
```

That command passed 3/3 compact Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini
strict native cases on 2026-06-25.

Before running expensive GPU matrices, audit the patched llama.cpp source for
the required adapter shape:

```sh
python3 scripts/llama_cpp_live_vram_adapter_audit.py \
  --llama-cpp /path/to/patched/llama.cpp \
  --require-live-paging \
  --require-runtime-security \
  --output /tmp/qatq-live-vram-adapter-audit.json
```

Use `--require-live-paging` when validating a patch that claims transparent
token-page VRAM reduction. Add `--require-runtime-security` for production-
shaped evidence so the same audit also requires lifecycle traceability,
page-staging, backend page self-tests, restore-slot rejection, physical
per-page allocation attestation, and fail-closed unsupported backend-op cases.
The current adapter passes the export checks and now emits attention-read
telemetry from `llama_kv_cache_context::get_k/get_v`. It also passes the backend
page self-test hook through an adapter-visible logical page residency table with
`evict_page` and `restore_page` operations. The current pinned patch also
exposes the page-segment API, the bounded restore-slot pressure self-test, and a
compile-tested multi-segment non-K/V concat `ggml_segmented_kqv` graph bridge,
plus compile-tested Metal kernel source targets and a masked f32/f16/bf16
backend-op route. The audit reports this
distinction explicitly: `page_staging_ready`, `segmented_graph_bridge`,
`backend_scheduled_segmented_attention`, and
`accelerated_runtime_attention_graph`. The current source-level audit now
reports `page_staging_ready: true` and `live_paging_ready: true`: the
backend-op route consumes retained multi-page K/V page tables directly and
uses a byte-budgeted transient page pool when mixed CPU/GPU residency prevents
direct retained-table consumption. Set
`--qatq-native-page-streaming-transient-pool-max-bytes <n>` to fail closed when
the graph-local transient K/V page pool would exceed the configured ceiling.
Local Qwen2.5 1.5B Metal smokes matched the full-GPU baseline generated tokens
for 2-token and 8-token continuations. The latest cleaned focused 8-token smoke
matched `[264, 943, 315, 56287, 429, 5711, 279, 56287]` between full-GPU and
retained page-table backend-op paths, consumed 784 page-segment records through
`backend_scheduled_segmented_attention` at 64-token pages, and reached
31.21 tok/s with explicit token-to-page/token-to-local tables, 1,575 graph
nodes, 58 splits, and a 4.10 MiB Metal compute buffer. That is a real native
route proof, not a production-complete claim. A follow-up 32-token run at
`/private/tmp/qatq-live-vram-retained-table-n32` matched the full-GPU output
manifest exactly, reused 30 graphs, and reached 22.19 ms/token, 45.06 tok/s in
the retained-table eval path after the small-window Metal launch was reduced to
one SIMD-width threadgroup. The latest page-aligned reserve run kept the same
32-token output match, staged `n_kv: 64` as one segment per K/V/layer, reduced
graph nodes to 1,071, lowered CPU compute buffer use to 0.0985 MiB, and reached
21.43 ms/token, 46.65 tok/s with 30 graph reuses. The corresponding full-GPU
baseline remained much faster at 11.37 ms/token, 87.98 tok/s. A page-table
upload cache experiment was rejected after it produced divergent tokens, so
future dirty-page optimisation must be execution-aware. A later retained
flattened Flash Attention route with cold-slot reuse cleared the scoped
four-layer long-context p95/p99 gate for eligible layouts, but production use
still requires broader coverage, repeated latency-tail runs, memory-pressure
burn-in, more aggressive reclaim-policy tuning, and version-pinned maintenance of the
llama.cpp patch.

The audit report now separates required backend-op live-paging failures from
legacy diagnostic compatibility failures. Current patch-file and applied-source
audits pass `--require-live-paging` with `required_live_paging_failures: []`
and `required_page_staging_failures: []`. Non-required failures for older
concat/persistent-source diagnostics are kept visible so the historical proof
paths do not get mistaken for the retained page-table product route.

For a cheap repository-local smoke check of the pinned patch, run:

```sh
python3 scripts/llama_cpp_live_vram_adapter_audit.py \
  --patch-file adapters/llama-cpp/qatq-kv-export-7992aa7c8.patch \
  --output /private/tmp/qatq-live-vram-adapter-patch-audit.json
```

Patch-file mode reports `audit_scope: "patch-snippet"` and
`authoritative_for_native_release: false`. It is useful for quick fail-closed
checks, but it does not replace the applied-source audit and rebuild before any
native production claim.

For live VRAM replay evidence from the manifest, use:

```sh
cargo run --bin qatq-kv-bench -- \
  --live-vram-export-dir captures/llama-kv \
  --live-vram-runtime-commit 7992aa7c8 \
  --live-vram-adapter-version qatq-kv-export-7992aa7c8 \
  --live-vram-model-id qwen2.5-1.5b-instruct:sha256:<model-hash> \
  --live-vram-prefetch-window-tokens 32 \
  --live-vram-page-seal-key-hex <64-hex-secret> \
  --live-vram-require-page-seals \
  --output docs/LLAMA_CPP_LIVE_VRAM_EVIDENCE.json
```

This replay path validates the patched export manifest, rejects unsafe manifest
file paths, converts raw KV files into QATQ live page descriptors, verifies QATQ
restore for every page, and compares page sizes against raw, zstd, and lz4. It
keeps pages resident when their next use is inside hot plus prefetch lead time.
It does not claim GPU VRAM reduction until a runtime hook actually frees and
restores pages during generation.

`qatq-kv-bench --live-vram-proof-gate` requires allocator evidence written by
the runtime manifest. Manual CLI allocator overrides remain useful for
exploratory estimates, but they are not accepted as proof-grade runtime
attestation.

Current Metal-backed exported-KV evidence is summarised in
[`docs/LLAMA_CPP_LIVE_VRAM_GPU_EVIDENCE.md`](../../docs/LLAMA_CPP_LIVE_VRAM_GPU_EVIDENCE.md).
That report records Qwen2.5 1.5B, Qwen2.5 Coder 3B, and Phi 3.5 mini GPU
captures replayed through QATQ with exact restore and raw/zstd/lz4 comparisons.
It also records a native llama.cpp control showing that `--no-kv-offload`
removes the KV context from Metal by placing it on host memory from the start;
that is a baseline for QATQ live paging, not QATQ live paging itself. The
`--qatq-kv-gpu-layers` control is a more granular runtime-adapter step: it can
split KV allocation between Metal and host memory while preserving deterministic
continuations. The 2026-06-24 four-profile matrix selected 24/28 KV GPU layers
for Qwen2.5 1.5B daily-driver, 28/32 for Phi 3.5 mini operations/security,
30/36 for Qwen2.5 Coder 3B software-engineering workhorse, and 24/28 for
Qwen2.5 Coder 7B powerhouse review. Every case preserved output, restored every
exported tensor exactly, beat zstd/lz4, stored every offloaded page through
QATQ with zero pass-through pages, and reduced Metal KV allocation. This
remains whole-tensor placement, not transparent token-page live paging.

The same four-profile matrix was rerun fresh on 2026-06-24 at
`/private/tmp/qatq-live-vram-matrix-attention-20260624` and again passed 4/4.
The deep exports also wrote real attention-path JSONL telemetry: Qwen2.5 1.5B
recorded 672 key/value read events across 28 layers, Phi 3.5 recorded 896
events across 32 layers, Qwen2.5 Coder 3B recorded 936 events across 36 layers,
and Qwen2.5 Coder 7B recorded 784 events across 28 layers. The stricter
`--live-vram-live-paging-gate` correctly rejected those whole-tensor runs
because allocator granularity was still whole-tensor/whole-context and no
token-page GPU reclaim was proven.

A dedicated token-page export pass then ran on Qwen2.5 1.5B with
`--qatq-page-tokens 1024` at
`/private/tmp/qatq-live-vram-page-evidence-1024-20260624`. That pass exported
224 token-range K/V pages, restored 224/224 exactly, stored every offloaded page
through QATQ with zero pass-through pages, and beat zstd/lz4 on 224/224 page
boundaries. The same setup with `--require-live-paging` still failed closed
because that run's allocator evidence remained `whole-tensor`, not `per-page`.

The follow-up hot-window replay at
`/private/tmp/qatq-live-vram-page-end-hot-window-20260624` used
`--page-tokens 1024`, `--hot-window-tokens 1024`, and
`--next-required page-end`. It kept 56 hot-window token pages resident, offloaded
168 colder token pages through QATQ, restored 224/224 pages exactly, and beat
zstd/lz4 on 224/224 page boundaries. This is page-aware exported-KV evidence,
not a transparent live-paging claim.

After the trace scheduler flags were added, the same Qwen2.5 1.5B profile was
rerun at `/private/tmp/qatq-live-vram-page-end-aligned-trace-20260624`. The
event trace now matches the evidence schedule: 224 snapshots, 168 offloads,
168 restores, and 224 attention-use events. The strict live-paging gate then
failed only on the real allocator boundary for that whole-tensor run:
whole-tensor allocation granularity, zero page-level reclaim, and zero
page-level GPU saved ratio.

For repeatable page-size tuning, use:

```sh
python3 scripts/llama_cpp_live_vram_page_size_sweep.py \
  --model /path/to/model.gguf \
  --model-id <stable-model-id> \
  --page-tokens 512,1024,2048 \
  --mixed-kv-gpu-layers 24 \
  --hot-window-tokens 1024 \
  --next-required page-end \
  --work-dir /tmp/qatq-live-vram-page-size-sweep
```

The 2026-06-24 Qwen2.5 1.5B sweep at
`/private/tmp/qatq-live-vram-page-size-sweep-20260624` kept the 512-token
failure visible, passed 1024 and 2048 tokens, and recommended 1024 tokens as
the first experimental page size because it is the smallest size that passed
the full all-pages compression gate in that run.

For strict native page-size breadth over real Metal/MLX cases, use:

```sh
python3 scripts/llama_cpp_live_vram_matrix.py \
  --llama-simple /private/tmp/qatq-llama-bootstrap-proof/build-qatq/bin/llama-simple \
  --llama-cpp-source /private/tmp/qatq-llama-bootstrap-proof \
  --config adapters/llama-cpp/live-vram-native-page-size.local.example.json \
  --work-dir /tmp/qatq-live-vram-page-size-bootstrap \
  --require-live-paging \
  --require-native-page-streaming \
  --aggregate-codec-gate \
  --host-memory-pressure-mib 1024 \
  --prune-bulk-artifacts \
  --timeout 1800
```

The current pinned patch has passed strict native page-size breadth from the
clean bootstrapped checkout. The run at
`/private/tmp/qatq-live-vram-page-size-bootstrap-20260625` passed 5/5
Qwen/Qwen-Coder Metal/MLX cases across 256, 1024, and 2048-token pages with
exact restores, zero pass-through pages, restore-slot pressure checks, native
flattened Flash page consumption, and QATQ beating raw/zstd/lz4 for every case.
The companion Phi config can be run the same way:

```sh
python3 scripts/llama_cpp_live_vram_matrix.py \
  --llama-simple /private/tmp/qatq-llama-bootstrap-proof/build-qatq/bin/llama-simple \
  --llama-cpp-source /private/tmp/qatq-llama-bootstrap-proof \
  --config adapters/llama-cpp/live-vram-native-phi-page-size.local.example.json \
  --work-dir /tmp/qatq-live-vram-phi-page-size-bootstrap \
  --require-live-paging \
  --require-native-page-streaming \
  --aggregate-codec-gate \
  --host-memory-pressure-mib 1024 \
  --prune-bulk-artifacts \
  --timeout 1800
```

`/private/tmp/qatq-live-vram-phi-page-size-bootstrap-20260625` passed 3/3
Phi 3.5 mini Metal/MLX cases across 256, 512, and 1024-token pages with exact
restores, zero pass-through pages, restore-slot pressure checks, native
flattened Flash page consumption, and QATQ beating raw/zstd/lz4 for every case.
Treat these examples as repeatable breadth gates, not as a universal production
claim; longer repetitions, longer contexts, and more runtime/model coverage
remain open.

The patch is deliberately a runtime adapter hook, not part of QATQ's codec
core. If upstream llama.cpp internals move, refresh the patch here and keep the
QATQ benchmark contract unchanged: raw `.f16le`, `.bf16le`, or `.f32le` files
plus a manifest.
