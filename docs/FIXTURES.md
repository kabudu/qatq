# Fixture Manifest

Use fixture manifests to keep tensor metadata separate from benchmark code.
Tensor files should be raw little-endian `f32` streams. QATQ ships with a small
generated public corpus so the project can validate itself without any external
runtime.

Generate the public corpus:

```sh
cargo run --bin qatq -- fixture generate \
  --manifest fixtures/public.manifest \
  --dir fixtures/generated
```

Add a validated fixture entry:

```sh
cargo run -- fixture add \
  --manifest fixtures/runtime.manifest \
  --group runtime-kv \
  --name llama-layer12-k \
  --path captures/llama-layer12-k.f32le \
  --shape "[layers=1, heads=32, tokens=128, dim=128]" \
  --notes "runtime source capture"
```

The command checks that the fixture path exists and that the byte length is
divisible by four before appending to the manifest. It records the f32 value
count in the `notes` field so paper tables can be audited against the raw file.

Run a manifest:

```sh
cargo run --release --bin qatq-bench -- \
  --output docs/PUBLIC_BENCHMARKS.md \
  --paper-output docs/PUBLIC_PAPER_TABLES.md \
  --manifest fixtures/public.manifest
```

Verify fixture files and produce an audit report:

```sh
cargo run -- fixture verify \
  --manifest fixtures/public.manifest \
  --output docs/PUBLIC_FIXTURE_AUDIT.md
```

Verification checks that every manifest entry has a readable raw `f32le` file
whose byte length is divisible by four. The audit report records total fixtures,
total values, total bytes, and an FNV-1a 64-bit fingerprint for each file. Use
this report with benchmark tables when preparing paper or white-paper evidence.

Run the production KV readiness gate with explicit throughput-normalized decode
thresholds:

```sh
cargo run --release --bin qatq-bench -- \
  --phase2-only \
  --no-synthetic \
  --manifest fixtures/public.manifest \
  --gate-output docs/PUBLIC_BENCHMARK_GATE.md \
  --gate-require-external \
  --gate-policy production-kv \
  --max-phase2-ratio 0.96 \
  --max-phase2-encode-us 5000 \
  --max-phase2-decode-ns-per-value 50.00 \
  --max-phase2-container-ratio 0.97 \
  --max-phase2-container-decode-ns-per-value 50.00
```

Run a fixed absolute-latency gate separately only when you need small-tensor or
deployment-specific service-budget analysis:

```sh
cargo run --release --bin qatq-bench -- \
  --phase2-only \
  --no-synthetic \
  --manifest fixtures/public.manifest \
  --gate-output docs/BENCHMARK_GATE.md \
  --gate-require-external \
  --gate-policy latency-budget \
  --max-phase2-ratio 0.95 \
  --max-phase2-encode-us 5000 \
  --max-phase2-decode-us 1000 \
  --max-phase2-container-ratio 0.96 \
  --max-phase2-container-decode-us 1200
```

The gate checks bit-identical `phase2-lossless` and `QATC` container
reconstruction plus any configured ratio or latency thresholds. It writes a
markdown report and exits nonzero when the criteria fail.

Manifest format:

```text
[fixture]
group = "runtime-kv"
name = "llama-layer12-k"
path = "../fixtures/llama-layer12-k.f32le"
shape = "[layers=1, heads=32, tokens=128, dim=128]"
notes = "runtime source capture"

[fixture]
group = "runtime-kv"
name = "llama-layer12-v"
path = "../fixtures/llama-layer12-v.f32le"
shape = "[layers=1, heads=32, tokens=128, dim=128]"
notes = "runtime destination validation capture"
```

Rules:

- `name` and `path` are required.
- `group`, `shape`, and `notes` are optional.
- Relative `path` values resolve relative to the manifest file.
- Lines may contain comments after `#`.
- The parser intentionally supports only this small format so the benchmark
  harness stays dependency-free.

Recommended groups:

- `qatq-public`: generated public fixtures committed with the repository.
- `runtime-kv`: actual KV-cache captures from runtime experiments.
- `runtime-activation`: activation-like intermediate tensors.
- `embedding`: vector/embedding payloads.
- `stress`: known outlier-heavy or adversarial numeric tensors.
