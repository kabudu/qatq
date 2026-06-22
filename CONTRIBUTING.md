# Contributing

QATQ is an exact tensor-compression project focused on exported LLM KV-cache and
runtime-migration payloads. Contributions should preserve that scope unless a
roadmap item explicitly expands it.

## Development Setup

Install stable Rust and run:

```sh
cargo fmt --check
cargo check --all-targets
cargo test
```

For release-facing changes, also run:

```sh
cargo metadata --locked --format-version 1
cargo tree -d
cargo audit
cargo llvm-cov --workspace --all-targets --locked --fail-under-lines 75
cargo package --allow-dirty
```

## Pull Request Expectations

- Keep changes scoped to one feature, fix, or documentation update.
- Add or update tests for codec behavior, parser hardening, CLI behavior, and
  benchmark gates when relevant.
- Do not commit private model captures, local runtime dumps, cloud logs, or
  credentials.
- Keep public claims scoped to evidence present in this repository or linked
  from runtime-agnostic evidence summaries.
- Do not rename frozen API or CLI surfaces without updating
  `docs/API_CLI_FREEZE.md` and the changelog.

## Evidence And Benchmarks

Public benchmark fixtures live under `fixtures/generated/` with the manifest at
`fixtures/public.manifest`. Regenerate or verify them with:

```sh
cargo run --bin qatq -- fixture verify \
  --manifest fixtures/public.manifest \
  --output docs/PUBLIC_FIXTURE_AUDIT.md
```

External runtime evidence should stay runtime-agnostic in this repository. Keep
runtime-specific runbooks and raw captures in the integrating runtime project.
