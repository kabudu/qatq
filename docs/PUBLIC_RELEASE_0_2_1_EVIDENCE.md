# QATQ 0.2.1 Release Evidence

QATQ 0.2.1 makes the primary lossless product path the CLI default:
`qatq encode` selects `qatq-exact` when `--mode` is omitted. Explicit
comparator and research modes remain available through `--mode`, and decoding
continues to read the required strategy from the payload.

The change is backward-compatible. Explicit commands retain their existing
behavior, default and explicit exact commands produce identical payloads, and
the QATQ/QATC wire formats are unchanged.

## Required Gates

| Gate | Result |
| --- | --- |
| format and all-target check | pass |
| default test suite | pass, 131 tests |
| default-versus-explicit payload equivalence | pass |
| native bf16 default-mode encode and exact restore | pass |
| explicit comparator modes | pass |
| locked metadata and duplicate dependency check | pass; no duplicate dependencies |
| RustSec audit | pass, no known vulnerabilities |
| line coverage | pass, 85.73% against a 75% floor |
| fuzz target compilation | pass |
| real CLI encode/decode smoke test | pass, payloads identical and input restored |
| crate package and crates.io dry run | pass, 67 files and 306.6 KiB compressed |

The release also sets Cargo's `default-run` to the `qatq` binary so repository
commands such as `cargo run -- encode ...` work without requiring an explicit
`--bin` selection despite the project containing benchmark binaries.

This patch does not modify compression algorithms, strategy selection within
`qatq-exact`, payload bytes, decoder behavior, or performance-sensitive codec
paths.
