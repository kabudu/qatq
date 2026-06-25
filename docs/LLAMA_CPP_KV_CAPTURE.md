# llama.cpp KV Capture

QATQ runtime KV-cache evidence should use llama.cpp rather than Ollama when the
experiment needs direct internal K/V tensor bytes. Ollama's public API exposes
model input/output behavior, not the live KV cache tensors. llama.cpp keeps the
KV cache in-process and names internal per-layer tensors `cache_k_l%d` and
`cache_v_l%d`, which makes it the better adapter target.

## Capture Contract

A QATQ-compatible llama.cpp adapter should export one raw little-endian tensor
file per K/V cache tensor:

| field | requirement |
| --- | --- |
| tensor names | Preserve layer and kind, such as `cache_k_l12` and `cache_v_l12`. |
| dtype | Export native `f16`, `bf16`, or `f32`; do not widen f16/bf16 unless the benchmark is explicitly about widened f32. |
| byte order | Little-endian element bytes. |
| shape metadata | Record layer, kind, tokens, heads, head dimension, dtype, model, prompt hash, and llama.cpp commit. |
| mutability | Capture after a deterministic prompt/prefill point, before any QATQ compression step. |

The preferred QATQ commands are:

```sh
cargo run -- encode --mode qatq-exact --dtype f16 captures/cache_k_l12.f16le cache_k_l12.qatq
cargo run -- encode --mode qatq-exact --dtype bf16 captures/cache_v_l12.bf16le cache_v_l12.qatq
```

For full-cache captures, use the chunked container:

```sh
cargo run -- encode-chunked --max-values-per-chunk 65536 \
  --dtype f16 captures/cache_k_all_layers.f16le cache_k_all_layers.qatc
```

Decode should reproduce byte-identical native tensor files:

```sh
cargo run -- decode cache_k_all_layers.qatc restored-cache-k.f16le
cmp captures/cache_k_all_layers.f16le restored-cache-k.f16le
```

## llama.cpp Integration Notes

The capture patch should be kept out of the QATQ codec core. Treat it as a
runtime adapter with three jobs:

1. Configure llama.cpp deterministically: fixed model file, prompt, seed,
   context length, batch size, and `type_k` / `type_v`.
2. Export the internal K/V tensors after prefill. For CPU-readable captures,
   prefer a configuration where KV tensors are resident on CPU or copied back
   through llama.cpp/ggml APIs before writing.
3. Emit raw tensor files and a manifest, then call QATQ as a separate step.

The adapter should not use `llama_state_get_data` as a substitute for tensor
capture. That API is useful for whole-session state persistence, but the binary
state blob is not the per-layer typed tensor fixture needed for QATQ compression
evidence.

QATQ owns the first exporter hook as
[`adapters/llama-cpp/qatq-kv-export-7992aa7c8.patch`](../adapters/llama-cpp/qatq-kv-export-7992aa7c8.patch).
It targets llama.cpp commit `7992aa7c8`, the commit reported by local
`llama-cli` build `8640`. This patch adds a public
`llama_qatq_export_kv_cache(ctx, dir, seq_id)` hook that writes active raw KV
tensors as `.f16le`, `.bf16le`, or `.f32le` files. The checked-in patch also
adds a trace-capable hook,
`llama_qatq_export_kv_cache_with_trace(ctx, dir, seq_id, trace_path, model_id,
current_token)`, and wires both hooks into the patched `llama-simple` runner.
QATQ keeps this as adapter code rather than baking llama.cpp internals into the
codec.

Build the pinned adapter with the maintained bootstrap script rather than a
hand-maintained local checkout:

```sh
python3 scripts/llama_cpp_adapter_bootstrap.py \
  --work-dir /private/tmp/qatq-llama.cpp \
  --output /private/tmp/qatq-llama.cpp/bootstrap-report.json
```

The bootstrap report records the upstream repository URL, pinned llama.cpp
commit, adapter patch path, applied-source audit path, requested build targets,
and every command executed. Use `--dry-run` to inspect the exact clone, checkout,
patch, audit, and build commands without network or filesystem side effects.

The QATQ-side benchmark for exported tensors is:

```sh
cargo build --release --bin qatq-kv-bench
target/release/qatq-kv-bench --dir captures/llama-kv --iters 5 \
  --output docs/LLAMA_CPP_KV_COMPRESSION_REPORT.md
```

The live-VRAM replay evidence path uses the patched export manifest rather than
blindly scanning files:

```sh
target/release/qatq-kv-bench \
  --live-vram-export-dir captures/llama-kv \
  --live-vram-runtime-commit 7992aa7c8 \
  --live-vram-adapter-version qatq-kv-export-7992aa7c8 \
  --live-vram-model-id qwen2.5-1.5b-instruct:sha256:<model-hash> \
  --live-vram-gpu-context-bytes <runtime-kv-context-bytes> \
  --live-vram-allocation-granularity whole-context \
  --live-vram-restore-bytes-per-token <measured-restore-budget> \
  --output docs/LLAMA_CPP_LIVE_VRAM_EVIDENCE.json
```

This command validates `manifest.json`, rejects manifest path traversal,
builds QATQ live page descriptors, schedules the pages through the live VRAM
policy, verifies every QATQ restore, and records per-page raw, QATQ, zstd, and
lz4 byte counts. It remains replay evidence until llama.cpp frees/restores
actual GPU KV pages during generation.

The `--live-vram-gpu-context-bytes` and
`--live-vram-allocation-granularity` flags make the evidence report allocator
aware. Use `whole-context` for the current llama.cpp Metal KV cache because the
runtime allocates the KV cache as whole backend buffers. In that mode QATQ will
still report logical offload and CPU storage savings, but it will report
`reclaimable_gpu_bytes = 0` unless a future runtime adapter proves page-granular
GPU allocation reclaim.

`--live-vram-restore-bytes-per-token` adds a `restore_deadline_report` block
that checks whether compressed pages can be restored before their next token
deadline under the supplied restore-throughput budget. Treat this as an
operator calibration value for a specific runtime and GPU.

The maintained llama.cpp patch writes allocator evidence into `manifest.json`:
`gpu_allocation_granularity`, `gpu_context_bytes`, `total_context_bytes`,
`gpu_resident_tensors`, and `total_tensors`. Default Metal exports attest
`whole-context`, which is intentionally conservative. Mixed KV-layer placement
with `--qatq-kv-gpu-layers <n>` attests `whole-tensor` and reports the reduced
GPU-resident KV bytes. Proof-gate mode requires these runtime-written fields and
does not accept manual allocator CLI overrides as proof.

The patched `llama-simple` runner can also write a deterministic generation
manifest with `--qatq-output-manifest <path>`. Use this on paired baseline and
mixed-placement runs to compare generated token IDs and generated text hashes
while holding the model, prompt, sampler, cache dtype, and decode length fixed.
This proves whether a reduced-KV placement changed model behaviour in the
tested runtime configuration.

The patched runner can also perform an adapter page-operation self-test with
`--qatq-live-page-self-test <n>`. After generation, llama.cpp snapshots one
active key page from real backend KV tensor storage, overwrites that page with
zeroes, verifies that the backend tensor changed, restores the original bytes,
and verifies the restored checksum. This proves real runtime tensor
snapshot/mutate/restore mechanics for the adapter. It is intentionally not a
live VRAM reduction claim because it does not free per-page Metal allocation or
drive attention-loop eviction.

The runner can also write an export-time event trace:

```sh
/tmp/qatq-llama.cpp/build/bin/llama-simple \
  -m /path/to/model.gguf \
  -ngl 999 \
  -n 32 \
  --cache-type-k f16 \
  --cache-type-v f16 \
  --qatq-kv-export-dir captures/llama-kv \
  --qatq-event-trace captures/llama-kv-trace.json \
  --qatq-trace-current-token 0 \
  --qatq-trace-hot-window-tokens 1024 \
  --qatq-trace-next-required page-end \
  --qatq-model-id qwen2.5-1.5b-instruct-q4_k_m.gguf \
  "<deterministic prompt>"
```

It can also write actual attention-path key/value read telemetry from
`llama_kv_cache_context::get_k/get_v`:

```sh
/tmp/qatq-llama.cpp/build/bin/llama-simple \
  -m /path/to/model.gguf \
  -ngl 999 \
  -n 32 \
  --cache-type-k f16 \
  --cache-type-v f16 \
  --qatq-kv-export-dir captures/llama-kv \
  --qatq-output-manifest captures/output-manifest.json \
  --qatq-attention-trace captures/attention-trace.jsonl \
  --qatq-page-tokens 1024 \
  --qatq-live-page-self-test 1024 \
  --qatq-model-id qwen2.5-1.5b-instruct-q4_k_m.gguf \
  "<deterministic prompt>"
```

Verify the trace against the exported manifest with:

```sh
target/release/qatq-kv-bench \
  --live-vram-export-dir captures/llama-kv \
  --live-vram-runtime-commit 7992aa7c8 \
  --live-vram-adapter-version qatq-kv-export-7992aa7c8 \
  --live-vram-model-id qwen2.5-1.5b-instruct-q4_k_m.gguf \
  --live-vram-hot-window-tokens 0 \
  --live-vram-event-trace captures/llama-kv-trace.json \
  --output captures/llama-kv-evidence-with-trace.json
```

That trace proves the QATQ event schema and restore-before-attention verifier
against real exported llama.cpp tensor pages. It does not prove transparent
live paging until the runtime emits events from an actual attention-loop page
evict/prefetch/restore path. The stricter `--live-vram-live-paging-gate` is
expected to fail for the current export-only adapter when allocator evidence is
`whole-context` or otherwise cannot prove per-page GPU reclaim.

The attention JSONL trace is deliberately separate from
`qatq-live-vram-event-trace-v1`. It proves the model reached the real llama.cpp
attention K/V read path during generation, but it does not by itself prove that
cold pages were evicted, restored, or reclaimed from Metal memory. Treat it as
runtime-read telemetry that must be combined with future per-page allocator
attestation before claiming transparent live VRAM reduction.

The page self-test is also deliberately separate from both traces. It proves
that the adapter can mutate and restore a real backend KV page. The
`--qatq-gpu-page-staging` mode now provides the first scoped strict
live-paging pass by keeping canonical K/V off GPU and staging only
scheduler-resident pages onto `MTL0`; production still needs a non-concat
page-bounded attention path and broader model coverage before claiming
transparent live VRAM reduction.

`--qatq-page-tokens <n>` changes the export boundary from one file per active
K/V tensor to bounded token-range files. The manifest includes `token_start`
and `token_end` for each file, and QATQ treats each entry as a separate live
VRAM page. This is the preferred evidence mode for developing live VRAM
reduction because it exercises the same page keys and restore checks that a
future runtime allocator must honour.

Use QATQ's manifest comparator as the behaviour gate:

```sh
target/release/qatq-kv-bench \
  --compare-output-baseline captures/llama-kv-full-output.json \
  --compare-output-candidate captures/llama-kv-mixed-output.json \
  --compare-output-gate \
  --output captures/llama-kv-output-comparison.json
```

The comparison report includes hashes, cache dtype, decode counts, placement
metadata, and generated token counts, but deliberately omits generated text so
behaviour evidence can be stored without echoing prompt/output contents.

For a one-command local Metal evidence run, use the fail-closed live-VRAM
runner:

```sh
python3 scripts/llama_cpp_live_vram_evidence.py \
  --llama-simple /path/to/patched/llama-simple \
  --model /path/to/model.gguf \
  --model-id <stable-model-id> \
  --mixed-kv-gpu-layers 18 \
  --page-tokens 1024 \
  --work-dir /tmp/qatq-live-vram-evidence
```

The runner performs three runtime executions: full-GPU KV continuation,
mixed-KV continuation, all-CPU-KV continuation, and a deep mixed-KV export. It
then requires:

- Metal-backed execution in the llama.cpp logs;
- identical generated-token evidence for full-GPU and mixed-KV continuation;
- identical generated-token evidence for full-GPU and all-CPU-KV continuation;
- mixed-KV decode within the configured full-GPU regression ceiling;
- mixed-KV decode faster than the native all-CPU-KV baseline;
- runtime-attested `whole-tensor` allocator fields;
- non-zero GPU KV reclaim at or above the configured threshold;
- exact QATQ restore for every exported page;
- zero restore-deadline misses;
- QATQ beating zstd, lz4, and the best general-codec baseline on every
  offloaded page.

The runner writes `summary.md`, `output-comparison.json`,
`runtime-reclaim-evidence.json`, raw export manifests, an
`attention-trace-summary.json`, and llama.cpp logs into the selected work
directory. By default, the deep export also writes an export-time
`event-trace.json`, requires QATQ to accept it, writes
`attention-trace.jsonl`, and requires that trace to contain valid key/value
reads from the actual attention path. Add `--require-live-paging` when
validating a future adapter that claims true attention-loop token-page eviction;
that mode uses
`qatq-kv-bench --live-vram-live-paging-gate` with a fresh per-run
`--live-vram-page-seal-key-hex` generated by the runner. The runtime-reclaim
fallback path also uses `--live-vram-require-page-seals` by default, so every
offloaded page in production-shaped evidence carries a keyed metadata seal. The
strict live-paging mode is expected to fail for the current export-only
adapter. Raw KV tensors and model-specific logs
should stay outside the repository unless the model and prompt are explicitly
public and redistributable.

A 2026-06-24 Qwen2.5 1.5B Metal run with `--page-tokens 1024` exported 224
token-range K/V pages. QATQ restored 224/224 exactly, stored every offloaded
page through QATQ with zero pass-through pages, and beat zstd/lz4 on 224/224
page boundaries. An earlier 512-token export-only page run restored
successfully but failed the compression gate because one tail page lost to the
best general codec, so 1024 tokens was the safer export-only page size for that
model and prompt shape.

The first page-aware hot-window replay used the same 1024-token page size with
`--hot-window-tokens 1024` and `--next-required page-end`. On the real
Qwen2.5 1.5B Metal export it kept 56 pages resident, offloaded 168 colder pages
through QATQ, restored 224/224 pages exactly, and beat zstd/lz4 on every
offloaded page boundary. The strict live-paging gate still failed closed for
that export-only run because it did not expose per-page allocator-backed GPU
reclaim.

The patched exporter now supports scheduler-aligned lifecycle traces with
`--qatq-trace-current-token`, `--qatq-trace-hot-window-tokens`, and
`--qatq-trace-next-required`. A fresh aligned Qwen2.5 1.5B run at
`/private/tmp/qatq-live-vram-page-end-aligned-trace-20260624` emitted 224
snapshots, 168 offloads, 168 restores, and 224 attention-use events, matching
the QATQ evidence split of 56 resident pages and 168 offloaded pages. The
strict live-paging gate then failed only on the real allocator proof boundary.

To reproduce that tuning step rather than testing one size at a time:

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

The sweep runner keeps failed page sizes in the report and returns success when
at least one page size passes, unless `--fail-on-any` is supplied.

The patched `llama-simple` runner also supports native runtime memory controls
for live-VRAM baselines:

```sh
# Print model/context/compute memory by backend with normal GPU KV.
/tmp/qatq-llama.cpp/build-qatq/bin/llama-simple \
  -m /path/to/model.gguf \
  -ngl 99 \
  -n 1 \
  --memory-breakdown \
  --cache-type-k f16 \
  --cache-type-v f16 \
  "<long deterministic prompt>"

# Move llama.cpp KV context to host memory from the start.
/tmp/qatq-llama.cpp/build-qatq/bin/llama-simple \
  -m /path/to/model.gguf \
  -ngl 99 \
  -n 1 \
  --memory-breakdown \
  --no-kv-offload \
  --cache-type-k f16 \
  --cache-type-v f16 \
  "<long deterministic prompt>"

# Keep only the first 16 KV layers on Metal and place the rest on host memory.
/tmp/qatq-llama.cpp/build-qatq/bin/llama-simple \
  -m /path/to/model.gguf \
  -ngl 99 \
  -n 32 \
  --memory-breakdown \
  --qatq-kv-gpu-layers 16 \
  --cache-type-k f16 \
  --cache-type-v f16 \
  --qatq-kv-export-dir captures/llama-kv-mixed \
  --qatq-output-manifest captures/llama-kv-mixed-output.json \
  "<long deterministic prompt>"
```

These controls prove the native runtime can reduce Metal KV allocation by
choosing host placement up front, either for the whole KV cache or for selected
KV layers. They are baselines and adapter stepping stones for future QATQ live
page eviction, not evidence that exported token slices can free an already
allocated Metal KV buffer.

Run `qatq-kv-bench --live-vram-runtime-reclaim-gate` on mixed KV-layer exports
to verify the coarse runtime allocator win from manifest-attested
`total_context_bytes` and `gpu_context_bytes`. For release or production-shaped
evidence, also pass `--live-vram-page-seal-key-hex` and
`--live-vram-require-page-seals` so every offloaded page carries a keyed
metadata seal. Keep
`--live-vram-proof-gate` for the stricter future adapter that can evict and
restore token pages during generation.

For a combined runtime prompt and exported-KV report:

```sh
python3 scripts/llama_cpp_runtime_kv.py \
  --llama-cli /path/to/patched/llama-cli \
  --kv-dir captures/llama-kv
```

The first direct capture proof is recorded in
[`docs/LLAMA_CPP_KV_COMPRESSION_REPORT.md`](LLAMA_CPP_KV_COMPRESSION_REPORT.md).
It uses a patched llama.cpp `llama-simple` runner against local Qwen2.5 1.5B,
exports native f16 K/V tensors, verifies exact QATQ/zstd/lz4 decode equality,
and shows QATQ beating zstd/lz4 on packed K, packed V, and packed all-KV
bundles.

The current Metal-backed exported-KV replay evidence is recorded in
[`docs/LLAMA_CPP_LIVE_VRAM_GPU_EVIDENCE.md`](LLAMA_CPP_LIVE_VRAM_GPU_EVIDENCE.md).
It covers Qwen2.5 1.5B, Qwen2.5 Coder 3B, and Phi 3.5 mini local GGUF models,
verifies exact restore for every exported page, and shows long-context QATQ
payloads beating raw, zstd, and lz4 on the same page boundaries. It remains
exported-KV evidence until llama.cpp performs live GPU page eviction and
restore during generation.

## Remaining Evidence To Add

The patched exporter and local Metal evidence now exist. The remaining runtime
evidence bundle for true live VRAM reduction is:

| artifact | purpose |
| --- | --- |
| attention-loop event trace | Prove every evicted token page is restored before attention consumes it. |
| runtime allocator trace | Prove GPU pages are actually freed or made non-resident, not only exported. |
| p95/p99 token timing | Include restore stalls in end-to-end latency evidence. |
| peak GPU residency samples | Show measured VRAM reduction against full-GPU KV and native CPU-offload baselines. |
| deterministic output manifests | Prove unchanged generated token IDs where deterministic decoding is available. |
| raw/QATQ/zstd/lz4 page table | Keep compression wins tied to the exact pages used by the runtime. |

This is the line between the implemented exported-KV product surface and the
experimental live VRAM roadmap item.
