# QATQ Capacity Oracle

The Capacity Oracle is an additive, feature-gated analysis tool outside the
QATQ/QATC codec trust boundary. It answers a normalized binary or spherical code
question with exactly one of `CONSTRUCTED`, `INFEASIBLE_UNDER_MODEL`, `UNKNOWN`,
or `REFUSED`.

The production release emits `INFEASIBLE_UNDER_MODEL` only for two exact finite
families:

- the binary Hamming sphere-packing bound; and
- Rankin's spherical negative/orthoplex bounds for maximum inner product
  `s < 0` and `s = 0`.

`CONSTRUCTED` is reserved in the schema but no construction producer ships in
this release. The `construct` and `derive-kv` commands fail closed with
`REFUSED`. Positive-`s` spherical requests are `UNKNOWN`, and asymptotic results are
planning-only.

## Build and run

```sh
cargo build --release --features oracle --bin qatq-oracle

target/release/qatq-oracle bound request.json --output evidence
target/release/qatq-oracle check evidence/certificate.json
target/release/qatq-oracle inspect evidence/certificate.json
```

Logical exit statuses are `0` for a constructed result or valid certificate,
`1` for certified infeasibility, `2` for unknown, `3` for refused, `4` for an
invalid/unsupported certificate, and `5` for an internal or I/O failure.

An output-directory run is published atomically and contains normalized request,
outcome, certificate, report, metrics, and SHA-256 manifest files. Existing
output directories are never overwritten.

## Scientific boundary

The result applies only to the declared code model. It does not establish a
universal limit for KV caches, embedding quality, codec distortion, or task
performance. A bridge must separately justify why real states satisfy the
normalization and separation assumptions.

See [CLAIM_BOUNDARY.md](CLAIM_BOUNDARY.md),
[MATHEMATICAL_MODEL.md](MATHEMATICAL_MODEL.md),
[CERTIFICATE_SCHEMA.md](CERTIFICATE_SCHEMA.md), and
[TRUST_BOUNDARY.md](TRUST_BOUNDARY.md).
