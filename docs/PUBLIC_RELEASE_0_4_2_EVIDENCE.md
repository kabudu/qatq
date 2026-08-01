# QATQ 0.4.2 Release Evidence

QATQ 0.4.2 packages the KV Geometry Relevance Gate as a separate, research-only production binary. It does not change QATQ or QATC wire formats, Capacity Oracle theorem families, certificate schemas, or checker semantics.

## Shipped surface

- `qatq-kv-geometry profile` consumes strict versioned capture metadata and emits bounded observations.
- Exact pairwise analysis is used below configured thresholds; larger populations use deterministic sampling with coverage and error metadata.
- Hard metadata, capture, scalar, vector, dimension, pair, and block ceilings prevent CLI flags from widening the compiled resource envelope.
- Each new output directory contains six SHA-256-bound evidence files and can be checked with `qatq-kv-geometry verify`.
- Sign and threshold binarizations are explicitly descriptive and do not claim semantic distinguishability.

The capture and result contracts are documented in [`oracle/KV_GEOMETRY_CAPTURE.md`](oracle/KV_GEOMETRY_CAPTURE.md).

## Real-capture corpus

The published corpus contains 36 profiles from 12 pinned llama.cpp captures across two model families, three prompt and context regimes, f16 and bf16, early, middle, and late layers, all exported KV heads in those layers, keys and values, and three partition layouts.

Every compact-corpus pair population was evaluated exactly. Spectral concentration is marked `APPROXIMATE` because its top eigenvalue uses deterministic power iteration. The corpus, per-bundle manifests, model and capture hashes, and deterministic policy are published in [`validation/geometry-v0.4.2`](../validation/geometry-v0.4.2).

## Decision and claim boundary

Post-RoPE key-group maximum cosine similarities ranged from 0.871583 to 0.998824, with median 0.954426. No credible application-level source for required state count or separation was established. The required decision report therefore freezes further theorem expansion and redirects product work toward live cold-page KV compression.

The pinned exporter did not expose matched pre-RoPE keys, so no pre-RoPE comparison is claimed. External human coding-theory review remains outstanding and is not claimed. See [`oracle/KV_GEOMETRY_RELEVANCE.md`](oracle/KV_GEOMETRY_RELEVANCE.md) for the complete decision record.

## Verification

The release commit passes formatting, all-target all-feature compilation, the full locked test suite, two geometry CLI integration tests, the repository typography gate, independent SageMath reproduction of all 27 Capacity Oracle rows, and `scripts/verify_kv_geometry_corpus.py` over all 36 published profiles.
