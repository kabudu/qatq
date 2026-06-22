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
tensors as `.f16le`, `.bf16le`, or `.f32le` files. The hook still needs to be
called from the selected llama.cpp prompt runner at the capture point; QATQ
keeps that as adapter code rather than baking llama.cpp internals into the
codec.

The QATQ-side benchmark for exported tensors is:

```sh
cargo build --release --bin qatq-kv-bench
target/release/qatq-kv-bench --dir captures/llama-kv --iters 5 \
  --output docs/LLAMA_CPP_KV_COMPRESSION_REPORT.md
```

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

## Evidence To Add

Once a patched llama.cpp exporter is available, add a runtime evidence bundle:

| artifact | purpose |
| --- | --- |
| raw `.f16le` / `.bf16le` K/V tensors | Ground truth native cache bytes. |
| manifest | Reproducibility metadata and tensor shapes. |
| QATQ `.qatq` / `.qatc` outputs | Compressed exact artifacts. |
| `cmp` logs or hashes | Proof of byte-identical decode. |
| task transcript | Prompt/output behavior at capture point. |
| benchmark table | Size and throughput compared with raw, zstd, and lz4. |

This will prove direct live KV-cache tensor ingestion. The existing local
model-output tensor experiment proves task-decision preservation, but it is not
the same claim.
