# llama.cpp GPU KV Evidence

This report records local Metal-backed llama.cpp runs that exported real
internal K/V cache tensors and replayed them through QATQ's live-VRAM evidence
path. The evidence proves direct GPU-runtime KV tensor ingestion, exact restore,
and compression-positive storage for long-context exported KV pages.

> Latest correction, 2026-06-25: stricter residency accounting now includes
> retained tiled page-table allocations in `gpu_page_staging_bytes`. Under that
> accounting, older backend-op residency claims below are historical work-log
> entries rather than current production proof. The compact residency follow-up
> now passes the strict native backend-op gate at
> `/private/tmp/qatq-live-vram-backend-op-compact-native-gate`. It preserved
> full-GPU output for a 705-token Qwen2.5 1.5B prompt, restored `112/112`
> pages exactly, offloaded `56/56` compressed pages with zero pass-through,
> consumed bounded K/V page segments through
> `backend_scheduled_segmented_attention`, passed MLX GPU streaming-attention
> equivalence, beat raw/zstd/lz4 in aggregate, and reduced persistent GPU K/V
> residency from `22,020,096` bytes to `14,680,064` bytes while passing the
> configured p95/p99 token-latency gate.
>
> Production live VRAM reduction still remains experimental, but the current
> four-layer Qwen2.5 Coder 3B long-context native path now clears the
> fail-closed latency gate. The adapter keeps all-resident layers on stock
> llama.cpp attention and routes only `live_offloaded` cold-page layers through
> native page consumption. The custom segmented Metal backend-op reduced p95
> from `+756%` to `+19.6%`; the newer flattened native route then maps eligible
> bounded page tables into llama.cpp's backend-scheduled Flash Attention path.
> A further cold-slot reuse fix stopped repeatedly syncing immutable offloaded
> rows after their retained table slot was already populated; mutable tail rows
> still sync every decode step.
> `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l4-q96-coldreuse-129tok-2x-20260625`
> passed the checked-in strict live-paging and strict native page-streaming
> matrix across two iterations with output preserved, `504/504` exact restores
> per iteration, `96/96` compressed offloads, zero pass-through pages, MLX GPU
> page-streaming equivalence, 137.00 MiB reclaimable GPU K/V, QATQ 179.00 MiB
> versus zstd 218.30 MiB and lz4 237.57 MiB, worst deep p95 `+8.4%`, and worst
> deep p99 `+6.4%` against the 15%/20% gates under 1 GiB host-memory
> pressure.
> The mechanism still needs broader model, dtype, page-size, memory-pressure,
> security, more aggressive reclaim-policy, and soak evidence before it becomes a general
> production-complete product claim.

> Fresh breadth/dtype update, 2026-06-25: the adapter now keeps pages smaller
> than `LLAMA_QATQ_TRACE_MIN_OFFLOAD_PAGE_TOKENS` resident by default
> (`16` tokens) so runtime traces do not offload one-token codec-negative tails
> that QATQ evidence correctly refuses to store. With that guard, the fresh
> breadth matrix at
> `/private/tmp/qatq-live-vram-native-breadth-coldreuse-min16tail-20260625`
> passed 3/3 real Metal/MLX cases across Qwen2.5 1.5B, Qwen2.5 3B, and
> Phi 3.5 mini with strict live-paging, native page-streaming, aggregate codec,
> GPU page-staging, and 1 GiB host-memory pressure. The fresh dtype matrix at
> `/private/tmp/qatq-live-vram-native-dtype-coldreuse-min16tail-20260625`
> passed 8/8 bf16/f32 cases across Qwen2.5 Coder 3B, Qwen2.5 3B,
> Qwen2.5 Coder 7B, and Phi 3.5 mini under the same gates.

> Process-level stress update, 2026-06-25: the parallel stress wrapper at
> `scripts/llama_cpp_live_vram_parallel_stress.py` now shards matrix cases into
> concurrent one-case jobs and fails closed when any child matrix fails. A
> bounded Qwen2.5 1.5B smoke at
> `/private/tmp/qatq-live-vram-parallel-stress-qwen15b-2x-20260625` passed 2/2,
> and the broader 16-token breadth stress at
> `/private/tmp/qatq-live-vram-parallel-stress-breadth-3case-16tok-20260625`
> passed 3/3 concurrent Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini child
> matrices with strict live-paging, native page-streaming, aggregate codec,
> GPU page-staging, MLX equivalence, page seals, and artifact pruning enabled.
> The broader pass restored all pages exactly, kept zero pass-through pages,
> and beat zstd/lz4 in aggregate for every child: Qwen2.5 1.5B stored
> 25.99 MiB with QATQ versus 28.64 MiB with zstd and 31.12 MiB with lz4;
> Qwen2.5 3B stored 80.79 MiB versus 98.34 MiB and 106.99 MiB; Phi 3.5 mini
> stored 402.61 MiB versus 448.82 MiB and 493.38 MiB. A two-token cold-start
> breadth diagnostic failed the Qwen2.5 3B performance gate while the same
> case passed serially, so the 16-token pass is the current process-level
> breadth evidence. This is real patched-runtime process pressure evidence,
> not a replacement for broader shared-server restore-race burn-in.

> Accepted-policy burn-in update, 2026-06-26: the backend-memory-gated
> llama-server policy now has bounded three-repeat full-family coverage across
> Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini. The full-family run at
> `/private/tmp/qatq-live-vram-server-family-policy-soak-burnin3-p256q4-p05-tailgate-20260626`
> passed three complete native/QATQ repeats, eighteen real matrix cases total,
> with empty comparison and aggregate gate failures. QATQ/native backend K/V
> ratios were `0.964x`, `0.972x`, and `0.969x`; projected-device ratios were
> `0.995x`, `0.997x`, and `0.982x`. Qwen2.5 3B's weakest QATQ/native
> p05/p50/p95 throughput ratios were `0.929x`/`0.945x`/`0.959x`, while
> backend K/V stayed `288->280 MiB`. Fail-closed precursor runs hardened the
> proof: the non-soak family policy now uses the same conservative Phi prompt
> pressure as the soak policy, the accepted comparison gate caps positive QATQ
> RSS tail-growth delta over native, and the Qwen2.5 3B policy now uses
> 256-token pages with q4 after q8, q2, and p128/q4 candidates missed repeat
> gates. This is the current strongest
> accepted-policy repeatability evidence, but it is still bounded burn-in
> rather than overnight soak or direct hardware peak-VRAM counter proof.

> In-process server cancellation update, 2026-06-25: the new
> `scripts/llama_cpp_live_vram_server_cancel_probe.py` starts patched
> `llama-server`, enables QATQ GPU page staging through environment variables,
> opens a streaming `/completion`, closes the client connection mid-stream,
> checks `/health`, and sends a follow-up completion to the same process. The
> Qwen2.5 1.5B conservative 1024-token page run at
> `/private/tmp/qatq-live-vram-server-cancel-qwen15b-releasepages-20260625`
> passed: 256 streamed bytes were observed before cancellation, health
> recovered, the follow-up request completed, and the page-segment trace
> recorded 896 attention-page-segment events, 128 attention-consumed events,
> and 128 live-offloaded segments. The first 64-token page server stress attempt
> failed closed with `QATQ persistent attention segment count exceeds safe graph
> object budget`. The probe now derives page-segment and graph-node budgets from
> the configured `ctx_size / page_tokens` policy; the budgeted 64-token rerun at
> `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-budgeted-20260625`
> passed with 952 attention-page-segment events, 136 attention-consumed events,
> and 584 live-offloaded segments while preserving health and follow-up serving.
> The two-slot unified-KV concurrent run at
> `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-concurrent-kvu-20260625`
> then started a follow-up request before stream cancellation, cancelled the
> stream, completed the follow-up after cancellation, recovered health, and
> recorded 1,120 page-segment events with 1,312 live-offloaded segments. The
> retained tiled page-pool route now returns no live QATQ segments for
> unsupported multi-stream reserve graphs instead of aborting, while supported
> one-stream slot work still uses live page staging. With that fallback, the
> two-slot non-unified run at
> `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-concurrent-nonunified-20260625`
> also passed: follow-up started before cancellation, completed after
> cancellation, health recovered, and the trace recorded 1,120 page-segment
> events with 544 live-offloaded segments.
> The server probe now also supports bounded repeated cancellation/follow-up
> cycles against one process. The 20-iteration non-unified run at
> `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-soak20-20260625`
> passed 20/20 cycles with 5,120 total streamed bytes before cancellation,
> health recovery and follow-up completion on every iteration, 10,696
> page-segment events, 1,288 attention-consumed events, and 10,728
> live-offloaded segments. The matching 20-iteration unified-KV run at
> `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-kvu-soak20-20260625`
> also passed 20/20 cycles with 9,632 page-segment events, 1,376
> attention-consumed events, and 24,568 live-offloaded segments. These are
> bounded shared-server soak passes, not a substitute for overnight burn-in.
> A follow-up pressure-gated server soak added touched host-memory pressure and
> an RSS-growth gate to the same probe. The 1 GiB host-pressure non-unified run
> at
> `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-pressure1g-soak20-20260625`
> passed 20/20 cycles with 10,728 live-offloaded segments; server RSS grew from
> 1,274,192 KiB after readiness to a 1,334,896 KiB peak, or 59.3 MiB. The
> matching unified-KV run at
> `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-kvu-pressure1g-soak20-20260625`
> passed 20/20 cycles with 24,568 live-offloaded segments; server RSS grew from
> 1,274,192 KiB to a 1,320,064 KiB peak, or 44.8 MiB.
> The same probe now records per-iteration and follow-up latency and can fail
> when `--max-iteration-seconds` or `--max-followup-seconds` is exceeded. The
> longer latency-gated 1 GiB pressure non-unified burn-in at
> `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-pressure1g-latency-soak100-20260625`
> passed 100/100 cycles with 53,608 live-offloaded segments, 61.25 MiB RSS
> growth, p95/p99 iteration latency 4.80s/4.84s, and p95/p99 concurrent
> follow-up latency 2.61s/2.64s under a 15s/10s gate. The matching unified-KV
> burn-in at
> `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-kvu-pressure1g-latency-soak100-20260625`
> passed 100/100 cycles with 122,488 live-offloaded segments, 44.8 MiB RSS
> growth, p95/p99 iteration latency 4.83s/4.85s, and p95/p99 follow-up latency
> 2.68s/2.70s under the same gate.
> Harsher and broader follow-ups added 2 GiB host-pressure Qwen2.5 1.5B
> 50-cycle runs and Qwen2.5 3B 20-cycle runs. The 1.5B non-unified 2 GiB run at
> `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-pressure2g-latency-soak50-20260625`
> passed with 59.92 MiB RSS growth and p95/p99 iteration latency
> 4.78s/4.78s; the matching unified-KV run at
> `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-kvu-pressure2g-latency-soak50-20260625`
> passed with 44.69 MiB RSS growth and p95/p99 iteration latency
> 4.88s/4.93s. The Qwen2.5 3B non-unified run at
> `/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-pressure1g-latency-soak20-20260625`
> passed 20/20 with p95/p99 iteration latency 7.65s/7.66s,
> p95/p99 follow-up latency 4.31s/4.32s, and 61.36 MiB RSS growth. The
> matching Qwen2.5 3B unified-KV run at
> `/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-kvu-pressure1g-latency-soak20-20260625`
> passed 20/20 with p95/p99 iteration latency 7.73s/7.73s,
> p95/p99 follow-up latency 4.39s/4.51s, 24,568 live-offloaded segments, and
> 46.41 MiB RSS growth. These strengthened shared-server cancellation and reuse
> evidence before the native multi-stream retained-table fix.
> The follow-up native multi-stream adapter run at
> `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-multistream-pressure1g-soak20-20260625`
> passed 20/20 Qwen2.5 1.5B non-unified two-slot cycles under 1 GiB host
> pressure. It used `backend_scheduled_flattened_flash_attention`, traced
> explicit `stream_index: 0` and `stream_index: 1` retained-table segments,
> recorded 10,696 attention-page-segment events, 1,528 attention-consumed
> events, and 16,008 live-offloaded segments, and stayed within p95/p99
> iteration latency 4.74s/4.76s plus p95/p99 follow-up latency 2.61s/2.63s.
> After adding strict server-probe trace gates, the integrated CLI path was
> rerun at
> `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-strictgates-soak10-ctx8192-20260625`.
> It passed 10/10 with `--require-flattened-flash-consumer` and
> `--require-live-offloaded-stream-count 2`, proving the probe itself now
> rejects fallback-like native traces. The run recorded 7,336
> attention-page-segment events, 1,048 attention-consumed events, 1,048
> `backend_scheduled_flattened_flash_attention` consumers, 17,368
> live-offloaded segments, live-offloaded stream indices `0` and `1`, p95/p99
> iteration latency 6.44s/6.44s, p95/p99 follow-up latency 2.62s/2.62s, and
> 106.73 MiB RSS growth under 1 GiB host pressure.
> A Qwen2.5 3B follow-up at
> `/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-native-multistream-pressure1g-soak20-ctx8192-20260625`
> passed 20/20 non-unified two-slot cycles under 1 GiB host pressure with an
> 8192-token server context. It recorded 18,072 attention-page-segment events,
> 2,008 attention-consumed events, 33,928 live-offloaded segments, p95/p99
> iteration latency 11.19s/11.21s, p95/p99 follow-up latency 4.34s/4.35s, and
> 111.81 MiB RSS growth. Its consumed rows all used
> `backend_scheduled_flattened_flash_attention` and traced both stream indices.
> The strict-gated Qwen2.5 3B integrated server rerun at
> `/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-native-strictgates-soak5-ctx8192-20260625`
> passed 5/5 with `--require-flattened-flash-consumer` and
> `--require-live-offloaded-stream-count 2`. It recorded 5,112
> attention-page-segment events, 568 attention-consumed events, 568
> `backend_scheduled_flattened_flash_attention` consumers, 8,768
> live-offloaded segments, live-offloaded stream indices `0` and `1`, p95/p99
> iteration latency 11.11s/11.11s, p95/p99 follow-up latency 4.34s/4.34s, and
> 111.53 MiB RSS growth under 1 GiB host pressure. This broadens the strict
> reusable server-probe evidence beyond Qwen2.5 1.5B.
> A Phi 3.5 mini follow-up at
> `/private/tmp/qatq-live-vram-server-cancel-phi35mini-p64-nonunified-native-multistream-pressure1g-soak20-ctx8192-20260625`
> passed 20/20 with the same stream-split native route under 1 GiB host
> pressure. It recorded 16,064 attention-page-segment events, 2,008
> attention-consumed events, 33,768 live-offloaded segments, p95/p99 iteration
> latency 15.81s/15.82s, p95/p99 follow-up latency 4.77s/4.77s, and 836.23 MiB
> RSS growth after the server probe was fixed to include per-iteration RSS
> peaks in its memory gate. Its consumed rows all used
> `backend_scheduled_flattened_flash_attention` and traced both stream indices
> with shape `[96,32,64,1]`.
> The strict-gated Phi 3.5 mini integrated server rerun at
> `/private/tmp/qatq-live-vram-server-cancel-phi35mini-p64-nonunified-native-strictgates-soak3-ctx8192-20260625`
> passed 3/3 with `--require-flattened-flash-consumer` and
> `--require-live-offloaded-stream-count 2`. It recorded 3,008
> attention-page-segment events, 376 attention-consumed events, 376
> `backend_scheduled_flattened_flash_attention` consumers, 5,328
> live-offloaded segments, live-offloaded stream indices `0` and `1`, p95/p99
> iteration latency 15.99s/15.99s, p95/p99 follow-up latency 4.99s/4.99s, and
> 836.47 MiB RSS growth under 1 GiB host pressure. This closes strict reusable
> server-probe coverage across the current Qwen2.5 1.5B, Qwen2.5 3B, and
> Phi 3.5 mini model-family set.
> The same three strict server probes are now repeatable through
> `scripts/llama_cpp_live_vram_server_cancel_matrix.py` and
> `adapters/llama-cpp/live-vram-server-strict.local.example.json`. The first
> aggregate matrix at
> `/private/tmp/qatq-live-vram-server-cancel-strict-matrix-20260625` passed
> all three cases with the strict flattened Flash consumer and two
> live-offloaded stream-index gates enabled: Qwen2.5 1.5B 10/10, Qwen2.5 3B
> 5/5, and Phi 3.5 mini 3/3.
> A current-binary rerun at
> `/private/tmp/qatq-live-vram-server-strict-current-20260625` kept those gates
> and exposed an intentionally fail-closed resource limit: the Qwen2.5 1.5B and
> Qwen2.5 3B cases passed, while Phi 3.5 mini aborted when the retained tiled
> page-pool hit the old 1 GiB default despite a 1.5 GiB RSS-growth gate. The
> probe now exports
> `LLAMA_QATQ_ATTENTION_PERSISTENT_PAGE_SOURCE_MAX_RETAINED_BYTES` and derives
> its default retained-pool ceiling from
> `max(1024 MiB, max_server_rss_growth_mib)`. The focused Phi rerun at
> `/private/tmp/qatq-live-vram-server-phi35-strict-retained-budget-20260625`
> passed 3/3 with 5,328 live-offloaded segments, 376 flattened Flash consumers,
> p50 predicted-token throughput 33.21 tok/s, p95/p99 iteration latency
> 23.84s/23.84s, p95/p99 follow-up latency 5.47s/5.47s, and 1,046.11 MiB RSS
> growth under the 1.5 GiB gate. The full retained-budget matrix at
> `/private/tmp/qatq-live-vram-server-strict-retained-budget-current-20260625`
> then passed all three strict cases: Qwen2.5 1.5B 10/10 with 77.84 tok/s p50
> predicted throughput and 14,808 live-offloaded segments, Qwen2.5 3B 5/5 with
> 43.98 tok/s and 7,488 live-offloaded segments, and Phi 3.5 mini 3/3 with
> 33.13 tok/s and 5,328 live-offloaded segments.
> A longer Qwen2.5 1.5B native multi-stream burn-in at
> `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-multistream-pressure1g-soak100-ctx8192-20260625`
> then passed 100/100 under 1 GiB host pressure with the corrected RSS gate. It
> recorded 67,816 attention-page-segment events, 9,688 attention-consumed
> events, 168,968 live-offloaded segments, p95/p99 iteration latency
> 6.56s/6.58s, p95/p99 follow-up latency 2.64s/2.65s, and 108.19 MiB RSS growth
> across 202 RSS samples. Its consumed rows all used
> `backend_scheduled_flattened_flash_attention` and traced both stream indices.
> A harsher Qwen2.5 1.5B native multi-stream pressure run at
> `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-multistream-pressure2g-soak50-ctx8192-20260625`
> then passed 50/50 under 2 GiB host pressure with the corrected RSS gate. It
> recorded 34,216 attention-page-segment events, 4,888 attention-consumed
> events, 84,568 live-offloaded segments, p95/p99 iteration latency
> 6.60s/6.61s, p95/p99 follow-up latency 2.66s/2.66s, and 106.48 MiB RSS growth
> across 102 RSS samples. Its consumed rows all used
> `backend_scheduled_flattened_flash_attention` and traced both stream indices.
> A Qwen2.5 3B native multi-stream long soak at
> `/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-native-multistream-pressure1g-soak50-ctx8192-20260625`
> then passed 50/50 under 1 GiB host pressure with the corrected RSS gate. It
> recorded 43,992 attention-page-segment events, 4,888 attention-consumed
> events, 84,568 live-offloaded segments, p95/p99 iteration latency
> 11.22s/12.19s, p95/p99 follow-up latency 4.36s/4.91s, and 114.61 MiB RSS
> growth across 102 RSS samples. Its consumed rows all used
> `backend_scheduled_flattened_flash_attention` and traced both stream indices.
> A harsher Qwen2.5 3B 2 GiB pressure rerun at
> `/private/tmp/qatq-live-vram-server-cancel-qwen3b-p64-nonunified-native-multistream-pressure2g-soak50-ctx8192-20260625`
> then passed 50/50 with the corrected RSS gate. It recorded 43,992
> attention-page-segment events, 4,888 attention-consumed events, 84,568
> live-offloaded segments, p95/p99 iteration latency 11.03s/12.17s, p95/p99
> follow-up latency 4.25s/5.48s, and 115.25 MiB RSS growth across 102 RSS
> samples. Its consumed rows all used
> `backend_scheduled_flattened_flash_attention`, traced both stream indices, and
> carried shape `[128,2,64,1]`.
> A Phi 3.5 mini native multi-stream long soak at
> `/private/tmp/qatq-live-vram-server-cancel-phi35mini-p64-nonunified-native-multistream-pressure1g-soak50-ctx8192-20260625`
> then passed 50/50 under 1 GiB host pressure with the corrected RSS gate. It
> recorded 39,104 attention-page-segment events, 4,888 attention-consumed
> events, 84,168 live-offloaded segments, p95/p99 iteration latency
> 16.40s/18.13s, p95/p99 follow-up latency 5.05s/6.71s, and 832.47 MiB RSS
> growth across 102 RSS samples. Its consumed rows all used
> `backend_scheduled_flattened_flash_attention`, traced both stream indices, and
> covered the four configured cold K/V layers with shape `[96,32,64,1]`.
> A focused Phi 3.5 mini page-size variant at
> `/private/tmp/qatq-live-vram-server-cancel-phi35mini-p128-nonunified-native-multistream-pressure1g-soak20-ctx8192-20260625`
> then passed 20/20 under the same 1 GiB pressure gate with 128-token pages. It
> recorded 14,784 attention-page-segment events, 1,848 attention-consumed
> events, 16,648 live-offloaded segments, p95/p99 iteration latency
> 16.16s/16.16s, p95/p99 follow-up latency 5.00s/5.04s, and 822.64 MiB RSS
> growth. Its consumed rows all used
> `backend_scheduled_flattened_flash_attention`, traced both stream indices, and
> carried shape `[96,32,128,1]`.
> A Qwen2.5 1.5B long-context stress rerun at
> `/private/tmp/qatq-live-vram-server-cancel-qwen15b-p64-nonunified-native-multistream-pressure1g-soak20-ctx16384-repeat160-20260625`
> now passes after removing the flattened Flash route's over-strict aggregate
> segmented-backend contract check. The first attempt failed closed after one
> iteration with `QATQ segmented KQV backend contract exceeded max total
> tokens`, because the old validation summed both live streams against the
> single-stream 8192-token cap before the stream-local flattened Flash boundary
> could run. After rebuilding the patched server, the same 16,384-context,
> 160-repeat prompt profile passed 20/20 under 1 GiB host pressure, recorded
> 19,656 attention-page-segment events, 2,808 attention-consumed events, 60,168
> live-offloaded segments, p95/p99 iteration latency 9.96s/9.96s, p95/p99
> follow-up latency 2.66s/2.68s, and 243.30 MiB RSS growth. This is a concrete
> long-context crash fix for the native multi-stream server path, not a general
> proof for all longer context lengths.
> The extended strict server matrix at
> `/private/tmp/qatq-live-vram-server-cancel-extended-matrix-20260625` then
> reran the long-context class through
> `scripts/llama_cpp_live_vram_server_cancel_matrix.py`: Qwen2.5 1.5B passed
> 10/10 at 16,384 context with 35,288 live-offloaded segments, p95/p99
> iteration latency 11.06s/11.06s, p95/p99 follow-up latency 2.68s/2.68s, and
> 317,424 KiB RSS growth. The same matrix passed Phi 3.5 mini with 128-token
> pages for 5/5, recording 7,288 live-offloaded segments, p95/p99 iteration
> latency 19.13s/19.13s, p95/p99 follow-up latency 5.16s/5.16s, and
> 1,071,296 KiB RSS growth. Both cases kept the strict flattened Flash consumer
> and two live-offloaded stream-index gates enabled.
> The current retained-budget rerun at
> `/private/tmp/qatq-live-vram-server-extended-retained-budget-current-20260625`
> passed the same extended wrapper with current binaries: Qwen2.5 1.5B 16,384
> context passed 10/10 with p50 predicted throughput 77.37 tok/s, p95/p99
> iteration latency 13.86s/13.86s, p95/p99 follow-up latency 2.56s/2.56s,
> 32,728 live-offloaded segments, 1,528 flattened Flash consumers, and
> 445,904 KiB RSS growth; Phi 3.5 mini with 128-token pages passed 5/5 with
> 33.95 tok/s p50 predicted throughput, p95/p99 iteration latency
> 24.58s/24.58s, p95/p99 follow-up latency 5.14s/5.14s, 7,288 live-offloaded
> segments, 568 flattened Flash consumers, and 969,904 KiB RSS growth.
> The first multi-model native-server comparison matrix at
> `/private/tmp/qatq-live-vram-server-cancel-baseline-multimodel-20260625`
> passed all six native/QATQ cases across Qwen2.5 1.5B, Qwen2.5 3B, and
> Phi 3.5 mini. Native mode used `--native-baseline` and disabled QATQ
> live-VRAM environment variables; QATQ mode kept strict flattened Flash and
> two live-offloaded stream-index gates enabled. QATQ/native p95 iteration
> ratios were 1.246x, 1.163x, and 1.150x. Follow-up p95 ratios were 1.680x,
> 1.494x, and 1.370x. RSS-growth ratios were 4.169x, 3.615x, and 9.302x. This
> establishes a first multi-model shared-server latency/RSS comparison, not a
> peak-VRAM or tokens/sec benchmark.
> The current retained-budget baseline rerun at
> `/private/tmp/qatq-live-vram-server-baseline-retained-budget-current-20260625`
> passed all six native/QATQ cases and added predicted-token throughput ratios.
> QATQ/native predicted-token p50 ratios were 0.903x for Qwen2.5 1.5B, 0.938x
> for Qwen2.5 3B, and 0.880x for Phi 3.5 mini. Iteration p95 ratios were
> 1.153x, 1.105x, and 1.727x; follow-up p95 ratios were 1.217x, 1.132x, and
> 1.607x. RSS growth ratios were 4.547x, 4.675x, and 12.786x respectively. The
> Qwen cases are now close enough for further experimental optimisation, while
> Phi remains the current memory-overhead outlier.
> The follow-up Phi policy sweep at
> `/private/tmp/qatq-live-vram-server-phi-policy-current-20260625` tested fewer
> QATQ-routed layers with 128-token pages. The best strict candidate,
> `phi35-mini-qatq-l1-p128-strict`, passed 5/5 with p50 predicted throughput
> 37.12 tok/s, 1,822 live-offloaded segments, 142 flattened Flash consumers, a
> QATQ/native predicted-token p50 ratio of 0.949x, iteration p95 ratio 1.469x,
> follow-up p95 ratio 1.355x, and RSS-growth ratio 4.161x. The checked-in strict
> matrix now uses that conservative Phi policy. The threshold-gated backend-memory rerun at
> `/private/tmp/qatq-live-vram-server-strict-backend-memory-current-20260625`
> passed all three model cases and now records llama.cpp/Metal allocation
> diagnostics in the summary. Qwen2.5 1.5B passed 10/10 at 78.30 tok/s p50
> with 14,808 live-offloaded segments, 968 flattened Flash consumers, 16.50 MiB
> max retained page-source bytes, 1,426 MiB backend self, 192 MiB backend KV,
> 299 MiB backend compute, and 121,488 KiB RSS growth. Qwen2.5 3B passed 5/5
> at 43.86 tok/s p50 with 7,488 live-offloaded segments, 528 flattened Flash
> consumers, 22.00 MiB max retained page-source bytes, 2,391 MiB backend self,
> 256 MiB backend KV, 300 MiB backend compute, and 138,720 KiB RSS growth.
> Phi 3.5 mini passed 3/3 at 36.33 tok/s p50 with 882 live-offloaded segments,
> 88 flattened Flash consumers, 5,339 MiB backend self, 2,976 MiB backend KV,
> 134 MiB backend compute, and 321,760 KiB RSS growth. Phi is no longer the
> same severe outlier under the recommended policy, but it still needs longer
> soaks and hardware-counter peak-VRAM validation.
> A scoped mixed-prompt server pass at
> `/private/tmp/qatq-live-vram-server-mixed-prompts-current-20260625` then
> exercised the same strict cancellation, flattened Flash, two-stream,
> backend-memory diagnostic, and backend-memory ceiling gates across three
> Qwen2.5 1.5B prompt classes: daily-driver assistant handover, software
> engineering review, and retrieval-heavy incident memory. All three cases
> passed 3/3 iterations with empty `gate_failures`; p50 predicted throughput
> was 78.49, 78.58, and 78.24 tok/s respectively, backend self/KV/compute stayed
> fixed at 1,426/192/299 MiB, and RSS growth stayed below the 1 GiB gate. This
> is prompt-breadth evidence for the shared-server Qwen2.5 1.5B profile, not a
> replacement for longer mixed-model soaks.
> The next mixed-model prompt pass at
> `/private/tmp/qatq-live-vram-server-mixed-model-prompts-current-20260625`
> exercised daily-driver, software-engineering, and operations-incident prompt
> classes across Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini respectively. All
> three cases passed 3/3 strict cancellation/follow-up iterations with empty
> `gate_failures`. P50 predicted throughput was 78.03, 44.33, and 36.88 tok/s.
> Backend self/KV/compute were 1,426/192/299 MiB, 2,391/256/300 MiB, and
> 5,312/2,976/107 MiB respectively. This broadens shared-server model/prompt
> shape coverage, while longer mixed-model soaks remain open.
> The follow-up mixed-model soak at
> `/private/tmp/qatq-live-vram-server-mixed-model-soak-current-20260625`
> exercised the same three model/prompt classes for 10/10 strict
> cancellation/follow-up iterations each. All three cases passed with empty
> `gate_failures`. P50 predicted throughput was 78.05, 43.95, and
> 35.71 tok/s. Backend self/KV/compute were 1,426/192/299 MiB,
> 2,391/256/300 MiB, and 5,312/2,976/107 MiB respectively, with RSS growth of
> 83,120, 106,096, and 231,200 KiB. This is now the current scoped
> mixed-model prompt soak evidence, but it is not a substitute for
> hardware-counter peak-VRAM validation or broader runtime/context coverage.
> The server-cancellation probe now also parses `llama-server` non-streaming
> `/completion` timing payloads when present and carries predicted-token
> throughput into matrix summaries. A focused Qwen2.5 1.5B timing smoke at
> `/private/tmp/qatq-live-vram-server-timing-smoke-20260625` passed the native
> and QATQ comparison pair: native predicted-token p50 was `89.31 tok/s`, QATQ
> predicted-token p50 was `51.43 tok/s`, QATQ/native predicted-token p50 ratio
> was `0.576x`, predicted-token p95 ratio was `0.601x`, iteration p95 ratio
> was `1.235x`, follow-up p95 ratio was `1.731x`, and RSS-growth ratio was
> `4.119x`. The QATQ case kept the strict flattened Flash and two-stream
> live-offload gates with `8,768` live-offloaded segments. This proves the
> benchmark path now captures real server throughput metrics; it also confirms
> throughput and RSS overhead remain production blockers for the current shared
> server route.
> A follow-up allocation-policy fix keeps non-selected KV layers on llama.cpp's
> native GPU KV buffers instead of leaving them as canonical CPU KV when QATQ
> page staging is enabled. Only selected QATQ layers now use canonical CPU KV
> plus accelerator page staging. The strict trace-enabled timing smoke at
> `/private/tmp/qatq-live-vram-server-timing-smoke-gpufallback-20260625`
> passed after this fix: native predicted-token p50 was `88.02 tok/s`, QATQ
> predicted-token p50 was `77.97 tok/s`, QATQ/native predicted-token p50 ratio
> was `0.886x`, predicted-token p95 ratio was `0.882x`, iteration p95 ratio
> was `1.153x`, follow-up p95 ratio was `1.227x`, and the strict QATQ trace
> gates still recorded `7,488` live-offloaded segments with `528` flattened
> Flash consumers. This supersedes the earlier `0.576x` timing smoke as a
> current performance datapoint.
> The Qwen2.5 1.5B queue-depth matrix at
> `/private/tmp/qatq-live-vram-server-queue-depth-20260625` compared native
> against QATQ `max_queued_pages` 8, 16, and 32. All three QATQ candidates
> passed the strict flattened Flash and two live-offloaded stream-index gates.
> q8 had the best QATQ iteration-p95 ratio at 1.247x and 3,328 live-offloaded
> segments; q32 was close at 1.256x with 10,048 live-offloaded segments and the
> lowest RSS-growth ratio at 4.156x; q16 was a tail-latency outlier at 1.593x
> iteration-p95 and 2.891x follow-up-p95.
> After timing extraction was added, the queue-depth matrix was rerun at
> `/private/tmp/qatq-live-vram-server-queue-depth-timing-20260625`. Native
> predicted-token p50 was `87.04 tok/s`; q8, q16, and q32 recorded
> `50.37 tok/s`, `49.86 tok/s`, and `50.59 tok/s` respectively. Their
> QATQ/native predicted-token p50 ratios were `0.579x`, `0.573x`, and
> `0.581x`; predicted-token p95 ratios were `0.578x`, `0.570x`, and `0.594x`.
> q32 was slightly best on throughput and follow-up p95 in this run, while q8
> stayed close with fewer live-offloaded segments. The result suggests queue
> depth alone is not the main throughput bottleneck in this shared-server
> cancellation shape.
> A no-trace layer-count sweep was rerun from the clean bootstrapped
> `llama-server` with explicit backend-memory and projected device-memory
> ceilings at
> `/private/tmp/qatq-live-vram-server-layer-sweep-bootstrap-notrace-projected-gated-20260625`.
> It passed native plus QATQ l0/l1/l2/l4, five cancellation/follow-up
> iterations per case, 1 GiB host-memory pressure, and empty `gate_failures`.
> QATQ/native predicted-token p50 ratios were `0.956x`, `0.923x`, `0.884x`,
> and `0.858x` respectively. This proves the selected-layer design now keeps
> non-selected layers on the native GPU path and makes the throughput trade-off
> scale with the number of QATQ-routed layers. The l4 case reduced backend KV
> memory from `224 MiB` to `192 MiB` and projected device memory from
> `1458 MiB` to `1426 MiB`, with iteration/follow-up p95 ratios of `1.052x`
> and `1.159x`; RSS-growth ratios across l0/l1/l2/l4 were `1.002x`, `1.892x`,
> `2.720x`, and `4.152x`, so host-RSS pressure and direct peak-VRAM accounting
> remain production gates.
> This broad sweep is exploratory. The stricter comparison-gated repeat at
> `/private/tmp/qatq-live-vram-server-layer-sweep-bootstrap-notrace-comparison-gated-20260625`
> rejected l2/l4 candidates on latency, throughput, and host-RSS policy.
> The accepted no-trace policy is therefore the l1-only config at
> `adapters/llama-cpp/live-vram-server-layer-policy-notrace.local.example.json`.
> Its bootstrapped proof at
> `/private/tmp/qatq-live-vram-server-layer-policy-bootstrap-notrace-warmup-gated-20260625`
> passed native plus QATQ l1 with one required warmup cycle and five measured
> steady-state cycles under 1 GiB host-memory pressure. QATQ/native p50 and
> p95 throughput ratios were `0.956x` and `0.965x`, iteration/follow-up p95
> ratios were `1.014x` and `1.028x`, steady-state RSS-growth ratio was
> `1.279x`, backend KV memory dropped from `224 MiB` to `216 MiB`, and
> projected device memory dropped from `1458 MiB` to `1450 MiB`. The warmup
> cost remains visible: QATQ grew `59.44 MiB` during lazy initialisation versus
> `6.16 MiB` during the measured steady-state window.
> The accepted no-trace policy was then broadened with
> `adapters/llama-cpp/live-vram-server-family-policy-notrace.local.example.json`.
> `/private/tmp/qatq-live-vram-server-family-policy-bootstrap-notrace-warmup-gated-20260625`
> passed all six native/QATQ cases across Qwen2.5 1.5B, Qwen2.5 3B, and
> Phi 3.5 mini with backend-memory diagnostics, one required warmup cycle, five
> measured steady-state cycles, 1 GiB host-memory pressure, and global
> comparison gates. Qwen2.5 1.5B recorded QATQ/native throughput ratios of
> `0.972x`/`0.970x`, latency ratios of `1.041x`/`1.043x`, RSS-growth ratio
> `0.940x`, and backend KV `224->216 MiB`. Qwen2.5 3B recorded
> `0.980x`/`0.992x`, `1.003x`/`1.017x`, RSS `0.955x`, and backend KV
> `288->280 MiB`. Phi 3.5 mini with l1 and 128-token pages recorded
> `0.983x`/`0.975x`, `1.011x`/`1.219x`, RSS `1.447x`, and backend KV
> `3072->2976 MiB`; Phi remains the pressure outlier with `257.56 MiB` warmup
> RSS growth and `83.83 MiB` measured steady-state growth.
> The first 10-cycle family-policy soak rejected the default Qwen2.5 3B q32
> queue policy on a follow-up p95 outlier and steady-state RSS ratio. A targeted
> q8 Qwen2.5 3B probe removed the tail spike, so the accepted family configs now
> initially use q8 for Qwen2.5 3B QATQ. Later slower-host repeats rejected q8,
> q2 with 64-token pages, and p128/q4 as full-family candidates; the current
> checked-in Qwen2.5 3B policy uses 256-token pages with q4. The corrected bootstrapped soak at
> `/private/tmp/qatq-live-vram-server-family-policy-soak-bootstrap-notrace-warmup-gated-20260625`
> passed all six native/QATQ cases for 10 measured cycles each. Qwen2.5 1.5B
> recorded throughput ratios `0.981x`/`0.984x`, latency ratios
> `1.008x`/`1.028x`, RSS `0.941x`, and backend KV `224->216 MiB`. Qwen2.5 3B
> q8 recorded `0.943x`/`0.958x`, `1.068x`/`1.078x`, RSS `1.637x`, and backend
> KV `288->280 MiB`. Phi 3.5 mini recorded `1.003x`/`0.999x`,
> `0.889x`/`0.747x`, RSS `1.372x`, and backend KV `3072->2976 MiB`.
> The accepted soak now also carries an explicit steady-state RSS tail-growth
> gate: `max_rss_tail_growth_kib: 8192` over the last four measured iterations.
> The tail-gated bootstrapped run at
> `/private/tmp/qatq-live-vram-server-family-policy-soak-bootstrap-notrace-tail-gated-20260625`
> passed all six cases. Qwen2.5 1.5B recorded QATQ/native throughput ratios
> `0.977x`/`0.956x`, latency ratios `1.012x`/`1.022x`, RSS `1.345x`, backend
> KV `224->216 MiB`, and `0 KiB` tail RSS growth. Qwen2.5 3B q8 recorded
> `1.012x`/`1.004x`, `0.978x`/`0.801x`, RSS `0.594x`, backend KV
> `288->280 MiB`, and `0 KiB` tail RSS growth. Phi 3.5 mini recorded
> `0.989x`/`0.980x`, `0.995x`/`1.027x`, RSS `1.461x`, backend KV
> `3072->2976 MiB`, and `0 KiB` tail RSS growth. This is strong
> accepted no-trace shared-server soak evidence, but it still is not overnight
> burn-in or direct hardware-counter peak-VRAM proof.
> The accepted policy now also fails closed unless QATQ reduces backend K/V
> memory versus native (`max_backend_accelerator_context_ratio: 0.99`) and
> avoids projected device-memory regression
> (`max_projected_device_memory_ratio: 1.0`). The device-memory-gated run at
> `/private/tmp/qatq-live-vram-server-family-policy-soak-bootstrap-notrace-device-gated-20260625`
> passed all six cases with empty comparison gate failures. Backend K/V ratios
> were `0.964x`, `0.972x`, and `0.969x`; projected device-memory ratios were
> `0.995x`, `0.997x`, and `0.982x`. The run also kept the existing throughput,
> latency, RSS-growth, and RSS-tail gates. This is strong accepted
> no-trace shared-server policy proof from llama.cpp backend diagnostics, while
> direct hardware peak-VRAM counters remain an open production gate.
> Burn-in orchestration is now explicit too:
> `scripts/llama_cpp_live_vram_server_burnin.py` repeats a server matrix,
> stops on the first failed run, and can enforce aggregate jitter gates across
> repeated case metrics. The first layer-policy burn-in at
> `/private/tmp/qatq-live-vram-server-layer-policy-burnin2-device-jitter-20260625`
> failed correctly on the existing QATQ/native RSS-growth ratio gate
> (`2.401x > 2.0x`), while backend K/V and projected device memory still
> improved. The scoped accepted-family Qwen2.5 1.5B burn-in at
> `/private/tmp/qatq-live-vram-server-family-qwen15b-policy-burnin2-device-jitter-20260625`
> passed two native/QATQ matrix repeats with empty comparison and aggregate
> gate failures. Backend K/V and projected-device jitter ratios were `1.0` for
> both native and QATQ cases; QATQ RSS-growth jitter was `1.010x`, and QATQ
> RSS tail growth stayed at or below `64 KiB`. This is bounded repetition
> evidence, not an overnight burn-in claim.
> The bounded burn-in was then broadened across the two Qwen families at
> `/private/tmp/qatq-live-vram-server-family-qwen15b-qwen3b-policy-burnin2-device-jitter-20260625`.
> That run repeated the accepted family policy twice over Qwen2.5 1.5B and
> Qwen2.5 3B native/QATQ pairs, for eight real matrix cases total. Both repeats
> passed with empty comparison and aggregate gate failures. Backend K/V and
> projected-device jitter ratios were `1.0` for all four cases. Qwen2.5 1.5B
> QATQ/native p50 throughput ratios were `0.952x` and `0.978x`; backend K/V
> ratio stayed `0.964x` and projected device ratio stayed `0.995x`. Qwen2.5 3B
> QATQ/native p50 throughput ratios were `0.996x` and `0.988x`; backend K/V
> ratio stayed `0.972x` and projected device ratio stayed `0.997x`.
> The later full-family burn-in at
> `/private/tmp/qatq-live-vram-server-family-policy-soak-burnin3-p256q4-p05-tailgate-20260626`
> supersedes the separate Qwen-family and earlier two-repeat burn-ins as the
> current accepted-policy repeatability proof. It passed three complete
> Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini native/QATQ repeats with empty
> comparison and aggregate gate failures.
> Direct hardware peak-VRAM counter availability is now checked explicitly with
> `scripts/llama_cpp_live_vram_hardware_counters.py`. The report at
> `/private/tmp/qatq-live-vram-server-family-policy-soak-burnin2-taildelta-security-gated-20260625/hardware-counters.json`
> confirms that the latest accepted burn-in repeat has backend projected
> device memory and accelerator-breakdown diagnostics for every case, but does
> not have direct peak-VRAM counter evidence. On this host, `powermetrics`
> requires superuser and documents per-process GPU time rather than per-process
> peak GPU memory, while `vmmap` reports virtual memory maps rather than direct
> peak GPU memory. Backend memory and RSS gates remain valid engineering
> evidence, but they are not claimed as hardware peak-VRAM proof.
> A direct memory-accounting follow-up then separated a useful selected-layer
> proof from an over-aggressive one. Reusing the old 14-layer breadth shape
> failed correctly because strict accounting reported `44,040,192` staged bytes
> against a `36,700,160` byte total K/V context. The smaller selected-layer
> config at
> `/private/tmp/qatq-live-vram-native-layer-memory-gpufallback-pressure1g-repeat2-20260625`
> passed the strict live-paging, native page-streaming, aggregate codec,
> stable-reclaim, stable-codec, and MLX gates twice under `1 GiB` touched
> host-memory pressure for Qwen2.5 1.5B. Both iterations selected four
> QATQ-routed layers, restored `168/168` pages exactly, offloaded `112/112`
> QATQ pages with zero pass-through, kept `56` pages resident, reclaimed
> `23.00 MiB` of persistent GPU K/V, and stored `25.00 MiB` with QATQ versus
> `28.65 MiB` with zstd and `31.12 MiB` with lz4. MLX checked `28` layers and
> `336` heads with streaming/materialised ratios of `1.768` and `1.776`. This
> is the current direct residency proof for the
> fixed selected-layer path, but it is still a single-model profile rather than
> a broad production claim.
> The same selected-layer memory-accounting proof has now been broadened across
> the local three-model runtime set. The repeat matrix at
> `/private/tmp/qatq-live-vram-native-layer-memory-breadth-pressure1g-repeat2-20260625`
> passed `6/6` cases under `1 GiB` touched host-memory pressure: Qwen2.5 1.5B,
> Qwen2.5 3B, and Phi 3.5 mini, each over two iterations. All cases selected
> four QATQ-routed layers, restored every page exactly, used zero pass-through
> pages, passed strict live-paging and native page-streaming gates, passed MLX
> page-bounded streaming attention over the reported coverage, and kept stable
> reclaim/QATQ byte counts. Reclaim/QATQ/zstd/lz4 were
> `23.00/25.00/28.65/31.12` MiB for Qwen2.5 1.5B,
> `52.00/49.62/57.42/62.43` MiB for Qwen2.5 3B, and
> `432.00/391.57/448.79/493.38` MiB for Phi 3.5 mini. This is the strongest
> current direct-residency breadth proof; longer latency-tail and server soak
> evidence remain separate production gates.
> This closes the first three-model-family scoped native multi-stream
> retained-table proof, the first 100-cycle native multi-stream server burn-in,
> Qwen2.5 1.5B and Qwen2.5 3B 2 GiB native multi-stream pressure passes, and
> 50-cycle native multi-stream long-soak coverage for Qwen2.5 3B and Phi 3.5
> mini, plus one focused Phi page-size variation and one Qwen2.5 1.5B
> long-context crash-fix rerun; it does not replace broader
> page/context/pressure/runtime variation.

> Current-state correction: this report contains historical live-VRAM
> hardening notes from 2026-06-23 and 2026-06-24. A fresh structural audit of
> the pinned adapter patch now reports `export_ready: true`,
> `page_staging_ready: true`, and `live_paging_ready: true` with the
> structural native live-paging gate. The current patch contains a
> backend-scheduled multi-page retained page-table
> `GGML_OP_QATQ_SEGMENTED_KQV` route with typed
> f16/f32 mask handling and a bounded Metal threadgroup kernel that computes
> the per-query/head softmax logits once before writing output dimensions. Fresh
> local Qwen2.5 1.5B Metal smokes preserved the full-GPU baseline generated
> token IDs for 2-token and 8-token continuations. The latest token-page-table
> 8-token focused
> run matched `[264, 943, 315, 56287, 429, 5711, 279, 56287]` between full-GPU
> and page-pool backend-op paths while writing 784 page-segment records, all
> consumed through `backend_scheduled_segmented_attention` with 64-token pages.
> That focused run reached 31.21 tok/s with 1,575 graph nodes, 58 graph
> splits, and a 4.10 MiB Metal compute buffer after replacing repeated V-side
> page lookups with page-range accumulation, carrying explicit
> token-to-page/token-to-local tables into the Metal backend op, and consuming a
> retained tiled page table directly, versus 30.74 tok/s for graph-arena,
> 21.7 tok/s for the earlier strict staged-arena route, and 56.3 tok/s
> full-GPU on the same rebuilt binary family. A later 32-token Qwen2.5 1.5B
> Metal smoke restored graph reuse to 30 reused graphs, matched the full-GPU
> 32-token output manifest exactly, and improved the retained-table eval path
> to 22.19 ms/token, 45.06 tok/s after sizing the small-window Metal
> threadgroup to one SIMD width. The latest page-aligned reserve run kept the
> same 32-token output manifest match while staging only one 64-token segment
> per K/V/layer, reducing graph nodes from 1,575 to 1,071, lowering the CPU
> compute buffer to 0.0985 MiB, and reaching 21.43 ms/token, 46.65 tok/s with
> 30 graph reuses. A tighter `--n-kv-pad-tokens 48` runner setting preserved
> the same generated output while staging `n_kv: 48`, lowered CPU compute
> buffer use to 0.0946 MiB, and reached 21.11 ms/token, 47.36 tok/s, with
> 28 graph reuses. The corresponding full-GPU baseline remains materially
> faster at 11.37 ms/token, 87.98 tok/s.
> Latency optimisation remains open for longer contexts and broader pressure
> profiles, but the focused retained-table route no longer builds a second
> per-graph page-pool tensor before the backend op. Current patch-file and
> applied-source audits pass `--require-live-paging` with empty required
> live-paging and page-staging failure lists; legacy concat/persistent-source
> diagnostic failures remain visible as non-required compatibility checks. A strict
> backend-op breadth matrix on real Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini
> Metal generation now passes with exact page restore, MLX streaming-attention
> equivalence, sealed page evidence, 512 MiB host-memory pressure, and QATQ
> beating raw/zstd/lz4 on the same page boundaries. A rebuilt backend-op dtype
> slice also passed bf16 and f32 Qwen2.5 Coder 3B cases with the same strict
> gates. Any older paragraph that describes the native route as missing is
> superseded. Production-complete live VRAM still requires broader dtype,
> page-size, long-context,
> latency-tail, memory-pressure, and soak coverage.
>
> 2026-06-25 correction: fresh diagnostics found and fixed adapter
> correctness/attestation issues in the retained page-table route. The
> segmented graph bridge now consumes a retained page-table `sync_tensor` when
> one exists, preventing stale retained page views from being read before the
> graph-copy from live K/V bytes executes. The Metal backend-op dispatch is
> currently pinned to the conservative 32-thread path because the larger
> 256-thread path produced divergent generated tokens on a 705-token Qwen2.5
> 1.5B prompt. The manifest writer now counts retained tiled page-table pools in
> `gpu_page_staging_bytes`, so the live-paging gate no longer hides Metal
> allocation overhead. The follow-up compact allocator now keeps cold/offloaded
> pages CPU-backed, retains only scheduler-selected pages in a compact GPU pool,
> and falls back to a graph-local transient page pool for the current attention
> window when mixed residency prevents direct retained-table consumption. With
> those fixes, the backend-op path preserves output, emits real offload/restore
> events, passes strict live-paging/native page-streaming on the focused
> Qwen2.5 1.5B diagnostic, and proves compact persistent K/V residency. Live
> VRAM reduction is therefore no longer blocked on the first backend-op compact
> proof, but production readiness is still blocked on breadth, long-context
> latency, pressure, security, and soak evidence.

It started as a coarse runtime GPU KV reduction proof at whole-tensor/layer
granularity. The current patched llama.cpp adapter also has experimental
token-page staging evidence: cold K/V page data can be staged through
QATQ-backed page exports and checked by `qatq-kv-bench
--live-vram-live-paging-gate` plus an external MLX streaming-attention
reference. This is still experimental substrate rather than a
production-complete, transparent live paging implementation across all
runtimes, models, prompts, sequence mixes, pressure profiles, and native
attention kernels.

## Environment

| field | value |
| --- | --- |
| date | 2026-06-23 to 2026-06-24 |
| host OS | macOS 26.5.1 build 25F80 |
| GPU runtime | llama.cpp Metal backend |
| GPU reported by llama.cpp | Apple M4, 18,185 MiB free at model load |
| llama.cpp commit | `7992aa7c8e21ea2eb7a5e4802da56eec7b376036` |
| QATQ commit under test | `87be0cc327a1e6a2ac94c13e584d7f4eae821c5d` |
| adapter patch | `adapters/llama-cpp/qatq-kv-export-7992aa7c8.patch` |
| llama.cpp build | `CMAKE_BUILD_TYPE=Release`, `GGML_METAL=ON`, `GGML_METAL_EMBED_LIBRARY=ON` |
| QATQ command path | `cargo run --bin qatq-kv-bench -- --live-vram-export-dir ...` |
| reproducible adapter bootstrap | `scripts/llama_cpp_adapter_bootstrap.py` |
| clean bootstrap proof | `/private/tmp/qatq-llama-bootstrap-proof/bootstrap-report.json`; applied-source audit `/private/tmp/qatq-llama-bootstrap-proof/qatq-adapter-audit.json` |
| clean bootstrap strict server proof | `/private/tmp/qatq-live-vram-server-strict-bootstrap-proof-20260625` passed the strict three-model server-cancellation matrix using `/private/tmp/qatq-llama-bootstrap-proof/build-qatq/bin/llama-server` |
| reproducible evidence runner | `scripts/llama_cpp_live_vram_evidence.py` |
| KV dtype | f16 keys and f16 values |

The default sandboxed command environment could not create a Metal command
queue. The GPU runs were therefore executed with normal host GPU access.

For future local reproductions, run:

```sh
python3 scripts/llama_cpp_live_vram_evidence.py \
  --llama-simple /path/to/patched/llama-simple \
  --model /path/to/model.gguf \
  --model-id <stable-model-id> \
  --sweep-kv-gpu-layers 18,24,30 \
  --short-prompt "..." \
  --deep-prompt-seed "..." \
  --work-dir /tmp/qatq-live-vram-evidence
```

The runner writes a `summary.md` plus the gated output-comparison and runtime
reclaim JSON evidence into the selected work directory. It is intentionally
limited to the current whole-tensor/layer allocator proof. By default it also
runs llama.cpp's native all-CPU-KV baseline, requires the mixed-KV path to
preserve output against that baseline, requires mixed-KV decode to stay within
the configured full-GPU regression ceiling, and requires mixed-KV decode to be
faster than all-CPU-KV.

For multi-model regression evidence, run:

```sh
python3 scripts/llama_cpp_live_vram_matrix.py \
  --config adapters/llama-cpp/live-vram-matrix.local.example.json \
  --llama-simple /path/to/patched/llama-simple \
  --work-dir /tmp/qatq-live-vram-matrix \
  --iterations 2
```

That wrapper executes the same fail-closed per-case runner for every configured
model/prompt/frontier and writes an aggregate `summary.md`. Every repeated
iteration has to pass the same per-case gates.

## Results

| model / workload | prompt tokens | tensors | llama.cpp GPU KV buffer | raw active KV | QATQ stored | zstd | lz4 | QATQ saving vs raw | QATQ saving vs zstd | exact restores | pages beating best general codec |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Qwen2.5 1.5B Instruct, short smoke | 19 | 56 | 7.00 MiB | 0.52 MiB | 0.49 MiB | 0.49 MiB | 0.52 MiB | 5.93% | -0.18% | 56 / 56 | 3 / 56 |
| Qwen2.5 1.5B Instruct, migration/control prompt | 4,224 | 56 | not captured in retained log | 115.50 MiB | 85.81 MiB | 104.77 MiB | 113.87 MiB | 25.71% | 18.10% | 56 / 56 | 56 / 56 |
| Qwen2.5 Coder 3B Instruct, software-engineering prompt | 3,600 | 72 | 135.00 MiB | 126.56 MiB | 95.90 MiB | 115.18 MiB | 125.29 MiB | 24.23% | 16.74% | 72 / 72 | 72 / 72 |
| Phi 3.5 mini Instruct, daily-driver prompt | 3,072 | 64 | 1,248.00 MiB | 1,152.00 MiB | 884.37 MiB | 1,047.21 MiB | 1,155.02 MiB | 23.23% | 15.55% | 64 / 64 | 64 / 64 |
| Phi 3.5 mini Instruct, fresh Apple M4 smoke | 50 | 64 | 96.00 MiB | 18.75 MiB | 16.13 MiB | 17.35 MiB | 18.81 MiB | 13.99% | 7.07% | 64 / 64 | 64 / 64 |
| Phi 3.5 mini Instruct, attested manifest smoke | 41 | 64 | 96.00 MiB | 15.38 MiB | 13.35 MiB | 14.23 MiB | 15.41 MiB | 13.17% | 6.16% | 64 / 64 | 64 / 64 |
| Phi 3.5 mini Instruct, real Metal export rerun | 51 | 64 | 96.00 MiB | 19.13 MiB | 16.41 MiB | 17.69 MiB | 19.17 MiB | 14.21% | 7.27% | 64 / 64 | 64 / 64 |
| Phi 3.5 mini Instruct, mixed 16/32 KV layers on Metal | 51 | 64 | 48.00 MiB | 19.13 MiB | 16.41 MiB | 17.69 MiB | 19.17 MiB | 14.21% | 7.27% | 64 / 64 | 64 / 64 |
| Phi 3.5 mini Instruct, deterministic reduced-KV continuation check | 29 | 64 | 48.00 MiB | 10.88 MiB | 9.87 MiB | 10.09 MiB | 10.92 MiB | 9.28% | 2.22% | 64 / 64 | 64 / 64 |
| Qwen2.5 Coder 3B Instruct, fresh mixed 18/36 KV layers on Metal | 3,521 | 72 | 63.00 MiB | 123.79 MiB | 92.30 MiB | 112.59 MiB | 122.53 MiB | 25.43% | 18.02% | 72 / 72 | 72 / 72 |
| Qwen2.5 Coder 3B Instruct, frontier-selected 30/36 KV layers on Metal | 3,584 | 72 | 105.00 MiB | 123.79 MiB | 92.30 MiB | 112.59 MiB | 122.53 MiB | 25.43% | 18.02% | 72 / 72 | 72 / 72 |
| Qwen2.5 Coder 7B Instruct, frontier-selected 24/28 KV layers on Metal | 3,584 | 56 | 168.00 MiB | 192.55 MiB | 149.08 MiB | 174.78 MiB | 189.84 MiB | 22.58% | 14.70% | 56 / 56 | 56 / 56 |
| Qwen2.5 1.5B Instruct, daily-driver prompt frontier | 2,816 | 56 | 66.00 MiB | 70.03 MiB | 52.55 MiB | 63.48 MiB | 69.03 MiB | 24.96% | 17.22% | 56 / 56 | 56 / 56 |
| Phi 3.5 mini Instruct, operations/security prompt frontier | 4,096 | 64 | 1,344.00 MiB | 1,530.38 MiB | 1,166.43 MiB | 1,389.85 MiB | 1,533.57 MiB | 23.78% | 16.08% | 64 / 64 | 64 / 64 |

Interpretation:

- Tiny exported pages can lose to zstd because the fixed container and strategy
  overhead dominates the 19-token smoke capture.
- Long-context real GPU captures are compression-positive and beat zstd/lz4 on
  every exported page in this run.
- Every QATQ candidate restored byte-identically before the evidence report
  counted the page.
- The 2026-06-24 frontier runner selected the fastest mixed-KV layer count that
  still preserved output, reclaimed GPU KV, stayed within the decode regression
  ceiling, and beat the all-CPU-KV baseline.
- The Phi run is the strongest stress point in this pass: 1.13 GiB of active
  f16 KV exported from a Metal-backed runtime and replayed through QATQ with
  exact restore on all 64 tensors.

## 2026-06-24 Strict Backend-Op Smoke

A fresh rebuilt patched llama.cpp checkout at
`/private/tmp/qatq-llama-live-work` was run through the strict backend-op route
with real Apple Metal execution and MLX GPU validation:

```sh
python3 scripts/llama_cpp_live_vram_matrix.py \
  --llama-simple /private/tmp/qatq-llama-live-work/build/bin/llama-simple \
  --config adapters/llama-cpp/live-vram-native-breadth.local.example.json \
  --work-dir /private/tmp/qatq-live-vram-backend-op-smoke-20260624 \
  --max-cases 1 \
  --require-live-paging \
  --require-native-page-streaming \
  --aggregate-codec-gate \
  --prune-bulk-artifacts \
  --timeout 900
```

The configured case passed 1/1:

| case | selected KV GPU layers | exact restores | offloaded QATQ pages | resident pages | sealed pages | pass-through | event trace | MLX layers / heads | MLX stream/materialised | reclaimable GPU KV | QATQ | zstd | lz4 | elapsed |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-15b-daily-native-512p-aggregate` | 14 | 168 / 168 | 112 / 112 | 56 | 112 | 0 | pass / 560 | 28 / 336 | 1.712 | 28.00 MiB | 27.53 MiB | 29.25 MiB | 31.12 MiB | 118.05 s |

`native-page-streaming-status.json` reported
`backend_scheduled_segmented_attention: true`,
`accelerated_runtime_attention_graph: true`,
`page_segments_attention_consumed: true`,
`page_bounded_attention_equivalence_passed: true`,
`page_bounded_attention_equivalence_max_abs_error: 0.0`, and
`external_mlx_streaming_attention_passed: true`. The external MLX gate checked
28 layers and 336 heads on `Device(gpu, 0)` with max absolute error
`2.6226043701171875e-06` against a `0.0001` tolerance and peak page-KV ratio
`0.4440589765828274` against a `0.75` gate.

This is the first strict backend-op proof for the current rebuilt adapter:
page-staged runtime KV placement reduced persistent GPU K/V residency, every
QATQ-restored page verified exactly, metadata seals were present for all
offloaded pages, the backend scheduled the segmented attention consumer, and
QATQ beat raw/zstd/lz4 in aggregate. It is a one-case smoke proof, not a
production-complete live-VRAM claim.

## 2026-06-24 Focused Backend-Op Kernel Optimisation Smoke

The Metal backend-op kernel was then reshaped from one single-thread
threadgroup per output element to one 256-thread threadgroup per query/head.
The new kernel uses bounded threadgroup memory to compute logits and the
softmax denominator once per query/head, then fans out output dimensions. The
host dispatch caps the fast path at 8,192 staged K/V tokens to avoid
unbounded threadgroup memory use; larger live contexts still need a tiled
kernel or a policy fallback before production-complete status.

The focused validation used the same rebuilt pinned checkout and local
Qwen2.5 1.5B Metal model:

```sh
LLAMA_QATQ_GRAPH_EXTRA_NODES=262144 \
/private/tmp/qatq-llama-live-work/build/bin/llama-simple \
  -m /Users/kabudu/projex/deliberium-group/deliberium/models/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf \
  -n 8 -ngl 99 \
  --qatq-output-manifest /private/tmp/qatq-live-vram-fast-baseline-n8/output-manifest.json \
  --qatq-token-timings /private/tmp/qatq-live-vram-fast-baseline-n8/token-timings.csv \
  "QATQ live KV paging is"

LLAMA_QATQ_GRAPH_EXTRA_NODES=262144 \
/private/tmp/qatq-llama-live-work/build/bin/llama-simple \
  -m /Users/kabudu/projex/deliberium-group/deliberium/models/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf \
  -n 8 -ngl 99 \
  --qatq-gpu-page-staging \
  --qatq-page-tokens 16 \
  --qatq-native-page-streaming-attention \
  --qatq-native-page-streaming-attention-ggml \
  --qatq-native-page-streaming-attention-backend-op \
  --qatq-attention-page-segments-trace /private/tmp/qatq-live-vram-fast-staged-n8/page-segments.jsonl \
  --qatq-output-manifest /private/tmp/qatq-live-vram-fast-staged-n8/output-manifest.json \
  --qatq-token-timings /private/tmp/qatq-live-vram-fast-staged-n8/token-timings.csv \
  "QATQ live KV paging is"
```

| mode | generated token IDs | reported decode speed | eval latency | total manifest time | page-segment records |
| --- | --- | ---: | ---: | ---: | ---: |
| full-GPU baseline | `[264, 943, 315, 56287, 429, 5711, 279, 56287]` | 56.32 tok/s | 11.58 ms/token | 142,047 us | n/a |
| staged backend-op | `[264, 943, 315, 56287, 429, 5711, 279, 56287]` | 21.70 tok/s | 32.76 ms/token | 368,662 us | 784 |

This is a material improvement over the previous staged-arena backend-op smoke,
which decoded around 9.9 tok/s with roughly 82.7 ms/token post-prompt average
decode time. It is still not good enough to call live VRAM reduction
production-complete: the fast kernel remains slower than full GPU, is bounded
to 8,192 staged tokens, and needs long-context tiling, broader runtime coverage,
memory-pressure soaks, and latency-tail gates.

The same patched binary was also run with
`LLAMA_QATQ_NATIVE_PAGE_STREAMING_DIRECT_SOURCE_FALLBACK=1`. This is a
diagnostic ceiling path: it preserves the logical page schedule and emits
separate `backend_scheduled_direct_source_attention` trace records, but reads
from the contiguous K/V source instead of the strict staged page arena. It is
not counted as the production page-staging proof.

| mode | generated token IDs | reported decode speed | eval latency | total manifest time | page-segment records |
| --- | --- | ---: | ---: | ---: | ---: |
| direct-source diagnostic | `[264, 943, 315, 56287, 429, 5711, 279, 56287]` | 33.47 tok/s | 23.01 ms/token | 239,001 us | 784 |

The retained page-table result is useful because it keeps token correctness,
reduces graph nodes from the graph-arena smoke's 1,799 nodes to 1,575 nodes at
the same 58 splits, and improves the focused page-table decode from 26.74 tok/s
to 31.21 tok/s by walking V page ranges directly, using explicit
token-to-page/token-to-local tables, and avoiding a second per-graph page-pool
copy. A later 32-token run at the same prompt/model matched the full-GPU output
manifest exactly, reused 30 graphs, and improved the retained page-table eval
slice to 22.19 ms/token, 45.06 tok/s with a one-SIMD-width Metal threadgroup
for small token windows. A page-table upload cache experiment was rejected
because it produced divergent tokens; page metadata must still be updated per
decode step unless an execution-aware dirty-state protocol proves otherwise.
The remaining production optimisation targets are broader long-context latency,
safe dirty-page tracking, and sustained pressure coverage without reopening the
whole K/V source.

## 2026-06-24 Strict Backend-Op Breadth Matrix

The rebuilt patched runtime was then widened to the three-case native breadth
matrix with the backend-op route explicitly enabled and 512 MiB of host-memory
pressure:

```sh
python3 scripts/llama_cpp_live_vram_matrix.py \
  --llama-simple /private/tmp/qatq-llama-live-work/build/bin/llama-simple \
  --config adapters/llama-cpp/live-vram-native-breadth.local.example.json \
  --work-dir /private/tmp/qatq-live-vram-breadth-backend-op-20260624 \
  --require-live-paging \
  --require-native-page-streaming \
  --native-page-streaming-attention-backend-op \
  --aggregate-codec-gate \
  --host-memory-pressure-mib 512 \
  --prune-bulk-artifacts \
  --timeout 1200
```

The matrix passed 3/3 cases:

| case | selected KV GPU layers | exact restores | offloaded QATQ pages | resident pages | pass-through | event trace | MLX layers / heads | MLX stream/materialised | reclaimable GPU KV | QATQ | zstd | lz4 | elapsed |
| --- | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-15b-daily-native-512p-aggregate` | 14 | 168 / 168 | 112 / 112 | 56 | 0 | pass / 560 | 28 / 336 | 1.692 | 28.00 MiB | 27.53 MiB | 29.25 MiB | 31.12 MiB | 121.95 s |
| `qwen25-3b-general-native-1025p-l12-aggregate` | 12 | 216 / 216 | 216 / 216 | 0 | 0 | pass / 864 | 36 / 576 | 1.416 | 111.00 MiB | 90.73 MiB | 98.51 MiB | 106.95 MiB | 840.30 s |
| `phi35-mini-ops-native-512p-aggregate` | 16 | 192 / 192 | 128 / 128 | 64 | 0 | pass / 640 | 32 / 1024 | 1.432 | 480.00 MiB | 420.46 MiB | 455.42 MiB | 493.38 MiB | 289.41 s |

Every `native-page-streaming-status.json` reported
`backend_scheduled_segmented_attention: true`,
`accelerated_runtime_attention_graph: true`,
`page_segments_attention_consumed: true`,
`page_bounded_attention_equivalence_passed: true`,
`page_bounded_attention_equivalence_max_abs_error: 0.0`,
`external_mlx_streaming_attention_passed: true`, and no failures. The
backend-op path correctly reports `segmented_graph_bridge: false` because the
runtime is using the backend-scheduled consumer rather than the diagnostic
graph bridge.

The two generated-token latency samples per case were not configured as a
tail-latency gate, but they were favourable in this run: mixed p95/p99 was
lower than full-GPU p95/p99 by `19.43%`, `14.39%`, and `1.68%` for the three
cases respectively. Treat that as smoke telemetry, not as a production
p95/p99 latency proof.

This breadth matrix supports a stronger experimental claim than the single
smoke: scoped native backend-op live-VRAM reduction now passes across two model
families, three real GGUF models, two page sizes, exact restore, MLX
equivalence, aggregate codec gates, and 512 MiB host-memory pressure. Remaining
production gates are dtype breadth on the rebuilt backend-op route, broader
page-size breadth, long-context deep latency, repeated soaks, and comparisons
against runtime-native alternatives such as CPU offload or KV quantisation.

## 2026-06-24 Strict Backend-Op Dtype Slice

The rebuilt patched runtime was then run through the first two cases of
`adapters/llama-cpp/live-vram-native-dtype.local.example.json`, covering bf16
and f32 Qwen2.5 Coder 3B cache pages with the backend-op route explicitly
enabled and 512 MiB of host-memory pressure:

```sh
python3 scripts/llama_cpp_live_vram_matrix.py \
  --llama-simple /private/tmp/qatq-llama-live-work/build/bin/llama-simple \
  --config adapters/llama-cpp/live-vram-native-dtype.local.example.json \
  --work-dir /private/tmp/qatq-live-vram-dtype-backend-op-20260624 \
  --max-cases 2 \
  --require-live-paging \
  --require-native-page-streaming \
  --native-page-streaming-attention-backend-op \
  --aggregate-codec-gate \
  --host-memory-pressure-mib 512 \
  --prune-bulk-artifacts \
  --timeout 1200
```

The dtype slice passed 2/2 cases:

| case | dtype | selected KV GPU layers | exact restores | offloaded QATQ pages | resident pages | pass-through | event trace | MLX layers / heads | MLX stream/materialised | reclaimable GPU KV | QATQ | zstd | lz4 | elapsed |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-coder-3b-bf16-review-512p` | bf16 | 18 | 144 / 144 | 72 / 72 | 72 | 0 | pass / 432 | 36 / 576 | 1.264 | 27.00 MiB | 19.87 MiB | 22.17 MiB | 27.60 MiB | 101.48 s |
| `qwen25-coder-3b-f32-review-512p` | f32 | 18 | 144 / 144 | 72 / 72 | 72 | 0 | pass / 432 | 36 / 576 | 1.168 | 54.00 MiB | 47.31 MiB | 51.55 MiB | 55.26 MiB | 105.84 s |

Both `native-page-streaming-status.json` files reported
`backend_scheduled_segmented_attention: true`,
`accelerated_runtime_attention_graph: true`,
`page_segments_attention_consumed: true`,
`page_bounded_attention_equivalence_passed: true`,
`page_bounded_attention_equivalence_max_abs_error: 0.0`,
`external_mlx_streaming_attention_passed: true`, and no failures. This proves
that the rebuilt backend-op route is not limited to f16 K/V pages on the tested
Qwen2.5 Coder 3B path. The full dtype production gate remains open until the
larger dtype matrix, repeated soaks, and long-context latency gates pass.

## 2026-06-24 Current Binary Live-Paging Matrix

Fresh host-side reruns on 2026-06-24 used the patched llama.cpp checkout at
`7992aa7c8e21ea2eb7a5e4802da56eec7b376036`, the local
`/private/tmp/qatq-llama.cpp/build/bin/llama-simple` binary, fresh QATQ release
binaries, local GGUF models, Apple Metal, MLX `Device(gpu, 0)`, GPU page
staging, strict live-paging gates, native `ggml_segmented_kqv`, aggregate codec
gates, stable reclaim/codec-byte gates, and 512 MiB host-memory pressure.

| matrix | work dir | result | coverage |
| --- | --- | ---: | --- |
| breadth | `/private/tmp/qatq-live-vram-real-gpu-breadth-rerun-20260624` | 3/3 pass | Qwen2.5 1.5B, Qwen2.5 3B, Phi 3.5 mini |
| sustained Coder | `/private/tmp/qatq-live-vram-real-gpu-sustained-rerun-20260624` | 4/4 pass | Qwen2.5 Coder 3B/7B review and memory profiles |
| dtype breadth | `/private/tmp/qatq-live-vram-real-gpu-dtype-rerun-20260624` | 8/8 pass | bf16 and f32 K/V cache settings across Qwen/Phi profiles |
| page-size breadth | `/private/tmp/qatq-live-vram-real-gpu-page-size-rerun-20260624` | 5/5 pass | 256, 1024, and 2048-token page profiles |
| latency tail | `/private/tmp/qatq-live-vram-real-gpu-latency-tail-20260624` | 1/1 pass | 32-token p95/p99 decode-latency gate for Qwen2.5 1.5B |
| sustained Coder latency | `/private/tmp/qatq-live-vram-real-gpu-sustained-latency-32tok-20260624` | 4/4 pass | 32-token p95/p99 decode-latency gates for Qwen2.5 Coder 3B/7B profiles |
| sustained Coder latency soak | `/private/tmp/qatq-live-vram-real-gpu-sustained-latency-soak-2x-20260624` | 8/8 pass | repeated 2x p95/p99 decode-latency gates for Qwen2.5 Coder 3B/7B profiles |
| breadth latency | `/private/tmp/qatq-live-vram-real-gpu-breadth-latency-32tok-20260624` | 3/3 pass | 32-token p95/p99 gates for Qwen2.5 1.5B, Qwen2.5 3B, and Phi 3.5 mini |
| dtype latency | `/private/tmp/qatq-live-vram-real-gpu-dtype-latency-32tok-20260624` | 8/8 pass | 32-token p95/p99 gates for bf16/f32 K/V cache settings |
| page-size latency | `/private/tmp/qatq-live-vram-real-gpu-page-size-latency-32tok-20260624` | 5/5 pass | 32-token p95/p99 gates for 256, 1024, and 2048-token page profiles |
| strict long-context latency | `/private/tmp/qatq-live-vram-long-context-config-gated-20260624` | 0/1 fail | Qwen2.5 Coder 3B 6.8k-token native backend-op profile timed out before deep mixed-KV timing evidence |

Across those fresh reruns, every configured case preserved deterministic output,
restored all exported QATQ pages exactly, used zero pass-through pages, passed
the strict live-paging gate, passed the native non-concat page-streaming gate,
and beat raw, zstd, and lz4 in aggregate on the same page boundaries.

## 2026-06-25 Bootstrapped Native Page-Size Matrix

The page-size breadth proof was rerun from the clean pinned llama.cpp bootstrap
at `/private/tmp/qatq-llama-bootstrap-proof`, using the bootstrapped
`llama-simple` binary and `--llama-cpp-source` so strict native verification
audited the exact patched source tree. The matrix command was:

```sh
python3 scripts/llama_cpp_live_vram_matrix.py \
  --llama-simple /private/tmp/qatq-llama-bootstrap-proof/build-qatq/bin/llama-simple \
  --llama-cpp-source /private/tmp/qatq-llama-bootstrap-proof \
  --config adapters/llama-cpp/live-vram-native-page-size.local.example.json \
  --work-dir /private/tmp/qatq-live-vram-page-size-bootstrap-20260625 \
  --require-live-paging \
  --require-native-page-streaming \
  --aggregate-codec-gate \
  --host-memory-pressure-mib 1024 \
  --prune-bulk-artifacts \
  --timeout 1800
```

It passed all five configured real Metal/MLX cases with strict live-paging,
strict native page-streaming, native flattened Flash page consumption, aggregate
codec gates, restore-slot pressure checks, 1 GiB host-memory pressure, zero
pass-through pages, and QATQ beating raw/zstd/lz4 on the same page boundaries.

| case | selected KV GPU layers | page tokens | exact restores | offloaded pages | resident pages | reclaimable GPU MiB | QATQ MiB | zstd MiB | lz4 MiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Qwen2.5 1.5B page256 | 4 | 256 | 840/840 | 728/728 | 112 | 33.00 | 82.97 | 93.09 | 101.15 |
| Qwen2.5 1.5B page2048 | 9 | 2048 | 112/112 | 56/56 | 56 | 51.00 | 76.10 | 92.82 | 100.94 |
| Qwen2.5 Coder 3B page256 | 4 | 256 | 1152/1152 | 1008/1008 | 144 | 71.00 | 115.46 | 129.41 | 140.61 |
| Qwen2.5 Coder 3B page2048 | 12 | 2048 | 144/144 | 72/72 | 72 | 72.00 | 105.62 | 128.90 | 140.36 |
| Qwen2.5 3B page1024 | 12 | 1024 | 288/288 | 216/216 | 72 | 24.00 | 102.62 | 122.82 | 133.69 |

The checked-in page-size config now uses compact `[1, 2, 4]` selected-layer
frontiers for the 256-token cases so page staging does not exceed the reclaimed
resident K/V memory. The 2048-token cases keep broader page-segment traces so
the strict verifier can observe multi-segment streaming rather than only the
live-offloaded subset. This is stronger page-size evidence for the current
Qwen/Qwen-Coder local proof set, not a production-complete claim for every
model family or runtime.

The same bootstrap was then used for the Phi 3.5 mini page-size matrix:

```sh
python3 scripts/llama_cpp_live_vram_matrix.py \
  --llama-simple /private/tmp/qatq-llama-bootstrap-proof/build-qatq/bin/llama-simple \
  --llama-cpp-source /private/tmp/qatq-llama-bootstrap-proof \
  --config adapters/llama-cpp/live-vram-native-phi-page-size.local.example.json \
  --work-dir /private/tmp/qatq-live-vram-phi-page-size-bootstrap-20260625 \
  --require-live-paging \
  --require-native-page-streaming \
  --aggregate-codec-gate \
  --host-memory-pressure-mib 1024 \
  --prune-bulk-artifacts \
  --timeout 1800
```

It passed all three configured Phi cases with the same strict native, MLX,
restore-slot pressure, aggregate codec, and host-pressure gates.

| case | selected KV GPU layers | page tokens | exact restores | offloaded pages | resident pages | reclaimable GPU MiB | QATQ MiB | zstd MiB | lz4 MiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Phi 3.5 mini page256 | 4 | 256 | 384/384 | 256/256 | 128 | 420.00 | 424.97 | 482.14 | 529.52 |
| Phi 3.5 mini page512 | 4 | 512 | 192/192 | 128/128 | 64 | 432.00 | 429.63 | 492.44 | 541.58 |
| Phi 3.5 mini page1024 | 4 | 1024 | 128/128 | 64/64 | 64 | 432.00 | 427.59 | 492.29 | 541.58 |

The Phi 1024-token case uses broader page-segment tracing for the same reason
as the Qwen 2048-token cases: strict native verification must be able to see
multi-segment page consumption, not only the live-offloaded subset.

The latency-tail run adds a focused performance gate over actual per-token
decode timings from `tokens.csv`: mixed-KV p95 was `31,026 us` versus full-GPU
p95 `31,467 us`, and mixed-KV p99 was `74,207 us` versus full-GPU p99
`75,977 us`. That proves the matrix can now fail closed on p95/p99 regressions
for configured runs; it does not replace longer burn-in or broader latency
coverage.

The sustained Coder latency run promoted that gate to four Qwen2.5 Coder 3B/7B
review and memory profiles with 32 token samples per case. Mixed-KV p95
regressions ranged from `-4.30%` to `-0.17%`; mixed-KV p99 ranged from
`-5.58%` to `+0.87%`, while all four cases also passed strict live-paging,
native attention, MLX equivalence, exact restore, and aggregate codec gates.

A repeated sustained-Coder latency soak then reran the same four Qwen2.5 Coder
3B/7B profiles twice at
`/private/tmp/qatq-live-vram-real-gpu-sustained-latency-soak-2x-20260624`. It
passed 8/8 cases with strict live-paging, native `ggml_segmented_kqv`, GPU page
staging, aggregate codec gates, stable reclaim/codec-byte gates, a 35%
elapsed-jitter gate, 512 MiB host-memory pressure, MLX equivalence, exact
restore, and zero pass-through pages. The largest observed mixed-KV p95
regression was `+10.86%`; the largest p99 regression was `+1.58%`. This is a
local repeated-latency stability proof for the current sustained Coder matrix,
not a replacement for 1-hour or overnight burn-in.

The same p95/p99 gate now covers the current local breadth, dtype, and
page-size matrices as well. Breadth passed 3/3 with largest p95/p99 regressions
`+2.78%` and `+5.80%`; dtype passed 8/8 with largest `+5.21%` and `+2.80%`;
page-size passed 5/5 with largest `+3.21%` and `+0.02%`. These runs keep the
latency claim scoped to 32-token local matrix samples; they do not replace
long-context or long-duration burn-in measurements.

Long-context active-decode diagnostics on 2026-06-24 then found a real
production blocker. The early capped profile behind the later
`adapters/llama-cpp/live-vram-native-long-context-latency.local.example.json`
used Qwen2.5 Coder 3B, a 6.8k-token prompt, 1024-token pages,
`cold-after-hot`, `max_queued_pages: 8`, deep full-GPU output comparison, and
deep p95/p99 gates. Correctness and exact restore passed in the viable
allocator cases, but the latency gate failed:

| work dir | configuration | result | reclaimable GPU KV | offloaded pages | deep p95 regression | deep p99 regression |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| `/private/tmp/qatq-live-vram-long-context-capped-96-20260624` | 18/36 KV layers, native segmented, cap 96 | fail latency | 58.50 MiB | 96 | `+434%` | `+64.7%` |
| `/private/tmp/qatq-live-vram-long-context-capped-16-reuse-20260624` | 18/36 KV layers, native segmented, cap 16, retained-page reuse | fail latency | 58.50 MiB | 16 | `+442%` | `+71.9%` |
| `/private/tmp/qatq-live-vram-long-context-capped-16-nonnative-20260624` | 18/36 KV layers, non-native fallback, cap 16 | fail latency | 58.50 MiB | 16 | `+269%` | `+32.7%` |
| `/private/tmp/qatq-live-vram-long-context-capped-16-l24-nonnative-20260624` | 24/36 KV layers, non-native fallback, cap 16 | fail allocator | over-staged | - | - | - |
| `/private/tmp/qatq-live-vram-long-context-capped-16-l23-min02-nonnative-20260624` | 23/36 KV layers, non-native fallback, cap 16, 2% reclaim diagnostic | fail latency | 7.25 MiB | 16 | `+627%` | `+20.6%` |
| `/private/tmp/qatq-live-vram-long-context-low-overhead-native-all-staged-cap96-20260624` | 36/36 KV layers, native segmented, cap 96, page-segment trace disabled | fail latency | 126.00 MiB | 96 | `+98.4%` | `+57.6%` |
| `/private/tmp/qatq-live-vram-long-context-low-overhead-persistent-all-staged-cap96-20260624` | 36/36 KV layers, persistent page-source, cap 96, page-segment trace disabled | fail latency | 126.00 MiB | 96 | `+103.3%` | `+92.9%` |
| `/private/tmp/qatq-live-vram-long-context-config-gated-20260624` | 18/36 KV layers, backend-op native page streaming, cap 16, 1 GiB host pressure, 32 deep tokens | fail timeout | not reached | not reached | no samples | no samples |

These runs are intentionally negative evidence. They prove that capped
offloading, retained page allocation reuse, and layer-count tuning are not
enough for production long-context active decode in the current adapter. The
low-overhead follow-up did fix native page-segment residency accounting: the
physical page-staging allocation now honours the same offload predicate as the
event trace, allowing all-layer page staging to reclaim 126.00 MiB instead of
failing the allocator gate. The next live-VRAM implementation task is still a
deeper runtime optimisation or latency-budgeted fallback that prevents page
staging from creating a p95 cliff.

The allocator fix was also validated by a strict short native proof at
`/private/tmp/qatq-live-vram-long-context-native-proof-all-staged-cap96-v3-20260624`.
That run used all 36 Qwen2.5 Coder 3B KV layers, 1024-token pages, cap 96,
strict live-paging, strict native page-streaming, and MLX page-streaming
attention. It reclaimed 126.00 MiB, restored 432/432 pages exactly, stored
96/96 offloaded pages through QATQ with zero pass-through pages, proved
`ggml_segmented_kqv` non-concat attention consumption, and checked all 36 layers
and 576 heads in MLX. It generated only 8 tokens, so it is correctness and
allocator proof rather than p95/p99 long-context latency proof.

The sealed latency-budget fallback path then passed a 128-token long-context
run at
`/private/tmp/qatq-live-vram-long-context-runtime-reclaim-sealed-fallback-128tok-r4-20260624`.
That run intentionally used the runtime-reclaim gate rather than the strict
native page-streaming gate. It restored 432/432 pages exactly, stored 96/96
offloaded pages through QATQ with 96/96 metadata seals, used zero pass-through
pages, beat raw/zstd/lz4 on 432/432 page boundaries, and reduced persistent GPU
K/V residency from 207.00 MiB to 0.00 MiB. The 127-token deep timing comparison
stayed inside the 15% p95 / 20% p99 latency budget: p50 `+0.32%`, p95
`+0.43%`, and p99 `+13.02%` versus the full-GPU baseline. This is now the
strongest production-shaped long-context result, but its claim boundary is
host-backed runtime reclaim with exact restore, sealed page metadata, and
preserved output. It does not replace the strict native page-streaming proof,
whose long-context p95 work remains open.

A later strict config-gated rerun kept the production-shaped long-context
matrix settings in
`adapters/llama-cpp/live-vram-native-long-context-latency.local.example.json`:
stable reclaim and codec-byte gates, 32 short/deep generated-token samples,
15% p95 and 20% p99 budgets, deep full-GPU baseline, 1 GiB host-memory
pressure, strict live-paging, strict native page streaming, and the backend-op
path. The run completed the short full-GPU, short mixed-KV, and deep full-GPU
stages, then timed out after 2,400 seconds in the deep mixed-KV
`llama-simple` command before writing `token-timings.csv` or
`output-manifest.json` for that stage. It had emitted 1,224 page-segment
records and 576 attention-fixture files before timeout. This is a clean
fail-closed frontier: it does not undermine exact QATQ restore or the
short-context native proofs, but it confirms that the custom segmented
backend-op long-context active-decode path is not production-ready by itself.

A 2026-06-25 follow-up moved that frontier forward. The patched graph now
marks each segment with `live_offloaded` and uses the backend-op route only for
layers with cold/offloaded pages; resident-only layers fall back to stock
llama.cpp attention. The verifier now treats mixed traces as valid only when
cold segments are native and consumed, and resident fast-path segments are not
claimed as native streaming. The key reruns were:

| work dir | configuration | native status | output | deep p95 regression | deep p99 regression |
| --- | --- | --- | --- | ---: | ---: |
| `/private/tmp/qatq-live-vram-long-context-backend-op-l4-fast8192-20260625` | 4 cold KV layers before selective fast path | pass | pass | `+756%` | `+479%` |
| `/private/tmp/qatq-live-vram-long-context-backend-op-l4-selective-r2-20260625` | 4 cold KV layers, selective resident fast path | pass; 152 cold backend-op events, 1,216 resident fast-path events | pass | `+73.9%` | `+58.3%` |
| `/private/tmp/qatq-live-vram-long-context-backend-op-l1-selective-20260625` | 1 cold KV layer, selective resident fast path, shared retained pool | pass; 38 cold backend-op events, 1,330 resident fast-path events | pass | `+20.1%` | `+14.9%` |
| `/private/tmp/qatq-live-vram-long-context-backend-op-l1-default-privatepool-20260625` | 1 cold KV layer, selective resident fast path, per-layer retained pool default | pass; 38 cold backend-op events, 1,330 resident fast-path events, 3.00 MiB GPU page staging | pass | `+19.6%` | `+19.4%` |
| `/private/tmp/qatq-live-vram-long-context-flattened-flash-l1-strict-20260625` | 1 cold KV layer, selective resident fast path, flattened page table into backend-scheduled Flash Attention | pass; 38 cold flattened Flash Attention events, 1,330 resident fast-path events, 3.00 MiB GPU page staging | pass | `+12.6%` | `+4.3%` |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-flash-20260625-r2` | earlier one-cold-layer single matrix, flattened Flash Attention, 1 GiB host pressure, superseded for latency-tail evidence | pass; `504/504` exact restores, `16/16` compressed offloads, zero pass-through, 237.00 MiB reclaimable GPU K/V | pass | `-7.5%` | `+13.4%` |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-129tok-2x-20260625` | earlier one-cold-layer matrix, retained flattened table, 2 iterations, 1 GiB host pressure | pass; `504/504` exact restores each iteration, `16/16` compressed offloads, zero pass-through, 223.25 MiB reclaimable GPU K/V | pass | worst `+3.1%` | worst `+4.2%` |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l2-129tok-20260625` | two-layer matrix, retained flattened table, 1 GiB host pressure | pass; `504/504` exact restores, `16/16` compressed offloads, zero pass-through, 194.50 MiB reclaimable GPU K/V | pass | `+7.3%` | `-2.5%` |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l3-q8-129tok-2x-20260625` | previous checked-in three-layer matrix, retained flattened table, cap 8, 2 iterations, 1 GiB host pressure | pass; `504/504` exact restores each iteration, `8/8` compressed offloads, zero pass-through, 165.75 MiB reclaimable GPU K/V | pass | worst `+8.6%` | better than baseline |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l4-q8-129tok-2x-20260625` | four-layer matrix, retained flattened table, cap 8, 2 iterations, 1 GiB host pressure | fail strict p95; exactness, compression, reclaim, and p99 still passed | pass | worst `+18.4%` | `+11.3%` |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l4-q4-129tok-2x-20260625` | four-layer matrix, retained flattened table, cap 4, before cold-slot reuse | fail strict p95; exactness, compression, reclaim, and p99 still passed | pass | worst `+19.2%` | better than baseline |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l4-q4-coldreuse-129tok-2x-20260625` | four-layer matrix, retained flattened table, cap 4, cold-slot reuse, 2 iterations, 1 GiB host pressure | pass; `504/504` exact restores each iteration, `4/4` compressed offloads, zero pass-through, 137.00 MiB reclaimable GPU K/V | pass | worst `+8.9%` | better than baseline |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l5-q4-coldreuse-129tok-2x-20260625` | five selected-layer breadth matrix, retained flattened table, cap 4, cold-slot reuse, 2 iterations, 1 GiB host pressure | pass; `504/504` exact restores each iteration, `4/4` compressed offloads, zero pass-through, 108.25 MiB reclaimable GPU K/V | pass | worst `+10.7%` | `+15.0%` |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l6-q4-coldreuse-129tok-2x-20260625` | six selected-layer breadth matrix, retained flattened table, cap 4, cold-slot reuse, 2 iterations, 1 GiB host pressure | pass; `504/504` exact restores each iteration, `4/4` compressed offloads, zero pass-through, 79.50 MiB reclaimable GPU K/V | pass | worst `+8.1%` | better than baseline |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l4-q8-coldreuse-129tok-2x-20260625` | four-layer matrix, retained flattened table, cap 8, cold-slot reuse, 2 iterations, 1 GiB host pressure | pass; `504/504` exact restores each iteration, `8/8` compressed offloads, zero pass-through, 137.00 MiB reclaimable GPU K/V | pass | worst `+1.7%` | `+0.9%` |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l4-q16-coldreuse-129tok-2x-20260625` | four-layer matrix, retained flattened table, cap 16, cold-slot reuse, 2 iterations, 1 GiB host pressure | pass; `504/504` exact restores each iteration, `16/16` compressed offloads, zero pass-through, 137.00 MiB reclaimable GPU K/V | pass | better than baseline | better than baseline |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l4-q32-coldreuse-129tok-2x-20260625` | four-layer matrix, retained flattened table, cap 32, cold-slot reuse, 2 iterations, 1 GiB host pressure | pass; `504/504` exact restores each iteration, `32/32` compressed offloads, zero pass-through, 137.00 MiB reclaimable GPU K/V | pass | worst `+7.7%` | better than baseline |
| `/private/tmp/qatq-live-vram-long-context-matrix-flattened-retained-l4-q96-coldreuse-129tok-2x-20260625` | four-layer matrix, retained flattened table, cap 96, cold-slot reuse, 2 iterations, 1 GiB host pressure | pass; `504/504` exact restores each iteration, `96/96` compressed offloads, zero pass-through, 137.00 MiB reclaimable GPU K/V | pass | worst `+8.4%` | `+6.4%` |
| `/private/tmp/qatq-live-vram-long-context-prefetch-active-20260625` | four-layer matrix, explicit prefetch 32, cap 96, cold-slot reuse, 2 iterations, 1 GiB host pressure | fail deep latency; exactness, compression, stable reclaim, stable codec bytes, and zero pass-through still passed | pass | iter 2 `+36.1%` | iter 2 `+40.7%` |
| `/private/tmp/qatq-live-vram-long-context-prefetch-active-q32-20260625` | checked-in four-layer matrix, explicit prefetch 32, cap 32, cold-slot reuse, 2 iterations, 1 GiB host pressure | pass; `504/504` exact restores each iteration, `32/32` compressed offloads, zero pass-through, 137.00 MiB reclaimable GPU K/V | pass | worst `+7.6%` | better than baseline |
| `/private/tmp/qatq-live-vram-long-context-latency-repeat2-current-20260625` | current binaries, checked-in four-layer matrix, explicit prefetch 32, cap 32, cold-slot reuse, 2 iterations, 1 GiB host pressure, 128 short/deep samples | pass; `504/504` exact restores each iteration, `32/32` compressed offloads, zero pass-through, 137.00 MiB reclaimable GPU K/V, QATQ 179.00 MiB versus zstd 218.30 MiB and lz4 237.57 MiB | pass | better than baseline in both iterations | better than baseline in both iterations |
| `/private/tmp/qatq-live-vram-long-context-latency-bootstrap-repeat2-20260625` | clean bootstrapped pinned source/binary, checked-in four-layer matrix, explicit prefetch 32, cap 32, cold-slot reuse, 2 iterations, 1 GiB host pressure, 128 short/deep samples | pass; `504/504` exact restores each iteration, `32/32` compressed offloads, zero pass-through, 137.00 MiB reclaimable GPU K/V, stable QATQ 179.00 MiB versus zstd 218.30 MiB and lz4 237.57 MiB | pass | better than baseline in both iterations | better than baseline in both iterations |
| `/private/tmp/qatq-live-vram-long-context-backend-op-l1-p2048-selective-20260625` | 1 cold KV layer, 2,048-token pages | pass; 38 cold backend-op events, 1,330 resident fast-path events | pass | `+41.0%` | `+16.5%` |

This is material progress and a scoped strict native long-context pass, not a
production-complete live-VRAM claim. It proves the native path can preserve
output, avoid penalising resident layers, keep retained page pools proportional
to the actual cold-layer frontier by default, and clear the configured p95/p99
gate when eligible one-stream or stream-split multi-stream page tables are
flattened into llama.cpp's backend-scheduled Flash Attention route. The retained
flattened table removes
the repeated full transient-table rebuild from mixed resident/offloaded windows.
The cold-slot reuse follow-up also removes repeated syncs for immutable
live-offloaded rows after their retained table slot is populated; first use and
mutable tail rows still sync before attention. The current checked-in stable
strict frontier is four selected KV layers with explicit 32-token prefetch and
a 32-page offload cap for this eligible Qwen2.5 Coder 3B Flash Attention
layout. A fresh current-binary repeat strengthened that frontier with 128 short
and 128 deep generated-token samples per iteration; the bootstrapped repeat at
`/private/tmp/qatq-live-vram-long-context-latency-bootstrap-repeat2-20260625`
then reproduced it from the clean pinned checkout. Its short p95 regressions
were `-89.2%` and `-87.8%`; short p99 regressions were `-83.9%` and `-84.3%`;
deep p95 regressions were `-30.9%` and `-31.1%`; and deep p99 regressions were
`-50.6%` and `-27.9%`.
A fresh 96-page prefetch-active rerun kept correctness and compression intact
but exposed an unacceptable deep latency tail, so larger reclaim caps remain
tuning work.
Five/six selected-layer breadth runs also passed with lower reclaim, and
broader runtime/model coverage, more aggressive reclaim policies, and longer
soaks remain open. The custom Metal
segmented K/Q/V path still costs too much for this long-context p95 gate, so it remains a fallback
and development target rather than the preferred long-context route.

After rebuilding the patched llama.cpp checkout with the QATQ graph-metadata
reserve fix, the production-relevant Coder matrix was rerun at
`/private/tmp/qatq-live-vram-coder-native-gate-status-20260624` with
`--require-live-paging`, `--gpu-page-staging`, the Metal backend, and MLX
streaming-attention gates enabled. It passed 2/2 cases:

| case | selected KV GPU layers | exact restores | offloaded QATQ pages | resident pages | pass-through | MLX layers / heads | MLX QATQ ratio | reclaimable GPU KV | QATQ | zstd | lz4 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-coder-3b-workhorse` | 30 / 36 | 288 / 288 | 216 / 216 | 72 | 0 | 36 / 576 | 0.618498 | 66.00 MiB | 44.53 MiB | 60.92 MiB | 68.68 MiB |
| `qwen25-coder-7b-powerhouse` | 14 / 28 | 224 / 224 | 168 / 168 | 56 | 0 | 28 / 784 | 0.548739 | 112.00 MiB | 61.46 MiB | 84.70 MiB | 102.82 MiB |

This is the strongest current live-VRAM evidence: real GGUF models, real Apple
Metal execution, page-staged runtime KV placement, exact deterministic
continuation, exact QATQ restore, zstd/lz4 comparison on the same page
boundaries, and MLX all-layer page-bounded streaming attention equivalence.

The same pass also deliberately stress-tested Qwen2.5 1.5B and Phi 3.5 mini.
Those smaller/model-shape cases exposed useful hardening gaps rather than
release-gate success:

- 160-token persistent page sources exceeded the current concat-composed graph
  object budget.
- Long Qwen/Phi prompts made concat-composed persistent page sources
  operationally slow; Qwen 1.5B took roughly 300 seconds for a 32k-token deep
  prompt with this diagnostic path.
- Auxiliary attention-event traces exceeded the secure 16 MiB parser cap unless
  that optional diagnostic was disabled.
- Some Qwen 1.5B pages did not beat the best general codec, and the strict
  live-paging gate correctly rejected runtime traces whose offload events did
  not match the evidence-side resident/compression decisions.

Those failures are tracked as adapter-hardening evidence. They are not included
in the default local matrix because the default matrix should reproduce the
passing live-paging proof, not a known adversarial failure set.

## 2026-06-24 Real GPU Frontier Runs

The patched llama.cpp checkout was rebuilt at commit
`7992aa7c8e21ea2eb7a5e4802da56eec7b376036`, and MLX was separately smoke
tested on `Device(gpu, 0)`. The fail-closed runner then exercised real Apple
Metal inference against local GGUF models.

For `Qwen2.5-Coder-3B-Instruct-Q4_K_M.gguf`, the runner swept 18, 24, and 30
KV GPU layers:

| KV GPU layers | status | decode us | regression | GPU saved | GPU KV MiB | tensors on GPU |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 18 | pass | 1,200,243 | 0.144 | 0.500 | 4.50 | 36 / 72 |
| 24 | pass | 1,178,153 | 0.123 | 0.333 | 6.00 | 48 / 72 |
| 30 | pass | 1,058,758 | 0.009 | 0.167 | 7.50 | 60 / 72 |

The selected frontier point was 30 KV GPU layers. The deep export verified
72/72 exact restores and stored 96,784,543 bytes versus 129,798,144 raw bytes,
118,057,317 zstd bytes, and 128,483,967 lz4 bytes. Runtime residency evidence
reported 132,120,576 total KV bytes, 110,100,480 GPU KV bytes, and 22,020,096
reclaimable GPU KV bytes.

For `Qwen2.5-Coder-7B-Instruct-Q4_K_M.gguf`, the runner swept 14, 21, and 24 KV
GPU layers:

| KV GPU layers | status | decode us | regression | GPU saved | GPU KV MiB | tensors on GPU |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 14 | pass | 2,413,443 | -0.018 | 0.500 | 7.00 | 28 / 56 |
| 21 | pass | 2,214,729 | -0.099 | 0.250 | 10.50 | 42 / 56 |
| 24 | pass | 2,056,568 | -0.164 | 0.143 | 12.00 | 48 / 56 |

The selected frontier point was 24 KV GPU layers. The deep export verified
56/56 exact restores and stored 156,318,832 bytes versus 201,908,224 raw bytes,
183,271,658 zstd bytes, and 199,064,650 lz4 bytes. Runtime residency evidence
reported 205,520,896 total KV bytes, 176,160,768 GPU KV bytes, and 29,360,128
reclaimable GPU KV bytes.

The runner was then extended with explicit prompt-profile arguments so evidence
can cover real usage shapes rather than one baked-in prompt. The gate semantics
were also tightened to the right production invariant: every offloaded page must
be stored through QATQ, pass-through pages must be zero, every page must restore
exactly, and QATQ must beat zstd/lz4 on every offloaded page. Pages that are
hot, budget-blocked, unknown, or codec-negative are kept resident; they are not
counted as compressed VRAM offload wins.

For `Qwen2.5-1.5B-Instruct-Q4_K_M.gguf`, a daily-driver handover prompt swept
14, 21, and 24 KV GPU layers:

| KV GPU layers | status | decode us | regression | GPU saved | GPU KV MiB | tensors on GPU | result |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | --- |
| 14 | fail | 1,329,453 | 0.995 | 0.500 | 3.50 | 28 / 56 | decode regression exceeded gate |
| 21 | fail | 1,030,168 | 0.546 | 0.250 | 5.25 | 42 / 56 | decode regression exceeded gate |
| 24 | pass | 769,716 | 0.155 | 0.143 | 6.00 | 48 / 56 | selected |

The selected frontier point was 24 KV GPU layers. The deep export verified
56/56 exact restores and stored 55,107,799 bytes versus 73,428,992 raw bytes,
66,561,441 zstd bytes, and 72,379,044 lz4 bytes. Runtime residency evidence
reported 80,740,352 total KV bytes, 69,206,016 GPU KV bytes, and 11,534,336
reclaimable GPU KV bytes.

For `Phi-3.5-mini-instruct-Q4_K_M.gguf`, an operations/security incident-review
prompt swept 16, 24, and 28 KV GPU layers:

| KV GPU layers | status | decode us | regression | GPU saved | GPU KV MiB | tensors on GPU | result |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | --- |
| 16 | pass | 1,786,239 | 0.325 | 0.500 | 48.00 | 32 / 64 |  |
| 24 | pass | 1,623,711 | 0.204 | 0.250 | 72.00 | 48 / 64 |  |
| 28 | pass | 1,403,393 | 0.041 | 0.125 | 84.00 | 56 / 64 | selected |

The selected frontier point was 28 KV GPU layers. The deep export verified
64/64 exact restores, stored 29/29 offloaded pages through QATQ with zero
pass-through pages, and stored 1,223,090,830 bytes versus 1,604,714,496 raw
bytes, 1,457,358,674 zstd bytes, and 1,608,067,067 lz4 bytes. Runtime residency
evidence reported 1,610,612,736 total KV bytes, 1,409,286,144 GPU KV bytes, and
201,326,592 reclaimable GPU KV bytes.

The same two prompt-profile cases were then run through
`scripts/llama_cpp_live_vram_matrix.py` as a single reproducible matrix. The
matrix wrote `/private/tmp/qatq-live-vram-matrix-20260624/summary.md` and
passed 2/2 cases:

| case | selected KV GPU layers | exact restores | offloaded QATQ pages | resident pages | pass-through | reclaimable GPU KV |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-15b-daily-driver` | 24 | 56 / 56 | 56 / 56 | 0 | 0 | 11.00 MiB |
| `phi35-ops-security` | 28 | 64 / 64 | 29 / 29 | 35 | 0 | 192.00 MiB |

A repeated soak matrix then ran the same two cases twice with
`--iterations 2`, writing
`/private/tmp/qatq-live-vram-matrix-soak-20260624/summary.md`. It passed 4/4
gated runs:

| case | runs | failures | elapsed min/max | reclaimable GPU min/max | QATQ min/max |
| --- | ---: | ---: | ---: | ---: | ---: |
| `qwen25-15b-daily-driver` | 2 | 0 | 10.73 / 11.31 s | 11.00 / 11.00 MiB | 52.55 / 52.55 MiB |
| `phi35-ops-security` | 2 | 0 | 54.08 / 56.21 s | 192.00 / 192.00 MiB | 1,166.43 / 1,166.43 MiB |

The local matrix was then expanded to four real usage profiles and run against
the patched Metal llama.cpp binary in
`/private/tmp/qatq-live-vram-matrix-4case-20260624`. It passed 4/4 gated cases:

| case | selected KV GPU layers | exact restores | offloaded QATQ pages | resident pages | pass-through | reclaimable GPU KV | QATQ | zstd | lz4 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-15b-daily-driver` | 24 | 56 / 56 | 56 / 56 | 0 | 0 | 11.00 MiB | 52.55 MiB | 63.48 MiB | 69.03 MiB |
| `phi35-ops-security` | 28 | 64 / 64 | 29 / 29 | 35 | 0 | 192.00 MiB | 1,166.43 MiB | 1,389.85 MiB | 1,533.57 MiB |
| `qwen25-coder-3b-workhorse` | 30 | 72 / 72 | 72 / 72 | 0 | 0 | 19.50 MiB | 86.41 MiB | 104.89 MiB | 114.18 MiB |
| `qwen25-coder-7b-powerhouse` | 24 | 56 / 56 | 56 / 56 | 0 | 0 | 32.00 MiB | 161.13 MiB | 190.68 MiB | 207.09 MiB |

The 3B software-engineering profile rejected the most aggressive 18/36 KV-layer
candidate because its decode regression exceeded the 50% gate, then selected
30/36 layers as the fastest passing frontier point. The 7B production code
review profile selected 24/28 layers and ran faster than the paired full-GPU KV
baseline in this local pass. All four cases preserved deterministic output
against the full-GPU baseline, beat the all-CPU-KV baseline, restored every
exported page exactly, stored every offloaded page through QATQ, and had zero
raw pass-through pages.

The same four-profile matrix was rerun fresh at
`/private/tmp/qatq-live-vram-matrix-fresh-20260624` after adding the
event-trace verifier and refreshing the checked-in llama.cpp patch. It again
passed 4/4 gated cases:

| case | selected KV GPU layers | exact restores | offloaded QATQ pages | resident pages | pass-through | reclaimable GPU KV | QATQ | zstd | lz4 | elapsed |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-15b-daily-driver` | 24 / 28 | 56 / 56 | 56 / 56 | 0 | 0 | 11.00 MiB | 52.55 MiB | 63.48 MiB | 69.03 MiB | 9.73 s |
| `phi35-ops-security` | 28 / 32 | 64 / 64 | 29 / 29 | 35 | 0 | 192.00 MiB | 1,166.43 MiB | 1,389.85 MiB | 1,533.57 MiB | 50.38 s |
| `qwen25-coder-3b-workhorse` | 30 / 36 | 72 / 72 | 72 / 72 | 0 | 0 | 19.50 MiB | 86.41 MiB | 104.89 MiB | 114.18 MiB | 17.10 s |
| `qwen25-coder-7b-powerhouse` | 24 / 28 | 56 / 56 | 56 / 56 | 0 | 0 | 32.00 MiB | 161.13 MiB | 190.68 MiB | 207.09 MiB | 35.92 s |

The patched export path was also smoke-tested with `--qatq-event-trace` on
`Qwen2.5-1.5B-Instruct-Q4_K_M.gguf`, `-ngl 999`, f16 K/V, and a 32-token
decode on Apple Metal. llama.cpp reported MTL0/Apple M4 execution, exported 56
active K/V tensors, and wrote 224 lifecycle events: 56 snapshots, 56 offload
commits, 56 restore commits, and 56 attention-use records. QATQ accepted the
trace with zero unfinished offloads. The stricter `--live-vram-live-paging-gate`
correctly rejected the same export-only adapter: the manifest reported
`whole-context` allocation granularity, reclaimable GPU bytes were zero, and
QATQ beat the best general codec on only 2/56 tiny smoke pages. That failure is
the desired safety behaviour; it prevents an export-time trace from being
mislabelled as true token-page live paging.

MLX was separately checked on the same host using the PermeantOS MLX
environment. A real local Qwen2.5 0.5B MLX generation ran on `Device(gpu, 0)`,
generating 24 tokens at 98.988 tokens/s after a 45-token prompt prefill at
290.924 tokens/s with 1.063 GB peak memory. This confirms the local MLX runtime
is operational for follow-on adapter work, but no direct MLX live page eviction
proof is claimed in this QATQ report.

The reproducible runner was then updated so the deep export emits and verifies
an event trace by default. A fresh Qwen2.5 1.5B single-frontier run at
`/private/tmp/qatq-live-vram-runner-trace-20260624` passed with
`--mixed-kv-gpu-layers 24`: output comparison passed, the mixed-KV decode stayed
within the 50% regression ceiling, all-CPU-KV remained slower, QATQ restored
56/56 pages exactly, QATQ beat zstd/lz4 on 56/56 pages, and the event trace
reported 224 events with zero unfinished offloads. The same command with
`--require-live-paging` was run at
`/private/tmp/qatq-live-vram-runner-strict-20260624` and failed closed with
`allocation granularity whole-tensor cannot prove page-level GPU reclaim`. That
is the expected result until a runtime adapter performs page-granular
attention-loop eviction and restore.

The patched llama.cpp runtime was then extended with an actual attention-path
telemetry hook in `llama_kv_cache_context::get_k/get_v`. This hook writes
`qatq-live-vram-attention-trace-v1` JSONL records when `llama-simple` is run
with `--qatq-attention-trace <path>`. A direct Metal smoke run on
`Qwen2.5-1.5B-Instruct-Q4_K_M.gguf` wrote 448 attention-read records, covering
28 layers and both key and value reads.

The full four-profile Metal matrix was rerun after wiring that attention trace
into `scripts/llama_cpp_live_vram_evidence.py`. It wrote
`/private/tmp/qatq-live-vram-matrix-attention-20260624/summary.md` and passed
4/4 gated cases:

| case | selected KV GPU layers | exact restores | offloaded QATQ pages | resident pages | pass-through | event trace | attention reads | reclaimable GPU KV | QATQ | zstd | lz4 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-15b-daily-driver` | 24 / 28 | 56 / 56 | 56 / 56 | 0 | 0 | 224 | 672 / 28 layers | 11.00 MiB | 52.55 MiB | 63.48 MiB | 69.03 MiB |
| `phi35-ops-security` | 28 / 32 | 64 / 64 | 29 / 29 | 35 | 0 | 256 | 896 / 32 layers | 192.00 MiB | 1,166.43 MiB | 1,389.85 MiB | 1,533.57 MiB |
| `qwen25-coder-3b-workhorse` | 30 / 36 | 72 / 72 | 72 / 72 | 0 | 0 | 288 | 936 / 36 layers | 19.50 MiB | 86.41 MiB | 104.89 MiB | 114.18 MiB |
| `qwen25-coder-7b-powerhouse` | 24 / 28 | 56 / 56 | 56 / 56 | 0 | 0 | 224 | 784 / 28 layers | 32.00 MiB | 161.13 MiB | 190.68 MiB | 207.09 MiB |

This new attention trace closes an important evidence gap: the run now proves
that generation reached the real llama.cpp K/V attention read path. It still
does not prove transparent token-page live VRAM reduction, because the runtime
now has only a logical page residency primitive, lacks physical per-page Metal
reclaim attestation, and still uses whole-cache tensor views for the attention
K/V source. The strict adapter audit at
`/private/tmp/qatq-live-vram-adapter-audit-attention-strict-20260624.json`
therefore fails closed while reporting that the attention-loop trace hook is
present. A matching strict matrix smoke at
`/private/tmp/qatq-live-vram-matrix-attention-strict-smoke-20260624` also fails
closed with `allocation granularity whole-tensor cannot prove page-level GPU
reclaim`.

The patched exporter was then extended with `--qatq-page-tokens <n>`, which
splits exported active K/V tensors into token-range files and records
`token_start`/`token_end` in the manifest. A Qwen2.5 1.5B Metal run with
`--page-tokens 512` produced 392 token pages and restored them, but the
runtime-reclaim gate rejected the run because QATQ beat the best general codec
on 391/392 pages. That is useful negative evidence: for this model/prompt,
512-token pages introduce enough tail-page overhead to miss the all-pages
compression gate.

The same run with `--page-tokens 1024` passed at
`/private/tmp/qatq-live-vram-page-evidence-1024-20260624`:

| metric | value |
| --- | ---: |
| exported token pages | 224 |
| exact restores | 224 / 224 |
| offloaded QATQ pages | 224 / 224 |
| pass-through pages | 0 |
| pages beating best general codec | 224 / 224 |
| event-trace lifecycle records | 896 |
| attention-path reads | 728 across 28 layers |
| raw KV pages | 96.28 MiB |
| QATQ stored | 73.09 MiB |
| zstd | 87.39 MiB |
| lz4 | 94.96 MiB |
| reclaimable GPU KV from mixed-layer placement | 14.00 MiB |

This is the strongest page-granular export evidence so far: QATQ is now tested
on real token-range K/V pages rather than only whole active tensors. The strict
live-paging control at
`/private/tmp/qatq-live-vram-page-evidence-1024-strict-20260624` still fails
closed with `allocation granularity whole-tensor cannot prove page-level GPU
reclaim`, because the runtime has not implemented a page allocator or actual
page eviction from Metal.

The page-aware replay mode was then exercised on the same Qwen2.5 1.5B Metal
profile at `/private/tmp/qatq-live-vram-page-end-hot-window-20260624` with
`--page-tokens 1024`, `--hot-window-tokens 1024`, and
`--next-required page-end`. That run selected 24/28 KV GPU layers, preserved the
32-token continuation, and reported 56 resident hot-window pages plus 168
offloaded QATQ-compressed pages:

| metric | value |
| --- | ---: |
| exact restores | 224 / 224 |
| resident pages | 56 |
| offloaded compressed pages | 168 / 168 |
| pass-through pages | 0 |
| pages beating best general codec | 224 / 224 |
| event-trace lifecycle records | 896 |
| attention-path reads | 728 across 28 layers |
| raw KV pages | 96.28 MiB |
| QATQ stored | 73.09 MiB |
| zstd | 87.39 MiB |
| lz4 | 94.96 MiB |
| reclaimable GPU KV from mixed-layer placement | 14.00 MiB |

This is the first real GPU run where QATQ's evidence report distinguishes
hot-window resident token pages from colder offloaded token pages. The strict
`--live-vram-live-paging-gate` was also run against the same capture and failed
closed with `allocation granularity whole-tensor cannot prove page-level GPU
reclaim`, `reclaimable GPU bytes is zero`, and `GPU saved ratio 0.000000 is
below required 0.100000`.

After hardening the live-paging verifier to reject event traces that offload
pages the scheduler kept resident, the same strict control failed with the more
precise operator message: `GPU saved ratio 0.000000 is below required
0.100000`; `allocation granularity whole-tensor cannot prove page-level GPU
reclaim`; `live VRAM trace offloaded a page that evidence kept resident (x56)`;
and `reclaimable GPU bytes is zero`. This is the desired fail-closed result for
the current export-time lifecycle trace: it proves the verifier will not accept
a synthetic trace that offloads hot-window resident pages.

The patched llama.cpp exporter was then updated to emit scheduler-aligned event
traces when launched with `--qatq-trace-current-token`,
`--qatq-trace-hot-window-tokens`, and `--qatq-trace-next-required`. A fresh
Metal run at `/private/tmp/qatq-live-vram-page-end-aligned-trace-20260624`
used the same 1024-token page and page-end hot-window policy. It preserved the
same continuation hash, restored 224/224 pages, kept 56 pages resident, and
offloaded 168 pages. The event trace now matches that split:

| trace metric | value |
| --- | ---: |
| total lifecycle events | 784 |
| snapshots | 224 |
| offload commits | 168 |
| restore commits | 168 |
| attention uses | 224 |
| unfinished offloads | 0 |

Replaying that aligned trace through `--live-vram-live-paging-gate` fails only
on the remaining allocator proof: `GPU saved ratio 0.000000 is below required
0.100000`; `allocation granularity whole-tensor cannot prove page-level GPU
reclaim`; and `reclaimable GPU bytes is zero`.

The page-size sweep runner was then added and run against the same model at
`/private/tmp/qatq-live-vram-page-size-sweep-20260624`:

| page tokens | status | exact restores | pages beating best codec | attention reads | QATQ | zstd | lz4 |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 512 | fail | 0 / 0 | 0 / 0 | 0 | 0.00 MiB | 0.00 MiB | 0.00 MiB |
| 1024 | pass | 224 / 224 | 224 / 224 | 728 | 73.09 MiB | 87.39 MiB | 94.96 MiB |
| 2048 | pass | 112 / 112 | 112 / 112 | 728 | 72.52 MiB | 87.35 MiB | 94.92 MiB |

The runner recommends 1024 tokens as the first experimental page size because
it is the smallest passing size in the sweep. 2048 compresses slightly better
in this run, but halves the number of independently reclaimable token pages.

MLX was also checked on the same host using the existing PermeantOS MLX
environment. A local `Qwen/Qwen2.5-0.5B-Instruct` generation ran on
`Device(gpu, 0)`, produced 24 tokens at 101.004 tokens/s after a 15-token
prefill at 313.754 tokens/s, and reported 1.025 GB peak memory. This confirms
the local MLX runtime is operational for follow-on adapter work, but no direct
MLX KV page eviction proof is claimed here.

## Runtime VRAM Control Baseline

The patched `llama-simple` runner also exposes `--memory-breakdown` and
`--no-kv-offload` so the QATQ adapter work can compare a real GPU-KV baseline
against llama.cpp's native CPU-resident KV mode.

The control run used the same Phi 3.5 mini prompt shape as the largest exported
KV test, with `-ngl 99`, `-n 1`, f16 K/V cache, and 3,072 prompt tokens.

| mode | Metal model | Metal KV context | Metal compute | Metal self total | Host context | prompt eval |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| GPU KV, default `offload_kqv=true` | 2,228 MiB | 1,152 MiB | 80 MiB | 3,461 MiB | 0 MiB | 8,082.30 ms / 3,072 tokens |
| CPU KV, `--no-kv-offload` | 2,228 MiB | 0 MiB | 99 MiB | 2,328 MiB | 1,152 MiB | 8,597.08 ms / 3,072 tokens |

This is not QATQ live paging. It is an important native-runtime control:
llama.cpp can remove the 1,152 MiB KV context from Metal only when KV is placed
on CPU from the start. Current llama.cpp KV tensors are allocated as whole
backend buffers, so marking individual cells cold or exporting slices does not
release Metal memory. A QATQ live adapter therefore needs an allocator-aware
runtime path that can replace or bypass those GPU-resident KV buffers for cold
pages.

On 2026-06-23, the patched runner was exercised again on the same Apple M4 host
with `Phi-3.5-mini-instruct-Q4_K_M.gguf`, `-ngl 999`, f16 K/V cache, a
50-token engineering prompt, and 32 generated tokens. The GPU-KV run reported
2,228 MiB Metal model memory, 96 MiB Metal KV context, 38 MiB Metal compute
memory, 112.21 prompt tokens/s, and 22.52 decode tokens/s. The matching
`--no-kv-offload` baseline moved the 96 MiB KV buffer to host memory and
reduced Metal self memory from 2,363 MiB to 2,269 MiB, but decode throughput
fell to 13.45 tokens/s.

The fresh export was replayed through QATQ with allocator and restore-deadline
evidence:

```sh
cargo run --bin qatq-kv-bench -- \
  --live-vram-export-dir /private/tmp/qatq-live-vram-phi35-fresh-20260623-escalated \
  --live-vram-runtime-commit 7992aa7c8e21ea2eb7a5e4802da56eec7b376036 \
  --live-vram-adapter-version qatq-kv-export-7992aa7c8 \
  --live-vram-model-id phi-3.5-mini-instruct-q4_k_m.gguf \
  --live-vram-current-token 50 \
  --live-vram-hot-window-tokens 0 \
  --live-vram-gpu-context-bytes 100663296 \
  --live-vram-allocation-granularity whole-context \
  --live-vram-restore-bytes-per-token 67108864 \
  --output /private/tmp/qatq-live-vram-phi35-fresh-20260623-evidence.json
```

That report recorded 64/64 exact restores, 16,909,894 QATQ bytes versus
18,196,677 zstd bytes and 19,719,821 lz4 bytes, 64/64 pages beating the best
general codec, zero restore-deadline misses, 19,660,800 logical offloaded raw
bytes, and zero reclaimable GPU bytes under the conservative `whole-context`
allocator model.

The maintained patch was then rebuilt and rerun with runtime allocator fields in
the manifest. The attested manifest recorded `gpu_allocation_granularity:
"whole-context"` and `gpu_context_bytes: 100663296`. QATQ replay verified all
64 pages, stored 13,998,795 bytes versus 16,121,856 raw bytes, 14,917,705 zstd
bytes, and 16,154,710 lz4 bytes, and reported zero restore-deadline misses.
Running `--live-vram-proof-gate` while manually passing
`--live-vram-allocation-granularity per-page` still failed from the
runtime-attested manifest values:

```text
allocation granularity whole-context cannot prove page-level GPU reclaim;
reclaimable GPU bytes is zero;
GPU saved ratio 0.000000 is below required 0.100000
```

The same path was rerun after adding the measured-reclaim controller check. The
patched llama.cpp runner was rebuilt from the pinned `7992aa7c8` checkout, ran
on Apple M4 Metal with `Phi-3.5-mini-instruct-Q4_K_M.gguf`, `-ngl 999`, f16
K/V cache, `--memory-breakdown`, and exported tensors to
`/private/tmp/qatq-live-vram-gpu-real-20260623-2350`. llama.cpp reported a
96.00 MiB Metal KV buffer and wrote 64 active K/V tensors with 51 active cells.
QATQ replay verified 64/64 exact restores and stored 17,204,692 bytes versus
20,054,016 raw bytes, 18,554,220 zstd bytes, and 20,102,295 lz4 bytes. The
strict proof gate again rejected the run because the runtime-attested allocator
granularity remained `whole-context`, leaving zero reclaimable GPU bytes.

The native llama.cpp control was rerun with `--no-kv-offload`. That moved the
same 96.00 MiB KV buffer from Metal to host memory, reducing Metal context
memory to zero for KV, but it is still an all-or-nothing runtime placement mode,
not QATQ-managed live page eviction.

The maintained patch now also exposes a mixed KV placement control:
`llama-simple --qatq-kv-gpu-layers <n>`. On the same Apple M4 Phi 3.5 mini
smoke workload, `--qatq-kv-gpu-layers 16` kept KV layers 0 through 15 on Metal
and placed layers 16 through 31 on host memory. llama.cpp reported a 48.00 MiB
Metal KV buffer and a 48.00 MiB host KV buffer, compared with 96.00 MiB Metal KV
for the default run. Prompt evaluation completed at 238.02 tokens/s and decode
completed at 23.44 tokens/s. The exported manifest attested:
`gpu_allocation_granularity: "whole-tensor"`, `gpu_context_bytes: 50331648`,
`total_context_bytes: 100663296`, `gpu_resident_tensors: 32`, and
`total_tensors: 64`.

QATQ replay over that mixed-placement export verified 64/64 exact restores and
stored 17,204,692 bytes versus 20,054,016 raw bytes, 18,554,220 zstd bytes, and
20,102,295 lz4 bytes. The runtime reclaim gate passes for this class of
evidence because it uses the runtime-attested `total_context_bytes` and
`gpu_context_bytes` fields and does not require token-page reclaim. The strict
page-level proof gate still failed, as it should:

The gated report recorded:

| field | value |
| --- | ---: |
| allocation granularity | `whole-tensor` |
| GPU context before | 100,663,296 bytes |
| GPU context after | 50,331,648 bytes |
| reclaimable GPU bytes | 50,331,648 bytes |
| exact restores | 64 / 64 |
| QATQ pages beating best general codec | 64 / 64 |
| restore-deadline misses | 0 |

```text
allocation granularity whole-tensor cannot prove page-level GPU reclaim;
reclaimable GPU bytes is zero;
GPU saved ratio 0.000000 is below required 0.100000
```

This mixed placement is the first runtime allocator step toward live VRAM
reduction: it proves llama.cpp can continue generation with lower Metal KV
allocation at layer granularity. It is not the final QATQ live pager because the
KV placement is chosen during context construction rather than by evicting and
restoring cold token pages during generation.

On the latest deterministic Apple M4 check, the patched runner was rebuilt with
`--qatq-output-manifest`. The same Phi 3.5 mini model, prompt, greedy sampler,
f16 K/V cache, `-ngl 999`, and 32-token continuation were run twice:

- default full-GPU KV placement;
- mixed placement with `--qatq-kv-gpu-layers 16`.

The default run reported a 96.00 MiB Metal KV buffer, wrote
`/private/tmp/qatq-output-default.json`, and decoded 32 tokens in 1.75 seconds
at 18.31 tokens/s. The mixed run reported a 48.00 MiB Metal KV buffer plus a
48.00 MiB host KV buffer, wrote `/private/tmp/qatq-output-mixed.json`, and
decoded 32 tokens in 2.78 seconds at 11.51 tokens/s. The generated token arrays
were identical and the generated text hashes matched:
`10514066536455701873`.

The behaviour check is now automatable:

```sh
cargo run --bin qatq-kv-bench -- \
  --compare-output-baseline /private/tmp/qatq-output-default.json \
  --compare-output-candidate /private/tmp/qatq-output-mixed.json \
  --compare-output-gate \
  --output /private/tmp/qatq-output-comparison.json
```

QATQ replay over the mixed export passed the runtime reclaim gate:

| field | value |
| --- | ---: |
| allocation granularity | `whole-tensor` |
| GPU context before | 100,663,296 bytes |
| GPU context after | 50,331,648 bytes |
| reclaimable GPU bytes | 50,331,648 bytes |
| exact restores | 64 / 64 |
| QATQ pages beating zstd | 64 / 64 |
| QATQ pages beating lz4 | 64 / 64 |
| QATQ pages beating best general codec | 64 / 64 |
| restore-deadline misses | 0 |

The same run intentionally fails the stricter page-level proof gate:

```text
allocation granularity whole-tensor cannot prove page-level GPU reclaim;
reclaimable GPU bytes is zero;
GPU saved ratio 0.000000 is below required 0.450000
```

That negative control is important. It prevents a coarse layer-placement win
from being misreported as live token-page eviction.

On a fresh Qwen2.5 Coder 3B Apple M4 run, the patched runner was exercised with
the local `Qwen2.5-Coder-3B-Instruct-Q4_K_M.gguf` model, f16 K/V cache,
`-ngl 99`, and the maintained `--qatq-kv-gpu-layers 18` placement control.
The deterministic 32-token behaviour check first paired full-GPU KV with
mixed-placement KV on the same prompt. The output comparison gate passed:
both runs generated 32 tokens and the generated text hashes matched
`5861684910135804625`. The full-GPU run used a 9.00 MiB Metal KV buffer and
decoded at about 29.92 tokens/s; the mixed run split the same 9.00 MiB KV
context into 4.50 MiB Metal KV plus 4.50 MiB host KV and decoded at about
19.50 tokens/s.

The deep prefill run used 3,521 prompt tokens and emitted runtime allocator
metadata with `gpu_allocation_granularity: "whole-tensor"`,
`gpu_context_bytes: 66060288`, `total_context_bytes: 132120576`,
`gpu_resident_tensors: 36`, and `total_tensors: 72`. QATQ replay over the
mixed export passed `--live-vram-runtime-reclaim-gate`: it verified 72/72
restores, stored 96,784,543 bytes versus 129,798,144 raw bytes, 118,057,317
zstd bytes, and 128,483,967 lz4 bytes, and beat the best general codec on
72/72 pages. The evidence file for the local run was written to
`/private/tmp/qatq-real-gpu-20260623-fresh/deep-reduced/runtime-reclaim-evidence.json`.

This is still a layer-granularity allocator result. It proves real lower Metal
KV residency, exact QATQ replay, and deterministic output preservation for the
paired short continuation. It does not prove transparent live token-page
eviction and restore inside the attention loop.

The same runner was then executed with its native CPU-KV baseline gate enabled.
On the Qwen2.5 Coder 3B short continuation, it recorded full-GPU decode time
`1,276,485` microseconds, mixed-KV decode time `1,702,291` microseconds, and
all-CPU-KV decode time `1,750,491` microseconds. The mixed-KV run preserved the
same generated text hash, stayed below the configured 50% full-GPU regression
ceiling, and remained faster than all-CPU-KV while cutting runtime-attested GPU
KV context bytes from 126.00 MiB to 63.00 MiB on the deep export.

Running the same evidence through `qatq-kv-bench --live-vram-proof-gate` should
fail. This is intentional: exported KV replay validates codec correctness and
restore feasibility, but the strict proof gate is reserved for a runtime adapter
that can report `per-page` allocation granularity and non-zero reclaimable GPU
bytes. Current maintained llama.cpp export manifests attest
`gpu_allocation_granularity: "whole-context"` and `gpu_context_bytes`; older
manifests without these allocator fields are rejected by proof-gate mode before
any manual CLI override can be treated as proof.

The same Phi export was replayed with allocator-aware evidence:

```sh
cargo run --bin qatq-kv-bench -- \
  --live-vram-export-dir /private/tmp/qatq-live-vram-phi35-long \
  --live-vram-runtime-commit 7992aa7c8e21ea2eb7a5e4802da56eec7b376036 \
  --live-vram-adapter-version qatq-kv-export-7992aa7c8 \
  --live-vram-model-id phi-3.5-mini-instruct-q4_k_m.gguf \
  --live-vram-current-token 3072 \
  --live-vram-hot-window-tokens 0 \
  --live-vram-gpu-context-bytes 1207959552 \
  --live-vram-allocation-granularity whole-context \
  --live-vram-restore-bytes-per-token 67108864 \
  --output /private/tmp/qatq-live-vram-phi35-long-evidence-with-runtime-estimates.json
```

With the default 512 MiB CPU storage budget, the report verified all 64 pages,
logically offloaded 38 pages, and recorded this conservative residency estimate:

| field | value |
| --- | ---: |
| GPU context bytes before | 1,207,959,552 |
| logical offloaded raw bytes | 717,225,984 |
| stored CPU bytes | 534,165,022 |
| reclaimable GPU bytes | 0 |
| GPU context bytes after | 1,207,959,552 |

That zero is intentional: `whole-context` means the runtime allocation cannot
release individual logical pages. A future allocator-aware adapter may use
`per-page` only after it proves real GPU page reclaim.

The `--live-vram-restore-bytes-per-token` value adds a
`restore_deadline_report` block. It estimates whether compressed pages can be
restored before their next token deadline under a measured runtime restore
budget. It does not by itself prove that llama.cpp has freed and later
repopulated Metal KV pages.

## Reproduction Sketch

Build the patched llama.cpp checkout:

```sh
git clone https://github.com/ggml-org/llama.cpp.git /tmp/qatq-llama.cpp
cd /tmp/qatq-llama.cpp
git checkout 7992aa7c8e21ea2eb7a5e4802da56eec7b376036
git apply /path/to/qatq/adapters/llama-cpp/qatq-kv-export-7992aa7c8.patch
cmake -S . -B build-qatq -DCMAKE_BUILD_TYPE=Release -DGGML_METAL=ON
cmake --build build-qatq --target llama-simple
```

Run a Metal-backed export:

```sh
/tmp/qatq-llama.cpp/build-qatq/bin/llama-simple \
  -m /path/to/model.gguf \
  -ngl 99 \
  -n 8 \
  --cache-type-k f16 \
  --cache-type-v f16 \
  --qatq-kv-export-dir /tmp/qatq-live-vram-export \
  "<long deterministic prompt>"
```

Print native runtime memory baselines:

```sh
# GPU-resident KV baseline
/tmp/qatq-llama.cpp/build-qatq/bin/llama-simple \
  -m /path/to/model.gguf \
  -ngl 99 \
  -n 1 \
  --memory-breakdown \
  --cache-type-k f16 \
  --cache-type-v f16 \
  --qatq-output-manifest /tmp/qatq-output-default.json \
  "<long deterministic prompt>"

# CPU-resident KV control
/tmp/qatq-llama.cpp/build-qatq/bin/llama-simple \
  -m /path/to/model.gguf \
  -ngl 99 \
  -n 1 \
  --memory-breakdown \
  --no-kv-offload \
  --cache-type-k f16 \
  --cache-type-v f16 \
  --qatq-output-manifest /tmp/qatq-output-cpu-kv.json \
  "<long deterministic prompt>"

# Mixed GPU/host KV placement control
/tmp/qatq-llama.cpp/build-qatq/bin/llama-simple \
  -m /path/to/model.gguf \
  -ngl 99 \
  -n 32 \
  --memory-breakdown \
  --qatq-kv-gpu-layers 16 \
  --cache-type-k f16 \
  --cache-type-v f16 \
  --qatq-kv-export-dir /tmp/qatq-live-vram-mixed \
  --qatq-output-manifest /tmp/qatq-output-mixed.json \
  "<long deterministic prompt>"
```

Replay the exported manifest through QATQ:

```sh
cargo run --bin qatq-kv-bench -- \
  --live-vram-export-dir /tmp/qatq-live-vram-export \
  --live-vram-runtime-commit 7992aa7c8e21ea2eb7a5e4802da56eec7b376036 \
  --live-vram-adapter-version qatq-kv-export-7992aa7c8 \
  --live-vram-model-id <model-id> \
  --live-vram-current-token <prompt-token-count> \
  --live-vram-hot-window-tokens 0 \
  --live-vram-restore-bytes-per-token <measured-restore-budget> \
  --output /tmp/qatq-live-vram-evidence.json
```

## 2026-06-24 Backend Page Self-Test

The patched llama.cpp runner was then extended with
`--qatq-live-page-self-test <n>`. This hook runs after generation and performs
a real backend KV tensor operation: snapshot one active key page, overwrite the
page with zeroes through `ggml_backend_tensor_set`, verify the mutation,
restore the original bytes, and verify the restored checksum.

A direct Metal smoke on `Qwen2.5-1.5B-Instruct-Q4_K_M.gguf` passed with
24 KV layers on Metal, f16 K/V cache, `--qatq-page-tokens 1024`, and
`--qatq-live-page-self-test 1024`. llama.cpp reported Apple M4/MTL0 execution,
exported QATQ KV tensors, wrote the event trace, and logged:
`QATQ live page self-test restored layer 0 stream 0 key page with 25 tokens
(12800 bytes)`.

The same hook was then exercised through the reproducible evidence runner at
`/private/tmp/qatq-live-vram-self-test-runner-20260624`:

| metric | value |
| --- | ---: |
| selected KV GPU layers | 24 / 28 |
| full-GPU decode | 411,258 us |
| mixed-KV decode | 447,948 us |
| all-CPU-KV decode | 701,155 us |
| exact restores | 224 / 224 |
| resident pages | 56 |
| offloaded compressed pages | 168 / 168 |
| pass-through pages | 0 |
| pages beating best general codec | 224 / 224 |
| event-trace lifecycle records | 784 |
| attention-path reads | 728 across 28 layers |
| raw KV pages | 96.28 MiB |
| QATQ stored | 73.09 MiB |
| zstd | 87.39 MiB |
| lz4 | 94.96 MiB |
| reclaimable GPU KV from mixed-layer placement | 14.00 MiB |

MLX was also sanity-checked in the existing PermeantOS environment with real GPU
compute on `Device(gpu, 0)`: a 1024x1024 float16 matmul with float32
accumulated sum returned `16777216.0`.

This is stronger than trace-only evidence because the adapter now proves
runtime tensor mutation and byte-identical restore mechanics. It is still not a
transparent live VRAM reduction claim: the test restores the page in-place and
does not free per-page Metal allocation.

## 2026-06-24 Real Q/K/V Attention Equivalence

The patched llama.cpp runner was then extended with
`--qatq-attention-fixture-dir <dir>`. This hook captures a real computed query
vector from llama.cpp's graph during a Metal-backed run, while the normal QATQ
KV exporter writes the matching K/V token pages. QATQ then slices the selected
layer/head out of those real K/V pages and runs
`qatq-kv-bench --attention-equivalence-gate`.

A Qwen2.5 1.5B Apple Metal run at
`/private/tmp/qatq-real-attention-fixture-pages-20260624` used
`--qatq-page-tokens 4`, f16 K/V cache, and the real local patched
`llama-simple` binary. The attention fixture gate passed:

| metric | value |
| --- | ---: |
| query dtype | f32 |
| K/V page dtype | f16 |
| pages | 6 |
| tokens | 23 |
| head dim | 128 |
| max absolute error | 0.000000000 |
| max relative error | 0.000000000 |
| peak page KV values | 1,024 |
| materialised KV values | 5,888 |
| peak page/materialised KV ratio | 0.173913043 |

This proves that QATQ's page-bounded attention reference can reproduce
materialised attention over real llama.cpp Q/K/V artefacts while only holding
one page's K/V working set at a time. It does not prove token-page GPU reclaim:
llama.cpp still has to consume QATQ-backed pages inside a native paged
attention/allocator path before transparent live VRAM reduction can be claimed.

The same check is now part of the reproducible evidence runner. A fresh
Qwen2.5 1.5B Apple Metal run at
`/private/tmp/qatq-live-vram-integrated-attention-20260624-rerun` passed the
runtime-reclaim gate with `--page-tokens 1024`, selected 24/28 KV GPU layers,
preserved the 8-token deterministic continuation, restored 224/224 pages
exactly, stored 224/224 offloaded pages through QATQ with zero pass-through
pages, and beat zstd/lz4 on 224/224 page boundaries. The same deep export
captured 728 actual attention-path K/V reads across 28 layers and passed the
real Q/K/V attention-equivalence gate over four f16 K/V pages and 3,521 tokens:
zero max absolute error, zero max relative error, and a
peak page/materialised KV ratio of 0.290826470. Coarse mixed-layer placement
reduced Metal KV allocation from 98.00 MiB to 84.00 MiB; strict token-page live
VRAM reduction remains unclaimed.

The latest local GPU evidence pass was then rerun after adding per-`llama_decode`
timing capture:
`/private/tmp/qatq-live-vram-gpu-real-20260624`. This run used the same pinned
llama.cpp commit, the local
`Qwen2.5-1.5B-Instruct-Q4_K_M.gguf` model, `--page-tokens 1024`, and selected
24/28 KV GPU layers. It preserved the deterministic continuation, restored
224/224 pages exactly, routed 224/224 offloaded pages through QATQ with zero
pass-through pages, beat zstd/lz4 on 224/224 page boundaries, passed the
attention-path lifecycle verifier, and passed the real Q/K/V
attention-equivalence gate over four f16 K/V pages and 3,521 tokens with zero
error. The measured coarse Metal KV allocation again moved from 98.00 MiB to
84.00 MiB, while QATQ stored 73.09 MiB versus 96.28 MiB raw, 87.39 MiB zstd,
and 94.96 MiB lz4. The aggregate `tokens.csv` for that run includes four
run-level summary rows and 25 per-decode timing rows from the patched
`llama-simple --qatq-token-timings` hook.

## 2026-06-24 Attention-Path Lifecycle Trace

The patched llama.cpp runner now also supports
`--qatq-attention-event-trace <path>`. Unlike the export-time lifecycle trace,
this append-only JSONL stream is emitted from the actual
`llama_kv_cache_context::get_k/get_v` attention read path. QATQ validates it
with `qatq-kv-bench --live-vram-event-trace-only
--live-vram-event-trace-gate`.

A fresh Qwen2.5 1.5B Apple Metal run at
`/private/tmp/qatq-live-vram-attention-events-20260624` passed the integrated
runner with `--page-tokens 1024`:

| metric | value |
| --- | ---: |
| selected KV GPU layers | 24 / 28 |
| deterministic continuation | pass |
| exact restores | 224 / 224 |
| QATQ pages beating zstd/lz4 | 224 / 224 |
| pass-through pages | 0 |
| raw KV pages | 96.28 MiB |
| QATQ stored | 73.09 MiB |
| zstd | 87.39 MiB |
| lz4 | 94.96 MiB |
| coarse Metal KV before | 98.00 MiB |
| coarse Metal KV after | 84.00 MiB |
| attention-read telemetry events | 728 |
| attention-path lifecycle events | 8,960 |
| lifecycle snapshots | 2,240 |
| lifecycle offload commits | 2,240 |
| lifecycle restore commits | 2,240 |
| lifecycle attention uses | 2,240 |
| lifecycle unfinished offloads | 0 |
| attention equivalence | pass, 4 pages, 3,521 tokens, zero error |

This is the strongest ordering proof so far: the lifecycle events are emitted
at the attention-read hook and QATQ verifies snapshot, offload, restore,
attention-use ordering with matching restore checksums. It is still not the
final live-VRAM feature because the event hook and logical residency table do
not yet free per-page Metal allocation or replace llama.cpp's whole-cache
attention view with a native paged attention consumer.

## 2026-06-24 Logical Page Residency Primitive

The patched llama.cpp adapter now exposes experimental `qatq_live_evict_page`,
`qatq_live_restore_page`, and `qatq_live_page_resident` methods. The existing
`--qatq-live-page-self-test <n>` path now uses those methods instead of a
one-off zero/restore block. A real Apple Metal smoke on
`Qwen2.5-1.5B-Instruct-Q4_K_M.gguf` passed with `--qatq-live-page-self-test 16`:
the runtime evicted and restored layer 0, stream 0, key page rows for 16 active
tokens, 8,192 bytes, and verified the restored checksum.

The same primitive then passed inside the normal integrated evidence runner at
`/private/tmp/qatq-live-vram-logical-residency-20260624`. That run preserved the
deterministic continuation, restored 224/224 exported pages exactly, beat
zstd/lz4 on 224/224 page boundaries, passed the attention lifecycle and
attention-equivalence gates, and executed the live page self-test after the deep
run. The self-test log recorded layer 0, stream 0, key page restore for 16
active tokens, 8,192 bytes. The exported manifest now records
`live_page_residency_granularity: "per-page"` alongside
`gpu_allocation_granularity: "whole-tensor"`.

This moves the adapter from ad hoc backend mutation to an adapter-visible
page-keyed residency table. It is still logical residency over llama.cpp's
persistent KV buffer, not physical page-granular Metal allocation reclaim.

A subsequent Qwen2.5 1.5B Apple Metal run at
`/private/tmp/qatq-live-vram-real-gpu-20260624-gated` revalidated the current
path after the QATQ CLI began parsing `live_page_residency_granularity`. The run
passed deterministic continuation, exact restore, compression, attention
equivalence, attention lifecycle, and the logical page self-test. Replaying the
same export through `--live-vram-live-paging-gate` then failed closed because
the manifest still reported `gpu_allocation_granularity: "whole-tensor"`:
logical page residency exists, but physical page-granular GPU reclaim is not
yet proven.

The adapter then added `--qatq-live-physical-page-alloc-self-test <n>`, which
allocates a page-sized non-host backend tensor on the same backend as an active
key page, round-trips real KV bytes through that tensor, and frees it. The
integrated Qwen2.5 1.5B Apple Metal evidence run at
`/private/tmp/qatq-live-vram-real-gpu-20260624-physical-page-tensor` passed this
stronger primitive while also preserving deterministic continuation, exact
restore, compression, attention equivalence, and attention lifecycle evidence.
The deep-run log recorded: `QATQ physical page tensor self-test round-tripped
and freed MTL0 page tensor for layer 0 stream 0 key page with 16 tokens (8192
requested bytes, 8192 allocated bytes)`.

That is real allocator evidence, not a finished live-paging claim. The
remaining gap is still that llama.cpp attention reads whole-cache K/V views and
persistent KV is still allocated as backend buffers rather than sustained
page-resident allocation units.

The latest adapter step moved the page-tensor proof into the actual
`get_k/get_v` attention path with
`--qatq-attention-page-tensor-self-test <path>`. The integrated Apple Metal
Qwen2.5 1.5B evidence run at
`/private/tmp/qatq-live-vram-real-gpu-20260624-attention-page-tensor`
materialised real attention-path key/value pages into temporary `MTL0` page
tensors, round-tripped the bytes exactly, and freed those tensors. The JSONL
evidence recorded 16 attention-path page tensor events, 7,340,032 requested
bytes, and 7,340,032 allocated bytes. The same run preserved deterministic
continuation, restored 224/224 exported pages exactly, beat zstd/lz4 on 224/224
page boundaries, passed attention equivalence with zero error, passed 8,960
attention-path lifecycle events, and executed the logical plus physical page
self-tests. The integrated evidence runner now requests and validates that
JSONL by default, rejecting missing, malformed, or CPU-backed events. This
closes another proof gap in the adapter mechanics, but not the final one: the
attention graph still needs to consume page tensors, not merely prove they can
be created from attention-path bytes.

The next bounded adapter step added
`--qatq-attention-materialized-source-trace <path>`, which makes `get_k/get_v`
wrap the K/V attention source in `ggml_cont` before returning it to the
llama.cpp attention graph. A paired Apple Metal smoke at
`/private/tmp/qatq-materialized-source-smoke` compared native full-GPU attention
against the materialised-source path on the same Qwen2.5 1.5B model, prompt,
greedy continuation, f16 K/V cache, and 8-token decode. QATQ's output comparison
gate passed with identical generated text hash `8821614236871069616`, and the
materialised-source trace recorded 448 key/value source events across 28
layers.

The same gate is now part of the integrated evidence runner. The run at
`/private/tmp/qatq-live-vram-real-gpu-20260624-materialized-source-integrated`
preserved the native/materialised generated text hash
`16975742283144246935`, recorded 448 short-run and 728 deep-run
materialised-source events, restored 224/224 exported pages exactly, beat
zstd/lz4 on 224/224 page boundaries, passed attention equivalence with zero
error, and passed the `MTL0` attention-path page tensor self-test. The short
integrated comparison made the materialised-source cost visible: native decode
was 137,005 us and materialised-source decode was 145,583 us for the same 8
generated tokens. This proves the graph can consume a materialised K/V source
without output drift. It does not yet save VRAM because the materialised source
is still copied from the persistent KV cache rather than replacing it with
page-resident QATQ-backed tensors.

The adapter then moved to a page-composed source proof with
`--qatq-attention-page-composed-source-trace <path>`. This splits the K/V
attention source into bounded token pages, materialises each page, composes the
pages with `ggml_concat`, and feeds the resulting tensor into the existing
llama.cpp attention graph. A standalone Apple Metal smoke at
`/private/tmp/qatq-page-composed-source-smoke` passed with 448 trace events,
64-token pages, four pages per early K/V source, and identical generated output
versus the native source path. The Metal log compiled and used
`kernel_concat`, proving page composition was exercised on the backend.

The same gate is now part of the integrated evidence runner. The run at
`/private/tmp/qatq-live-vram-real-gpu-20260624-page-composed-source-integrated`
recorded 448 short-run page-composed events and 728 deep-run page-composed
events. The deep run required multi-page composition and reached a max page
count of 4 with page-token values 512 and 1024. The native and page-composed
generated text hash matched exactly at `16975742283144246935`; native short
decode was 134,755 us and page-composed short decode was 145,799 us. The same
bundle restored 224/224 pages exactly, beat zstd/lz4 on 224/224 page
boundaries, passed attention equivalence with zero error, passed 8,960
attention lifecycle events, and passed the `MTL0` page tensor self-test. This
is the strongest attention-consumption evidence so far. It still does not
claim live VRAM reduction because the composed pages are currently sourced from
the persistent KV allocation.

The adapter then added a persistent backend page-source attention path with
`--qatq-attention-persistent-page-source-trace <path>`. This path allocates
retained page tensors on the same non-host backend, uses graph-native
`ggml_cpy` operations to fill each retained page tensor during execution, and
returns a `ggml_concat` composition of those retained page tensors to the
llama.cpp attention graph. A standalone Apple Metal smoke at
`/private/tmp/qatq-persistent-page-source-smoke` passed with 112 persistent
page-source events, backend `MTL0`, 64-token pages, four pages per early K/V
source, and identical generated output versus the native full-GPU attention
source. This is stronger than the page-composed source proof because attention
consumes independently allocated backend page tensors. It still does not claim
live VRAM reduction because those page tensors are currently filled from the
default persistent KV cache, so the whole-layer KV allocation remains resident.

The same hook is now part of the integrated evidence runner. The run at
`/private/tmp/qatq-live-vram-real-gpu-20260624-persistent-page-source-integrated`
recorded 112 short-run persistent page-source events and 192 deep-run
persistent page-source events on backend `MTL0`. The deep persistent source
path reached a max page count of 2 with page-token values 512 and 1024, retained
up to 288 backend page tensors, and preserved the native generated text hash
exactly at `13136292135900150276`. Native short decode was 109,878 us and
persistent-source short decode was 147,561 us. The same bundle restored 112/112
exported pages exactly, stored 112/112 offloaded pages through QATQ with zero
pass-through pages, beat zstd/lz4 on 112/112 page boundaries, passed 4,032
attention lifecycle events, passed attention equivalence over 1,761 tokens with
zero max absolute/relative error, and retained 16 verified persistent page-pool
buffers. Runtime-reclaim evidence reported 49.00 MiB GPU KV before, 42.00 MiB
after, and 7.00 MiB reclaimable GPU KV at whole-tensor/layer granularity. The
strict `--live-vram-live-paging-gate` rejected the same capture with
`allocation granularity whole-tensor cannot prove page-level GPU reclaim`, so
the report remains honest about the remaining allocator gap.

The follow-up allocator experiment added `--qatq-gpu-page-staging`, which keeps
canonical KV tensors off GPU and stages attention pages into retained `MTL0`
page tensors through the persistent page-source path. A paired smoke at
`/private/tmp/qatq-gpu-page-staging-smoke` preserved generated output versus
the native full-GPU baseline. The same smoke failed the strict live-paging gate
for the correct reason: `gpu_page_staging_bytes` equalled the full
`total_context_bytes` (`7340032 >= 7340032`). QATQ now rejects that condition so
a page-staging graph that materialises every page at once cannot be treated as
real live VRAM reduction.

The attention-equivalence gate was then tightened with a bounded-residency
policy: `--attention-max-peak-page-kv-ratio`. Replaying the real llama.cpp
Q/K/V fixture from
`/private/tmp/qatq-live-vram-real-gpu-20260624-persistent-page-source-integrated`
with a `0.75` threshold passed at peak page/materialised KV ratio
`0.581487791` and zero max absolute/relative error. This is the acceptance test
the runtime graph must satisfy when the streaming softmax recurrence is moved
from QATQ's reference implementation into llama.cpp execution.

The adapter then added a retained backend page-pool proof with
`--qatq-live-persistent-page-pool-self-test <n>` and
`--qatq-live-persistent-page-pool-trace <path>`. This allocates, exact-byte
verifies, and retains a bounded pool of independent non-host K/V page tensors
until `llama_context` teardown. A standalone Apple Metal smoke at
`/private/tmp/qatq-persistent-page-pool-smoke` retained 8 verified `MTL0` page
buffers. The integrated evidence run at
`/private/tmp/qatq-live-vram-real-gpu-20260624-persistent-page-pool-integrated`
required 16 retained page buffers and passed with backend `MTL0`, key and
value pages, 8,388,608 requested bytes, and 8,388,608 allocated bytes. The same
run preserved deterministic output, restored 112/112 exported pages exactly,
stored 112/112 offloaded pages through QATQ, beat zstd/lz4 on 112/112 page
boundaries, passed attention lifecycle verification, passed page-composed
source verification, and passed attention equivalence over 1,761 tokens with
zero max absolute/relative error. Replaying that capture through the strict
`--live-vram-live-paging-gate` still fails with `allocation granularity
whole-tensor cannot prove page-level GPU reclaim`, which is correct: retained
page-pool storage is real, but it still coexists with the default whole-layer
KV allocation.

The evidence runner now writes two replayable CSV artefacts in each work
directory. `pages.csv` records page-level runtime metadata, schedule decisions,
storage strategy, raw/QATQ/zstd/lz4 sizes, and restore verification. `tokens.csv`
records run-level proof summaries plus one row per `llama_decode` call from
the patched llama.cpp runner when `--qatq-token-timings` is enabled. These rows
cover prompt-prefill and generated-token decode steps. They are real runtime
timings, but they still are not final native live-paging latency attribution:
future page-granular allocator work must add restore-stall and prefetch-window
timing before QATQ can publish policy-grade p50/p95/p99 live paging numbers.

## 2026-06-24 Fresh Local Metal And MLX Verification

The patched llama.cpp checkout was verified at commit
`7992aa7c8e21ea2eb7a5e4802da56eec7b376036`, with the checked-out adapter patch
applied to `examples/simple`, `include/llama.h`, and the llama KV/cache context
sources. The built `llama-simple` binary links `libggml-metal.0.9.11.dylib`,
and the PermeantOS MLX environment was smoke-tested with a real 2048 by 2048
GPU matrix multiply on `Device(gpu, 0)`.

A heavy Qwen2.5 Coder 3B run at
`/private/tmp/qatq-live-vram-real-gpu-20260624-coder3b-heavy` exercised the
full evidence path with physical page allocation, persistent page-pool, page
tensor, persistent page-source, attention lifecycle, and attention-equivalence
checks enabled:

| metric | result |
| --- | ---: |
| selected KV GPU layers | 32 / 36 |
| full-GPU decode | 423,672 us |
| mixed-KV decode | 432,673 us |
| all-CPU-KV decode | 641,443 us |
| exact restores | 288 / 288 |
| QATQ pages beating best general codec | 288 / 288 |
| raw KV pages | 129,798,144 bytes |
| QATQ stored | 97,748,161 bytes |
| zstd | 118,137,534 bytes |
| lz4 | 128,546,942 bytes |
| GPU KV before / after | 126.00 MiB / 112.00 MiB |
| reclaimable coarse GPU KV | 14.00 MiB |
| attention peak page/materialised ratio | 0.290826470 |

A three-model Metal matrix at
`/private/tmp/qatq-live-vram-real-gpu-matrix-20260624` then passed 3/3 cases:

| case | selected KV GPU layers | exact restores | offloaded QATQ pages | resident pages | pass-through | reclaimable GPU KV | QATQ | zstd | lz4 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-15b-daily-driver` | 24 | 112 / 112 | 112 / 112 | 0 | 0 | 7.00 MiB | 32.43 MiB | 38.15 MiB | 41.45 MiB |
| `qwen25-coder3b-engineering` | 32 | 216 / 216 | 216 / 216 | 0 | 0 | 12.00 MiB | 75.00 MiB | 90.17 MiB | 98.08 MiB |
| `phi35-mini-reasoning` | 28 | 128 / 128 | 119 / 119 | 9 | 0 | 96.00 MiB | 550.34 MiB | 649.79 MiB | 715.54 MiB |

A larger Qwen2.5 Coder 7B run at
`/private/tmp/qatq-live-vram-real-gpu-20260624-coder7b` also passed, selecting
24 / 28 KV GPU layers, preserving the deterministic continuation, restoring
112 / 112 pages exactly, and storing 64,759,719 bytes through QATQ versus
73,510,399 zstd bytes and 79,723,918 lz4 bytes. The page-bounded attention
equivalence gate passed over 1,409 tokens with zero max absolute and relative
error, at a peak page/materialised ratio of 0.726756565.

The first MLX-backed runtime primitive is now reproducible through
`scripts/mlx_live_vram_streaming_attention.py`. The script consumes the real
llama.cpp attention fixture and exported K/V page files, then runs both
materialised attention and page-streaming online-softmax attention on MLX
`Device(gpu, 0)`. The streaming pass loads one K/V page at a time, carries only
the softmax max/sum/output state across pages, clears the page from the MLX
cache after evaluation, and fails if the result drifts or the peak page K/V
working set exceeds the configured ratio.

Two real local model exports passed this MLX GPU gate:

| export | pages | tokens | max abs error | max relative error | materialised peak | streaming peak | peak page/materialised ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Qwen2.5 Coder 3B, layer 0 head 0 | 4 | 3,521 | 4.470348e-08 | 3.123090e-06 | 7,303,692 bytes | 1,058,340 bytes | 0.290826470 |
| Qwen2.5 Coder 7B, layer 0 head 0 | 2 | 1,409 | 4.470348e-08 | 2.019424e-05 | 2,961,420 bytes | 1,486,864 bytes | 0.726756565 |

This is a stronger runtime primitive than the Rust reference because the
attention recurrence executes on the local MLX GPU over real llama.cpp K/V page
files. It still is not the final llama.cpp live-paging adapter: the production
path must move the same bounded-resident recurrence into the generation loop,
with allocator-backed page eviction, restore-stall attribution, and end-to-end
token preservation.

The same MLX primitive was then tightened to stream from QATQ-compressed page
storage rather than raw page files. With `--stream-from-qatq-store`, the script
encodes every selected K/V page through the release `qatq` binary, decodes it
back into a per-page restore buffer, verifies the restored bytes exactly
against the raw llama.cpp export, and only then sends one restored page at a
time to MLX GPU attention. This is the first end-to-end primitive that combines
QATQ offload storage, exact page restore, bounded GPU page residency, and
attention-output preservation.

Two real local exports passed this compressed-page GPU gate:

| export | QATQ pages | raw selected pages | QATQ stored | QATQ ratio | max abs error | streaming peak | peak page/materialised ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Qwen2.5 Coder 3B, layer 0 head 0 | 8 | 3,605,504 bytes | 1,325,597 bytes | 0.367659279 | 4.470348e-08 | 1,833,736 bytes | 0.290826470 |
| Qwen2.5 Coder 7B, layer 0 head 0 | 4 | 2,885,632 bytes | 1,057,488 bytes | 0.366466687 | 4.470348e-08 | 1,060,884 bytes | 0.726756565 |

The proof remains scoped to one layer/head attention primitive. It does not yet
replace llama.cpp's native attention graph, but it proves the exact dataflow the
native adapter must implement: QATQ-compressed cold pages in CPU/disk storage,
verified restore of one page at a time, bounded GPU residency during attention,
and numerically equivalent output.

The patched llama.cpp attention-fixture exporter now writes one query fixture
per query head for the captured layer. The MLX verifier accepts `--head -1` to
run the compressed-page streaming gate across all exported heads. For grouped
query attention, it maps query heads onto the exported KV heads using the
runtime-reported query-head and KV-head counts, so Qwen's many query heads share
the correct smaller K/V head set.

Two fresh all-head Metal plus MLX runs passed:

| export | query heads | KV heads | QATQ pages | raw selected pages | QATQ stored | QATQ ratio | max abs error | peak page/materialised ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Qwen2.5 Coder 3B, layer 0 all heads | 16 | 2 | 4 | 1,803,264 bytes | 660,857 bytes | 0.366478231 | 2.384186e-07 | 0.581487791 |
| Qwen2.5 Coder 7B, layer 0 all heads | 28 | 4 | 4 | 2,885,632 bytes | 1,057,488 bytes | 0.366466687 | 2.384186e-07 | 0.726756565 |

The 3B all-head report lives at
`/private/tmp/qatq-live-vram-all-head-fixture-coder3b-20260624/mlx-streaming-attention-qatq-store-all-heads.json`.
The 7B report lives at
`/private/tmp/qatq-live-vram-all-head-fixture-coder7b-20260624/mlx-streaming-attention-qatq-store-all-heads.json`.
Both use the release `qatq` binary for page encode/decode, verify exact page
restore before GPU attention, and preserve the materialised-attention output
within the configured absolute-error gate. This widens the primitive from a
single cherry-picked head to every query head in the captured layer.

The fixture exporter now also supports `--qatq-attention-fixture-max-layers`,
which captures one query fixture per head for several layers. Two fresh
long-context Metal plus MLX runs passed the compressed-page gate across every
captured layer and head:

| export | layers | query heads checked | pages per layer | tokens | raw selected pages | QATQ stored | QATQ ratio | max abs error | peak page/materialised ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Qwen2.5 Coder 3B, layers 0-3 all heads | 4 | 64 | 5 | 4,800 | 19,660,800 bytes | 11,600,543 bytes | 0.590034129 | 1.549721e-06 | 0.213333333 |
| Qwen2.5 Coder 7B, layers 0-1 all heads | 2 | 56 | 5 | 4,512 | 18,481,152 bytes | 9,480,990 bytes | 0.513008605 | 5.960464e-07 | 0.226950355 |

The 3B report lives at
`/private/tmp/qatq-live-vram-multilayer-fixture-coder3b-long-20260624/mlx-streaming-attention-qatq-store-all-layers-heads.json`.
The 7B report lives at
`/private/tmp/qatq-live-vram-multilayer-fixture-coder7b-long-20260624/mlx-streaming-attention-qatq-store-all-layers-heads.json`.
These runs are the current strongest local proof of the runtime primitive:
real llama.cpp Metal KV pages, QATQ-compressed page storage, exact decode,
MLX GPU streaming attention, all heads for multiple layers, and a peak
page/materialised residency gate below 23%.

## 2026-06-24 Native Page-Staged llama.cpp Strict Pass

The patched llama.cpp adapter now has a native page-staged attention source mode
behind `--qatq-gpu-page-staging`. In that mode canonical K/V tensors remain off
GPU, the persistent attention page-source adapter stages only scheduler-resident
pages onto the accelerator, and cold pages remain CPU-backed page views. The
same hot-window/page-end predicate drives the event trace and the staged-page
selection, so the QATQ live-paging gate can compare the runtime manifest,
restore-before-attention events, and page schedule.

A Qwen2.5 1.5B Metal run at
`/private/tmp/qatq-live-vram-native-page-staging-512p-exact-20260624` passed
the strict `qatq-kv-bench --live-vram-live-paging-gate`:

| model/run | prompt tokens | page tokens | total pages | resident pages | offloaded pages | GPU KV before | GPU KV after | QATQ stored | zstd | lz4 | output |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Qwen2.5 1.5B native page staging | 1,536 | 512 | 168 | 56 | 112 | 51,380,224 bytes | 14,680,064 bytes | 21,626,351 bytes | 39,913,547 bytes | 43,413,044 bytes | exact vs full GPU |

The manifest attested `live_page_residency_granularity: "per-page"` and
`gpu_allocation_granularity: "per-page"`. It staged 56 page tensors on `MTL0`
and left 112 logical K/V pages offloaded. The strict evidence report recorded
168/168 verified restores and 168/168 pages beating the best general codec.
The same prompt was run through a full-GPU baseline and
`qatq-kv-bench --compare-output-gate` passed against the page-staged candidate.
The page-staged candidate decoded the short continuation in 1,904,401 us versus
1,559,015 us for the full-GPU baseline, so this first native strict pass trades
about 22% decode latency for about 71% lower persistent GPU K/V residency in
the measured fixture.

This is a real native llama.cpp live-VRAM proof increment, not merely exported
tensor compression: the generation path consumed page-staged K/V sources and
the strict QATQ gate accepted runtime-attested per-page allocation, reduced GPU
K/V bytes, QATQ-compressed offloaded pages, restore-before-attention events, and
exact output preservation. It is still not the final production form because
the attention graph composes page views through concat rather than executing a
fully streaming page-by-page attention kernel.

The proof was rerun from the installed patched llama.cpp checkout at
`/private/tmp/qatq-llama.cpp` on 2026-06-24 with `mlx==0.31.2` installed into
an isolated `/private/tmp/qatq-mlx-venv` environment for the MLX check. The
patched `llama-simple` binary reported commit `7992aa7c8`, exposed the QATQ
page-staging flags, and executed the model runs through the Apple Metal backend.

| model/run | prompt tokens | page tokens | total pages | resident pages | offloaded pages | GPU KV before | GPU KV after | QATQ stored | zstd | lz4 | output |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Qwen2.5 1.5B, fresh page staging with fixture | 1,536 | 512 | 168 | 56 | 112 | 51,380,224 bytes | 12,582,912 bytes | 26,096,699 bytes | 36,646,975 bytes | 41,995,217 bytes | exact vs full GPU |
| Qwen2.5 Coder 3B, fresh page staging | 2,048 | 512 | 288 | 72 | 216 | 84,934,656 bytes | 15,728,640 bytes | 46,695,070 bytes | 63,883,569 bytes | 72,018,606 bytes | exact vs full GPU |
| Qwen2.5 Coder 7B, fresh page staging | 2,048 | 512 | 224 | 56 | 168 | 132,120,576 bytes | 25,165,824 bytes | 64,444,206 bytes | 88,811,046 bytes | 107,815,718 bytes | exact vs full GPU |

The fresh run directories are:

- `/private/tmp/qatq-live-vram-real-metal-page-staging-fixture-20260624`
- `/private/tmp/qatq-live-vram-coder3b-real-metal-page-staging-20260624`
- `/private/tmp/qatq-live-vram-coder7b-real-metal-page-staging-20260624`

All three fresh native runs passed the strict live-paging gate, restored every
exported page exactly, used zero pass-through pages, and beat zstd/lz4 on every
page boundary in the evidence set. The 1.5B fixture run also emitted a real
llama.cpp attention query fixture and passed the built-in page-bounded attention
equivalence gate with max absolute error `0.000000000` and a peak
page/materialised K/V ratio of `0.333333333`.

The same 1.5B fixture was then consumed by MLX on `Device(gpu, 0)` through
`scripts/mlx_live_vram_streaming_attention.py --stream-from-qatq-store`.
MLX streamed decoded QATQ pages for all exported query heads in layer 0 and
reported:

| layers checked | heads checked | QATQ pages | raw selected pages | QATQ stored | QATQ ratio | max abs error | max relative error | peak page/materialised ratio |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 12 | 6 | 1,572,864 bytes | 377,169 bytes | 0.239797592 | 4.768372e-07 | 7.143469e-07 | 0.333333333 |

The MLX report lives at
`/private/tmp/qatq-live-vram-real-metal-page-staging-fixture-20260624/mlx-streaming-attention-qatq-store-all-layers-heads.json`.
This is still a scoped validation of the page-bounded attention primitive, not
a claim that the production runtime has a final non-concat paged attention
kernel.

The MLX verifier is now integrated into the fail-closed live-VRAM evidence
runner with explicit coverage and performance gates. A fresh 2026-06-24 run at
`/private/tmp/qatq-live-vram-all-layer-mlx-perf-gated-page-staging-20260624`
executed the patched installed `llama-simple`, strict live-paging QATQ replay,
and MLX GPU streaming attention in one command:

```sh
python3 scripts/llama_cpp_live_vram_evidence.py \
  --llama-simple /private/tmp/qatq-llama.cpp/build/bin/llama-simple \
  --qatq-kv-bench target/release/qatq-kv-bench \
  --model /path/to/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf \
  --model-id qwen2.5-1.5b-all-layer-mlx-perf-gated-page-staging-20260624 \
  --work-dir /private/tmp/qatq-live-vram-all-layer-mlx-perf-gated-page-staging-20260624 \
  --sweep-kv-gpu-layers 24 \
  --short-predict 4 \
  --deep-predict 2 \
  --deep-prompt-seed "memory " \
  --deep-repeat 1535 \
  --page-tokens 512 \
  --current-token 512 \
  --hot-window-tokens 0 \
  --next-required page-end \
  --gpu-page-staging \
  --require-live-paging \
  --skip-cpu-kv-baseline \
  --skip-attention-trace \
  --skip-attention-event-trace \
  --skip-attention-page-tensor-self-test \
  --attention-fixture-max-layers 28 \
  --mlx-streaming-attention-gate \
  --mlx-python /private/tmp/qatq-mlx-venv/bin/python \
  --mlx-qatq-bin target/release/qatq \
  --mlx-min-layers-checked 28 \
  --mlx-min-heads-checked 336 \
  --mlx-max-streaming-slowdown 3.0
```

That integrated run selected 24 KV GPU layers, preserved deterministic output,
passed the strict live-paging gate, restored 168/168 pages exactly, stored
112/112 offloaded pages through QATQ with zero pass-through pages, and beat
zstd/lz4 on 168/168 page boundaries. It reported GPU K/V residency falling
from 51,380,224 bytes to 12,582,912 bytes. The integrated MLX gate ran on
`Device(gpu, 0)`, checked all 28 captured layers and all 336 query heads,
streamed decoded QATQ pages from the page store, and reported max absolute
error `1.907349e-06`, max relative error `4.8414e-05`, peak
page/materialised K/V ratio `0.333333333`, 168 QATQ pages, QATQ store ratio
`0.592565514`, and streaming/materialised attention time ratio `1.664277`,
passing the configured `3.0` slowdown gate.

The same stricter gate was then widened through
`scripts/llama_cpp_live_vram_matrix.py` for the Coder workhorse and powerhouse
profiles. The first attempt failed closed because the local example matrix
combined a long prose seed with `deep_repeat=2047`, exceeding the persistent
page-source graph-object budget. The matrix config was corrected to use the
controlled 2048-token page-staging prefill shape, then rerun at
`/private/tmp/qatq-live-vram-coder-mlx-matrix-20260624`.

| matrix case | selected KV GPU layers | exact restores | offloaded QATQ pages | resident pages | pass-through | event trace | attention reads | MLX layers | MLX heads | MLX QATQ ratio | MLX stream/materialised | reclaimable GPU KV | QATQ | zstd | lz4 |
| --- | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Qwen2.5 Coder 3B workhorse | 30 | 288 / 288 | 216 / 216 | 72 | 0 | pass / 1008 | 792 / 36 layers | 36 | 576 | 0.618498 | 1.868 | 66.00 MiB | 44.53 MiB | 60.92 MiB | 68.68 MiB |
| Qwen2.5 Coder 7B powerhouse | 14 | 224 / 224 | 168 / 168 | 56 | 0 | pass / 784 | 616 / 28 layers | 28 | 784 | 0.548739 | 1.822 | 112.00 MiB | 61.46 MiB | 84.70 MiB | 102.82 MiB |

This matrix is now the strongest reproducible larger-model proof bundle:
strict live-paging, QATQ compression wins over zstd/lz4 on every offloaded page,
zero pass-through offloads, deterministic continuation preservation, all
captured layers and heads checked by MLX from QATQ-restored pages, and bounded
streaming/materialised attention slowdown under the configured `4.0` gate.

## 2026-06-24 Persistent Page-Source Byte Budgets

The patched llama.cpp adapter now bounds the persistent page-source path by
bytes as well as page counts:

- `--qatq-attention-persistent-page-source-max-source-bytes <n>` caps the
  requested bytes staged for a single attention source.
- `--qatq-attention-persistent-page-source-max-retained-bytes <n>` caps the
  total retained backend page tensor pool.

The JSONL trace records `max_source_bytes`, `max_retained_bytes`, and
`retained_bytes`; `scripts/llama_cpp_live_vram_evidence.py` rejects traces that
omit these fields or report bytes beyond the configured budgets.

A good-path Metal run at
`/private/tmp/qatq-live-vram-byte-budgeted-page-staging-retained-20260624`
passed the strict live-paging gate with explicit budgets of 256 MiB per source
and 1 GiB retained:

| model/run | prompt tokens | page tokens | GPU KV before | GPU KV after | max requested/source budget | max retained/budget | exact restores | output |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Qwen2.5 1.5B byte-budgeted page staging | 1,536 | 512 | 51,380,224 bytes | 12,582,912 bytes | 262,144 / 268,435,456 bytes | 12,582,912 / 1,073,741,824 bytes | 168 / 168 | exact vs full GPU |

Two hostile-budget probes were also run against the same local Metal adapter:

- `/private/tmp/qatq-byte-budget-fail-20260624-clean2` set
  `--qatq-attention-persistent-page-source-max-source-bytes 1`. The runner
  exited with code `1`, logged `QATQ persistent page source byte budget
  exceeded`, freed the Metal context, and did not abort.
- `/private/tmp/qatq-live-vram-retained-budget-fail-deep-20260624` set
  `--qatq-attention-persistent-page-source-max-retained-bytes 1` on the deep
  staged path. The evidence runner exited with code `1`, logged `QATQ
  persistent page source retained byte budget exceeded`, freed the Metal
  context, and did not abort.

This is a production-hardening improvement for resource exhaustion and
operator-bounded memory growth. It does not remove the remaining architectural
gap: the current native adapter still composes page sources through
`ggml_concat` rather than a final page-bounded attention kernel.

## Remaining Live-VRAM Gap

The next proof step is no longer exported tensor compression or coarse
whole-layer placement. It is to turn the native page-staged path into a
production-quality llama.cpp runtime adapter:

1. identify cold KV pages;
2. snapshot and verify the page;
3. commit the QATQ offload;
4. free or reuse the GPU page only after commit;
5. prefetch and restore before attention needs the page;
6. replace concat-composed page sources with a page-bounded attention path or
   equivalent native paged attention path so the full logical KV set is not
   materialised as one attention source;
7. include restore stalls in per-token latency; and
8. make `scripts/llama_cpp_live_vram_evidence.py --require-live-paging
   --require-native-page-streaming` pass against real Metal/MLX evidence.

QATQ now includes `live_vram_streaming_attention_reference`,
`live_vram_materialized_attention_reference`, and
`compare_live_vram_streaming_attention_reference` as executable references for
item 6. The comparison reference matches materialised softmax attention in unit
tests, checks split invariance across page boundaries, handles large score
separation with stable online accumulation, rejects malformed/non-finite pages,
and reports the peak page KV working set separately from the full materialised
KV set. `decode_tensor_le_bytes_to_f32` and
`compare_live_vram_typed_streaming_attention_reference` extend the same gate to
native little-endian f32, f16, and bf16 page bytes, which matches the exported
KV tensor formats used by the llama.cpp adapter. The evidence runner writes
`native-page-streaming-status.json` and exposes `--require-native-page-streaming`
as a production-shaped gate. Earlier page-staged runs reported native page
streaming as unsupported because the runtime composed page sources with
`ggml_concat`. In the current pinned patch, native page-streaming flags fail
closed instead of claiming success: the patch still needs a real
accelerator-schedulable `ggml_segmented_kqv` consumer before the scoped Qwen2.5
Coder fixtures or broader matrix can be treated as native live-VRAM evidence.
The external MLX streaming gate remains an independent equivalence reference
for the page-bounded attention maths, not proof that llama.cpp has consumed
those pages through its production attention graph.

The 2026-06-24 composition-schema refresh makes that boundary explicit in the
runtime traces. `page-composed-source` and `persistent-page-source` JSONL rows
now include `composition: "ggml_concat"` and `native_page_streaming: false`.
The focused Metal run at
`/private/tmp/qatq-live-vram-composition-schema-gpu-20260624-v2` passed the
scoped live-paging gate for Qwen2.5-Coder 3B, while
`/private/tmp/qatq-live-vram-native-gate-composition-schema-20260624` failed
`--require-native-page-streaming` with the expected explicit sources:
`page-composed-source`, `persistent-page-source`, and
`deep-persistent-page-source`.

The next same-day segment API refresh added
`--qatq-attention-page-segments-trace`. The focused Metal run at
`/private/tmp/qatq-live-vram-segment-api-gpu-20260624` passed the scoped
live-paging gate and emitted 792 page-segment events across 36 layers with both
key and value reads, max segment count 5, `composition: "none"`, and
`attention_consumed: false`. The strict gate at
`/private/tmp/qatq-live-vram-native-gate-segment-api-20260624` still failed, now
explicitly reporting that the page segment API is present but not yet consumed
by a native attention path.

The patched llama.cpp checkout was then rebuilt and rerun through the full
local Metal plus MLX matrix at
`/private/tmp/qatq-live-vram-real-gpu-demand-20260624`. The matrix passed both
configured Coder cases under `--require-live-paging --gpu-page-staging`:

| case | selected KV GPU layers | exact restores | offloaded QATQ pages | MLX coverage | reclaimable GPU | QATQ / zstd / lz4 |
| --- | ---: | ---: | ---: | --- | ---: | --- |
| Qwen2.5-Coder 3B workhorse | 18 | 288 / 288 | 216 / 216 | 36 layers, 576 heads | 72.00 MiB | 44.53 / 60.92 / 68.68 MiB |
| Qwen2.5-Coder 7B powerhouse | 21 | 224 / 224 | 168 / 168 | 28 layers, 784 heads | 105.00 MiB | 61.46 / 84.70 / 102.82 MiB |

This run used the installed patched `llama-simple` binary at
`/private/tmp/qatq-llama.cpp/build/bin/llama-simple`, QATQ release binaries from
`target/release`, real GGUF model files, Metal execution in llama.cpp, and MLX
streaming-attention checks on `Device(gpu, 0)`. It strengthens the scoped
live-paging evidence. A strict follow-up run at
`/private/tmp/qatq-live-vram-native-gate-demand-20260624` enabled
`--require-native-page-streaming` and failed closed because the page-source path
still used `ggml_concat` and the page-segment API had not yet been consumed.

The next native-consumer increment added
`--qatq-native-page-streaming-attention`, a fail-closed CPU custom-op attention
consumer for bounded K/V page segments. The focused correctness run at
`/private/tmp/qatq-live-vram-native-cpu-custom-correctness-20260624` passed
`--require-native-page-streaming --native-page-streaming-attention
--skip-runtime-reclaim-gate` against Qwen2.5-Coder 3B:

| evidence | result |
| --- | --- |
| output preservation | pass, generated text hash `18104483715415384002` |
| exact restores | 144 / 144 |
| QATQ vs general codecs | QATQ 47.88 MiB, zstd 56.40 MiB, lz4 61.31 MiB |
| attention page segments | 720 events, 36 layers, max segment count 2 |
| segment consumption | `native_page_streaming: true`, `attention_consumed: true`, consumer `ggml_custom_4d_cpu` |
| MLX reference | `Device(gpu, 0)`, 36 layers, 576 heads, max abs error `0.000004768` |

This is an exactness and graph-integration proof for non-concat page
consumption. It is retained as the CPU-backed debug path.

The next native-consumer increment added
`--qatq-native-page-streaming-attention-ggml`, which keeps the same bounded
page-segment contract but builds the attention graph from ggml operations:
per-page KQ matmuls, score concat only, one full softmax, per-page V matmuls,
and a summed output. It does not concat-compose full K/V page sources. The
strict Metal plus MLX run at
`/private/tmp/qatq-live-vram-native-ggml-strict-20260624` passed
`--require-live-paging --require-native-page-streaming
--native-page-streaming-attention-ggml` against Qwen2.5-Coder 3B:

| evidence | result |
| --- | --- |
| output preservation | pass, generated text hash `18104483715415384002` |
| exact restores | 144 / 144 |
| QATQ vs general codecs | QATQ 47.87 MiB, zstd 56.41 MiB, lz4 61.31 MiB |
| GPU K/V residency | before 63.00 MiB, after 49.50 MiB, reclaimable 13.50 MiB |
| strict live-paging gate | pass, `gpu_allocation_granularity: "per-page"` |
| attention page segments | 720 events, 36 layers, max segment count 2 |
| segment consumption | `native_page_streaming: true`, `attention_consumed: true`, consumer `ggml_segmented_kqv` |
| MLX reference | `Device(gpu, 0)`, 36 layers, 576 heads, max abs error `0.000002861` |

The Qwen2.5-Coder 7B stress case then passed the same strict live-paging and
native page-streaming gates with the explicit aggregate codec policy. That
policy keeps the default per-page gate intact for release-sensitive compression
claims, but allows live-VRAM runs to offload QATQ-compressed pages that shrink
raw bytes even when tiny tail pages are marginally smaller under zstd, provided
QATQ beats zstd/lz4 in aggregate. The run lives at
`/private/tmp/qatq-live-vram-native-ggml-7b-aggregate-strict-20260624`:

| evidence | result |
| --- | --- |
| output preservation | pass, generated text hash `10284620652055069672` |
| exact restores | 168 / 168 |
| offloaded compressed pages | 112 / 112, zero pass-through |
| resident pages | 56 |
| QATQ vs general codecs | QATQ 46.94 MiB, zstd 52.66 MiB, lz4 57.10 MiB |
| GPU K/V residency | before 70.00 MiB, after 30.00 MiB, reclaimable 40.00 MiB |
| strict live-paging gate | pass, aggregate codec policy |
| attention page segments | 504 events, 28 layers, max segment count 3 |
| segment consumption | `native_page_streaming: true`, `attention_consumed: true`, consumer `ggml_segmented_kqv` |
| MLX reference | `Device(gpu, 0)`, 28 layers, 784 heads, max abs error `0.000004768` |

## 2026-06-24 Sustained Native Matrix

The patched llama.cpp binary was then exercised through the sustained native
matrix at
`/private/tmp/qatq-live-vram-native-sustained-matrix-20260624-r2` using real
local Qwen2.5 Coder GGUF models, Apple Metal execution, QATQ release binaries,
native `ggml_segmented_kqv` attention, GPU page staging, strict live-paging
gates, and fail-closed MLX streaming-attention checks. The matrix passed 4/4
cases:

| case | selected KV GPU layers | exact restores | offloaded QATQ pages | resident pages | event trace | attention reads | MLX layers / heads | MLX stream/materialised | reclaimable GPU KV | QATQ | zstd | lz4 |
| --- | ---: | ---: | ---: | ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-coder-3b-review-512p` | 18 | 216 / 216 | 144 / 144 | 72 | pass / 720 | 648 / 36 layers | 36 / 576 | 1.980 | 22.50 MiB | 34.92 MiB | 40.03 MiB | 43.51 MiB |
| `qwen25-coder-3b-memory-1024p` | 18 | 144 / 144 | 144 / 144 | 0 | pass / 576 | 720 / 36 layers | 36 / 576 | 1.486 | 18.00 MiB | 51.52 MiB | 60.28 MiB | 65.49 MiB |
| `qwen25-coder-7b-review-512p-aggregate` | 12 | 112 / 112 | 56 / 56 | 56 | pass / 336 | 448 / 28 layers | 28 / 784 | 1.608 | 32.00 MiB | 38.17 MiB | 43.04 MiB | 46.68 MiB |
| `qwen25-coder-7b-memory-512p-aggregate` | 12 | 168 / 168 | 112 / 112 | 56 | pass / 560 | 504 / 28 layers | 28 / 784 | 1.825 | 40.00 MiB | 52.27 MiB | 58.97 MiB | 63.98 MiB |

Every passing case preserved the deterministic continuation, restored exported
pages exactly before counting the page, used zero raw pass-through pages, beat
raw/zstd/lz4 on the reported page boundaries, and consumed bounded page
segments through the non-concat native attention path. The 512-token review
profiles deliberately use the aggregate codec gate because tail pages can be
policy-negative under a per-page zstd comparison even while QATQ wins in
aggregate and still shrinks raw bytes.

## 2026-06-24 Native Breadth Matrix

The native live-VRAM path was then widened beyond the Coder-only fixtures. The
clean breadth matrix at
`/private/tmp/qatq-live-vram-native-breadth-matrix-20260624-r7` passed 3/3
strict cases with real Apple Metal execution, native `ggml_segmented_kqv`,
GPU page staging, QATQ release binaries, and MLX `Device(gpu, 0)` external
streaming-attention gates:

| case | selected KV GPU layers | exact restores | offloaded QATQ pages | resident pages | event trace | attention reads | MLX layers / heads | MLX stream/materialised | reclaimable GPU KV | QATQ | zstd | lz4 |
| --- | ---: | ---: | ---: | ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-15b-daily-native-512p-aggregate` | 14 | 168 / 168 | 112 / 112 | 56 | pass / 560 | 504 / 28 layers | 28 / 336 | 1.935 | 17.50 MiB | 25.03 MiB | 28.65 MiB | 31.12 MiB |
| `qwen25-3b-general-native-1025p-l12-aggregate` | 12 | 216 / 216 | 216 / 216 | 0 | pass / 864 | 936 / 36 layers | 36 / 576 | 1.381 | 24.07 MiB | 82.22 MiB | 98.22 MiB | 106.95 MiB |
| `phi35-mini-ops-native-512p-aggregate` | 16 | 192 / 192 | 128 / 128 | 64 | pass / 640 | 576 / 32 layers | 32 / 1024 | 1.300 | 288.00 MiB | 391.64 MiB | 448.77 MiB | 493.38 MiB |

This pass also fixed a real adapter robustness issue found by Phi 3.5 mini:
native segmented attention now reserves bounded extra ggml graph metadata even
when the older persistent-page-source trace is disabled. Before that fix, Phi
failed during graph reserve with a ggml context-memory assertion. The reserve is
metadata-only and still bounded by the adapter's existing
`LLAMA_QATQ_GRAPH_EXTRA_NODES` cap.

The same breadth work exposed and resolved a Qwen2.5 3B general-instruct
frontier issue. At 18 staged KV layers, tested 1025/1537-token page shapes
staged more GPU page bytes than the original KV context
(`146,165,760 >= 122,683,392` and `174,532,608 >= 122,683,392`), so
`--live-vram-live-paging-gate` rejected them. The passing frontier uses 12
staged KV layers, restores 216/216 pages exactly, offloads 216/216 through
QATQ, and reclaims 24.07 MiB while preserving deterministic output. This makes
page-staging byte pressure an explicit frontier-selection constraint, not a
codec exactness problem.

## 2026-06-24 Repeated Native Soaks

The strict native matrices were then rerun with `--iterations 2` to verify
repeatability across context creation, Metal graph build, QATQ page
encode/restore, native segmented attention, and MLX page-streaming validation.

The breadth soak at
`/private/tmp/qatq-live-vram-native-breadth-soak-2x-20260624` passed 6/6 runs:

| case | runs | failures | elapsed min/max | reclaimable GPU min/max | QATQ min/max |
| --- | ---: | ---: | ---: | ---: | ---: |
| `qwen25-15b-daily-native-512p-aggregate` | 2 | 0 | 8.62 / 9.31 s | 17.50 / 17.50 MiB | 25.03 / 25.03 MiB |
| `qwen25-3b-general-native-1025p-l12-aggregate` | 2 | 0 | 24.30 / 24.53 s | 24.07 / 24.07 MiB | 82.22 / 82.22 MiB |
| `phi35-mini-ops-native-512p-aggregate` | 2 | 0 | 47.96 / 48.09 s | 288.00 / 288.00 MiB | 391.64 / 391.64 MiB |

The Coder sustained soak at
`/private/tmp/qatq-live-vram-native-sustained-soak-2x-20260624` passed 8/8
runs:

| case | runs | failures | elapsed min/max | reclaimable GPU min/max | QATQ min/max |
| --- | ---: | ---: | ---: | ---: | ---: |
| `qwen25-coder-3b-review-512p` | 2 | 0 | 13.09 / 13.29 s | 22.50 / 22.50 MiB | 34.92 / 34.92 MiB |
| `qwen25-coder-3b-memory-1024p` | 2 | 0 | 14.28 / 14.35 s | 18.00 / 18.00 MiB | 51.52 / 51.52 MiB |
| `qwen25-coder-7b-review-512p-aggregate` | 2 | 0 | 15.46 / 17.05 s | 32.00 / 32.00 MiB | 38.17 / 38.17 MiB |
| `qwen25-coder-7b-memory-512p-aggregate` | 2 | 0 | 18.45 / 18.51 s | 40.00 / 40.00 MiB | 52.27 / 52.27 MiB |

All 14 repeated runs preserved deterministic continuations, restored every
exported page exactly, used zero raw pass-through pages, passed the strict
live-paging gate, consumed bounded K/V page segments through
`ggml_segmented_kqv`, and passed the MLX GPU page-streaming equivalence gate.

The matrix runner was then hardened with optional stability gates for repeated
runs: `--require-stable-reclaim`, `--require-stable-qc-bytes`, and
`--max-elapsed-jitter-ratio`. It also gained a bounded
`--host-memory-pressure-mib` stress knob that allocates and page-touches host
memory while the real Metal/MLX cases execute. These gates are fail-closed:
changes in reclaimable GPU bytes, raw/QATQ/zstd/lz4 byte totals, or excessive
elapsed-time jitter cause the matrix command to exit non-zero.

A stricter breadth stress run at
`/private/tmp/qatq-live-vram-native-breadth-stress-3x-512m-20260624` used
`--iterations 3`, `--host-memory-pressure-mib 512`,
`--require-stable-reclaim`, `--require-stable-qc-bytes`, and
`--max-elapsed-jitter-ratio 0.75`. It passed 9/9 real Metal/MLX runs:

| case | runs | failures | elapsed min/max | reclaimable GPU min/max | QATQ min/max |
| --- | ---: | ---: | ---: | ---: | ---: |
| `qwen25-15b-daily-native-512p-aggregate` | 3 | 0 | 8.51 / 8.61 s | 17.50 / 17.50 MiB | 25.03 / 25.03 MiB |
| `qwen25-3b-general-native-1025p-l12-aggregate` | 3 | 0 | 22.75 / 23.02 s | 24.07 / 24.07 MiB | 82.22 / 82.22 MiB |
| `phi35-mini-ops-native-512p-aggregate` | 3 | 0 | 45.88 / 47.06 s | 288.00 / 288.00 MiB | 391.64 / 391.64 MiB |

A matching sustained Coder stress run at
`/private/tmp/qatq-live-vram-native-sustained-stress-3x-512m-20260624` passed
12/12 real Metal/MLX runs under the same bounded host-memory pressure and
stability gates:

| case | runs | failures | elapsed min/max | reclaimable GPU min/max | QATQ min/max |
| --- | ---: | ---: | ---: | ---: | ---: |
| `qwen25-coder-3b-review-512p` | 3 | 0 | 13.01 / 13.78 s | 22.50 / 22.50 MiB | 34.92 / 34.92 MiB |
| `qwen25-coder-3b-memory-1024p` | 3 | 0 | 13.63 / 15.01 s | 18.00 / 18.00 MiB | 51.52 / 51.52 MiB |
| `qwen25-coder-7b-review-512p-aggregate` | 3 | 0 | 14.77 / 17.25 s | 32.00 / 32.00 MiB | 38.17 / 38.17 MiB |
| `qwen25-coder-7b-memory-512p-aggregate` | 3 | 0 | 18.17 / 21.17 s | 40.00 / 40.00 MiB | 52.27 / 52.27 MiB |

Together, the stability-gated stress runs add 21 more strict native passes
under bounded host-memory pressure. They preserve the same claim boundary:
real local Metal/MLX live-paging evidence is strong, but production burn-in
still needs longer duration, more context lengths and dtypes, and explicit
restore-slot allocation failure tests.

## 2026-06-24 Fresh Installed llama.cpp GPU Runs

The patched llama.cpp checkout was rebuilt again and run from the installed
binary path `/private/tmp/qatq-llama.cpp/build/bin/llama-simple` with local
GGUF models, QATQ release binaries, Apple Metal, MLX `Device(gpu, 0)`, native
`ggml_segmented_kqv`, GPU page staging, strict live-paging gates, aggregate
codec gates, and 512 MiB of page-touched host-memory pressure.

The breadth matrix at `/private/tmp/qatq-live-vram-real-gpu-breadth-20260624`
passed 3/3 cases:

| case | selected KV GPU layers | exact restores | offloaded QATQ pages | resident pages | event trace | attention reads | MLX layers / heads | MLX stream/materialised | reclaimable GPU KV | QATQ | zstd | lz4 |
| --- | ---: | ---: | ---: | ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-15b-daily-native-512p-aggregate` | 14 | 168 / 168 | 112 / 112 | 56 | pass / 560 | 504 / 28 layers | 28 / 336 | 1.920 | 17.50 MiB | 25.03 MiB | 28.65 MiB | 31.12 MiB |
| `qwen25-3b-general-native-1025p-l12-aggregate` | 12 | 216 / 216 | 216 / 216 | 0 | pass / 864 | 936 / 36 layers | 36 / 576 | 1.459 | 24.07 MiB | 82.22 MiB | 98.22 MiB | 106.95 MiB |
| `phi35-mini-ops-native-512p-aggregate` | 16 | 192 / 192 | 128 / 128 | 64 | pass / 640 | 576 / 32 layers | 32 / 1024 | 1.273 | 288.00 MiB | 391.64 MiB | 448.77 MiB | 493.38 MiB |

The sustained Coder matrix at
`/private/tmp/qatq-live-vram-real-gpu-sustained-20260624` passed 4/4 cases:

| case | selected KV GPU layers | exact restores | offloaded QATQ pages | resident pages | event trace | attention reads | MLX layers / heads | MLX stream/materialised | reclaimable GPU KV | QATQ | zstd | lz4 |
| --- | ---: | ---: | ---: | ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-coder-3b-review-512p` | 18 | 216 / 216 | 144 / 144 | 72 | pass / 720 | 648 / 36 layers | 36 / 576 | 1.930 | 22.50 MiB | 34.92 MiB | 40.03 MiB | 43.51 MiB |
| `qwen25-coder-3b-memory-1024p` | 18 | 144 / 144 | 144 / 144 | 0 | pass / 576 | 720 / 36 layers | 36 / 576 | 1.446 | 18.00 MiB | 51.52 MiB | 60.28 MiB | 65.49 MiB |
| `qwen25-coder-7b-review-512p-aggregate` | 12 | 112 / 112 | 56 / 56 | 56 | pass / 336 | 448 / 28 layers | 28 / 784 | 1.537 | 32.00 MiB | 38.17 MiB | 43.04 MiB | 46.68 MiB |
| `qwen25-coder-7b-memory-512p-aggregate` | 12 | 168 / 168 | 112 / 112 | 56 | pass / 560 | 504 / 28 layers | 28 / 784 | 1.815 | 40.00 MiB | 52.27 MiB | 58.97 MiB | 63.98 MiB |

Both matrices preserved deterministic continuations, restored every exported
page exactly, used zero raw pass-through pages, consumed bounded K/V page
segments through the non-concat native attention path, passed MLX
page-streaming equivalence, and beat raw, zstd, and lz4 in aggregate for every
configured case.

The focused restore-slot pressure proof at
`/private/tmp/qatq-live-vram-restore-slot-pressure-20260624-r2` passed the same
strict native gate on Qwen2.5 1.5B and exercised a deliberately tiny one-byte
restore-slot limit. The patched runtime found a real `MTL0` key page and
rejected the oversized restore before allocation:

```text
QATQ restore slot pressure self-test rejected MTL0 key page for layer 0 stream 0 with 512 tokens (262144 requested bytes > 1 byte restore slot limit)
```

That run also preserved output, restored 168/168 pages, offloaded 112/112 pages
through QATQ, kept 56 pages resident, passed 560 event-trace records, checked
504 attention reads across 28 layers, verified MLX across 28 layers and 336
heads, reclaimed 17.50 MiB of GPU K/V, and stored 25.03 MiB through QATQ versus
28.65 MiB zstd and 31.12 MiB lz4.

The same restore-slot pressure boundary was rerun on 2026-06-25 from the clean
bootstrapped pinned llama.cpp checkout at
`/private/tmp/qatq-llama-bootstrap-proof`, using the freshly built
`build-qatq/bin/llama-simple`. The strict backend-op proof at
`/private/tmp/qatq-live-vram-restore-slot-pressure-bootstrap-20260625-r4`
passed with `--require-live-paging`, `--require-native-page-streaming`,
`--native-page-streaming-attention-backend-op`, and
`--native-page-streaming-flatten-flash`.

The run selected the compact 4-layer frontier because 14 selected layers
correctly failed the live-paging gate: staged page bytes exceeded the source KV
context footprint. The passing compact proof preserved output, restored
168/168 pages, offloaded 112/112 compressed pages, used zero pass-through pages,
consumed 16 live-offloaded page-segment events through
`backend_scheduled_flattened_flash_attention`, verified MLX GPU streaming
attention over 28 layers and 336 heads, reduced persistent GPU K/V from
35.00 MiB to 12.00 MiB, and stored 25.00 MiB through QATQ versus 28.65 MiB zstd
and 31.12 MiB lz4. The runtime measured a real Metal key page and rejected it
before allocation against the deliberately tiny restore slot:

```text
QATQ restore slot pressure self-test rejected MTL0 key page for layer 0 stream 0 with 512 tokens (262144 requested bytes > 1 byte restore slot limit)
```

This is the current strongest OOM-adjacent safety proof: it exercises bounded
allocation rejection in the real runtime adapter path without attempting to
force an unsafe device OOM.

The same bootstrapped checkout was then used for a compact three-model breadth
matrix at
`/private/tmp/qatq-live-vram-layer-memory-breadth-bootstrap-20260625`. This
run used `scripts/llama_cpp_live_vram_matrix.py --llama-cpp-source
/private/tmp/qatq-llama-bootstrap-proof` so the strict native verifier audited
the exact source tree that produced the `build-qatq/bin/llama-simple` binary.
It ran with 1 GiB of page-touched host-memory pressure, strict live-paging,
strict native page-streaming, backend-scheduled flattened Flash Attention,
MLX GPU page-streaming verification, aggregate codec gates, restore-slot
pressure checks, and pruned bulk artifacts. The matrix passed 3/3 cases:

| case | selected KV GPU layers | exact restores | offloaded QATQ pages | resident pages | pass-through | event trace | MLX layers / heads | MLX stream/materialised | reclaimable GPU KV | QATQ | zstd | lz4 | elapsed |
| --- | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-15b-layer-memory-breadth-512p` | 4 | 168 / 168 | 112 / 112 | 56 | 0 | pass / 560 | 28 / 336 | 1.737 | 23.00 MiB | 25.00 MiB | 28.65 MiB | 31.12 MiB | 7.97 s |
| `qwen25-3b-layer-memory-breadth-512p` | 4 | 288 / 288 | 216 / 216 | 72 | 0 | pass / 1008 | 36 / 576 | 1.762 | 52.00 MiB | 49.62 MiB | 57.42 MiB | 62.43 MiB | 13.39 s |
| `phi35-mini-layer-memory-breadth-512p` | 4 | 192 / 192 | 128 / 128 | 64 | 0 | pass / 640 | 32 / 1024 | 1.165 | 432.00 MiB | 391.57 MiB | 448.79 MiB | 493.38 MiB | 32.93 s |

This proof strengthens the reproducibility story because it exercises the same
strict runtime gates from the clean bootstrap layout rather than a hand-built
working tree. It is still a compact selected-layer breadth proof; production
burn-in still needs longer latency-tail runs, more context lengths, more page
sizes, and shared-server pressure.

## 2026-06-24 Native Dtype Breadth

The same patched installed llama.cpp binary was then probed with real Apple
Metal one-token runs for non-f16 KV cache dtypes. Both `--cache-type-k bf16
--cache-type-v bf16` and `--cache-type-k f32 --cache-type-v f32` instantiated
successfully on the Apple M4 Metal backend. The bf16 probe confirmed Metal
reported `has bfloat = true` and allocated a 7.00 MiB bf16 KV buffer for the
Qwen2.5 1.5B 256-token smoke. The f32 probe allocated a 14.00 MiB f32 KV
buffer for the same smoke.

After that support probe, Qwen2.5 1.5B was rerun through the strict native
live-paging evidence stack with bf16 and f32 K/V caches: real Metal execution,
native `ggml_segmented_kqv`, GPU page staging, aggregate codec gate, exact
restore, event-trace gates, attention fixture equivalence, MLX GPU
page-streaming equivalence, and bounded restore-slot pressure rejection.

| dtype | work dir | exact restores | offloaded QATQ pages | resident pages | attention reads | MLX layers / heads | MLX stream/materialised | reclaimable GPU KV | raw KV | QATQ | zstd | lz4 | best-codec pages |
| --- | --- | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| bf16 | `/private/tmp/qatq-live-vram-dtype-bf16-20260624` | 168 / 168 | 112 / 112 | 56 | 504 / 28 layers | 28 / 336 | 1.831 | 17.50 MiB | 30.65 MiB | 19.38 MiB | 23.54 MiB | 30.22 MiB | 167 / 168 |
| f32 | `/private/tmp/qatq-live-vram-dtype-f32-20260624` | 168 / 168 | 112 / 112 | 56 | 504 / 28 layers | 28 / 336 | 1.948 | 35.00 MiB | 61.30 MiB | 49.28 MiB | 56.04 MiB | 60.54 MiB | 168 / 168 |

Both dtype runs preserved the deterministic continuation, restored every page
exactly, used zero raw pass-through pages, passed the strict live-paging gate,
consumed bounded page segments through `ggml_segmented_kqv`, and passed MLX GPU
page-streaming over QATQ-restored pages. The bf16 run won against raw, zstd,
and lz4 in aggregate while one tiny/tail page remained marginally better under
the best general codec. The f32 run beat the best general codec on every page
boundary. This broadens the runtime proof from f16 into bf16/f32, but it is
still a Qwen2.5 1.5B dtype-breadth proof rather than coverage across all model
families and context lengths.

The dtype proof was then widened with the reproducible matrix config
`adapters/llama-cpp/live-vram-native-dtype.local.example.json`. The matrix ran
from the same installed patched llama.cpp binary with real Apple Metal, MLX
`Device(gpu, 0)`, native `ggml_segmented_kqv`, GPU page staging, aggregate
codec gates, strict live-paging gates, bounded restore-slot pressure rejection,
and 512 MiB of host-memory pressure. The first pass covered Qwen2.5 3B and
Qwen2.5 Coder 3B at `/private/tmp/qatq-live-vram-native-dtype-matrix-20260624`.
The expanded pass then covered Phi 3.5 mini and Qwen2.5 Coder 7B too, passing
8/8 cases at `/private/tmp/qatq-live-vram-native-dtype-heavy-matrix-20260624`:

| case | dtype | selected KV GPU layers | exact restores | offloaded QATQ pages | resident pages | attention reads | MLX layers / heads | MLX stream/materialised | reclaimable GPU KV | QATQ | zstd | lz4 |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-coder-3b-bf16-review-512p` | bf16 | 18 | 144 / 144 | 72 / 72 | 72 | 576 / 36 layers | 36 / 576 | 1.711 | 18.00 MiB | 18.00 MiB | 21.51 MiB | 27.59 MiB |
| `qwen25-coder-3b-f32-review-512p` | f32 | 18 | 144 / 144 | 72 / 72 | 72 | 576 / 36 layers | 36 / 576 | 1.739 | 36.00 MiB | 45.35 MiB | 51.13 MiB | 55.26 MiB |
| `qwen25-3b-bf16-general-1025p-l12` | bf16 | 12 | 144 / 144 | 144 / 144 | 0 | 720 / 36 layers | 36 / 576 | 1.400 | 18.01 MiB | 37.43 MiB | 46.81 MiB | 60.37 MiB |
| `qwen25-3b-f32-general-1025p-l12` | f32 | 12 | 144 / 144 | 144 / 144 | 0 | 720 / 36 layers | 36 / 576 | 1.503 | 36.02 MiB | 97.36 MiB | 111.95 MiB | 120.97 MiB |
| `qwen25-coder-7b-bf16-review-512p-aggregate` | bf16 | 12 | 112 / 112 | 56 / 56 | 56 | 448 / 28 layers | 28 / 784 | 1.504 | 24.00 MiB | 26.53 MiB | 31.21 MiB | 39.97 MiB |
| `qwen25-coder-7b-f32-review-512p-aggregate` | f32 | 12 | 112 / 112 | 56 / 56 | 56 | 448 / 28 layers | 28 / 784 | 1.635 | 48.00 MiB | 66.29 MiB | 74.13 MiB | 81.16 MiB |
| `phi35-mini-bf16-ops-512p-aggregate` | bf16 | 16 | 128 / 128 | 64 / 64 | 64 | 512 / 32 layers | 32 / 1024 | 1.155 | 192.00 MiB | 246.87 MiB | 291.36 MiB | 379.16 MiB |
| `phi35-mini-f32-ops-512p-aggregate` | f32 | 16 | 128 / 128 | 64 / 64 | 64 | 512 / 32 layers | 32 / 1024 | 1.143 | 384.00 MiB | 604.70 MiB | 676.53 MiB | 740.82 MiB |

All eight cases preserved deterministic continuations, restored every page
exactly, used zero raw pass-through pages, passed the strict live-paging gate,
consumed bounded K/V page segments through the native non-concat attention
path, passed MLX page-streaming equivalence, and beat raw, zstd, and lz4 on the
same page boundaries. This extends bf16/f32 evidence beyond Qwen2.5 1.5B into
Qwen2.5 3B, Qwen2.5 Coder 3B, Qwen2.5 Coder 7B, and Phi 3.5 mini. Remaining
dtype work is now longer-duration burn-in, longer contexts, and other page
sizes.

The full dtype matrix was then rerun with `--iterations 2`,
`--host-memory-pressure-mib 1024`, `--require-stable-reclaim`,
`--require-stable-qc-bytes`, and `--max-elapsed-jitter-ratio 0.85`. The run at
`/private/tmp/qatq-live-vram-native-dtype-heavy-soak-2x-1g-20260624` passed
16/16 real Metal/MLX cases:

| case | runs | failures | elapsed min/max | reclaimable GPU min/max | QATQ min/max |
| --- | ---: | ---: | ---: | ---: | ---: |
| `qwen25-coder-3b-bf16-review-512p` | 2 | 0 | 9.94 / 10.08 s | 18.00 / 18.00 MiB | 18.00 / 18.00 MiB |
| `qwen25-coder-3b-f32-review-512p` | 2 | 0 | 10.93 / 11.16 s | 36.00 / 36.00 MiB | 45.35 / 45.35 MiB |
| `qwen25-3b-bf16-general-1025p-l12` | 2 | 0 | 14.64 / 14.77 s | 18.01 / 18.01 MiB | 37.43 / 37.43 MiB |
| `qwen25-3b-f32-general-1025p-l12` | 2 | 0 | 17.48 / 17.74 s | 36.02 / 36.02 MiB | 97.36 / 97.36 MiB |
| `qwen25-coder-7b-bf16-review-512p-aggregate` | 2 | 0 | 15.91 / 16.00 s | 24.00 / 24.00 MiB | 26.53 / 26.53 MiB |
| `qwen25-coder-7b-f32-review-512p-aggregate` | 2 | 0 | 16.23 / 16.24 s | 48.00 / 48.00 MiB | 66.29 / 66.29 MiB |
| `phi35-mini-bf16-ops-512p-aggregate` | 2 | 0 | 40.02 / 40.26 s | 192.00 / 192.00 MiB | 246.87 / 246.87 MiB |
| `phi35-mini-f32-ops-512p-aggregate` | 2 | 0 | 63.49 / 65.37 s | 384.00 / 384.00 MiB | 604.70 / 604.70 MiB |

The stability gates proved that reclaimable GPU bytes and raw/QATQ/zstd/lz4
byte totals stayed identical across repeated heavy dtype runs under 1 GiB of
page-touched host-memory pressure. This is stronger than the single dtype
matrix pass, but still shorter than an overnight or hour-scale burn-in.

## 2026-06-24 Native Page-Size Breadth

The installed patched llama.cpp binary was then rerun through a stricter native
page-size matrix with real Apple Metal execution, MLX `Device(gpu, 0)`, native
`ggml_segmented_kqv`, GPU page staging, strict live-paging gates, aggregate
codec gates, stable reclaim/codec-byte gates, and 1 GiB of page-touched
host-memory pressure. The fresh run at
`/private/tmp/qatq-live-vram-native-page-size-matrix-fresh-20260624` passed 5/5
cases:

| case | page tokens | selected KV GPU layers | exact restores | offloaded QATQ pages | resident pages | event trace | attention reads | MLX layers / heads | MLX stream/materialised | reclaimable GPU KV | QATQ | zstd | lz4 |
| --- | ---: | ---: | ---: | ---: | ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-15b-page256-deep` | 256 | 14 | 840 / 840 | 728 / 728 | 112 | pass / 3136 | 784 / 28 layers | 28 / 336 | 3.082 | 52.50 MiB | 82.95 MiB | 93.09 MiB | 101.15 MiB |
| `qwen25-15b-page2048-deep` | 2048 | 9 | 112 / 112 | 112 / 112 | 0 | pass / 448 | 784 / 28 layers | 28 / 336 | 1.226 | 17.25 MiB | 76.10 MiB | 92.81 MiB | 100.94 MiB |
| `qwen25-coder-3b-page256-deep` | 256 | 18 | 1152 / 1152 | 1008 / 1008 | 144 | pass / 4320 | 1008 / 36 layers | 36 / 576 | 2.950 | 72.00 MiB | 115.47 MiB | 129.38 MiB | 140.61 MiB |
| `qwen25-coder-3b-page2048-deep` | 2048 | 12 | 144 / 144 | 144 / 144 | 0 | pass / 576 | 1008 / 36 layers | 36 / 576 | 1.163 | 24.00 MiB | 105.63 MiB | 128.91 MiB | 140.36 MiB |
| `qwen25-3b-page1024-deep` | 1024 | 12 | 288 / 288 | 288 / 288 | 0 | pass / 1152 | 1008 / 36 layers | 36 / 576 | 1.681 | 72.00 MiB | 102.65 MiB | 122.81 MiB | 133.69 MiB |

This proves strict native page-size breadth across 256, 1024, and 2048-token
page configurations on the current local Qwen/Qwen-Coder fixture set. Every
case preserved deterministic continuation, restored every exported page
exactly, used zero raw pass-through pages, consumed bounded K/V page segments
through the native non-concat attention path, passed MLX page-streaming
equivalence, and beat raw, zstd, and lz4 in aggregate.

The same exploratory pass also kept two useful Phi hardening failures visible
instead of hiding them. A 2048-token Phi profile exceeded the configured peak
page-KV residency ratio, and a 1024-token Phi profile exposed a scheduler trace
mismatch where the runtime offloaded a page that the evidence kept resident.
Those are adapter-hardening items, not codec exactness failures.

QATQ can now claim experimental native llama.cpp live-VRAM proofs across
Qwen2.5 1.5B, Qwen2.5 Coder 3B, Qwen2.5 Coder 7B, and Phi 3.5 mini fixtures.
It should still avoid a production-complete live VRAM claim until the same gates
are broadened across more model families, prompts, context lengths, dtypes,
page sizes, adverse memory-pressure cases, and repeated latency runs.

## 2026-06-24 Fresh Installed llama.cpp / MLX GPU Rerun

The installed patched llama.cpp checkout at
`/private/tmp/qatq-llama.cpp` was verified at commit
`7992aa7c8e21ea2eb7a5e4802da56eec7b376036` with the QATQ adapter patch applied.
MLX was verified under unsandboxed host execution on `Device(gpu, 0)`. The
current QATQ release binaries were rebuilt with `cargo build --release --locked
--bins`, then the native live-VRAM matrices were rerun with real Apple Metal
execution, MLX GPU page-streaming equivalence, native `ggml_segmented_kqv`,
GPU page staging, strict live-paging gates, aggregate codec gates, bounded
restore-slot pressure rejection, and 1 GiB of page-touched host-memory pressure.

The native breadth matrix at
`/private/tmp/qatq-live-vram-native-breadth-fresh-20260624` passed 3/3 cases:

| case | exact restores | offloaded QATQ pages | resident pages | pass-through | MLX layers / heads | MLX stream/materialised | reclaimable GPU KV | QATQ | zstd | lz4 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-15b-daily-native-512p-aggregate` | 168 / 168 | 112 / 112 | 56 | 0 | 28 / 336 | 1.987 | 17.50 MiB | 25.03 MiB | 28.65 MiB | 31.12 MiB |
| `qwen25-3b-general-native-1025p-l12-aggregate` | 216 / 216 | 216 / 216 | 0 | 0 | 36 / 576 | 1.398 | 24.07 MiB | 82.22 MiB | 98.22 MiB | 106.95 MiB |
| `phi35-mini-ops-native-512p-aggregate` | 192 / 192 | 128 / 128 | 64 | 0 | 32 / 1024 | 1.283 | 288.00 MiB | 391.64 MiB | 448.77 MiB | 493.38 MiB |

The sustained coding-agent matrix at
`/private/tmp/qatq-live-vram-native-sustained-fresh-20260624` passed 4/4 cases:

| case | exact restores | offloaded QATQ pages | resident pages | pass-through | MLX layers / heads | MLX stream/materialised | reclaimable GPU KV | QATQ | zstd | lz4 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-coder-3b-review-512p` | 216 / 216 | 144 / 144 | 72 | 0 | 36 / 576 | 1.917 | 22.50 MiB | 34.92 MiB | 40.03 MiB | 43.51 MiB |
| `qwen25-coder-3b-memory-1024p` | 144 / 144 | 144 / 144 | 0 | 0 | 36 / 576 | 1.458 | 18.00 MiB | 51.52 MiB | 60.28 MiB | 65.49 MiB |
| `qwen25-coder-7b-review-512p-aggregate` | 112 / 112 | 56 / 56 | 56 | 0 | 28 / 784 | 1.540 | 32.00 MiB | 38.17 MiB | 43.04 MiB | 46.68 MiB |
| `qwen25-coder-7b-memory-512p-aggregate` | 168 / 168 | 112 / 112 | 56 | 0 | 28 / 784 | 1.792 | 40.00 MiB | 52.27 MiB | 58.97 MiB | 63.98 MiB |

The native dtype matrix at
`/private/tmp/qatq-live-vram-native-dtype-fresh-20260624` passed 8/8 cases:

| case | dtype | exact restores | offloaded QATQ pages | resident pages | pass-through | MLX layers / heads | reclaimable GPU KV | QATQ | zstd | lz4 |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-coder-3b-bf16-review-512p` | bf16 | 144 / 144 | 72 / 72 | 72 | 0 | 36 / 576 | 18.00 MiB | 18.00 MiB | 21.51 MiB | 27.59 MiB |
| `qwen25-coder-3b-f32-review-512p` | f32 | 144 / 144 | 72 / 72 | 72 | 0 | 36 / 576 | 36.00 MiB | 45.35 MiB | 51.13 MiB | 55.26 MiB |
| `qwen25-3b-bf16-general-1025p-l12` | bf16 | 144 / 144 | 144 / 144 | 0 | 0 | 36 / 576 | 18.01 MiB | 37.43 MiB | 46.81 MiB | 60.37 MiB |
| `qwen25-3b-f32-general-1025p-l12` | f32 | 144 / 144 | 144 / 144 | 0 | 0 | 36 / 576 | 36.02 MiB | 97.36 MiB | 111.95 MiB | 120.97 MiB |
| `qwen25-coder-7b-bf16-review-512p-aggregate` | bf16 | 112 / 112 | 56 / 56 | 56 | 0 | 28 / 784 | 24.00 MiB | 26.53 MiB | 31.21 MiB | 39.97 MiB |
| `qwen25-coder-7b-f32-review-512p-aggregate` | f32 | 112 / 112 | 56 / 56 | 56 | 0 | 28 / 784 | 48.00 MiB | 66.29 MiB | 74.13 MiB | 81.16 MiB |
| `phi35-mini-bf16-ops-512p-aggregate` | bf16 | 128 / 128 | 64 / 64 | 64 | 0 | 32 / 1024 | 192.00 MiB | 246.87 MiB | 291.36 MiB | 379.16 MiB |
| `phi35-mini-f32-ops-512p-aggregate` | f32 | 128 / 128 | 64 / 64 | 64 | 0 | 32 / 1024 | 384.00 MiB | 604.70 MiB | 676.53 MiB | 740.82 MiB |

Across the fresh installed-runtimes rerun, 15/15 real Metal/MLX cases passed.
Every case preserved deterministic continuation, restored every exported page
exactly, used zero raw pass-through pages, passed strict live-paging validation,
consumed bounded K/V page segments through the native non-concat attention path,
passed MLX page-streaming equivalence, and beat raw, zstd, and lz4 on the same
page boundaries.

## 2026-06-24 Sealed Strict Evidence Smoke

After making strict live-paging evidence require keyed metadata seals, the
native breadth runner was rerun through one real Apple Metal/MLX case with
`--require-live-paging`, native `ggml_segmented_kqv`, GPU page staging,
aggregate codec gates, and 512 MiB of page-touched host-memory pressure. The
runner generated a fresh per-run `--live-vram-page-seal-key-hex` and did not
write the secret into the evidence bundle.

The same seal mechanism is now also required by the production-shaped
runtime-reclaim fallback evidence path through `--live-vram-require-page-seals`.

The sealed smoke at `/private/tmp/qatq-live-vram-sealed-smoke-20260624` passed
1/1 cases:

| case | exact restores | offloaded QATQ pages | resident pages | sealed pages | pass-through | MLX layers / heads | reclaimable GPU KV | QATQ | zstd | lz4 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `qwen25-15b-daily-native-512p-aggregate` | 168 / 168 | 112 / 112 | 56 | 112 | 0 | 28 / 336 | 17.50 MiB | 25.03 MiB | 28.65 MiB | 31.12 MiB |

The generated `live-paging-evidence.json` contained `sealed_pages: 112`; all
112 offloaded pages carried `metadata_seal` objects, and zero resident pages
carried seals. This verifies the end-to-end sealed evidence path through the
installed patched llama.cpp binary, QATQ release binary, and MLX GPU
page-streaming gate.
