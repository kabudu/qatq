# Fixture Manifest

Use fixture manifests to keep real PermeantOS or KV-cache tensor metadata
separate from benchmark code. Tensor files should be raw little-endian `f32`
streams. Large or private tensors do not need to be committed.

For the full PermeantOS workflow, see
[PERMEANTOS_HANDOFF.md](PERMEANTOS_HANDOFF.md).

Add a validated fixture entry:

```sh
cargo run -- fixture add \
  --manifest fixtures/permeantos.manifest \
  --group permeantos-kv \
  --name llama-layer12-k \
  --path captures/llama-layer12-k.f32le \
  --shape "[layers=1, heads=32, tokens=128, dim=128]" \
  --notes "MLX source capture before migration"
```

The command checks that the fixture path exists and that the byte length is
divisible by four before appending to the manifest. It records the f32 value
count in the `notes` field so paper tables can be audited against the raw file.

Run a manifest:

```sh
cargo run --release --bin qatq-bench -- \
  --output docs/BENCHMARKS.md \
  --paper-output docs/PAPER_TABLES.md \
  --manifest fixtures/permeantos.manifest
```

Verify fixture files and produce an audit report:

```sh
cargo run -- fixture verify \
  --manifest fixtures/permeantos.manifest \
  --output docs/FIXTURE_AUDIT.md
```

Verification checks that every manifest entry has a readable raw `f32le` file
whose byte length is divisible by four. The audit report records total fixtures,
total values, total bytes, and an FNV-1a 64-bit fingerprint for each file. Use
this report with benchmark tables when preparing paper or white-paper evidence.

Run a benchmark gate with explicit readiness thresholds:

```sh
cargo run --release --bin qatq-bench -- \
  --manifest fixtures/permeantos.manifest \
  --gate-output docs/BENCHMARK_GATE.md \
  --gate-require-external \
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
group = "permeantos-kv"
name = "llama-layer12-k"
path = "../fixtures/llama-layer12-k.f32le"
shape = "[layers=1, heads=32, tokens=128, dim=128]"
notes = "MLX source capture before migration"

[fixture]
group = "permeantos-kv"
name = "llama-layer12-v"
path = "../fixtures/llama-layer12-v.f32le"
shape = "[layers=1, heads=32, tokens=128, dim=128]"
notes = "vLLM destination validation capture"
```

Rules:

- `name` and `path` are required.
- `group`, `shape`, and `notes` are optional.
- Relative `path` values resolve relative to the manifest file.
- Lines may contain comments after `#`.
- The parser intentionally supports only this small format so the benchmark
  harness stays dependency-free.

Recommended real-data groups:

- `permeantos-kv`: actual KV-cache captures from migration experiments.
- `permeantos-activation`: activation-like intermediate tensors.
- `embedding`: vector/embedding payloads.
- `stress`: known outlier-heavy or adversarial numeric tensors.
