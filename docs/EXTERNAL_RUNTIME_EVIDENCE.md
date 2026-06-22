# External Runtime Evidence

QATQ is standalone and does not require an external runtime to build, test, or
benchmark. External runtime evidence is optional provenance for claims about
real live-migration behavior.

## 2026-06-22 Cloud Live-Migration Compression Proof

An external Rust live-migration integration validated the standalone QATQ crate
in a real cloud GPU migration path. The integration resolved QATQ from a sibling
checkout of this repository, copied that checkout to the target host, and built
`qatq v0.1.0` from that source during the run.

Scope:

| field | value |
| --- | --- |
| source runtime | local ML tensor runtime |
| target runtime | cloud GPU LLM runtime |
| model family | Qwen2.5 0.5B class |
| migrated prefix | 1,920 tokens |
| continuation horizon | 16, 32, 64, and 128 tokens |
| QATQ chunks | 384 |
| pass-through chunks | 0 |
| task behavior | exact continuation at every measured horizon |
| reverse import | passed |
| return-home continuation | passed |
| cleanup | cloud instance, security group, and key pair removed |

Compression results for the same streamed block artifacts:

| baseline | bytes | ratio vs raw |
| --- | ---: | ---: |
| raw streamed blocks | 50,331,648 | 1.0000 |
| zstd | 20,405,381 | 0.4054 |
| lz4 | 28,739,217 | 0.5709 |
| QATQ exact | 14,004,990 | 0.2783 |

Relative result:

| comparison | result |
| --- | ---: |
| QATQ reduction vs raw | 72.2% smaller |
| QATQ reduction vs zstd | 31.4% smaller |
| QATQ reduction vs lz4 | 51.3% smaller |

The external integration enforced a live compression gate that failed unless
QATQ beat raw, zstd, and lz4 for the same streamed block artifacts.

## Claim Scope

This evidence proves that standalone QATQ can be integrated into a real Rust
live-migration path, preserve exact task behavior for the measured migration,
and beat raw, zstd, and lz4 transfer-size baselines for that run.

This is still scoped evidence, not a universal compression claim:

- one model family and size class;
- one source/target runtime pair;
- one migrated prefix length;
- one cloud GPU profile;
- one block-streaming layout and chunking policy.

Broader production claims still require more runtime pairs, model families,
context lengths, dtypes, and chunk layouts.
