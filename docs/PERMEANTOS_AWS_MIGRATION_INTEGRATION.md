# PermeantOS AWS Migration Integration

This guide defines how PermeantOS should integrate QATQ into a real end-to-end
AWS live migration trial before QATQ freezes its public API or publishes to
crates.io.

The goal is to validate the latest QATQ product surface under a real migration
flow: export runtime tensor state, compress it exactly, transfer it through AWS,
restore it on the target, and prove that the migrated task resumes with
byte-identical tensor state and acceptable latency.

## Integration Status

PermeantOS is a Rust project, so the primary integration should use QATQ as a
Rust crate dependency. Use a pinned source dependency for this trial; do not
depend on a crates.io package yet.

Recommended pin for the first PermeantOS integration pass:

```toml
[dependencies]
qatq = { git = "https://github.com/kabudu/qatq", rev = "d1ce4ef" }
```

For local joint development in a PermeantOS workspace, use a path dependency
instead:

```toml
[dependencies]
qatq = { path = "../qatq" }
```

PermeantOS feedback from this integration should be captured before QATQ accepts
the API/CLI freeze in `docs/API_CLI_FREEZE.md`.

Once the API freeze is accepted and crates.io publishing is approved,
PermeantOS should be able to switch this to the published crate without changing
the migration adapter shape:

```toml
[dependencies]
qatq = "0.1"
```

## Product Surface To Use

Use the exact QATQ path only:

| surface | use |
| --- | --- |
| `qatq-exact` | Primary exact tensor codec. |
| `QATC` v2 | Large tensor and migration bundle container. |
| native `f32`, `f16`, `bf16` bytes | Preserve runtime dtype exactly; do not widen half-precision captures. |
| `try_encode_qatq_exact_tensor_le` | Preferred Rust library function for exact typed tensor bytes. |
| `decode_qatq_exact_tensor_le` | Preferred Rust library function for exact typed tensor restore. |
| `qatq encode-chunked` | CLI verification and fixture-generation path for large migration artifacts. |

The f32 production chunk helpers are valid when PermeantOS has f32 values
already materialized in memory:

```rust
use qatq::{restore_production_chunk, try_encode_production_chunk};

let encoded = try_encode_production_chunk(&values)?;
let restored = restore_production_chunk(&encoded.metadata, encoded.stored_bytes())?;
```

For native `f16` or `bf16` migration state, keep the tensor as raw little-endian
element bytes and use the typed tensor API instead. This avoids widening cache
state to f32 and keeps the migration proof aligned with the runtime.

## Where QATQ Sits In Live Migration

On the source PermeantOS node:

1. Reach a migration checkpoint where the task/runtime state is quiesced or
   copy-on-write protected.
2. Export the KV cache or tensor state as raw little-endian typed bytes.
3. Record dtype, shape, tensor role, model/runtime identity, and checkpoint
   metadata in a manifest.
4. Pack related tensors into migration-friendly bundles, such as all K tensors,
   all V tensors, or one all-KV bundle. Avoid one tiny QATQ payload per layer
   unless that is required for random access.
5. Encode each bundle with QATQ exact.
6. Verify a local decode against the source bytes before upload.
7. Upload compressed artifacts and the manifest to the AWS migration channel.

On the target PermeantOS node:

1. Download the manifest and compressed artifacts.
2. Verify manifest identity, size limits, and encoded checksums.
3. Decode each QATQ artifact into raw typed tensor bytes.
4. Verify decoded byte checksums against the manifest.
5. Rehydrate runtime tensors with the recorded dtype and shape metadata.
6. Run the migration task probe before target activation.
7. Commit target activation only after every tensor and task probe passes.

If decode, checksum, shape validation, or the task probe fails, treat the
migration as failed and keep the source runtime authoritative.

## Rust Library Path

Use the typed tensor API in the PermeantOS migration service rather than
shelling out from the production migration path:

```rust
use qatq::{
    decode_qatq_exact_tensor_le, try_encode_qatq_exact_tensor_le, QatqError, TensorDType,
};

fn encode_bundle(bytes_le: &[u8], dtype: TensorDType) -> Result<Vec<u8>, QatqError> {
    try_encode_qatq_exact_tensor_le(bytes_le, dtype)
}

fn decode_bundle(payload: &[u8], expected_dtype: TensorDType) -> Result<Vec<u8>, QatqError> {
    let decoded = decode_qatq_exact_tensor_le(payload)?;
    if decoded.dtype != expected_dtype {
        return Err(QatqError::InvalidQatqExactBody);
    }
    Ok(decoded.bytes_le)
}
```

PermeantOS should wrap this in its own migration artifact type, so dtype, shape,
raw checksum, encoded checksum, AWS object URI, and restore layout travel
together with the payload. QATQ decodes the exact typed bytes; the PermeantOS
runtime adapter is responsible for restoring those bytes into the correct
tensor object and layout.

## CLI Verification Path

Use the CLI for release audits, local fixture generation, and independent
source/target verification. Build QATQ from the pinned source:

```sh
cargo build --release --bin qatq --bin qatq-kv-bench
```

Encode a native f16 KV bundle:

```sh
target/release/qatq encode-chunked \
  --max-values-per-chunk 65536 \
  --dtype f16 \
  cache_all.f16le \
  cache_all.qatc
```

Decode and verify before sending or after receiving:

```sh
target/release/qatq decode cache_all.qatc cache_all.restored.f16le
cmp cache_all.f16le cache_all.restored.f16le
```

Use `--dtype bf16` for native bf16 captures and omit `--dtype` or pass
`--dtype f32` for raw f32 little-endian captures.

## Manifest Contract

Write a manifest before target activation and include enough metadata to reject
wrong-model, wrong-shape, or wrong-checkpoint restores.

Recommended top-level fields:

```json
{
  "schema": "permeantos.qatq-migration.v1",
  "migration_id": "mig-2026-06-22T10-15-30Z-01",
  "qatq": {
    "commit": "d1ce4ef",
    "codec": "qatq-exact",
    "container": "QATC-v2",
    "max_values_per_chunk": 65536
  },
  "runtime": {
    "name": "permeantos-runtime",
    "version": "replace-with-runtime-version",
    "model_id": "replace-with-model-id-or-gguf-hash",
    "checkpoint_id": "replace-with-checkpoint-id"
  },
  "source": {
    "instance_id": "i-source",
    "region": "eu-west-2"
  },
  "target": {
    "instance_id": "i-target",
    "region": "eu-west-2"
  },
  "artifacts": [
    {
      "name": "cache_all",
      "tensor_kind": "kv-cache-packed",
      "dtype": "f16",
      "byte_order": "little",
      "layout": "packed-layer-major-kv",
      "shape": {
        "tokens": 512,
        "layers": 28,
        "kv_heads": 2,
        "head_dim": 128
      },
      "raw_bytes": 12345678,
      "encoded_bytes": 8765432,
      "raw_sha256": "replace-with-raw-sha256",
      "encoded_sha256": "replace-with-encoded-sha256",
      "uri": "s3://permeantos-migrations/mig-.../kv/cache_all.qatc"
    }
  ]
}
```

PermeantOS can add IAM role, KMS key, workload ID, prompt/session hash, or task
metadata fields as needed. QATQ needs the resulting feedback before API freeze,
especially if the manifest wants codec metadata embedded differently in QATC.

## AWS Transport

Recommended object layout:

```text
s3://permeantos-migrations/{migration_id}/manifest.json
s3://permeantos-migrations/{migration_id}/kv/cache_all.qatc
s3://permeantos-migrations/{migration_id}/reports/qatq-kv-bench.md
s3://permeantos-migrations/{migration_id}/logs/source-verify.log
s3://permeantos-migrations/{migration_id}/logs/target-verify.log
```

Transport rules:

- Upload compressed artifacts before the final manifest.
- Make object keys content-addressed or migration-ID scoped so retries are
  idempotent.
- Use SSE-KMS for S3 objects and least-privilege IAM for source and target
  migration roles.
- Do not stage raw private tensor captures in S3 unless the migration design
  explicitly requires it and the bucket policy is approved for that data.
- Use local temporary files with restrictive permissions and delete raw staging
  files after successful transfer and verification.
- Treat target decode failures as hard migration failures, not partial restores.

## Runtime Export Requirements

The runtime adapter must export raw tensor bytes before QATQ sees them:

| field | requirement |
| --- | --- |
| dtype | `f32`, `f16`, or `bf16`; do not widen half-precision state. |
| byte order | little-endian element bytes. |
| layout | deterministic and recorded in the manifest. |
| shape | enough dimensions to reconstruct the runtime tensor exactly. |
| checkpoint | capture after a deterministic pre-migration boundary. |
| validation | raw byte checksum before encode, decoded byte checksum after decode. |

For llama.cpp-backed PermeantOS workloads, use the version-pinned adapter patch
as the current source exporter:

```sh
git clone https://github.com/ggml-org/llama.cpp.git /tmp/qatq-llama.cpp
cd /tmp/qatq-llama.cpp
git checkout 7992aa7c8
git apply /path/to/qatq/adapters/llama-cpp/qatq-kv-export-7992aa7c8.patch
cmake -B build-qatq -S . -DCMAKE_BUILD_TYPE=Release
cmake --build build-qatq --target llama-simple
```

Run the patched exporter with explicit KV dtypes:

```sh
/tmp/qatq-llama.cpp/build-qatq/bin/llama-simple \
  -m /models/model.gguf \
  -ngl 0 \
  -n 16 \
  --cache-type-k f16 \
  --cache-type-v f16 \
  --qatq-kv-export-dir /tmp/permeantos-kv-export \
  "migration validation prompt"
```

Then pack and benchmark exported tensors:

```sh
target/release/qatq-kv-bench \
  --dir /tmp/permeantos-kv-export \
  --iters 5 \
  --output /tmp/permeantos-kv-export/qatq-kv-bench.md
```

If PermeantOS uses a non-llama.cpp runtime, implement the same export contract
directly in that runtime and bypass the llama.cpp patch.

## Acceptance Tests Before API Freeze

A PermeantOS migration trial should produce these artifacts and pass these
checks:

| check | pass condition |
| --- | --- |
| Source encode | Every migration tensor/bundle encodes with `qatq-exact` or QATC. |
| Source local verify | Decode equals source bytes for every artifact before upload. |
| AWS transfer | S3/EBS artifact checksums match the manifest on target. |
| Target restore | Decoded bytes match `raw_sha256` and restore into the target runtime shape/dtype. |
| Task behavior | Migrated task resumes and passes the PermeantOS task-decision probe. |
| Rollback | Corrupt or missing QATQ artifacts abort activation and leave source authoritative. |
| Resource limits | Oversized/corrupt QATC payloads are rejected before allocation-heavy decode. |
| Compression | Packed migration bundles report QATQ size and throughput against zstd and lz4. |

For direct benchmark evidence, run:

```sh
target/release/qatq-kv-bench \
  --input cache_all:f16:/tmp/permeantos/cache_all.f16le \
  --iters 10 \
  --output /tmp/permeantos/qatq-cache-all-report.md
```

For broader llama.cpp-backed coverage, run the matrix harness:

```sh
python3 scripts/llama_cpp_kv_matrix.py \
  --llama-simple /tmp/qatq-llama.cpp/build-qatq/bin/llama-simple \
  --model daily:/models/daily-driver.gguf \
  --model coding:/models/software-engineering.gguf \
  --prompt daily:"Summarize this project status for a teammate." \
  --prompt coding:"Review this Rust migration codec design for correctness risks." \
  --dtype f16 \
  --dtype bf16 \
  --dtype f32 \
  --predict 16 \
  --predict 128 \
  --iters 5 \
  --report /tmp/permeantos/llama-kv-matrix.md
```

## Security And Reliability Notes

- Verify encoded and decoded checksums before target activation.
- Bound raw byte size, encoded byte size, chunk count, and chunk length before
  decode.
- Never trust dtype, shape, or object URI fields from an unauthenticated
  manifest.
- Keep raw tensor captures out of logs.
- Prefer one migration bundle per logical cache group for compression, then add
  finer chunking only where PermeantOS needs random access or streaming restore.
- Record encode/decode latency in the migration timeline so QATQ can be judged
  against the live migration SLO.

## Feedback QATQ Needs From PermeantOS

Before QATQ freezes the API, PermeantOS should report:

- whether the typed tensor Rust API is ergonomic enough for f16/bf16 migration;
- whether QATC should embed more manifest fields, such as dtype, shape, or
  logical tensor name;
- preferred chunk sizing for live migration latency and memory limits;
- whether streaming decode or random-access chunks are required;
- whether storage labels and public function names are clear enough;
- compression and throughput results from at least one real AWS migration trial;
- any security review findings around corrupt containers, manifests, or AWS
  object handling.
