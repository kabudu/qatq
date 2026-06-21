# PermeantOS Handoff

## Current Next Run

The PermeantOS production decision-path run has completed successfully. QATQ has
a production storage-decision API, the throughput-normalized gate passes on the
50-fixture PermeantOS evidence set, and PermeantOS has now validated both live
migration storage branches.

Use this document for general evidence-run background. The latest completed live
integration result is summarized in:

```text
handoff/permeantos/LIVE_DECISION_PATH_RESULTS.md
```

The executed run exercised both production paths:

- `Phase2EncodeDecision::Compressed` for compression-positive bfloat16-derived
  KV tensors;
- `Phase2EncodeDecision::PassThroughRaw` for compression-negative float32 KV
  tensors.

The large-tensor readiness gate is now
`docs/BENCHMARK_GATE_THROUGHPUT.md`. The fixed absolute-latency gate in
`docs/BENCHMARK_GATE.md` remains useful for small-tensor/service-budget
analysis, but it should not block large-tensor readiness by itself.

The remaining sections are the retained evidence-capture runbook. They still
apply when PermeantOS is producing new raw tensor fixtures. The previous
production integration instructions in `handoff/permeantos/NEXT_RUN_INSTRUCTIONS.md`
should now be treated as a regression recipe for revalidating the live migration
path after codec or PermeantOS transfer changes.

## Direct Task For PermeantOS

PermeantOS should execute this document as an evidence handoff.

The task is:

```text
Use the QATQ repository to capture real PermeantOS tensor data, validate that the
current exact codec path reconstructs those tensors bit-for-bit, benchmark the
codec on those tensors, and return the generated reports plus enough metadata
for QATQ to analyze the results and decide the next codec or integration step.
```

The task is not:

- to integrate QATQ into PermeantOS runtime paths;
- to fork QATQ codec internals into PermeantOS;
- to tune thresholds until the gate passes;
- to convert private failures into informal summaries;
- to make paper or readiness claims from synthetic-only results.

The required codec under test is `phase2-lossless`. The required large-artifact
container is `QATC`. Other codec rows are baselines for comparison.

## Copy/Paste Instruction For PermeantOS

Use this as the top-level instruction when assigning the work to PermeantOS:

```text
You are running the QATQ evidence handoff for PermeantOS.

Work from the QATQ repository root, not from the PermeantOS source tree. Record
the exact QATQ commit, PermeantOS commit, runtime versions, model identity,
hardware, and capture date. Do not integrate QATQ into PermeantOS yet.

Export real PermeantOS tensor captures as raw little-endian IEEE-754 f32 files
with no container header. Prefer KV-cache tensors from migration scenarios, plus
activation-like and stress tensors if available. Add those files to
fixtures/permeantos.manifest with complete metadata, verify the manifest, run
the benchmark report, run the readiness gate, and perform at least one direct
phase2-lossless cmp check plus one QATC cmp check for a large capture.

Return the generated reports, manifest, run notes, command log, and either raw
captures or stable private artifact references. If a step fails, preserve the
exact command and output, mark the handoff as failed or analysis-only, and hand
the bundle back to QATQ for analysis. Do not tune thresholds, fork codec code,
or make readiness claims from synthetic-only results.
```

## Execution Rules

PermeantOS should follow these rules while executing the handoff:

- run every QATQ command from the QATQ repository root;
- treat `docs/PERMEANTOS_HANDOFF.md` as the source of truth for the run;
- keep raw captures outside git unless the project owner explicitly approves
  committing them;
- prefer relative fixture paths in `fixtures/permeantos.manifest`;
- preserve all generated reports even when the readiness gate fails;
- never edit codec internals during the evidence run;
- never relax benchmark thresholds without recording the reason in
  `handoff/permeantos/RUN_NOTES.md`;
- stop integration work if exact reconstruction fails;
- hand failures back to QATQ with enough artifacts to reproduce or inspect the
  failure.

The handoff has two possible successful outcomes:

- `passed`: real captures verified, benchmarks generated, gate passed, exact
  spot checks passed.
- `analysis-only`: real captures verified or partially verified, useful reports
  exist, but the readiness gate or another non-exactness criterion failed.

Any bit-exactness failure is `failed`, not `analysis-only`.

## Operator Summary

PermeantOS should treat the retained sections below as the data-capture and
evidence-generation runbook. The live runtime integration task has since been
completed and is summarized in `handoff/permeantos/LIVE_DECISION_PATH_RESULTS.md`.

The expected flow is:

1. Checkout QATQ and record the exact branch/commit.
2. Build and test QATQ before adding PermeantOS captures.
3. Export raw little-endian `.f32le` tensors from PermeantOS migration runs.
4. Add those captures to `fixtures/permeantos.manifest`.
5. Verify the manifest and write `docs/FIXTURE_AUDIT.md`.
6. Generate `docs/BENCHMARKS.md` and `docs/PAPER_TABLES.md`.
7. Run the production readiness gate and write
   `docs/BENCHMARK_GATE_THROUGHPUT.md`; run the fixed latency-budget analysis
   separately into `docs/BENCHMARK_GATE.md`.
8. Run at least one direct `cmp` spot check for `QATQ` and, for large captures,
   one `QATC` container spot check.
9. Return the manifest, reports, run notes, and either raw captures or stable
   private storage references to QATQ.

QATQ will then analyze the returned reports, update the paper/white-paper
tables, inspect failures or weak ratios, and decide the next codec development
step before any PermeantOS integration work proceeds.

## Canonical Command Sequence

This is the preferred command sequence for a complete handoff. The detailed
sections below explain each step and how to handle failures.

```sh
# 1. Start from the QATQ repository root.
pwd
git rev-parse --abbrev-ref HEAD
git rev-parse HEAD

# 2. Record toolchain and host information.
rustc --version
cargo --version
uname -a

# 3. Create handoff directories.
mkdir -p handoff/permeantos/failures fixtures captures docs

# 4. Validate QATQ before adding real captures.
cargo fmt --check
cargo check
cargo test

# 5. Export PermeantOS tensors into captures/<run-id>/*.f32le.
# This step is owned by PermeantOS. The files must be raw little-endian f32.

# 6. Add every exported tensor to the manifest.
cargo run -- fixture add \
  --manifest fixtures/permeantos.manifest \
  --group permeantos-kv \
  --name <stable-tensor-name> \
  --path captures/<run-id>/<stable-tensor-name>.f32le \
  --shape "<shape>" \
  --notes "<complete semicolon-separated metadata>"

# 7. Verify all fixture paths, byte counts, and fingerprints.
cargo run -- fixture verify \
  --manifest fixtures/permeantos.manifest \
  --output docs/FIXTURE_AUDIT.md

# 8. Generate benchmark and paper-table reports.
cargo run --release --bin qatq-bench -- \
  --output docs/BENCHMARKS.md \
  --paper-output docs/PAPER_TABLES.md \
  --manifest fixtures/permeantos.manifest

# 9. Run the production readiness gate against real external fixtures.
cargo run --release --bin qatq-bench -- \
  --phase2-only \
  --no-synthetic \
  --manifest fixtures/permeantos.manifest \
  --gate-output docs/BENCHMARK_GATE_THROUGHPUT.md \
  --gate-require-external \
  --gate-policy production-kv \
  --max-phase2-ratio 0.95 \
  --max-phase2-encode-us 5000 \
  --max-phase2-decode-ns-per-value 3.00 \
  --max-phase2-container-ratio 0.96 \
  --max-phase2-container-decode-ns-per-value 3.00

# 10. Run at least one direct exact payload spot check.
cargo run -- encode --mode phase2-lossless \
  captures/<run-id>/<tensor>.f32le \
  /tmp/<tensor>.qatq
cargo run -- decode /tmp/<tensor>.qatq /tmp/<tensor>.restored.f32le
cmp captures/<run-id>/<tensor>.f32le /tmp/<tensor>.restored.f32le

# 11. Run at least one QATC container exact spot check for a large capture.
cargo run -- encode-chunked \
  --max-values-per-chunk 65536 \
  captures/<run-id>/<large-tensor>.f32le \
  /tmp/<large-tensor>.qatc
cargo run -- decode /tmp/<large-tensor>.qatc /tmp/<large-tensor>.restored.f32le
cmp captures/<run-id>/<large-tensor>.f32le /tmp/<large-tensor>.restored.f32le
```

The final handoff must include the command transcript or a manually written
equivalent in `handoff/permeantos/commands.log`.

## Deliverable Contract

PermeantOS should hand back a bundle with enough information for QATQ to
reproduce or audit the run without guessing.

Required files:

- `fixtures/permeantos.manifest`
- `docs/FIXTURE_AUDIT.md`
- `docs/BENCHMARKS.md`
- `docs/PAPER_TABLES.md`
- `docs/BENCHMARK_GATE_THROUGHPUT.md`
- `docs/BENCHMARK_GATE.md`
- `handoff/permeantos/RUN_NOTES.md`
- `handoff/permeantos/commands.log` or equivalent command transcript

Required identifiers:

- QATQ branch and commit SHA;
- PermeantOS branch and commit SHA;
- source runtime name and version;
- destination runtime name and version;
- model name and variant;
- capture date;
- host OS, CPU, GPU, and memory;
- Rust toolchain versions.

Required fixture evidence:

- raw `.f32le` byte count for every capture;
- `f32` value count for every capture;
- fixture audit fingerprint for every capture;
- shape and tensor role for every capture;
- explicit original dtype before export;
- explicit conversion notes if the source dtype was not `f32`.

If raw tensors cannot be committed or attached, the handoff is still acceptable
only if the manifest, audit report, and run notes contain stable private artifact
references plus byte counts and fingerprints.

## Success Criteria

The handoff is successful when all of the following are true:

- baseline QATQ validation passes before real captures are added;
- every capture is raw little-endian `f32` with byte length divisible by four;
- `cargo run -- fixture verify` exits successfully;
- benchmark reports are generated from `fixtures/permeantos.manifest`;
- the readiness gate is run with external fixtures required;
- `phase2-lossless` is bit-exact for every PermeantOS fixture;
- the `QATC` container is bit-exact for large-container checks;
- pass/fail status and threshold deviations are recorded in run notes.

A failed gate can still be a useful analysis handoff, but it is not a readiness
handoff. Mark it as `analysis-only` in `RUN_NOTES.md`.

## Objective

Use real PermeantOS migration tensors to decide whether QATQ is ready for
runtime integration.

The codec under test is `phase2-lossless`. It must reconstruct raw `f32` bits
exactly. The lossy modes, including `phase1-q4` and `lossy-i4`, are research
baselines only. They must not be treated as migration-safe codecs.

QATQ currently has good synthetic coverage, but synthetic results are not enough
for paper, white-paper, or PermeantOS readiness claims. PermeantOS needs to
provide real KV-cache and tensor captures, run the QATQ fixture and benchmark
tools, then hand the generated artifacts back to this repository.

## Repository Map

Run all commands from the QATQ repository root.

Important files and directories:

- `README.md`: project overview and common commands.
- `src/lib.rs`: codec implementation and public library API.
- `src/main.rs`: QATQ CLI, including `encode`, `decode`, and `fixture`.
- `src/bin/qatq-bench.rs`: benchmark harness and readiness gate.
- `docs/FIXTURES.md`: fixture manifest format.
- `docs/BENCHMARKS.md`: generated benchmark report.
- `docs/PAPER_TABLES.md`: generated paper/white-paper draft tables.
- `docs/VALIDATION.md`: QATQ validation history.
- `docs/PERMEANTOS_HANDOFF.md`: this runbook.
- `fixtures/permeantos.manifest`: expected PermeantOS fixture manifest path.
- `captures/`: recommended local directory for raw PermeantOS tensor captures.
- `handoff/permeantos/`: recommended optional directory for run notes and command
  logs that should travel with the handoff bundle.

The `fixtures/` and `captures/` directories may need to be created by
PermeantOS during the handoff run.

Recommended handoff layout:

```text
captures/<run-id>/*.f32le
fixtures/permeantos.manifest
docs/FIXTURE_AUDIT.md
docs/BENCHMARKS.md
docs/PAPER_TABLES.md
docs/BENCHMARK_GATE.md
handoff/permeantos/RUN_NOTES.md
handoff/permeantos/commands.log
handoff/permeantos/failures/
```

Do not commit private captures unless the project owner has explicitly approved
that. The reports and manifest are still useful if the capture files live in a
private artifact store and can be identified by path, byte count, and
fingerprint.

## Preconditions

PermeantOS should start from a known QATQ commit or branch and record it in the
handoff notes:

```sh
git rev-parse --abbrev-ref HEAD
git rev-parse HEAD
```

Rust must be available:

```sh
rustc --version
cargo --version
```

Before adding real captures, verify that QATQ builds and its local tests pass:

```sh
cargo fmt --check
cargo check
cargo test
```

If any of these fail before PermeantOS captures are added, stop and hand the
failure back to QATQ. Do not mix environment failures with codec evidence.

Create the run-note directory before starting the capture workflow:

```sh
mkdir -p handoff/permeantos/failures fixtures captures docs
```

Record the baseline environment:

```sh
{
  echo "# PermeantOS QATQ Run Notes"
  echo
  echo "## QATQ"
  echo "- branch: $(git rev-parse --abbrev-ref HEAD)"
  echo "- commit: $(git rev-parse HEAD)"
  echo
  echo "## Toolchain"
  echo "- rustc: $(rustc --version)"
  echo "- cargo: $(cargo --version)"
  echo
  echo "## Host"
  uname -a
} > handoff/permeantos/RUN_NOTES.md
```

If PermeantOS has its own repository checkout available on the same machine,
append its branch and commit to `handoff/permeantos/RUN_NOTES.md`.

## What PermeantOS Must Export

Export raw little-endian `f32` files.

Do not wrap the tensor bytes in:

- NumPy `.npy`;
- safetensors;
- JSON;
- msgpack;
- custom runtime headers;
- compressed archives as the benchmark input itself.

Each fixture file must contain only contiguous little-endian IEEE-754 `f32`
values. Its byte length must be divisible by four.

The export step must preserve exact `f32` bits after conversion to little-endian
bytes. If the source runtime stores BF16, FP16, FP8, quantized, or packed tensor
data, PermeantOS must record that original dtype in the metadata and explicitly
state how it converted the values to exported `f32le`.

Required export check for each file:

```sh
wc -c captures/<run-id>/<tensor>.f32le
```

The byte count must be divisible by four. Empty tensors are allowed only if they
are an intentional runtime case and the `notes` field says why they are empty.

Recommended capture groups:

- `permeantos-kv`: K/V cache tensors from migration runs.
- `permeantos-activation`: activation-like intermediate tensors.
- `permeantos-stress`: outlier-heavy, sparse, repetitive, or adversarial runtime
  tensors.

Minimum useful KV capture set:

- one K tensor and one V tensor from the same model layer before migration;
- matching K/V tensors after migration when available;
- one short-context run;
- one longer-context run;
- at least one capture from the MLX source side;
- at least one capture from the vLLM destination side;
- model/runtime metadata recorded in the fixture `notes` field.

Recommended filename convention:

```text
captures/<run-id>/<model>-layer<layer>-<k|v>-<source|dest>.f32le
```

Example:

```text
captures/2026-06-21-mlx-vllm/llama-layer12-k-mlx-source.f32le
captures/2026-06-21-mlx-vllm/llama-layer12-v-mlx-source.f32le
captures/2026-06-21-mlx-vllm/llama-layer12-k-vllm-dest.f32le
captures/2026-06-21-mlx-vllm/llama-layer12-v-vllm-dest.f32le
```

Avoid reusing filenames across runs. A filename should identify the model,
layer, tensor side, runtime side, and capture direction well enough that the
same file can be interpreted months later.

## Required Metadata

Every fixture entry should record enough context for QATQ to interpret the
numbers later.

Use the manifest fields as follows:

- `group`: one of the `permeantos-*` groups above.
- `name`: stable short identifier for the tensor.
- `path`: path to the raw `.f32le` file, relative to the manifest when possible.
- `shape`: human-readable tensor shape.
- `notes`: semicolon-separated run metadata.

The `notes` field should include:

- model name;
- model size or variant when relevant;
- PermeantOS commit;
- source runtime and version;
- destination runtime and version;
- capture side, such as `source-before-migration` or
  `dest-after-migration`;
- layer index;
- K/V marker when relevant;
- head count;
- head dimension;
- token count;
- dtype before export;
- host CPU/GPU;
- OS;
- capture date.

Example `notes` value:

```text
model=llama; permeantos=abc1234; source=mlx-0.27; dest=vllm-0.9; layer=12; tensor=k; heads=32; tokens=128; dim=128; capture=source-before-migration; host=m3-max; os=macos; date=2026-06-21
```

Also include any data-handling caveat, for example:

```text
original_dtype=bf16; export=converted-to-f32le; privacy=private-artifact-store; artifact=permeantos://runs/2026-06-21/llama-layer12-k
```

For paper and white-paper use, missing metadata is a real limitation. If a field
is unknown, write `unknown` rather than leaving it implicit.

## Add Captures To The Manifest

Create the expected directories if needed:

```sh
mkdir -p fixtures captures docs
```

Add one fixture entry per tensor:

```sh
cargo run -- fixture add \
  --manifest fixtures/permeantos.manifest \
  --group permeantos-kv \
  --name llama-layer12-k-mlx-source \
  --path captures/2026-06-21-mlx-vllm/llama-layer12-k-mlx-source.f32le \
  --shape "[layers=1, heads=32, tokens=128, dim=128]" \
  --notes "model=llama; permeantos=abc1234; source=mlx; dest=vllm; layer=12; tensor=k; heads=32; tokens=128; dim=128; capture=source-before-migration"
```

Repeat this command for every capture.

The command validates:

- the file exists;
- the file length is divisible by four;
- the manifest entry can be appended.

If a fixture has already been added with wrong metadata, edit
`fixtures/permeantos.manifest` carefully or regenerate the manifest. The
manifest is intentionally small and plain text.

After adding entries, inspect the manifest:

```sh
sed -n '1,240p' fixtures/permeantos.manifest
```

Every `[fixture]` block must have a unique `name`, a valid `path`, and enough
`notes` metadata to map the row back to a PermeantOS run.

## Verify The Manifest

Run:

```sh
cargo run -- fixture verify \
  --manifest fixtures/permeantos.manifest \
  --output docs/FIXTURE_AUDIT.md
```

Expected result:

- command exits with status `0`;
- `docs/FIXTURE_AUDIT.md` exists;
- every row has the expected value count and byte count;
- every fixture has a stable `fingerprint fnv1a64`.

If verification fails, fix the file path, file format, shape, or notes before
benchmarking. Do not benchmark a partially valid manifest.

Append a short verification summary to the run notes:

```sh
{
  echo
  echo "## Fixture Audit"
  sed -n '1,80p' docs/FIXTURE_AUDIT.md
} >> handoff/permeantos/RUN_NOTES.md
```

## Run Codec Benchmarks

Run the benchmark harness with the PermeantOS manifest:

```sh
cargo run --release --bin qatq-bench -- \
  --output docs/BENCHMARKS.md \
  --paper-output docs/PAPER_TABLES.md \
  --manifest fixtures/permeantos.manifest
```

Expected outputs:

- `docs/BENCHMARKS.md`: detailed codec table with synthetic rows and
  PermeantOS fixture rows.
- `docs/PAPER_TABLES.md`: summarized tables intended for the refreshed QATQ
  paper and accompanying white paper.

For real claims, use only rows whose group starts with `permeantos-`. Synthetic
rows are development controls.

If PermeantOS wants to benchmark a single capture before creating a manifest,
use:

```sh
cargo run --release --bin qatq-bench -- \
  --output docs/BENCHMARKS.md \
  --paper-output docs/PAPER_TABLES.md \
  --input permeantos-kv:captures/<run-id>/<tensor>.f32le
```

Add `--no-synthetic` to smoke-test or gate only the provided PermeantOS captures.
The benchmark harness preflights external fixture paths and raw `.f32le` byte
lengths before running timing loops, so a missing or malformed capture fails
before replacing benchmark, paper-table, or gate reports.

That single-input path is useful for smoke testing, but the final handoff should
use `fixtures/permeantos.manifest` so metadata and fingerprints are preserved.

## Run The Production Readiness Gate

Run the production KV gate with explicit throughput-normalized thresholds:

```sh
cargo run --release --bin qatq-bench -- \
  --phase2-only \
  --no-synthetic \
  --manifest fixtures/permeantos.manifest \
  --gate-output docs/BENCHMARK_GATE_THROUGHPUT.md \
  --gate-require-external \
  --gate-policy production-kv \
  --max-phase2-ratio 0.95 \
  --max-phase2-encode-us 5000 \
  --max-phase2-decode-ns-per-value 3.00 \
  --max-phase2-container-ratio 0.96 \
  --max-phase2-container-decode-ns-per-value 3.00
```

Gate meaning:

- `--gate-require-external` fails the run if no external fixtures were loaded.
- `--gate-policy production-kv` fails the run if fixed decode-us ceilings are
  mixed into the production KV gate.
- `--max-phase2-ratio 0.95` requires `phase2-lossless` to produce output no
  larger than 95% of raw `f32` bytes for each benchmarked tensor.
- `--max-phase2-encode-us 5000` requires encode time at or below 5000
  microseconds per benchmarked tensor on the current machine.
- `--max-phase2-decode-ns-per-value 3.00` requires direct decode throughput at
  or below 3.00 nanoseconds per f32 value.
- `--max-phase2-container-ratio 0.96` requires the sequential `QATC` artifact
  to produce output no larger than 96% of raw `f32` bytes for each benchmarked
  tensor.
- `--max-phase2-container-decode-ns-per-value 3.00` requires `QATC` decode
  throughput at or below 3.00 nanoseconds per f32 value.
- `--phase2-only` skips unrelated codec rows during readiness gates and times
  only `phase2-lossless` plus QATC.
- bit-exact reconstruction is mandatory.

The command exits nonzero if the gate fails. Keep failed gate reports. A failed
gate is useful evidence and should be handed back to QATQ.

Run fixed absolute latency separately only as service-budget analysis:

```sh
cargo run --release --bin qatq-bench -- \
  --phase2-only \
  --no-synthetic \
  --manifest fixtures/permeantos.manifest \
  --gate-output docs/BENCHMARK_GATE.md \
  --gate-require-external \
  --gate-policy latency-budget \
  --max-phase2-ratio 0.95 \
  --max-phase2-encode-us 5000 \
  --max-phase2-decode-us 1000 \
  --max-phase2-container-ratio 0.96 \
  --max-phase2-container-decode-us 1200
```

Thresholds may be adjusted only when the reason is recorded in the run notes.
Do not silently relax thresholds and report the run as ready.

Append the gate result to the run notes:

```sh
{
  echo
  echo "## Benchmark Gate"
  sed -n '1,160p' docs/BENCHMARK_GATE.md
} >> handoff/permeantos/RUN_NOTES.md
```

If the gate fails, keep going only far enough to preserve artifacts and describe
the failure. Do not begin integration work against a failed gate.

## Optional Exact Round Trip Spot Check

For one selected capture from each important group, run a direct
encode/decode/cmp check:

```sh
cargo run -- encode --mode phase2-lossless \
  captures/2026-06-21-mlx-vllm/llama-layer12-k-mlx-source.f32le \
  /tmp/llama-layer12-k.qatq

cargo run -- decode \
  /tmp/llama-layer12-k.qatq \
  /tmp/llama-layer12-k.restored.f32le

cmp captures/2026-06-21-mlx-vllm/llama-layer12-k-mlx-source.f32le \
  /tmp/llama-layer12-k.restored.f32le
```

`cmp` must produce no output and exit with status `0`.

This spot check does not replace the benchmark gate. It is a simple independent
sanity check for the exact path.

Record the checked filenames and `cmp` exit status in
`handoff/permeantos/RUN_NOTES.md`.

## Optional Large Tensor Container Check

For captures that should be carried as a single chunked artifact, use the Phase
2 `QATC` container:

```sh
cargo run -- encode-chunked \
  --max-values-per-chunk 65536 \
  captures/2026-06-21-mlx-vllm/llama-layer12-k-mlx-source.f32le \
  /tmp/llama-layer12-k.qatc

cargo run -- decode \
  /tmp/llama-layer12-k.qatc \
  /tmp/llama-layer12-k.container-restored.f32le

cmp captures/2026-06-21-mlx-vllm/llama-layer12-k-mlx-source.f32le \
  /tmp/llama-layer12-k.container-restored.f32le
```

`cmp` must produce no output and exit with status `0`.

The `QATC` format is a sequential container around normal `phase2-lossless`
payloads. It is appropriate for large tensor files and handoff artifacts. It is
not yet a random-access runtime streaming protocol. The QATQ CLI decodes `QATC`
containers chunk by chunk through a temporary output file. For both `QATQ` and
`QATC`, CLI `decode` replaces the final target only after the full payload
validates and writes successfully.

Use `QATC` for any capture that is near or above the single-payload limit, or
when PermeantOS wants to evaluate a file artifact shaped like a future runtime
transport packet.

## Command Log

Prefer preserving a command transcript. One simple option is:

```sh
script handoff/permeantos/commands.log
```

Then run the commands in this document. Exit the transcript shell when complete.

If `script` is unavailable, manually append the commands and important stderr
or stdout excerpts to `handoff/permeantos/commands.log`.

The command log should include:

- the baseline `cargo fmt --check`, `cargo check`, and `cargo test` results;
- every `fixture add` command;
- the `fixture verify` command;
- the benchmark command;
- the readiness gate command;
- any direct `cmp` spot checks;
- any failure output.

## Run Notes Template

If the automatic note commands above are not used, create
`handoff/permeantos/RUN_NOTES.md` with this structure:

```markdown
# PermeantOS QATQ Run Notes

## Summary
- status: passed | failed | analysis-only
- reason:
- date:

## QATQ
- branch:
- commit:

## PermeantOS
- branch:
- commit:
- capture code path:

## Runtime
- source runtime:
- destination runtime:
- model:
- model variant:
- migration scenario:

## Host
- OS:
- kernel/build:
- CPU:
- GPU:
- memory:

## Captures
- capture directory:
- privacy/storage location:
- fixture manifest:
- fixture audit:

## Validation
- cargo fmt --check:
- cargo check:
- cargo test:
- fixture verify:
- benchmark generation:
- readiness gate:
- spot checks:

## Deviations
- threshold changes:
- missing metadata:
- failed commands:
- intentionally empty tensors:

## QATQ Follow-Up Requested
- codec analysis needed:
- benchmark interpretation needed:
- suspected bug:
- integration question:
```

## What To Hand Back To QATQ

Return this handoff bundle:

- `fixtures/permeantos.manifest`
- `docs/FIXTURE_AUDIT.md`
- `docs/BENCHMARKS.md`
- `docs/PAPER_TABLES.md`
- `docs/BENCHMARK_GATE_THROUGHPUT.md`
- `docs/BENCHMARK_GATE.md`
- `handoff/permeantos/RUN_NOTES.md`
- `handoff/permeantos/commands.log` when available
- QATQ branch name and commit SHA
- PermeantOS commit SHA
- MLX version
- vLLM version
- Rust version
- OS and kernel/build version
- host CPU/GPU
- model name and variant
- capture date
- any command output from failed steps

If raw tensor files are private or too large to commit, do not force them into
git. Store them in the agreed private location and ensure the manifest paths,
fixture audit byte counts, and fixture audit fingerprints identify them
unambiguously.

If raw tensor files can be shared with QATQ, keep their relative paths matching
`fixtures/permeantos.manifest`.

If the raw files cannot be shared, QATQ still needs:

- stable private artifact identifiers;
- byte counts from `docs/FIXTURE_AUDIT.md`;
- `fingerprint fnv1a64` values from `docs/FIXTURE_AUDIT.md`;
- enough metadata to request a specific capture again;
- permission details for who can access the private artifact store.

Do not summarize failed commands from memory. Preserve the original command and
error text in `commands.log` or `RUN_NOTES.md`.

## Handoff Message Back To QATQ

When PermeantOS returns the bundle, include a short top-level message with this
exact structure:

```markdown
# PermeantOS -> QATQ Handoff

## Status
- status: passed | failed | analysis-only
- reason:
- QATQ commit:
- PermeantOS commit:

## Artifacts
- manifest:
- fixture audit:
- benchmark report:
- paper tables:
- gate report:
- run notes:
- command log:
- raw captures location:

## Gate
- command:
- result:
- threshold changes:

## Exactness
- phase2-lossless exact for all fixtures: yes | no
- QATC spot check exact: yes | no | not-run
- failing fixture names:

## Requested QATQ Action
- analyze compression ratios
- analyze latency
- debug exactness failure
- add importer/exporter support
- update paper tables
- decide PermeantOS integration readiness
```

The `Requested QATQ Action` field should be specific. For example, prefer
`debug exactness failure on permeantos-kv/llama-layer12-k-mlx-source` over
`look at results`.

If the handoff is passed through a private artifact store, the message must also
include who can access the store and whether QATQ is allowed to copy captures
into local `captures/` paths for analysis.

## QATQ Intake Protocol

QATQ should treat every returned PermeantOS bundle as untrusted evidence until
the local intake checks pass.

Initial intake:

```sh
git status --short
sed -n '1,240p' handoff/permeantos/RUN_NOTES.md
sed -n '1,240p' fixtures/permeantos.manifest
sed -n '1,200p' docs/FIXTURE_AUDIT.md
sed -n '1,200p' docs/BENCHMARK_GATE.md
```

If raw captures are available locally, rerun the verification and benchmarks:

```sh
cargo fmt --check
cargo check
cargo test
cargo run -- fixture verify \
  --manifest fixtures/permeantos.manifest \
  --output docs/FIXTURE_AUDIT.md
cargo run --release --bin qatq-bench -- \
  --output docs/BENCHMARKS.md \
  --paper-output docs/PAPER_TABLES.md \
  --manifest fixtures/permeantos.manifest
cargo run --release --bin qatq-bench -- \
  --phase2-only \
  --manifest fixtures/permeantos.manifest \
  --gate-output docs/BENCHMARK_GATE.md \
  --gate-require-external \
  --max-phase2-ratio 0.95 \
  --max-phase2-encode-us 5000 \
  --max-phase2-decode-us 1000 \
  --max-phase2-container-ratio 0.96 \
  --max-phase2-container-decode-us 1200
```

If raw captures are not available locally, QATQ should still inspect:

- manifest completeness;
- fixture audit byte counts and fingerprints;
- benchmark rows for every `permeantos-*` fixture;
- gate report pass/fail reason;
- command log consistency;
- metadata sufficiency for paper/white-paper use.

QATQ should then update:

- `docs/VALIDATION.md` with intake commands and results;
- `docs/BENCHMARKS.md` and `docs/PAPER_TABLES.md` if regenerated locally;
- `docs/ROADMAP.md` with the next codec or integration decision;
- `docs/CODEX_HANDOFF.md` with the next recommended Codex task;
- paper and white-paper drafts only after the evidence is stable.

If QATQ finds weak compression or latency but exactness holds, the next
development task should be residual-pattern analysis and codec strategy tuning.
If exactness fails, the next task is a correctness debug against the smallest
reproducible fixture, not performance work.

## How QATQ Will Use The Handoff

QATQ will use the returned artifacts to:

- verify `phase2-lossless` exactness on real PermeantOS tensors;
- compare `phase2-lossless` against `lossless-f32`, `phase1-q4`, `lossy-i4`,
  and the local FP8 E4M3 baseline;
- inspect compression ratios and encode/decode latency on real KV-cache and
  activation-like tensors;
- identify whether the fast default Phase 2 strategy is sufficient;
- compare fast Phase 2 against exhaustive Phase 2 where deeper search is useful;
- inspect residual patterns for future QATQ-specific residual coding;
- update `docs/BENCHMARKS.md`, `docs/PAPER_TABLES.md`, and the refreshed
  paper/white-paper evidence tables;
- decide whether PermeantOS integration should proceed, wait for codec changes,
  or target a narrower tensor class first.

QATQ should not treat a PermeantOS handoff as successful unless:

- fixture verification passed;
- benchmark generation completed;
- the readiness gate passed or the failure was explicitly accepted for a
  research-only analysis run;
- `phase2-lossless` remained bit-exact for every PermeantOS fixture.

## Failure Handling

If fixture verification fails:

- fix manifest paths first;
- verify each file is raw little-endian `f32`;
- verify byte lengths are divisible by four;
- correct wrong shape or notes fields;
- rerun `cargo run -- fixture verify`.

If the benchmark command fails:

- keep the failing command and error output;
- confirm that all manifest paths resolve from the manifest location;
- confirm that the input files are not empty unless an empty tensor is
  intentional;
- hand the failure back to QATQ if the cause is inside codec or benchmark code.

If the readiness gate fails:

- keep `docs/BENCHMARK_GATE.md`;
- do not mark QATQ as PermeantOS-ready;
- return the gate report, benchmark report, manifest, and fixture audit to
  QATQ;
- include any run context that may explain the failure.

If exact reconstruction fails:

- stop integration work for that capture;
- preserve the source tensor;
- preserve the encoded `.qatq` payload if one was created;
- preserve the restored `.f32le` tensor if one was created;
- preserve the command output;
- hand all of those artifacts back to QATQ for codec debugging.

If PermeantOS cannot export raw `f32le`:

- stop the benchmark workflow;
- document the actual export format available;
- document dtype, endianness, shape, and any container format;
- provide one small sample file if allowed;
- hand this back to QATQ so an explicit importer can be designed.

If the manifest or reports contain sensitive model or customer data:

- keep the raw files out of git;
- redact only fields that must be redacted;
- preserve non-sensitive technical metadata such as shape, dtype, byte count,
  runtime versions, and fingerprints;
- record exactly what was redacted.

## Current Boundaries

Current QATQ boundaries that PermeantOS should respect:

- `phase2-lossless` is exact, but not yet proven compression-positive on real
  PermeantOS captures.
- The benchmark harness is a single-process codec microbenchmark, not an
  end-to-end runtime migration benchmark.
- The FP8 row is a local software E4M3 comparison, not a hardware/runtime FP8
  implementation.
- Single QATQ payloads are bounded to `67,108,864` `f32` values.
- Very large tensors should use the Phase 2 `QATC` chunk container or the QATQ
  chunked library APIs so each embedded payload stays within the single-payload
  decoder bound.
- `QATC` is sequential; random-access metadata and service-level streaming are
  still future runtime-integration work.
- PermeantOS should not vendor or fork codec internals based on this phase.
  Treat QATQ as the source of truth for codec behavior.
- This handoff does not prove end-to-end migration quality by itself. It proves
  codec exactness, compression ratio, and local encode/decode latency on exported
  tensors. Runtime migration success still needs PermeantOS integration tests
  after QATQ accepts the codec evidence.

## Minimal Successful Handoff Checklist

A complete handoff has:

- raw `.f32le` tensor captures;
- `fixtures/permeantos.manifest`;
- passing `docs/FIXTURE_AUDIT.md`;
- generated `docs/BENCHMARKS.md`;
- generated `docs/PAPER_TABLES.md`;
- generated `docs/BENCHMARK_GATE.md`;
- recorded QATQ commit;
- recorded PermeantOS/runtime/hardware metadata;
- explicit note saying whether the gate passed or failed.

Only after that bundle exists should QATQ perform deeper analysis and decide the
next development step.

## Stop Conditions

Stop and hand back to QATQ immediately if any of these occur:

- QATQ fails baseline `cargo fmt --check`, `cargo check`, or `cargo test`.
- PermeantOS cannot export raw little-endian `.f32le` tensors.
- Fixture verification fails after paths and file formats are corrected.
- `phase2-lossless` or `QATC` fails exact `cmp` reconstruction.
- The benchmark harness panics or rejects a valid manifest.
- The readiness gate fails and PermeantOS needs a go/no-go decision.

The correct next step after a stop condition is analysis in QATQ, not local
workarounds in PermeantOS.
