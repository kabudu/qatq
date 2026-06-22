# llama.cpp KV Compression Report

This report records the first direct llama.cpp KV-cache tensor capture exercised
through QATQ.

## Environment

| field | value |
| --- | --- |
| date | 2026-06-22 |
| llama.cpp commit | `7992aa7c8` |
| patched runner | `/tmp/qatq-llama.cpp/build-qatq/bin/llama-simple` |
| QATQ branch | `codex/qatq-standalone-production` |
| model | `Qwen2.5-1.5B-Instruct-Q4_K_M.gguf` |
| prompt | `Summarize the purpose of a Rust tensor compression codec in one sentence.` |
| KV dtype | native `f16` |
| export point | after prompt prefill, before sampled-token decode |
| active prompt cells | `15` |
| layers | `28` |
| exported tensors | `56` raw files plus manifest |

## Commands

```sh
git clone https://github.com/ggml-org/llama.cpp.git /tmp/qatq-llama.cpp
cd /tmp/qatq-llama.cpp
git checkout 7992aa7c8
git apply "$QATQ_REPO/adapters/llama-cpp/qatq-kv-export-7992aa7c8.patch"
cmake -B build-qatq -S . -DCMAKE_BUILD_TYPE=Release
cmake --build build-qatq --target llama-simple -j 6
```

```sh
/tmp/qatq-llama.cpp/build-qatq/bin/llama-simple \
  -m "$QATQ_LLAMA_MODEL_QWEN25_15B" \
  -ngl 0 \
  -n 16 \
  --qatq-kv-export-dir /tmp/qatq-real-kv \
  'Summarize the purpose of a Rust tensor compression codec in one sentence.'
```

```sh
cargo run --quiet --bin qatq-kv-bench -- \
  --dir /tmp/qatq-real-kv \
  --iters 5 \
  --output docs/LLAMA_CPP_KV_COMPRESSION_REPORT.md
```

The production comparison below packs the exported layer files into K, V, and
all-KV bundles before compression. That avoids measuring one QATQ header per
tiny per-layer file.

## Results

| input | raw bytes | QATQ exact bytes | QATQ ratio | zstd bytes | zstd ratio | lz4 bytes | lz4 ratio | winner |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| per-layer files, summed | 430080 | 407234 | 0.9469 | 405652 | 0.9432 | 432096 | 1.0047 | zstd by 1582 bytes |
| packed K bundle | 215040 | 193715 | 0.9008 | 201723 | 0.9381 | 215889 | 1.0039 | QATQ |
| packed V bundle | 215040 | 193989 | 0.9021 | 201548 | 0.9373 | 215889 | 1.0039 | QATQ |
| packed all-KV bundle | 430080 | 377585 | 0.8779 | 403350 | 0.9378 | 431772 | 1.0039 | QATQ |

## Interpretation

QATQ now has direct real-model KV-cache ingestion proof: a patched llama.cpp
runner exported native f16 internal K/V tensors from a real Qwen prompt, and
`qatq-kv-bench` verified exact QATQ, zstd, and lz4 decode equality against the
raw tensor bytes.

The superiority claim is scoped:

- QATQ does not beat zstd when each tiny layer tensor is compressed as its own
  independent file; per-file header overhead dominates at this size.
- QATQ beats zstd and lz4 when the same real KV bytes are compressed in packed
  K/V bundles, which is the relevant shape for migration/checkpoint transport.
- The result is exact and native dtype-aware: f16 bytes are not widened to f32.
- This is one real model and one prompt. It is strong proof that the pipeline
  works and that packed KV compression can beat zstd, not yet a broad benchmark
  across model families, prompt lengths, dtypes, and cache layouts.
