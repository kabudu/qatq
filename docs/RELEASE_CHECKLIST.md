# Release Checklist

QATQ is not published to crates.io yet. Until the crate has a stable public API
and an explicit publishing owner, releases are source releases only.

## Required Before Tagging

Run from the repository root:

```sh
cargo fmt --check
cargo check --all-targets
cargo test
cargo run --bin qatq -- fixture verify \
  --manifest fixtures/public.manifest \
  --output docs/PUBLIC_FIXTURE_AUDIT.md
cargo run --release --bin qatq-bench -- \
  --no-synthetic \
  --output docs/PUBLIC_COMPARATIVE_BASELINES.md \
  --paper-output docs/PUBLIC_COMPARATIVE_TABLES.md \
  --manifest fixtures/public.manifest
cargo run --release --bin qatq-bench -- \
  --exact-only \
  --no-synthetic \
  --output docs/PUBLIC_BENCHMARKS.md \
  --paper-output docs/PUBLIC_PAPER_TABLES.md \
  --manifest fixtures/public.manifest
cargo run --release --bin qatq-bench -- \
  --no-synthetic \
  --quality-output docs/PUBLIC_QUALITY_EXPERIMENTS.md \
  --manifest fixtures/public.manifest
cargo run --release --bin qatq-bench -- \
  --no-synthetic \
  --task-quality-output docs/PUBLIC_TASK_QUALITY_EXPERIMENTS.md \
  --manifest fixtures/public.manifest
cargo run --release --bin qatq-bench -- \
  --exact-only \
  --no-synthetic \
  --manifest fixtures/public.manifest \
  --gate-output docs/PUBLIC_BENCHMARK_GATE.md \
  --gate-require-external \
  --gate-policy production-kv \
  --max-exact-ratio 0.96 \
  --max-exact-encode-us 5000 \
  --max-exact-decode-ns-per-value 50.00 \
  --max-exact-container-ratio 0.97 \
  --max-exact-container-decode-ns-per-value 50.00
cargo run --release --bin qatq-bench -- \
  --exact-only \
  --no-synthetic \
  --manifest fixtures/public.manifest \
  --gate-output docs/PUBLIC_COMPETITIVE_COMPRESSION_GATE.md \
  --gate-require-external \
  --gate-policy competitive-compression
cargo test --test kv_stress -- --ignored --nocapture
```

After regenerating benchmark outputs, review
[`PUBLIC_COMPRESSION_SUMMARY.md`](PUBLIC_COMPRESSION_SUMMARY.md) and update it
if the public fixture ratios changed.

Optional external runtime validation:

```sh
cargo run --bin qatq -- fixture verify \
  --manifest fixtures/runtime.manifest \
  --output docs/RUNTIME_FIXTURE_AUDIT.md
python3 scripts/ollama_task_quality.py --model phi4-mini:latest
```

Do not tag a public release if:

- generated public fixtures cannot be regenerated and verified;
- `cargo test` fails;
- the deterministic KV stress matrix fails;
- the public production gate fails;
- the public competitive compression gate fails;
- docs claim external runtime data is required for QATQ to operate;
- raw private captures are staged.

## Tagging

Use annotated tags only:

```sh
git tag -a v0.1.0 -m "QATQ v0.1.0"
git push origin v0.1.0
```

Publishing crates, binaries, containers, or GitHub Releases remains deferred
until the repository documents that release mode and credentials are configured.
