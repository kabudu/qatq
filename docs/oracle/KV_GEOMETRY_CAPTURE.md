# KV Geometry Capture Contract

`qatq-kv-geometry` is a separate, feature-gated research tool. It consumes a binary tensor bundle and strict JSON metadata, then writes observations only. It does not infer application requirements, alter Capacity Oracle certificates, or emit an Oracle verdict.

## Capture schema v1

The binary file concatenates little-endian `f16`, `bf16`, or `f32` tensors. Metadata records immutable offsets and lengths plus capture, model, runtime, prompt, context, dtype, layer, key or value kind, RoPE stage, token range, KV-head count, per-head dimension, and layout. Schema v1 supports `token_head_dimension` layout. Unknown JSON fields and inconsistent shapes fail closed.

The llama.cpp converter verifies each source row size, selects declared layers, binds the prompt and source manifest by SHA-256, and publishes `capture.kv`, `capture.json`, and `provenance.json` atomically into a new directory.

## Result schema v1

Each profile bundle contains exactly:

- `capture-manifest.json`
- `geometry.json`
- `summary.md`
- `sampling-plan.json`
- `metrics.json`
- `manifest.json`

The final manifest binds every other result file by byte count and SHA-256. `qatq-kv-geometry verify <bundle>` checks those bindings. Existing output directories are never overwritten.

Every result uses one of `EXACT`, `DETERMINISTIC_SAMPLE`, `APPROXIMATE`, or `REFUSED`. Pairwise calculations are exact below the configured vector threshold and pair ceiling, otherwise they use a deterministic seed-bound sample. Spectral concentration includes a deterministic power-iteration estimate and is therefore marked approximate. Inputs over declared capture, vector, or dimension limits produce a result bundle with `REFUSED` status.

Binary sign and threshold mappings are descriptive sensitivity measurements. Their Hamming distances and collision rates do not establish semantic distinguishability.
