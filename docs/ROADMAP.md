# Roadmap

## Phase 0 - Seed

- [x] Split QATQ into its own repository.
- [x] Add a Rust library crate and CLI.
- [x] Preserve the original lossy int4 path as a seed baseline.
- [x] Add exact f32 envelope mode for bit-identical control tests.
- [x] Document that int4 QATQ is lossy and not the full paper implementation.

## Phase 1 - Lossy Predictor And Comparator Research

- [x] Implement quaternion grouping and Hamilton product rotation.
- [x] Add deterministic rotation seed/configuration handling.
- [x] Implement TurboQuant-style scalar quantization.
- [x] Add a base `turboquant-q4` comparator before the quaternion overlay.
- [x] Add QJL/residual side-channel experiments.
- [x] Benchmark against raw, zstd, lz4, FP8, base TurboQuant-style q4, and the
      seed lossy int4 baseline.

Phase 1 is implemented as the `phase1-q4` mode. It is retained as a lossy
predictor and measurement path, not as the main QATQ product surface. The
QJL/residual side channel is currently a compact global residual-magnitude plus
per-coordinate sign-bit experiment. It is useful for measurement but does not
claim lossless reconstruction.

The `turboquant-q4` mode is a local base reference path: deterministic
data-oblivious orthogonal rotation, scalar q4 quantization, and QJL residual
signs for query-side inner-product estimation using a structured signed-Hadamard
projection. It is included so QATQ can measure against a non-quaternion lossy
baseline. It is not an official Google implementation and is not the default
QATQ foundation.

## QATQ exact - Primary QATQ Exact Mode

- [x] Define exact reconstruction semantics.
- [x] Implement residual generation from QATQ reconstruction.
- [x] Entropy-code residuals and compare against the exact f32 envelope.
- [x] Add tests for bit-identical f32 reconstruction.

QATQ exact is implemented as `qatq-exact` and is the primary QATQ
implementation. It adaptively stores raw f32 bits, byte-RLE, byte-plane RLE,
byte-plane zstd, reversible quaternion-chain zstd, adjacent-bit delta-XOR
byte-plane residuals, or the Phase 1 predictor plus run-coded XOR residuals and
verifies final reconstruction with the payload checksum. Lossless QATQ claims
are scoped to QATQ exact and its `QATC` container. zstd/lz4 comparison rows are
included in benchmark reports as general-purpose byte-compression baselines over
raw f32le, and the competitive compression gate rejects public fixture
regressions against those baselines.

## Phase 3 - Runtime and Service Integration

- [x] Add production chunk APIs suitable for generic runtime adapters.
- [ ] Add a standalone codec service binary.
- [x] Add a generic runtime adapter contract and Rust production-chunk example.
- [ ] Add MLX, vLLM, and llama.cpp adapter examples as optional external
      integration examples.
- [x] Add chunked exact encode/decode APIs for large KV blocks.
- [x] Add a sequential QATQ exact chunk container for large tensor files.
- [x] Validate the QATQ exact production storage-decision API with generated public
      fixtures.
- [ ] Add random-access metadata and a true streaming container/service
      protocol.

The current QATQ implementation is usable for exact QATQ exact runtime transfer
experiments, but the broader project is not complete until service adapters,
release hygiene, and comparative paper baselines are finished.

The initial public release should stay focused on storage and transfer of
exported KV/tensor bytes: checkpoints, migration artifacts, runtime captures,
and fixture bundles. QATQ should not claim transparent live GPU VRAM reduction
for v0.1.

## Experimental Track - Live KV Paging and VRAM Reduction

Detailed implementation, test, stress, validation, and release gates are tracked
in [`docs/LIVE_VRAM_REDUCTION.md`](LIVE_VRAM_REDUCTION.md).

- [x] Add QATQ-side live KV page descriptors, bounded encode/restore APIs,
      conservative scheduler decisions, and offline simulation reporting.
- [x] Define a runtime KV page/offload adapter trait and versioned adapter
      identity contract.
- [x] Add per-page live VRAM evidence reports comparing QATQ, raw bytes, zstd,
      and lz4 over the same snapshots.
- [x] Add a llama.cpp export-manifest replay path via
      `qatq-kv-bench --live-vram-export-dir`.
- [x] Add QATQ-side cancellation safety, restore-stall accounting, and
      operator metrics for live adapter authors.
- [x] Add process-abort fail-closed evidence for the currently wired
      `llama-simple` adapter path.
- [x] Add in-process patched `llama-server` streaming request-cancellation
      evidence with QATQ live page staging enabled. The release-shaped
      1024-token page run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-releasepages-20260625`
      passed: the client disconnected after 256 streamed bytes, server health
      recovered, a follow-up request succeeded, and the page-segment trace
      recorded 896 attention events with 128 attention-consumed offloaded
      segments.
- [x] Harden smaller-page in-process server cancellation by deriving
      page-segment and graph-node budgets from the configured page policy.
      The previous 64-token page run failed closed at the adapter graph-object
      budget; the budgeted rerun at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-budgeted-20260625`
      passed with 952 page-segment events, 136 attention-consumed events, and
      584 live-offloaded segments while preserving server health and follow-up
      serving after client disconnect.
- [x] Add scoped multi-request, multi-slot shared-server cancellation evidence
      for the unified-KV adapter path. The two-slot run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-concurrent-kvu-20260625`
      started a follow-up completion while the streaming request was still
      open, cancelled the stream, completed the follow-up after cancellation,
      recovered health, and recorded 1,120 page-segment events with 1,312
      live-offloaded segments.
- [x] Add scoped non-unified two-slot server cancellation evidence. The retained
      tiled page-pool route now returns no QATQ live segments for unsupported
      multi-stream reserve shapes instead of aborting; real one-stream slot
      work can still use live page staging. The non-unified two-slot run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-concurrent-nonunified-20260625`
      started a follow-up request before cancellation, completed it after
      cancellation, recovered health, and recorded 1,120 page-segment events
      with 544 live-offloaded segments.
- [x] Add bounded shared-server cancellation soak evidence. The server probe
      now supports repeated cancellation/follow-up cycles against one
      `llama-server` process with per-iteration health gates. The 20-cycle
      non-unified run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-soak20-20260625`
      passed all 20/20 iterations and recorded 10,696 page-segment events,
      1,288 attention-consumed events, and 10,728 live-offloaded segments. The
      matching 20-cycle unified-KV run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-kvu-soak20-20260625`
      passed all 20/20 iterations and recorded 9,632 page-segment events, 1,376
      attention-consumed events, and 24,568 live-offloaded segments.
- [x] Add bounded host-pressure and RSS-growth gates to shared-server
      cancellation soak evidence. The 1 GiB host-pressure non-unified run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-pressure1g-soak20-20260625`
      passed 20/20 iterations with 10,728 live-offloaded segments and
      59.3 MiB server RSS growth after readiness. The matching unified-KV run
      at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-kvu-pressure1g-soak20-20260625`
      passed 20/20 iterations with 24,568 live-offloaded segments and
      44.8 MiB server RSS growth after readiness.
- [x] Add bounded shared-server latency gates to pressure soak evidence. The
      latency-gated 1 GiB pressure non-unified run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-pressure1g-latency-soak100-20260625`
      passed 100/100 iterations with p95 iteration latency 4.80s, p95
      concurrent follow-up latency 2.61s, 53,608 live-offloaded segments, and
      61.25 MiB RSS growth. The matching unified-KV run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-kvu-pressure1g-latency-soak100-20260625`
      passed 100/100 iterations with p95 iteration latency 4.83s, p95
      concurrent follow-up latency 2.68s, 122,488 live-offloaded segments, and
      44.8 MiB RSS growth.
- [x] Broaden shared-server cancellation pressure evidence beyond the 1.5B
      fixture. The Qwen2.5 1.5B harsher 2 GiB pressure runs at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-pressure2g-latency-soak50-20260625`
      and
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-kvu-pressure2g-latency-soak50-20260625`
      passed 50/50 iterations with p95 iteration/follow-up latency
      4.78s/2.62s and 4.88s/2.69s respectively. The Qwen2.5 3B 1 GiB pressure
      runs at
      `/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-pressure1g-latency-soak20-20260625`
      and
      `/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-kvu-pressure1g-latency-soak20-20260625`
      passed 20/20 iterations with p95 iteration/follow-up latency
      7.65s/4.31s and 7.73s/4.39s respectively.
- [x] Add patched llama.cpp attention-path and backend-storage proofs,
      including page-composed K/V source consumption, persistent backend
      page-source attention consumption, and retained non-host page-pool
      storage.
- [x] Implement the first concrete experimental runtime adapter against the
      pinned llama.cpp commit with `--qatq-gpu-page-staging`, strict
      live-paging gate evidence, and output preservation against a full-GPU
      baseline.
- [x] Choose llama.cpp as the first runtime target for the experimental
      live-VRAM adapter path.
- [x] Compress only cold/inactive KV pages and restore them before attention
      needs them in the scoped llama.cpp strict backend-op smoke.
- [ ] Broaden cold-page compression and restore-before-attention proof across
      model families, dtypes, page sizes, sequence mixes, pressure profiles,
      and long-context generation. The rebuilt backend-op route now has scoped
      breadth across Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini under 512 MiB
      host-memory pressure; scoped bf16/f32 dtype and page-size matrices have
      also passed on the local Metal/MLX stack. Broader sequence-mix,
      longer-duration soak, adverse pressure, and long-context latency gates
      remain open.
- [x] Add scoped native multi-stream retained page-table server evidence. The
      flattened Flash Attention route now splits retained page tables per
      stream and traces explicit `stream_index` values instead of falling back
      for non-unified two-slot cache layouts. The Qwen2.5 1.5B non-unified run
      at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-multistream-pressure1g-soak20-20260625`
      passed 20/20 iterations with p95 iteration/follow-up latency
      4.74s/2.61s, 16,008 live-offloaded segments, and 46.63 MiB RSS growth
      under 1 GiB host pressure. The Qwen2.5 3B follow-up at
      `/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-native-multistream-pressure1g-soak20-ctx8192-20260625`
      passed 20/20 with p95 iteration/follow-up latency 11.19s/4.34s,
      33,928 live-offloaded segments, and 111.81 MiB RSS growth under 1 GiB
      host pressure. The Phi 3.5 mini follow-up at
      `/private/tmp/qatq-live-vram-server-cancel-phi35mini-p64-nonunified-native-multistream-pressure1g-soak20-ctx8192-20260625`
      passed 20/20 with p95 iteration/follow-up latency 15.81s/4.77s,
      33,768 live-offloaded segments, and 836.23 MiB RSS growth under the same
      1 GiB pressure gate.
- [x] Add integrated strict native trace gates to the in-process server probe.
      The Qwen2.5 1.5B run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-strictgates-soak10-ctx8192-20260625`
      passed 10/10 with `--require-flattened-flash-consumer` and
      `--require-live-offloaded-stream-count 2`, proving that the reusable
      server probe rejects fallback-like native traces and requires both
      live-offloaded stream indices before accepting native multi-stream
      evidence.
- [x] Broaden integrated strict native trace-gate coverage to a second model.
      The Qwen2.5 3B run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-native-strictgates-soak5-ctx8192-20260625`
      passed 5/5 with the same strict gates, 568 flattened Flash consumer rows,
      8,768 live-offloaded segments, both live-offloaded stream indices,
      p95 iteration/follow-up latency 11.11s/4.34s, and 111.53 MiB RSS growth.
- [x] Broaden integrated strict native trace-gate coverage to the third current
      model family. The Phi 3.5 mini run at
      `/private/tmp/qatq-live-vram-server-cancel-phi35mini-p64-nonunified-native-strictgates-soak3-ctx8192-20260625`
      passed 3/3 with the same strict gates, 376 flattened Flash consumer rows,
      5,328 live-offloaded segments, both live-offloaded stream indices,
      p95 iteration/follow-up latency 15.99s/4.99s, and 836.47 MiB RSS growth.
- [x] Add a repeatable strict server-cancellation matrix wrapper. The checked-in
      `scripts/llama_cpp_live_vram_server_cancel_matrix.py` runner and
      `adapters/llama-cpp/live-vram-server-strict.local.example.json` config
      convert the current Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini strict
      server probes into a sequential, fail-closed matrix with per-case
      artifacts, aggregate JSON/Markdown summaries, dry-run command validation,
      and tests for dry-run and aggregate failure behaviour. The first real
      matrix run at
      `/private/tmp/qatq-live-vram-server-cancel-strict-matrix-20260625`
      passed 3/3 cases with the flattened Flash and two-stream live-offload
      gates enabled.
- [x] Add the first 100-cycle native multi-stream server burn-in. The
      Qwen2.5 1.5B run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-multistream-pressure1g-soak100-ctx8192-20260625`
      passed 100/100 with p95/p99 iteration latency 6.56s/6.58s, p95/p99
      follow-up latency 2.64s/2.65s, 168,968 live-offloaded segments, and
      108.19 MiB RSS growth under the corrected per-iteration RSS peak gate.
- [x] Add the first harsher-pressure native multi-stream server pass. The
      Qwen2.5 1.5B 2 GiB host-pressure run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-multistream-pressure2g-soak50-ctx8192-20260625`
      passed 50/50 with p95/p99 iteration latency 6.60s/6.61s, p95/p99
      follow-up latency 2.66s/2.66s, 84,568 live-offloaded segments, and
      106.48 MiB RSS growth under the corrected per-iteration RSS peak gate.
- [x] Add a second-model 50-cycle native multi-stream long soak. The Qwen2.5
      3B run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-native-multistream-pressure1g-soak50-ctx8192-20260625`
      passed 50/50 with p95/p99 iteration latency 11.22s/12.19s, p95/p99
      follow-up latency 4.36s/4.91s, 84,568 live-offloaded segments, and
      114.61 MiB RSS growth under the corrected per-iteration RSS peak gate.
- [x] Add second-model harsher-pressure native multi-stream server coverage.
      The Qwen2.5 3B 2 GiB host-pressure run at
      `/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-native-multistream-pressure2g-soak50-ctx8192-20260625`
      passed 50/50 with p95/p99 iteration latency 11.03s/12.17s, p95/p99
      follow-up latency 4.25s/5.48s, 84,568 live-offloaded segments, and
      115.25 MiB RSS growth under the corrected per-iteration RSS peak gate.
- [x] Add Phi 3.5 mini 50-cycle native multi-stream long-soak coverage. The
      Phi 3.5 mini run at
      `/private/tmp/qatq-live-vram-server-cancel-phi35mini-p64-nonunified-native-multistream-pressure1g-soak50-ctx8192-20260625`
      passed 50/50 with p95/p99 iteration latency 16.40s/18.13s, p95/p99
      follow-up latency 5.05s/6.71s, 84,168 live-offloaded segments, and
      832.47 MiB RSS growth under the corrected per-iteration RSS peak gate.
- [x] Add focused Phi 3.5 mini page-size variation for native server
      cancellation. The 128-token page run at
      `/private/tmp/qatq-live-vram-server-cancel-phi35mini-p128-nonunified-native-multistream-pressure1g-soak20-ctx8192-20260625`
      passed 20/20 with p95/p99 iteration latency 16.16s/16.16s, p95/p99
      follow-up latency 5.00s/5.04s, 16,648 live-offloaded segments, and
      822.64 MiB RSS growth under the corrected per-iteration RSS peak gate.
- [x] Add repeatable extended strict server-cancellation coverage. The
      `adapters/llama-cpp/live-vram-server-extended.local.example.json`
      matrix passed at
      `/private/tmp/qatq-live-vram-server-cancel-extended-matrix-20260625`,
      covering Qwen2.5 1.5B at 16,384 context for 10/10 iterations and
      Phi 3.5 mini with 128-token pages for 5/5 iterations. Both cases kept
      the flattened Flash and two-stream live-offload gates enabled.
- [x] Add the first multi-model native-server cancellation latency baseline. The
      `adapters/llama-cpp/live-vram-server-baseline.local.example.json`
      matrix passed at
      `/private/tmp/qatq-live-vram-server-cancel-baseline-multimodel-20260625`.
      It covered native/QATQ pairs for Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5
      mini. QATQ/native p95 iteration ratios were 1.246x, 1.163x, and 1.150x;
      follow-up p95 ratios were 1.680x, 1.494x, and 1.370x. Every QATQ case
      passed with strict flattened Flash and two-stream gates enabled. This
      does not close the broader peak-VRAM, tokens/sec, FP8, CPU-offload,
      zstd, and lz4 comparison gate.
- [x] Add first queue-depth tuning evidence for the native server path. The
      `adapters/llama-cpp/live-vram-server-queue-depth.local.example.json`
      matrix passed at `/private/tmp/qatq-live-vram-server-queue-depth-20260625`.
      Qwen2.5 1.5B q8/q16/q32 candidates all passed strict trace gates. q8
      had the best QATQ iteration-p95 ratio at 1.247x and 3,328 live-offloaded
      segments, q32 was close at 1.256x and had the lowest RSS-growth ratio at
      4.156x, and q16 was a tail-latency outlier.
- [x] Add warmup-gated accepted no-trace policy breadth. The
      `adapters/llama-cpp/live-vram-server-family-policy-notrace.local.example.json`
      matrix passed at
      `/private/tmp/qatq-live-vram-server-family-policy-bootstrap-notrace-warmup-gated-20260625`.
      It covered native/QATQ pairs for Qwen2.5 1.5B, Qwen2.5 3B, and
      Phi 3.5 mini with backend-memory diagnostics, one warmup cycle, five
      measured steady-state cycles, 1 GiB host-memory pressure, and global
      comparison gates. QATQ/native throughput p50 ratios were 0.972x,
      0.980x, and 0.983x; steady-state RSS-growth ratios were 0.940x, 0.955x,
      and 1.447x. This improves the accepted server policy evidence, but does
      not close the broader peak-VRAM, long-burn-in, and runtime-diversity
      gates below.
- [x] Add a longer accepted no-trace policy soak. The first 10-cycle soak
      rejected the default Qwen2.5 3B q32 policy on follow-up p95 and
      steady-state RSS ratio. After a targeted Qwen2.5 3B q8 probe removed the
      tail spike, `adapters/llama-cpp/live-vram-server-family-policy-soak-notrace.local.example.json`
      passed at
      `/private/tmp/qatq-live-vram-server-family-policy-soak-bootstrap-notrace-warmup-gated-20260625`.
      It covered the same three native/QATQ model pairs for 10 measured cycles
      each. QATQ/native p50 throughput ratios were 0.981x, 0.943x, and
      1.003x; steady-state RSS ratios were 0.941x, 1.637x, and 1.372x.
      This is still not a substitute for overnight soak or hardware-counter
      peak-VRAM proof.
- [x] Restore full-family accepted-policy repeatability after the slower-host
      burn-in failures. q2 with 64-token pages passed focused Qwen2.5 3B but
      failed the full-family repeat, and 128-token pages with q4 also failed
      the corrected full-family p05/p50 rerun. The current checked-in
      candidate is 256-token pages with q4. It passed the same full-family
      burn-in gates at
      `/private/tmp/qatq-live-vram-server-family-policy-soak-burnin3-p256q4-p05-tailgate-20260626`
      for 3/3 repeats, 18 real server cases total, without relaxing memory or
      throughput limits.
- [x] Add a steady-state RSS tail-growth gate to the accepted no-trace policy
      soak. The server probe can now enforce `--max-rss-tail-growth-kib`
      over a configurable `--rss-tail-window`, and
      `adapters/llama-cpp/live-vram-server-family-policy-soak-notrace.local.example.json`
      sets an 8192 KiB gate over the last four measured iterations. The
      tail-gated bootstrapped run at
      `/private/tmp/qatq-live-vram-server-family-policy-soak-bootstrap-notrace-tail-gated-20260625`
      passed all six native/QATQ cases with 0 KiB tail RSS growth in every
      case. A later full-family burn-in exposed that raw tail range can be
      large when the server releases memory during the tail window, so the
      probe now gates positive tail growth while still reporting raw tail range
      as a volatility diagnostic. QATQ/native p50 throughput ratios were 0.977x, 1.012x, and
      0.989x; p95 throughput ratios were 0.956x, 1.004x, and 0.980x;
      steady-state RSS-growth ratios were 1.345x, 0.594x, and 1.461x.
      This proves the scoped 10-cycle policy settles after warmup, but still
      is not overnight burn-in or direct hardware-counter peak-VRAM proof.
- [x] Add native-vs-QATQ device-memory comparison gates to the accepted
      no-trace policy soak. The matrix runner now calculates
      `backend_accelerator_context_ratio` and `projected_device_memory_ratio`
      for every QATQ/native comparison, and the accepted policy configs fail
      unless QATQ backend K/V memory is below native (`<= 0.99x`) and projected
      device memory does not regress (`<= 1.0x`). The bootstrapped
      device-gated run at
      `/private/tmp/qatq-live-vram-server-family-policy-soak-bootstrap-notrace-device-gated-20260625`
      passed all six cases with empty comparison gate failures. Backend K/V
      ratios were 0.964x, 0.972x, and 0.969x; projected device-memory ratios
      were 0.995x, 0.997x, and 0.982x. This proves the scoped accepted policy
      is memory-reducing under llama.cpp backend diagnostics, but still is not
      direct hardware-counter peak-VRAM proof.
- [x] Add bounded live-VRAM burn-in orchestration. The new
      `scripts/llama_cpp_live_vram_server_burnin.py` repeats a configured
      server-cancellation matrix, fails on the first failed run, and can enforce
      aggregate jitter gates for RSS growth, backend K/V memory, and projected
      device memory across repeated runs. A first two-run layer-policy attempt
      at
      `/private/tmp/qatq-live-vram-server-layer-policy-burnin2-device-jitter-20260625`
      failed correctly because the QATQ/native RSS-growth ratio hit 2.401x
      against the 2.0x layer-policy gate, while backend K/V and projected device
      memory still improved. The scoped accepted-family Qwen2.5 1.5B burn-in at
      `/private/tmp/qatq-live-vram-server-family-qwen15b-policy-burnin2-device-jitter-20260625`
      then passed two native/QATQ matrix repeats with empty comparison and
      aggregate gate failures. Backend K/V and projected-device jitter ratios
      were 1.0 for both native and QATQ cases; QATQ RSS-growth jitter was
      1.010x and tail growth stayed at 64 KiB or below. This is bounded
      repetition evidence, not overnight burn-in.
- [x] Broaden bounded accepted-family burn-in across two Qwen model families.
      `/private/tmp/qatq-live-vram-server-family-qwen15b-qwen3b-policy-burnin2-device-jitter-20260625`
      repeated the accepted family policy twice over Qwen2.5 1.5B and
      Qwen2.5 3B native/QATQ pairs, for eight real matrix cases total. Both
      repeats passed with empty comparison and aggregate gate failures. Backend
      K/V and projected-device jitter ratios were 1.0 for all four cases.
      Qwen2.5 1.5B QATQ/native p50 throughput ratios were 0.952x and 0.978x
      across the two repeats, backend K/V ratio stayed 0.964x, and projected
      device ratio stayed 0.995x. Qwen2.5 3B QATQ/native p50 throughput ratios
      were 0.996x and 0.988x, backend K/V ratio stayed 0.972x, and projected
      device ratio stayed 0.997x. This is broader bounded burn-in evidence,
      but Phi still needed its own focused pressure repeat.
- [x] Add focused Phi accepted-policy burn-in. The config at
      `adapters/llama-cpp/live-vram-server-family-phi-policy-burnin-notrace.local.example.json`
      narrows the accepted family policy to the Phi 3.5 mini native/QATQ pair
      while keeping the same warmup, 10 measured cycles, backend-memory,
      projected-device, throughput, latency, RSS-ratio, and tail-RSS gates.
      `/private/tmp/qatq-live-vram-server-family-phi-policy-burnin2-device-jitter-20260625`
      repeated that pair twice for four real matrix cases total. Both repeats
      passed with empty comparison and aggregate gate failures. QATQ/native p50
      throughput ratios were 0.966x and 0.968x; p95 throughput ratios were
      0.961x and 0.974x. Backend K/V memory stayed at 3072->2976 MiB,
      projected device memory stayed at 5403->5304 MiB, backend K/V and
      projected-device jitter ratios were 1.0, QATQ RSS-growth jitter was
      1.0039x, and the QATQ RSS tail growth stayed at 32 KiB or below. This is
      bounded Phi repetition evidence, not overnight burn-in or direct
      hardware-counter peak-VRAM proof.
- [x] Repeat the full accepted-family policy with positive tail-delta
      semantics. The full-family burn-in at
      `/private/tmp/qatq-live-vram-server-family-policy-soak-burnin2-taildelta-security-gated-20260625`
      repeated the accepted Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini
      native/QATQ policy twice, for twelve real matrix cases total. Both
      repeats passed with empty comparison and aggregate gate failures.
      Backend K/V and projected-device jitter ratios were 1.0 for all six
      native/QATQ cases. QATQ/native backend K/V ratios stayed at 0.964x,
      0.972x, and 0.969x; projected-device ratios stayed at 0.995x, 0.997x,
      and 0.982x. QATQ/native p50 throughput ratios were 0.967x/0.973x for
      Qwen2.5 1.5B, 0.987x/0.986x for Qwen2.5 3B, and 0.977x/0.952x for Phi
      across the two repeats. The largest positive QATQ RSS tail delta over
      native was 1712 KiB against the 2048 KiB comparison gate. This supersedes
      the separate Qwen-only and
      Phi-only accepted-policy burn-ins as the current bounded full-family
      evidence, but still is not overnight burn-in or direct hardware-counter
      peak-VRAM proof.
- [x] Make the direct peak-VRAM counter blocker machine-checkable. The new
      `scripts/llama_cpp_live_vram_hardware_counters.py` inspects a matrix
      summary and local counter capabilities, then fails closed when direct
      peak-VRAM evidence is required but unavailable. It now supports explicit
      NVIDIA `nvidia-smi` process sampling with `--sample-pid`,
      `--sample-seconds`, `--sample-interval-ms`, and
      `--require-direct-peak-vram`; the direct gate only passes after captured
      `pid,used_memory` samples for the target runtime. The live
      `llama-server` cancellation probe also has integrated
      `--sample-direct-peak-vram` / `--require-direct-peak-vram-counter`
      support so summaries can include `direct_peak_vram_counter` while the
      server is under warmup and measured cancellation/follow-up load, and the
      matrix runner forwards the same policy keys from JSON configs for
      burn-in use on suitable hardware. Retained sample arrays are bounded with
      `direct_peak_vram_retain_samples` while `sample_count` and
      `peak_memory_mib` remain complete. Matrix `comparison_gates` can now
      require direct counters and cap QATQ/native direct peak-VRAM ratio with
      `require_direct_peak_vram_counters` and `max_direct_peak_vram_ratio`.
      The burn-in wrapper also supports `--max-direct-peak-vram-jitter-ratio`
      so repeated hardware-counter runs can fail on unstable direct peak-VRAM
      measurements. The server evidence probe also bounds JSONL trace
      ingestion with `--max-trace-bytes` and `--max-trace-line-bytes`; trace
      parsing is streaming, and oversized files, malformed JSONL rows, or
      individual oversized trace lines now fail closed instead of growing
      validator memory or silently dropping corrupt evidence during long soaks.
      The report at
      `/private/tmp/qatq-live-vram-server-family-policy-soak-burnin3-p256q4-p05-tailgate-20260626/hardware-counters.json`
      confirms that all six cases in the latest accepted burn-in repeat had
      llama.cpp backend projected-device and accelerator-breakdown diagnostics,
      while direct peak-VRAM counters were unavailable through current host
      tools: `nvidia-smi` is absent, `powermetrics` requires superuser and
      exposes per-process GPU time, not per-process peak GPU memory, and
      `vmmap` reports virtual memory maps, not a peak GPU memory counter.
- [ ] Broaden in-process server cancellation burn-in across more native
      multi-stream retained page-table models, harsher pressure variation, and
      broader runtime coverage. The scoped two-stream Qwen2.5 1.5B, Qwen2.5
      3B, and Phi 3.5 mini passes plus the 100-cycle Qwen2.5 1.5B soak and
      Qwen2.5 3B/Phi 3.5 mini 50-cycle soaks, Qwen2.5 1.5B/Qwen2.5 3B 2 GiB
      pressure passes, the focused Phi 128-token page-size pass, and the
      Qwen2.5 1.5B 16,384-context crash-fix rerun are not a substitute for
      broader pressure, page-size, context-length, and production runtime
      coverage. The crash-fix rerun at
      `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-multistream-pressure1g-soak20-ctx16384-repeat160-20260625`
      passed 20/20 after the flattened Flash route stopped applying the
      aggregate single-stream segmented-backend total-token cap to valid
      two-stream long-context work.
- [ ] Measure peak VRAM, tokens/sec, first-token latency, and per-token latency
      against the runtime's native KV cache, FP8/KV quantization, CPU offload,
      zstd, and lz4.
- [x] Prove byte-exact page restore and task/output preservation for exact mode
      in scoped real llama.cpp live-VRAM cases.
- [ ] Prove byte-exact page restore and task/output preservation across the
      broader long-context production matrix.
- [ ] Keep the feature behind experimental docs or feature flags until it beats
      simpler runtime offload strategies under realistic workloads.

This track is deliberately separate from the v0.1 release goal. QATQ can
already compress exported KV/tensor state for storage and transfer; live VRAM
reduction now has a QATQ-side page contract and simulator, but still requires
participation in a runtime's KV allocator or page scheduler, not just access to
exported tensors.

## Phase 4 - Open Release

- [x] Prepare public repository hygiene around generated public fixtures.
- [x] Add CI and fuzzing scaffold.
- [x] Add scheduled longer fuzzing for decoder and QATQ exact round-trip targets.
- [x] Add a public end-to-end retrieval task-quality experiment.
- [x] Add a local Ollama model-output task harness for runtime fixture
      ingestion and task-decision preservation.
- [x] Add direct live KV-cache extraction from at least one runtime that exposes
      internal transformer KV tensors.
- [x] Add an owned, version-pinned llama.cpp adapter patch and direct KV matrix
      runner.
- [x] Add Metal-backed llama.cpp exported-KV evidence across Qwen2.5 1.5B,
      Qwen2.5 Coder 3B, and Phi 3.5 mini, with exact restore and raw/zstd/lz4
      comparisons on the same page boundaries.
- [x] Record one scoped external Rust live-migration proof where standalone QATQ
      preserved exact continuation behavior and beat raw, zstd, and lz4
      transfer-size baselines.
- [x] Freeze v0.1.0 API/CLI names before crates.io publish.
- [x] Add coverage and supply-chain checks.
- [x] Wire GitHub Releases with cargo-dist binary archives, installers, and
      checksums.
- [x] Add manual crates.io publication workflow with environment approval.
- [ ] Publish to crates.io when the release owner performs and records the
      publication step.
