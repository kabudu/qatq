# llama.cpp Adapter

This directory tracks the QATQ-side contract for a patched llama.cpp KV-cache
exporter. The adapter target is direct internal K/V tensor capture, not
session-state serialization and not Ollama API output.

Expected exporter behavior:

- run llama.cpp with deterministic model, prompt, seed, and KV dtype settings;
- export `cache_k_l*` and `cache_v_l*` tensors as raw little-endian `.f16le`,
  `.bf16le`, or `.f32le` files;
- write a manifest with tensor names, shapes, dtypes, llama.cpp commit, model
  hash, prompt hash, and capture point;
- invoke QATQ exact with `--dtype` matching the exported tensor dtype.

Example QATQ calls:

```sh
cargo run -- encode --mode qatq-exact --dtype f16 cache_k_l0.f16le cache_k_l0.qatq
cargo run -- encode-chunked --max-values-per-chunk 65536 --dtype bf16 cache_v_all.bf16le cache_v_all.qatc
```

The full capture plan is in
[`docs/LLAMA_CPP_KV_CAPTURE.md`](../../docs/LLAMA_CPP_KV_CAPTURE.md).
