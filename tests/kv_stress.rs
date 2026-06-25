use std::{
    collections::BTreeMap,
    sync::{Arc, Barrier, Mutex},
    thread,
    time::{Duration, Instant},
};

use qatq::{
    FixedWindowLiveVramScheduler, KvPageDescriptor, KvPageKey, KvPageKind, KvPageLayout,
    KvPageSnapshot, LIVE_VRAM_ADAPTER_CONTRACT_VERSION, LiveVramAdapterError,
    LiveVramAdapterIdentity, LiveVramAdapterMetrics, LiveVramCancellationStage, LiveVramLimits,
    LiveVramOffloadOutcome, LiveVramOffloadStore, LiveVramPageEncodeResult, LiveVramPageSealPolicy,
    LiveVramRestoreLatencyBudget, LiveVramRestoreStatus, LiveVramRuntimeAdapter,
    LiveVramSchedulerPolicy, LiveVramSchedulerState, Phase1Config, QatcDecodeLimits,
    QatqExactStrategy, TensorDType, cancel_live_vram_offload, decode, decode_qatq_exact,
    decode_qatq_exact_container, decode_qatq_exact_container_with_limits,
    encode_qatq_exact_container_with_config, live_vram_page_checksum, qatq_exact_strategy,
    restore_live_vram_page, restore_production_chunk, try_encode_live_vram_page,
    try_encode_production_chunk_with_config, try_encode_qatq_exact_exhaustive_with_config,
    try_encode_qatq_exact_with_config, try_offload_live_vram_page,
    try_restore_live_vram_page_from_store_with_observed_latency,
};

const DEFAULT_CASES: usize = 4_096;
const EXHAUSTIVE_CASES: usize = 512;
const LARGE_VALUE_LIMIT: usize = 8_192;
const DEFAULT_LIVE_VRAM_CORRUPTION_CASES: usize = 2_048;
const DEFAULT_LIVE_VRAM_ISOLATION_CASES: usize = 2_048;
const DEFAULT_LIVE_VRAM_MULTI_SEQUENCE_CASES: usize = 2_048;
const DEFAULT_LIVE_VRAM_CPU_BUDGET_CASES: usize = 2_048;
const DEFAULT_LIVE_VRAM_RUNTIME_CONCURRENCY_CASES: usize = 2_048;
const DEFAULT_LIVE_VRAM_RUNTIME_RACE_CASES: usize = 1_024;
const DEBUG_DECODE_NS_PER_VALUE_CEILING: f64 = 2_500.0;
const RELEASE_DECODE_NS_PER_VALUE_CEILING: f64 = 150.0;

#[test]
#[ignore = "runs thousands of deterministic KV-cache codec stress cases"]
fn exact_kv_cache_stress_matrix() {
    let case_count = std::env::var("QATQ_KV_STRESS_CASES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_CASES);
    assert!(
        case_count >= 1_000,
        "stress proof must cover at least 1000 cases"
    );

    let mut summary = StressSummary::default();
    let mut strategy_counts = BTreeMap::<&'static str, usize>::new();
    let mut storage_counts = BTreeMap::<&'static str, usize>::new();

    for case_index in 0..case_count {
        let spec = KvSpec::for_index(case_index);
        let values = spec.generate();
        let config = Phase1Config {
            seed: 0x5141_5451_0000_0000 ^ mix64(case_index as u64),
        };
        let raw_len = values.len() * 4;

        let encode_start = Instant::now();
        let encoded = try_encode_qatq_exact_with_config(&values, config).unwrap();
        summary.encode_time += encode_start.elapsed();
        summary.encoded_bytes += encoded.len();
        summary.raw_bytes += raw_len;

        let strategy = qatq_exact_strategy(&encoded).unwrap();
        *strategy_counts.entry(strategy.as_str()).or_default() += 1;

        let decode_start = Instant::now();
        let decoded = decode_qatq_exact(&encoded).unwrap();
        summary.decode_time += decode_start.elapsed();
        assert_same_bits(&values, &decoded, case_index, "single-payload");

        let decoded_via_dispatch = decode(&encoded).unwrap();
        assert_same_bits(&values, &decoded_via_dispatch, case_index, "dispatch");

        let decision = try_encode_production_chunk_with_config(&values, config).unwrap();
        *storage_counts
            .entry(decision.metadata.storage_label())
            .or_default() += 1;
        let restored =
            restore_production_chunk(&decision.metadata, decision.stored_bytes()).unwrap();
        assert_same_bits(&values, &restored, case_index, "production-chunk");

        let chunk_size = spec.chunk_size();
        let container =
            encode_qatq_exact_container_with_config(&values, chunk_size, config).unwrap();
        summary.container_bytes += container.len();
        let decoded_container = decode_qatq_exact_container(&container).unwrap();
        assert_same_bits(&values, &decoded_container, case_index, "container");
        let decoded_container_via_dispatch = decode(&container).unwrap();
        assert_same_bits(
            &values,
            &decoded_container_via_dispatch,
            case_index,
            "container-dispatch",
        );

        if case_index < EXHAUSTIVE_CASES && values.len() <= 2_048 {
            let exhaustive = try_encode_qatq_exact_exhaustive_with_config(&values, config).unwrap();
            let decoded_exhaustive = decode_qatq_exact(&exhaustive).unwrap();
            assert_same_bits(&values, &decoded_exhaustive, case_index, "exhaustive");
            assert!(
                encoded.len() <= exhaustive.len(),
                "case {case_index} default encoder produced {} bytes but exhaustive produced {} bytes",
                encoded.len(),
                exhaustive.len()
            );
        }

        if case_index % 7 == 0 && encoded.len() > 40 {
            assert_mutation_rejected(&encoded, case_index, "exact-payload");
        }
        if case_index % 11 == 0 && container.len() > 48 {
            assert_mutation_rejected(&container, case_index, "qatc-container");
        }
        if case_index % 13 == 0 && !values.is_empty() {
            let limits = QatcDecodeLimits {
                max_total_values: values.len() - 1,
                ..QatcDecodeLimits::default()
            };
            assert!(
                decode_qatq_exact_container_with_limits(&container, limits).is_err(),
                "case {case_index} accepted a container above total-value decode limit"
            );
        }

        summary.values += values.len();
        summary.cases += 1;
        summary.max_values = summary.max_values.max(values.len());
        summary.max_encoded_bytes = summary.max_encoded_bytes.max(encoded.len());
        summary.best_ratio = summary
            .best_ratio
            .min(encoded.len() as f64 / raw_len.max(1) as f64);
        summary.worst_ratio = summary
            .worst_ratio
            .max(encoded.len() as f64 / raw_len.max(1) as f64);
    }

    assert!(
        strategy_counts.len() >= 4,
        "stress corpus did not exercise enough QATQ exact strategies: {strategy_counts:?}"
    );
    assert!(
        strategy_counts.contains_key(QatqExactStrategy::QuaternionChainZstd.as_str()),
        "stress corpus never selected reversible quaternion-chain zstd"
    );
    assert!(
        strategy_counts.contains_key(QatqExactStrategy::BytePlaneZstd.as_str()),
        "stress corpus never selected byte-plane zstd"
    );
    assert!(
        strategy_counts.contains_key(QatqExactStrategy::RawBits.as_str()),
        "stress corpus never selected raw-bits fallback"
    );
    assert!(
        storage_counts.contains_key("qatq-exact")
            && storage_counts.contains_key("raw-f32le-pass-through"),
        "stress corpus did not cover both production storage decisions: {storage_counts:?}"
    );

    let decode_ns_per_value = summary.decode_time.as_nanos() as f64 / summary.values.max(1) as f64;
    let ceiling = if cfg!(debug_assertions) {
        DEBUG_DECODE_NS_PER_VALUE_CEILING
    } else {
        RELEASE_DECODE_NS_PER_VALUE_CEILING
    };
    assert!(
        decode_ns_per_value <= ceiling,
        "decode throughput regressed: {decode_ns_per_value:.2}ns/value > {ceiling:.2}ns/value"
    );

    eprintln!(
        "qatq kv stress: cases={} values={} max_values={} raw_bytes={} encoded_bytes={} container_bytes={} avg_ratio={:.4} best_ratio={:.4} worst_ratio={:.4} encode_ns/value={:.2} decode_ns/value={:.2} strategies={:?} storage={:?}",
        summary.cases,
        summary.values,
        summary.max_values,
        summary.raw_bytes,
        summary.encoded_bytes,
        summary.container_bytes,
        summary.encoded_bytes as f64 / summary.raw_bytes.max(1) as f64,
        summary.best_ratio,
        summary.worst_ratio,
        summary.encode_time.as_nanos() as f64 / summary.values.max(1) as f64,
        decode_ns_per_value,
        strategy_counts,
        storage_counts
    );
}

#[test]
#[ignore = "runs deterministic live-VRAM corruption injection rates across thousands of pages"]
fn live_vram_corruption_injection_rates_fail_closed() {
    let case_count = std::env::var("QATQ_LIVE_VRAM_CORRUPTION_CASES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_LIVE_VRAM_CORRUPTION_CASES);
    assert!(
        case_count >= 1_000,
        "corruption proof must cover at least 1000 live-VRAM pages"
    );

    let rates = [
        CorruptionRate {
            label: "0.01%",
            numerator: 1,
            denominator: 10_000,
        },
        CorruptionRate {
            label: "0.1%",
            numerator: 1,
            denominator: 1_000,
        },
        CorruptionRate {
            label: "1%",
            numerator: 1,
            denominator: 100,
        },
    ];
    let limits = LiveVramLimits::default();
    let mut summary = LiveVramCorruptionSummary::default();

    for case_index in 0..case_count {
        let snapshot = live_vram_stress_snapshot(case_index);
        let encoded = try_encode_live_vram_page(&snapshot, limits)
            .expect("live-VRAM stress page should encode");
        let clean_restore = restore_live_vram_page(&encoded.metadata, &encoded.bytes, limits)
            .expect("clean live-VRAM stress page should restore");
        assert_eq!(
            clean_restore, snapshot.bytes_le,
            "case {case_index} clean restore changed bytes"
        );

        summary.pages += 1;
        summary.raw_bytes += snapshot.bytes_le.len();
        summary.stored_bytes += encoded.bytes.len();
        match encoded.metadata.storage_label() {
            "qatq-live" => summary.qatq_pages += 1,
            "raw-typed-pass-through" => summary.raw_pages += 1,
            label => panic!("case {case_index} unexpected live-VRAM storage label {label}"),
        }

        for rate in rates {
            let mut corrupted = encoded.bytes.clone();
            let mutations =
                inject_corruption(&mut corrupted, rate, mix64(case_index as u64 ^ rate.salt()));
            let first_diff = encoded
                .bytes
                .iter()
                .zip(&corrupted)
                .position(|(left, right)| left != right)
                .unwrap_or(usize::MAX);
            assert!(
                mutations > 0,
                "case {case_index} rate {} did not mutate payload",
                rate.label
            );
            assert!(
                restore_live_vram_page(&encoded.metadata, &corrupted, limits).is_err(),
                "case {case_index} accepted {} corruption across {mutations} stored byte(s); storage={} strategy={:?} stored_len={} first_diff={first_diff}",
                rate.label,
                encoded.metadata.storage_label(),
                encoded.metadata.strategy,
                encoded.bytes.len()
            );
            summary.corruptions += mutations;
        }
    }

    assert!(
        summary.qatq_pages > 0 && summary.raw_pages > 0,
        "corruption corpus must cover both QATQ and raw live-VRAM storage decisions: {summary:?}"
    );

    eprintln!(
        "qatq live-vram corruption stress: pages={} qatq_pages={} raw_pages={} raw_bytes={} stored_bytes={} injected_mutations={}",
        summary.pages,
        summary.qatq_pages,
        summary.raw_pages,
        summary.raw_bytes,
        summary.stored_bytes,
        summary.corruptions
    );
}

#[test]
#[ignore = "runs deterministic live-VRAM cross-request key isolation across thousands of pages"]
fn live_vram_cross_request_key_isolation_stress() {
    let case_count = std::env::var("QATQ_LIVE_VRAM_ISOLATION_CASES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_LIVE_VRAM_ISOLATION_CASES);
    assert!(
        case_count >= 1_000,
        "isolation proof must cover at least 1000 live-VRAM pages"
    );

    let limits = LiveVramLimits::default();
    let mut store = LiveVramOffloadStore::new_with_shadow_validation(
        limits,
        case_count,
        usize::MAX / 4,
        usize::MAX / 4,
    );
    let mut summary = LiveVramIsolationSummary::default();

    for case_index in 0..case_count {
        let snapshot = isolated_live_vram_stress_snapshot(case_index);
        let key = store
            .commit_snapshot(&snapshot)
            .expect("isolated live-VRAM stress page should commit");
        assert_eq!(key, KvPageKey::from_descriptor(&snapshot.descriptor));

        for forged in forged_neighbor_keys(&key) {
            assert!(
                !store.contains_key(&forged),
                "case {case_index} forged key unexpectedly matched committed entry: {forged:?}"
            );
            assert!(
                store.restore(&forged).is_err(),
                "case {case_index} restored page through forged key: {forged:?}"
            );
            summary.forged_restore_rejections += 1;
        }

        let restored = store
            .restore(&key)
            .expect("legitimate key should still restore after forged attempts");
        assert_eq!(
            restored, snapshot.bytes_le,
            "case {case_index} legitimate restore changed bytes after forged attempts"
        );
        summary.pages += 1;
        summary.raw_bytes += snapshot.bytes_le.len();
    }

    assert_eq!(store.len(), case_count);
    assert_eq!(
        store.metrics().restore_failures,
        summary.forged_restore_rejections
    );

    eprintln!(
        "qatq live-vram isolation stress: pages={} forged_restore_rejections={} raw_bytes={} pending_pages={}",
        summary.pages,
        summary.forged_restore_rejections,
        summary.raw_bytes,
        store.metrics().pending_pages
    );
}

#[test]
#[ignore = "runs deterministic sealed multi-sequence live-VRAM interleaving across thousands of pages"]
fn live_vram_sealed_multi_sequence_interleaving_stress() {
    let case_count = std::env::var("QATQ_LIVE_VRAM_MULTI_SEQUENCE_CASES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_LIVE_VRAM_MULTI_SEQUENCE_CASES);
    assert!(
        case_count >= 1_000,
        "multi-sequence proof must cover at least 1000 live-VRAM pages"
    );

    let worker_count = std::env::var("QATQ_LIVE_VRAM_MULTI_SEQUENCE_WORKERS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(8)
        .clamp(2, 32);
    let limits = LiveVramLimits::default();
    let store = Arc::new(Mutex::new(
        LiveVramOffloadStore::new_with_shadow_validation(
            limits,
            case_count,
            usize::MAX / 4,
            usize::MAX / 4,
        )
        .with_page_seal_policy(live_vram_stress_seal_policy()),
    ));

    let handles: Vec<_> = (0..worker_count)
        .map(|worker| {
            let store = Arc::clone(&store);
            thread::spawn(move || {
                let mut summary = LiveVramMultiSequenceSummary::default();
                for case_index in (worker..case_count).step_by(worker_count) {
                    let snapshot = multi_sequence_live_vram_stress_snapshot(case_index);
                    let key = {
                        let mut store = store.lock().expect("store mutex poisoned");
                        let key = store
                            .commit_snapshot(&snapshot)
                            .expect("multi-sequence page should commit");
                        assert_eq!(key, KvPageKey::from_descriptor(&snapshot.descriptor));
                        assert!(
                            store
                                .entry(&key)
                                .and_then(|entry| entry.metadata_seal.as_ref())
                                .is_some(),
                            "case {case_index} committed without a metadata seal"
                        );
                        key
                    };

                    for forged in forged_neighbor_keys(&key) {
                        let rejected = store
                            .lock()
                            .expect("store mutex poisoned")
                            .restore(&forged)
                            .is_err();
                        assert!(
                            rejected,
                            "case {case_index} restored page through forged key: {forged:?}"
                        );
                        summary.forged_restore_rejections += 1;
                    }

                    match case_index % 4 {
                        0 => {
                            let restored = store
                                .lock()
                                .expect("store mutex poisoned")
                                .restore_and_remove(&key)
                                .expect("sealed restore-and-remove should succeed");
                            assert_eq!(restored, snapshot.bytes_le);
                            summary.restored_and_removed += 1;
                        }
                        1 => {
                            let restored = store
                                .lock()
                                .expect("store mutex poisoned")
                                .restore(&key)
                                .expect("sealed restore should succeed");
                            assert_eq!(restored, snapshot.bytes_le);
                            summary.restored_and_kept += 1;
                        }
                        2 => {
                            let removed = store.lock().expect("store mutex poisoned").remove(&key);
                            assert!(
                                removed.is_some(),
                                "case {case_index} cancellation could not remove pending page"
                            );
                            assert!(
                                store
                                    .lock()
                                    .expect("store mutex poisoned")
                                    .restore(&key)
                                    .is_err(),
                                "case {case_index} restored cancelled page"
                            );
                            summary.cancelled_before_restore += 1;
                            summary.cancelled_restore_rejections += 1;
                        }
                        _ => {
                            let restored = store
                                .lock()
                                .expect("store mutex poisoned")
                                .restore(&key)
                                .expect("sealed restore should succeed before later cancellation");
                            assert_eq!(restored, snapshot.bytes_le);
                            let removed = store.lock().expect("store mutex poisoned").remove(&key);
                            assert!(
                                removed.is_some(),
                                "case {case_index} post-restore cancellation could not remove page"
                            );
                            summary.restored_then_cancelled += 1;
                        }
                    }

                    summary.pages += 1;
                    summary.raw_bytes += snapshot.bytes_le.len();
                }
                summary
            })
        })
        .collect();

    let mut summary = LiveVramMultiSequenceSummary::default();
    for handle in handles {
        summary += handle.join().expect("multi-sequence worker panicked");
    }

    assert_eq!(summary.pages, case_count);
    assert!(summary.restored_and_removed > 0);
    assert!(summary.restored_and_kept > 0);
    assert!(summary.cancelled_before_restore > 0);
    assert!(summary.restored_then_cancelled > 0);
    assert_eq!(summary.forged_restore_rejections, case_count * 7);
    let store = store.lock().expect("store mutex poisoned");
    let expected_pending = summary.restored_and_kept;
    assert_eq!(store.len(), expected_pending);
    assert_eq!(store.metrics().pending_pages, expected_pending);
    assert_eq!(
        store.metrics().restore_failures,
        summary.forged_restore_rejections + summary.cancelled_restore_rejections
    );

    eprintln!(
        "qatq live-vram sealed multi-sequence stress: pages={} workers={} restored_removed={} restored_kept={} cancelled={} restored_then_cancelled={} forged_restore_rejections={} cancelled_restore_rejections={} pending_pages={} raw_bytes={}",
        summary.pages,
        worker_count,
        summary.restored_and_removed,
        summary.restored_and_kept,
        summary.cancelled_before_restore,
        summary.restored_then_cancelled,
        summary.forged_restore_rejections,
        summary.cancelled_restore_rejections,
        store.metrics().pending_pages,
        summary.raw_bytes
    );
}

#[test]
#[ignore = "runs deterministic sealed CPU-tier budget pressure across thousands of live-VRAM pages"]
fn live_vram_sealed_cpu_tier_budget_pressure_stress() {
    let case_count = std::env::var("QATQ_LIVE_VRAM_CPU_BUDGET_CASES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_LIVE_VRAM_CPU_BUDGET_CASES);
    assert!(
        case_count >= 1_000,
        "CPU-tier budget proof must cover at least 1000 live-VRAM pages"
    );

    let limits = LiveVramLimits::default();
    let snapshots: Vec<_> = (0..case_count)
        .map(multi_sequence_live_vram_stress_snapshot)
        .collect();
    let encoded_lens: Vec<_> = snapshots
        .iter()
        .map(|snapshot| {
            try_encode_live_vram_page(snapshot, limits)
                .expect("budget stress page should encode")
                .bytes
                .len()
        })
        .collect();
    let total_stored_bytes = encoded_lens.iter().copied().sum::<usize>();
    let max_cpu_stored_bytes = total_stored_bytes / 2;
    assert!(max_cpu_stored_bytes > 0);

    let mut store = LiveVramOffloadStore::new_with_shadow_validation(
        limits,
        case_count,
        max_cpu_stored_bytes,
        usize::MAX / 4,
    )
    .with_page_seal_policy(live_vram_stress_seal_policy());
    let mut accepted = Vec::<(KvPageKey, Vec<u8>)>::new();
    let mut summary = LiveVramBudgetPressureSummary {
        attempted_pages: case_count,
        total_candidate_stored_bytes: total_stored_bytes,
        max_cpu_stored_bytes,
        ..LiveVramBudgetPressureSummary::default()
    };

    for (case_index, snapshot) in snapshots.iter().enumerate() {
        let before = store.metrics();
        match store.commit_snapshot(snapshot) {
            Ok(key) => {
                assert_eq!(key, KvPageKey::from_descriptor(&snapshot.descriptor));
                let entry = store.entry(&key).expect("accepted page should be in store");
                assert!(
                    entry.metadata_seal.is_some(),
                    "case {case_index} committed without metadata seal"
                );
                assert!(store.metrics().cpu_stored_bytes <= max_cpu_stored_bytes);
                summary.accepted_pages += 1;
                summary.accepted_raw_bytes += snapshot.bytes_le.len();
                summary.accepted_stored_bytes = store.metrics().cpu_stored_bytes;
                accepted.push((key, snapshot.bytes_le.clone()));
            }
            Err(error) => {
                assert!(
                    matches!(
                        error,
                        qatq::QatqError::ContainerLimitExceeded("live-vram-offload-cpu-bytes")
                    ),
                    "case {case_index} failed with unexpected error: {error:?}"
                );
                let after = store.metrics();
                assert_eq!(after.cpu_stored_bytes, before.cpu_stored_bytes);
                assert_eq!(after.pending_pages, before.pending_pages);
                assert_eq!(after.shadow_cpu_bytes, before.shadow_cpu_bytes);
                summary.rejected_pages += 1;
            }
        }
    }

    assert!(
        summary.accepted_pages > 0 && summary.rejected_pages > 0,
        "budget pressure corpus must accept and reject pages: {summary:?}"
    );
    assert_eq!(store.metrics().pending_pages, accepted.len());
    assert_eq!(store.metrics().encode_failures, summary.rejected_pages);
    assert!(store.metrics().cpu_stored_bytes <= max_cpu_stored_bytes);

    for (key, expected) in accepted.into_iter().rev() {
        let restored = store
            .restore_and_remove(&key)
            .expect("accepted sealed page should restore under budget pressure");
        assert_eq!(restored, expected);
        summary.restored_pages += 1;
    }

    assert!(store.is_empty());
    assert_eq!(store.metrics().pending_pages, 0);
    assert_eq!(store.metrics().cpu_stored_bytes, 0);
    assert_eq!(store.metrics().shadow_cpu_bytes, 0);
    assert_eq!(summary.restored_pages, summary.accepted_pages);

    eprintln!(
        "qatq live-vram sealed CPU-tier budget stress: attempted={} accepted={} rejected={} restored={} total_candidate_stored_bytes={} max_cpu_stored_bytes={} accepted_stored_bytes={} accepted_raw_bytes={}",
        summary.attempted_pages,
        summary.accepted_pages,
        summary.rejected_pages,
        summary.restored_pages,
        summary.total_candidate_stored_bytes,
        summary.max_cpu_stored_bytes,
        summary.accepted_stored_bytes,
        summary.accepted_raw_bytes
    );
}

#[test]
#[ignore = "runs deterministic runtime-adapter controller concurrency across thousands of live-VRAM pages"]
fn live_vram_runtime_adapter_concurrency_stress() {
    let case_count = std::env::var("QATQ_LIVE_VRAM_RUNTIME_CONCURRENCY_CASES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_LIVE_VRAM_RUNTIME_CONCURRENCY_CASES);
    assert!(
        case_count >= 1_000,
        "runtime-adapter concurrency proof must cover at least 1000 live-VRAM pages"
    );

    let worker_count = std::env::var("QATQ_LIVE_VRAM_RUNTIME_CONCURRENCY_WORKERS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(8)
        .clamp(2, 32);
    let limits = LiveVramLimits::default();
    let snapshots: Vec<_> = (0..case_count)
        .map(multi_sequence_live_vram_stress_snapshot)
        .collect();
    let harness = Arc::new(Mutex::new(RuntimeConcurrencyHarness {
        adapter: RuntimeConcurrencyAdapter::new(snapshots.clone()),
        store: LiveVramOffloadStore::new_with_shadow_validation(
            limits,
            case_count,
            usize::MAX / 4,
            usize::MAX / 4,
        )
        .with_page_seal_policy(live_vram_stress_seal_policy()),
    }));
    let scheduler = FixedWindowLiveVramScheduler {
        policy: LiveVramSchedulerPolicy {
            hot_window_tokens: 0,
            prefetch_window_tokens: 0,
            max_queued_pages: case_count,
            max_cpu_stored_bytes: usize::MAX / 4,
            require_qatq_beats_best_general_codec: false,
        },
    };

    let handles: Vec<_> = (0..worker_count)
        .map(|worker| {
            let harness = Arc::clone(&harness);
            let snapshots = snapshots.clone();
            thread::spawn(move || {
                let mut summary = LiveVramRuntimeConcurrencySummary::default();
                for case_index in (worker..case_count).step_by(worker_count) {
                    let snapshot = &snapshots[case_index];
                    let mut harness = harness.lock().expect("runtime harness mutex poisoned");
                    match case_index % 5 {
                        0 => {
                            let key = harness.offload(snapshot, &scheduler, limits);
                            summary.offloaded += 1;
                            let restored =
                                harness.restore_with_latency(&key, limits, 500, 1_000, false);
                            assert_eq!(restored, snapshot.bytes_le.len());
                            summary.restored += 1;
                        }
                        1 => {
                            let key = harness.offload(snapshot, &scheduler, limits);
                            summary.offloaded += 1;
                            let cancelled = harness
                                .cancel_after_runtime_commit(&key, limits)
                                .expect("after-commit cancellation should restore page");
                            assert!(matches!(
                                cancelled,
                                qatq::LiveVramCancellationOutcome::RestoredCommitted { .. }
                            ));
                            summary.cancelled_after_commit += 1;
                        }
                        2 => {
                            let pending_key = harness
                                .store
                                .commit_snapshot(snapshot)
                                .expect("before-commit snapshot should stage");
                            let cancelled = harness
                                .cancel_before_runtime_commit(&pending_key, limits)
                                .expect("before-commit cancellation should drop staged page");
                            assert!(matches!(
                                cancelled,
                                qatq::LiveVramCancellationOutcome::DroppedUncommitted { .. }
                            ));
                            summary.cancelled_before_commit += 1;
                        }
                        3 => {
                            let key = harness.offload(snapshot, &scheduler, limits);
                            summary.offloaded += 1;
                            harness.assert_duplicate_offload_rejected(snapshot, &scheduler, limits);
                            summary.duplicate_offload_rejections += 1;
                            let restored =
                                harness.restore_with_latency(&key, limits, 2_000, 1_000, true);
                            assert_eq!(restored, snapshot.bytes_le.len());
                            summary.restored += 1;
                            summary.restore_stalls += 1;
                        }
                        _ => {
                            let key = harness.offload(snapshot, &scheduler, limits);
                            summary.offloaded += 1;
                            for forged in forged_neighbor_keys(&key) {
                                assert!(
                                    harness.store.restore(&forged).is_err(),
                                    "forged key restored through runtime harness: {forged:?}"
                                );
                                summary.forged_restore_rejections += 1;
                            }
                            let cancelled = harness
                                .cancel_after_runtime_commit(&key, limits)
                                .expect("after forged attempts, real cancellation should restore");
                            assert!(matches!(
                                cancelled,
                                qatq::LiveVramCancellationOutcome::RestoredCommitted { .. }
                            ));
                            summary.cancelled_after_commit += 1;
                        }
                    }

                    let key = KvPageKey::from_descriptor(&snapshot.descriptor);
                    harness.assert_page_consistent(&key);
                    summary.pages += 1;
                    summary.raw_bytes += snapshot.bytes_le.len();
                }
                summary
            })
        })
        .collect();

    let mut summary = LiveVramRuntimeConcurrencySummary::default();
    for handle in handles {
        summary += handle.join().expect("runtime concurrency worker panicked");
    }

    assert_eq!(summary.pages, case_count);
    assert!(summary.offloaded > 0);
    assert!(summary.restored > 0);
    assert!(summary.cancelled_before_commit > 0);
    assert!(summary.cancelled_after_commit > 0);
    assert!(summary.duplicate_offload_rejections > 0);
    assert!(summary.forged_restore_rejections > 0);
    assert!(summary.restore_stalls > 0);
    let harness = harness.lock().expect("runtime harness mutex poisoned");
    assert_eq!(harness.store.len(), 0);
    assert_eq!(harness.store.metrics().pending_pages, 0);
    assert_eq!(
        harness.store.metrics().restore_stalls,
        summary.restore_stalls
    );
    assert_eq!(
        harness.store.metrics().restore_failures,
        summary.forged_restore_rejections
    );
    assert_eq!(harness.adapter.metrics().unwrap().offloaded_pages, 0);
    assert_eq!(
        harness.adapter.metrics().unwrap().resident_pages,
        case_count
    );

    eprintln!(
        "qatq live-vram runtime-adapter concurrency stress: pages={} workers={} offloaded={} restored={} cancel_before={} cancel_after={} duplicate_rejections={} forged_restore_rejections={} restore_stalls={} raw_bytes={}",
        summary.pages,
        worker_count,
        summary.offloaded,
        summary.restored,
        summary.cancelled_before_commit,
        summary.cancelled_after_commit,
        summary.duplicate_offload_rejections,
        summary.forged_restore_rejections,
        summary.restore_stalls,
        summary.raw_bytes
    );
}

#[test]
#[ignore = "runs deterministic shared-runtime restore/cancel races across thousands of live-VRAM pages"]
fn live_vram_runtime_restore_cancel_race_stress() {
    let case_count = std::env::var("QATQ_LIVE_VRAM_RUNTIME_RACE_CASES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_LIVE_VRAM_RUNTIME_RACE_CASES);
    assert!(
        case_count >= 1_000,
        "runtime restore/cancel race proof must cover at least 1000 live-VRAM pages"
    );

    let limits = LiveVramLimits::default();
    let snapshots: Vec<_> = (0..case_count)
        .map(multi_sequence_live_vram_stress_snapshot)
        .collect();
    let adapter = Arc::new(Mutex::new(RuntimeConcurrencyAdapter::new(
        snapshots.clone(),
    )));
    let store = Arc::new(Mutex::new(
        LiveVramOffloadStore::new_with_shadow_validation(
            limits,
            case_count,
            usize::MAX / 4,
            usize::MAX / 4,
        )
        .with_page_seal_policy(live_vram_stress_seal_policy()),
    ));
    let scheduler = FixedWindowLiveVramScheduler {
        policy: LiveVramSchedulerPolicy {
            hot_window_tokens: 0,
            prefetch_window_tokens: 0,
            max_queued_pages: case_count,
            max_cpu_stored_bytes: usize::MAX / 4,
            require_qatq_beats_best_general_codec: false,
        },
    };
    let mut summary = LiveVramRuntimeRaceSummary::default();

    for (case_index, snapshot) in snapshots.iter().enumerate() {
        let key = shared_runtime_offload(&adapter, &store, snapshot, &scheduler, limits);
        let forged = forged_neighbor_keys(&key)
            .into_iter()
            .next()
            .expect("forged key should exist");
        let barrier = Arc::new(Barrier::new(4));

        let restore_handle = {
            let adapter = Arc::clone(&adapter);
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            let key = key.clone();
            thread::spawn(move || {
                barrier.wait();
                let mut adapter = adapter.lock().expect("adapter mutex poisoned");
                let mut store = store.lock().expect("store mutex poisoned");
                try_restore_live_vram_page_from_store_with_observed_latency(
                    &mut *adapter,
                    &mut store,
                    &key,
                    limits,
                    500,
                    LiveVramRestoreLatencyBudget {
                        max_restore_ns_per_page: 1_000,
                    },
                )
                .map(|outcome| outcome.restored_len)
            })
        };
        let cancel_handle = {
            let adapter = Arc::clone(&adapter);
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            let key = key.clone();
            thread::spawn(move || {
                barrier.wait();
                let mut adapter = adapter.lock().expect("adapter mutex poisoned");
                let mut store = store.lock().expect("store mutex poisoned");
                cancel_live_vram_offload(
                    &mut *adapter,
                    &mut store,
                    &key,
                    LiveVramCancellationStage::AfterRuntimeCommit,
                    limits,
                )
            })
        };
        let forged_handle = {
            let adapter = Arc::clone(&adapter);
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let mut adapter = adapter.lock().expect("adapter mutex poisoned");
                let mut store = store.lock().expect("store mutex poisoned");
                try_restore_live_vram_page_from_store_with_observed_latency(
                    &mut *adapter,
                    &mut store,
                    &forged,
                    limits,
                    500,
                    LiveVramRestoreLatencyBudget {
                        max_restore_ns_per_page: 1_000,
                    },
                )
            })
        };

        barrier.wait();
        let restore_result = restore_handle.join().expect("restore race worker panicked");
        let cancel_result = cancel_handle.join().expect("cancel race worker panicked");
        let forged_result = forged_handle
            .join()
            .expect("forged restore race worker panicked");

        let restore_won = restore_result.is_ok();
        let cancel_won = cancel_result.is_ok();
        assert_ne!(
            restore_won, cancel_won,
            "case {case_index} restore/cancel race must have exactly one winner; restore={restore_result:?} cancel={cancel_result:?}"
        );
        assert!(
            forged_result.is_err(),
            "case {case_index} forged restore unexpectedly won the race"
        );
        if restore_won {
            summary.restore_wins += 1;
            assert_eq!(
                restore_result.expect("restore winner should carry length"),
                snapshot.bytes_le.len()
            );
            assert!(matches!(
                cancel_result,
                Err(qatq::LiveVramRestoreError::Codec(
                    qatq::QatqError::InvalidHeader
                ))
            ));
            summary.cancel_loser_rejections += 1;
        } else {
            summary.cancel_wins += 1;
            let cancelled = cancel_result.expect("cancel winner should carry outcome");
            assert!(matches!(
                cancelled,
                qatq::LiveVramCancellationOutcome::RestoredCommitted { .. }
            ));
            assert!(matches!(
                restore_result,
                Err(qatq::LiveVramRestoreError::Codec(
                    qatq::QatqError::InvalidHeader
                ))
            ));
            summary.restore_loser_rejections += 1;
        }

        let resident = adapter
            .lock()
            .expect("adapter mutex poisoned")
            .is_page_resident(&snapshot.descriptor)
            .expect("page should still be known to runtime");
        assert!(resident, "case {case_index} page ended non-resident");
        assert!(
            !store
                .lock()
                .expect("store mutex poisoned")
                .contains_key(&key),
            "case {case_index} CPU offload entry leaked after restore/cancel race"
        );
        summary.forged_restore_rejections += 1;
        summary.pages += 1;
        summary.raw_bytes += snapshot.bytes_le.len();
    }

    assert!(summary.restore_wins > 0);
    assert!(summary.cancel_wins > 0);
    assert_eq!(summary.pages, case_count);
    assert_eq!(summary.forged_restore_rejections, case_count);
    let store = store.lock().expect("store mutex poisoned");
    assert_eq!(store.len(), 0);
    assert_eq!(store.metrics().pending_pages, 0);
    assert_eq!(
        store.metrics().restore_failures,
        summary.forged_restore_rejections
            + summary.restore_loser_rejections
            + summary.cancel_loser_rejections
    );
    let runtime_metrics = adapter
        .lock()
        .expect("adapter mutex poisoned")
        .metrics()
        .expect("runtime metrics should be available");
    assert_eq!(runtime_metrics.offloaded_pages, 0);
    assert_eq!(runtime_metrics.resident_pages, case_count);

    eprintln!(
        "qatq live-vram runtime restore/cancel race stress: pages={} restore_wins={} cancel_wins={} forged_restore_rejections={} restore_loser_rejections={} cancel_loser_rejections={} raw_bytes={}",
        summary.pages,
        summary.restore_wins,
        summary.cancel_wins,
        summary.forged_restore_rejections,
        summary.restore_loser_rejections,
        summary.cancel_loser_rejections,
        summary.raw_bytes
    );
}

#[derive(Clone, Copy, Debug)]
struct KvSpec {
    tokens: usize,
    heads: usize,
    dim: usize,
    pattern: Pattern,
    seed: u64,
}

impl KvSpec {
    fn for_index(index: usize) -> Self {
        const TOKENS: [usize; 9] = [1, 2, 3, 4, 8, 16, 32, 64, 128];
        const HEADS: [usize; 7] = [1, 2, 3, 4, 8, 12, 16];
        const DIMS: [usize; 8] = [4, 8, 12, 16, 24, 32, 48, 64];
        let mixed = mix64(index as u64 ^ 0xa17a_5eed_5141_5451);
        let mut tokens = TOKENS[index % TOKENS.len()];
        let mut heads = HEADS[(index / TOKENS.len()) % HEADS.len()];
        let mut dim = DIMS[(index / (TOKENS.len() * HEADS.len())) % DIMS.len()];
        while tokens * heads * dim > LARGE_VALUE_LIMIT {
            if tokens > 1 {
                tokens /= 2;
            } else if heads > 1 {
                heads /= 2;
            } else {
                dim /= 2;
            }
        }
        Self {
            tokens,
            heads,
            dim,
            pattern: Pattern::for_index(index),
            seed: mixed,
        }
    }

    fn value_count(self) -> usize {
        self.tokens * self.heads * self.dim
    }

    fn chunk_size(self) -> usize {
        let base = match self.value_count() % 5 {
            0 => self.dim.max(1),
            1 => (self.heads * self.dim).max(1),
            2 => (self.tokens.max(1) * self.dim / 2).max(1),
            3 => 257.min(self.value_count().max(1)),
            _ => 1024.min(self.value_count().max(1)),
        };
        base.clamp(1, self.value_count().max(1))
    }

    fn generate(self) -> Vec<f32> {
        let mut values = Vec::with_capacity(self.value_count());
        let mut state = self.seed;
        for token in 0..self.tokens {
            for head in 0..self.heads {
                for channel in 0..self.dim {
                    state = mix64(state.wrapping_add(0x9e37_79b9_7f4a_7c15));
                    values.push(self.pattern.value(self, token, head, channel, state));
                }
            }
        }
        values
    }
}

#[derive(Clone, Copy)]
struct CorruptionRate {
    label: &'static str,
    numerator: usize,
    denominator: usize,
}

impl CorruptionRate {
    fn mutation_count(self, len: usize) -> usize {
        len.saturating_mul(self.numerator)
            .saturating_add(self.denominator - 1)
            / self.denominator
    }

    fn salt(self) -> u64 {
        ((self.numerator as u64) << 32) ^ self.denominator as u64
    }
}

#[derive(Debug, Default)]
struct LiveVramCorruptionSummary {
    pages: usize,
    qatq_pages: usize,
    raw_pages: usize,
    raw_bytes: usize,
    stored_bytes: usize,
    corruptions: usize,
}

#[derive(Debug, Default)]
struct LiveVramIsolationSummary {
    pages: usize,
    forged_restore_rejections: usize,
    raw_bytes: usize,
}

#[derive(Clone, Debug, Default)]
struct LiveVramMultiSequenceSummary {
    pages: usize,
    raw_bytes: usize,
    restored_and_removed: usize,
    restored_and_kept: usize,
    cancelled_before_restore: usize,
    restored_then_cancelled: usize,
    forged_restore_rejections: usize,
    cancelled_restore_rejections: usize,
}

#[derive(Clone, Debug, Default)]
struct LiveVramBudgetPressureSummary {
    attempted_pages: usize,
    accepted_pages: usize,
    rejected_pages: usize,
    restored_pages: usize,
    total_candidate_stored_bytes: usize,
    max_cpu_stored_bytes: usize,
    accepted_stored_bytes: usize,
    accepted_raw_bytes: usize,
}

#[derive(Clone, Debug, Default)]
struct LiveVramRuntimeConcurrencySummary {
    pages: usize,
    raw_bytes: usize,
    offloaded: usize,
    restored: usize,
    cancelled_before_commit: usize,
    cancelled_after_commit: usize,
    duplicate_offload_rejections: usize,
    forged_restore_rejections: usize,
    restore_stalls: usize,
}

#[derive(Clone, Debug, Default)]
struct LiveVramRuntimeRaceSummary {
    pages: usize,
    raw_bytes: usize,
    restore_wins: usize,
    cancel_wins: usize,
    forged_restore_rejections: usize,
    restore_loser_rejections: usize,
    cancel_loser_rejections: usize,
}

impl std::ops::AddAssign for LiveVramMultiSequenceSummary {
    fn add_assign(&mut self, rhs: Self) {
        self.pages += rhs.pages;
        self.raw_bytes += rhs.raw_bytes;
        self.restored_and_removed += rhs.restored_and_removed;
        self.restored_and_kept += rhs.restored_and_kept;
        self.cancelled_before_restore += rhs.cancelled_before_restore;
        self.restored_then_cancelled += rhs.restored_then_cancelled;
        self.forged_restore_rejections += rhs.forged_restore_rejections;
        self.cancelled_restore_rejections += rhs.cancelled_restore_rejections;
    }
}

impl std::ops::AddAssign for LiveVramRuntimeConcurrencySummary {
    fn add_assign(&mut self, rhs: Self) {
        self.pages += rhs.pages;
        self.raw_bytes += rhs.raw_bytes;
        self.offloaded += rhs.offloaded;
        self.restored += rhs.restored;
        self.cancelled_before_commit += rhs.cancelled_before_commit;
        self.cancelled_after_commit += rhs.cancelled_after_commit;
        self.duplicate_offload_rejections += rhs.duplicate_offload_rejections;
        self.forged_restore_rejections += rhs.forged_restore_rejections;
        self.restore_stalls += rhs.restore_stalls;
    }
}

fn shared_runtime_offload(
    adapter: &Arc<Mutex<RuntimeConcurrencyAdapter>>,
    store: &Arc<Mutex<LiveVramOffloadStore>>,
    snapshot: &KvPageSnapshot,
    scheduler: &FixedWindowLiveVramScheduler,
    limits: LiveVramLimits,
) -> KvPageKey {
    let mut adapter = adapter.lock().expect("adapter mutex poisoned");
    let mut store = store.lock().expect("store mutex poisoned");
    let queued_pages = store.len();
    let cpu_stored_bytes = store.metrics().cpu_stored_bytes;
    let outcome = try_offload_live_vram_page(
        &mut *adapter,
        &mut store,
        scheduler,
        &snapshot.descriptor,
        LiveVramSchedulerState {
            current_token: 0,
            queued_pages,
            cpu_stored_bytes,
        },
        limits,
    )
    .expect("shared runtime race offload should succeed");
    let LiveVramOffloadOutcome::Offloaded { key, .. } = outcome else {
        panic!("shared runtime race scheduler unexpectedly kept page resident");
    };
    assert!(store.contains_key(&key));
    assert!(!adapter.is_page_resident(&snapshot.descriptor).unwrap());
    key
}

struct RuntimeConcurrencyHarness {
    adapter: RuntimeConcurrencyAdapter,
    store: LiveVramOffloadStore,
}

impl RuntimeConcurrencyHarness {
    fn offload(
        &mut self,
        snapshot: &KvPageSnapshot,
        scheduler: &FixedWindowLiveVramScheduler,
        limits: LiveVramLimits,
    ) -> KvPageKey {
        let queued_pages = self.store.len();
        let cpu_stored_bytes = self.store.metrics().cpu_stored_bytes;
        let outcome = try_offload_live_vram_page(
            &mut self.adapter,
            &mut self.store,
            scheduler,
            &snapshot.descriptor,
            LiveVramSchedulerState {
                current_token: 0,
                queued_pages,
                cpu_stored_bytes,
            },
            limits,
        )
        .expect("runtime concurrency offload should succeed");
        let LiveVramOffloadOutcome::Offloaded { key, .. } = outcome else {
            panic!("runtime concurrency scheduler unexpectedly kept page resident");
        };
        assert!(self.store.contains_key(&key));
        assert!(!self.adapter.is_page_resident(&snapshot.descriptor).unwrap());
        key
    }

    fn assert_duplicate_offload_rejected(
        &mut self,
        snapshot: &KvPageSnapshot,
        scheduler: &FixedWindowLiveVramScheduler,
        limits: LiveVramLimits,
    ) {
        let before_pending = self.store.len();
        let cpu_stored_bytes = self.store.metrics().cpu_stored_bytes;
        assert!(
            try_offload_live_vram_page(
                &mut self.adapter,
                &mut self.store,
                scheduler,
                &snapshot.descriptor,
                LiveVramSchedulerState {
                    current_token: 0,
                    queued_pages: before_pending,
                    cpu_stored_bytes,
                },
                limits,
            )
            .is_err(),
            "duplicate offload should fail closed while page is non-resident"
        );
        assert_eq!(self.store.len(), before_pending);
    }

    fn cancel_before_runtime_commit(
        &mut self,
        key: &KvPageKey,
        limits: LiveVramLimits,
    ) -> Result<qatq::LiveVramCancellationOutcome, qatq::LiveVramRestoreError> {
        cancel_live_vram_offload(
            &mut self.adapter,
            &mut self.store,
            key,
            LiveVramCancellationStage::BeforeRuntimeCommit,
            limits,
        )
    }

    fn cancel_after_runtime_commit(
        &mut self,
        key: &KvPageKey,
        limits: LiveVramLimits,
    ) -> Result<qatq::LiveVramCancellationOutcome, qatq::LiveVramRestoreError> {
        cancel_live_vram_offload(
            &mut self.adapter,
            &mut self.store,
            key,
            LiveVramCancellationStage::AfterRuntimeCommit,
            limits,
        )
    }

    fn restore_with_latency(
        &mut self,
        key: &KvPageKey,
        limits: LiveVramLimits,
        observed_restore_ns: u128,
        max_restore_ns: u128,
        expect_stall: bool,
    ) -> usize {
        let outcome = try_restore_live_vram_page_from_store_with_observed_latency(
            &mut self.adapter,
            &mut self.store,
            key,
            limits,
            observed_restore_ns,
            LiveVramRestoreLatencyBudget {
                max_restore_ns_per_page: max_restore_ns,
            },
        )
        .expect("runtime concurrency restore should succeed");
        assert_eq!(outcome.stalled, expect_stall);
        outcome.restored_len
    }

    fn assert_page_consistent(&self, key: &KvPageKey) {
        let resident = self
            .adapter
            .pages
            .get(key)
            .expect("known runtime page")
            .resident;
        let pending = self.store.contains_key(key);
        assert_ne!(
            resident, pending,
            "page must be either resident or pending, never both or neither: {key:?}"
        );
    }
}

struct RuntimeConcurrencyAdapter {
    pages: BTreeMap<KvPageKey, RuntimeConcurrencyPage>,
}

struct RuntimeConcurrencyPage {
    snapshot: KvPageSnapshot,
    resident: bool,
}

impl RuntimeConcurrencyAdapter {
    fn new(snapshots: Vec<KvPageSnapshot>) -> Self {
        let pages = snapshots
            .into_iter()
            .map(|snapshot| {
                (
                    KvPageKey::from_descriptor(&snapshot.descriptor),
                    RuntimeConcurrencyPage {
                        snapshot,
                        resident: true,
                    },
                )
            })
            .collect();
        Self { pages }
    }
}

impl LiveVramRuntimeAdapter for RuntimeConcurrencyAdapter {
    fn identity(&self) -> LiveVramAdapterIdentity {
        LiveVramAdapterIdentity {
            runtime_id: "runtime-concurrency-stress".to_string(),
            runtime_commit: "runtime-concurrency-stress-commit".to_string(),
            adapter_version: "qatq-runtime-concurrency-stress/0".to_string(),
            adapter_contract_version: LIVE_VRAM_ADAPTER_CONTRACT_VERSION.to_string(),
        }
    }

    fn snapshot_page(
        &mut self,
        descriptor: &KvPageDescriptor,
        limits: LiveVramLimits,
    ) -> Result<KvPageSnapshot, LiveVramAdapterError> {
        descriptor
            .validate(limits)
            .map_err(|_| LiveVramAdapterError::SnapshotFailed("invalid descriptor"))?;
        let key = KvPageKey::from_descriptor(descriptor);
        let page = self
            .pages
            .get(&key)
            .ok_or(LiveVramAdapterError::SnapshotFailed("unknown page"))?;
        if !page.resident {
            return Err(LiveVramAdapterError::SnapshotFailed("page is not resident"));
        }
        Ok(page.snapshot.clone())
    }

    fn commit_offload(
        &mut self,
        encoded: &LiveVramPageEncodeResult,
    ) -> Result<(), LiveVramAdapterError> {
        let key = KvPageKey::from_descriptor(&encoded.metadata.descriptor);
        let page = self
            .pages
            .get_mut(&key)
            .ok_or(LiveVramAdapterError::CommitFailed("unknown page"))?;
        if !page.resident {
            return Err(LiveVramAdapterError::CommitFailed(
                "page is already offloaded",
            ));
        }
        page.resident = false;
        Ok(())
    }

    fn restore_committed_page(
        &mut self,
        metadata: &qatq::LiveVramPageMetadata,
        bytes: &[u8],
        limits: LiveVramLimits,
    ) -> Result<LiveVramRestoreStatus, LiveVramAdapterError> {
        let restored = restore_live_vram_page(metadata, bytes, limits)
            .map_err(|_| LiveVramAdapterError::RestoreFailed("codec rejected page"))?;
        let key = KvPageKey::from_descriptor(&metadata.descriptor);
        let page = self
            .pages
            .get_mut(&key)
            .ok_or(LiveVramAdapterError::RestoreFailed("unknown page"))?;
        if restored != page.snapshot.bytes_le {
            return Ok(LiveVramRestoreStatus::ChecksumFailure);
        }
        page.resident = true;
        Ok(LiveVramRestoreStatus::Restored)
    }

    fn is_page_resident(
        &self,
        descriptor: &KvPageDescriptor,
    ) -> Result<bool, LiveVramAdapterError> {
        let key = KvPageKey::from_descriptor(descriptor);
        self.pages
            .get(&key)
            .map(|page| page.resident)
            .ok_or(LiveVramAdapterError::ResidencyQueryFailed("unknown page"))
    }

    fn metrics(&self) -> Result<LiveVramAdapterMetrics, LiveVramAdapterError> {
        let mut metrics = LiveVramAdapterMetrics::default();
        for page in self.pages.values() {
            metrics.peak_gpu_bytes += page.snapshot.bytes_le.len();
            if page.resident {
                metrics.resident_pages += 1;
                metrics.current_gpu_bytes += page.snapshot.bytes_le.len();
            } else {
                metrics.offloaded_pages += 1;
            }
        }
        Ok(metrics)
    }
}

fn live_vram_stress_seal_policy() -> LiveVramPageSealPolicy {
    LiveVramPageSealPolicy::new(
        [0x71_u8; 32],
        b"qatq-live-vram-multi-sequence-stress".to_vec(),
    )
    .expect("stress seal policy should be valid")
}

fn multi_sequence_live_vram_stress_snapshot(case_index: usize) -> KvPageSnapshot {
    let mut snapshot = live_vram_stress_snapshot(case_index);
    let value_count = snapshot.bytes_le.len() / snapshot.descriptor.dtype.element_width();
    snapshot.descriptor.runtime_id = format!("runtime-{}", case_index % 4);
    snapshot.descriptor.runtime_commit = format!("multi-seq-{case_index:08x}");
    snapshot.descriptor.adapter_version = "qatq-live-vram-multi-sequence-stress/0".to_string();
    snapshot.descriptor.model_id = format!("model-family-{}", case_index % 9);
    snapshot.descriptor.seq_id = format!(
        "tenant-{}-session-{}",
        case_index % 29,
        (case_index / 29) % 97
    );
    snapshot.descriptor.layer_id = ((case_index / 3) % 80) as u32;
    snapshot.descriptor.kind = if (case_index / 5).is_multiple_of(2) {
        KvPageKind::Key
    } else {
        KvPageKind::Value
    };
    snapshot.descriptor.layout = match case_index % 5 {
        0 => KvPageLayout::Paged,
        1 => KvPageLayout::Contiguous,
        2 => KvPageLayout::Blocked,
        3 => KvPageLayout::Transposed,
        _ => KvPageLayout::RuntimeSpecific,
    };
    snapshot.descriptor.token_start = case_index as u64 * 8_192;
    snapshot.descriptor.token_end = snapshot.descriptor.token_start + value_count as u64;
    snapshot.descriptor.next_required_token = Some(snapshot.descriptor.token_end + 16_384);
    snapshot
}

fn isolated_live_vram_stress_snapshot(case_index: usize) -> KvPageSnapshot {
    let mut snapshot = live_vram_stress_snapshot(case_index);
    let value_count = snapshot.bytes_le.len() / snapshot.descriptor.dtype.element_width();
    snapshot.descriptor.runtime_id = format!("runtime-{}", case_index % 5);
    snapshot.descriptor.runtime_commit = format!("commit-{case_index:08x}");
    snapshot.descriptor.model_id = format!("model-family-{}-case-{case_index}", case_index % 13);
    snapshot.descriptor.seq_id = format!("tenant-{}-request-{case_index}", case_index % 17);
    snapshot.descriptor.layer_id = case_index as u32;
    snapshot.descriptor.kind = if case_index.is_multiple_of(2) {
        KvPageKind::Key
    } else {
        KvPageKind::Value
    };
    snapshot.descriptor.token_start = (case_index as u64) * 16_384;
    snapshot.descriptor.token_end = snapshot.descriptor.token_start + value_count as u64;
    snapshot.descriptor.next_required_token = Some(snapshot.descriptor.token_end + 8_192);
    snapshot
}

fn forged_neighbor_keys(key: &KvPageKey) -> Vec<KvPageKey> {
    let mut keys = Vec::with_capacity(7);

    let mut forged = key.clone();
    forged.runtime_id.push_str("-forged");
    keys.push(forged);

    let mut forged = key.clone();
    forged.model_id.push_str("-forged");
    keys.push(forged);

    let mut forged = key.clone();
    forged.seq_id.push_str("-forged");
    keys.push(forged);

    let mut forged = key.clone();
    forged.layer_id = forged.layer_id.wrapping_add(1);
    keys.push(forged);

    let mut forged = key.clone();
    forged.kind = match forged.kind {
        KvPageKind::Key => KvPageKind::Value,
        KvPageKind::Value => KvPageKind::Key,
    };
    keys.push(forged);

    let mut forged = key.clone();
    forged.token_start = forged.token_start.saturating_add(1);
    keys.push(forged);

    let mut forged = key.clone();
    forged.token_end = forged.token_end.saturating_add(1);
    keys.push(forged);

    keys
}

fn live_vram_stress_snapshot(case_index: usize) -> KvPageSnapshot {
    let dtype = match case_index % 3 {
        0 => TensorDType::F16,
        1 => TensorDType::BF16,
        _ => TensorDType::F32,
    };
    let value_count = match case_index % 9 {
        0 => 128,
        1 => 257,
        2 => 512,
        3 => 1024,
        4 => 1537,
        5 => 2048,
        6 => 3072,
        7 => 4096,
        _ => 8192,
    };
    let bytes = live_vram_stress_bytes(case_index, dtype, value_count);
    KvPageSnapshot {
        descriptor: KvPageDescriptor {
            runtime_id: "llama.cpp".to_string(),
            runtime_commit: "stress-commit".to_string(),
            adapter_version: "qatq-live-vram-stress/0".to_string(),
            model_id: format!("stress-model-{}", case_index % 7),
            seq_id: format!("seq-{}", case_index % 11),
            layer_id: (case_index % 48) as u32,
            kind: if case_index.is_multiple_of(2) {
                KvPageKind::Key
            } else {
                KvPageKind::Value
            },
            dtype,
            shape: vec![value_count],
            layout: match case_index % 4 {
                0 => KvPageLayout::Paged,
                1 => KvPageLayout::Contiguous,
                2 => KvPageLayout::Blocked,
                _ => KvPageLayout::RuntimeSpecific,
            },
            token_start: (case_index as u64 % 128) * 128,
            token_end: (case_index as u64 % 128) * 128 + value_count as u64,
            next_required_token: Some(16_384 + case_index as u64),
            raw_len: bytes.len(),
            checksum: live_vram_page_checksum(&bytes),
        },
        bytes_le: bytes,
    }
}

fn live_vram_stress_bytes(case_index: usize, dtype: TensorDType, value_count: usize) -> Vec<u8> {
    let width = dtype.element_width();
    let mut bytes = Vec::with_capacity(value_count * width);
    let mut state = mix64(case_index as u64 ^ 0x7176_7261_6d5f_636f);
    for index in 0..value_count {
        state = mix64(state.wrapping_add(index as u64).wrapping_add(0x9e37_79b9));
        match dtype {
            TensorDType::F16 | TensorDType::BF16 => {
                let value = match case_index % 5 {
                    0 => 0x3c00_u16.wrapping_add(((index % 8) as u16) << 4),
                    1 => 0x0000,
                    2 => 0x4000_u16 ^ ((index as u16) & 0x00ff),
                    3 => (state as u16) & 0xfff0,
                    _ => state as u16,
                };
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            TensorDType::F32 => {
                let value = match case_index % 5 {
                    0 => 0x3f80_0000_u32.wrapping_add(((index % 16) as u32) << 12),
                    1 => 0x0000_0000,
                    2 => 0x4000_0000_u32 ^ ((index as u32) << 3),
                    3 => (state as u32) & 0xffff_ff00,
                    _ => state as u32,
                };
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    bytes
}

fn inject_corruption(bytes: &mut [u8], rate: CorruptionRate, seed: u64) -> usize {
    if bytes.is_empty() {
        return 0;
    }
    let target = rate.mutation_count(bytes.len()).clamp(1, bytes.len());
    let mut touched = vec![false; bytes.len()];
    let mut mutations = 0;
    let mut state = seed;
    while mutations < target {
        state = mix64(
            state
                .wrapping_add(mutations as u64)
                .wrapping_add(0xa5a5_5a5a),
        );
        let offset = (state as usize) % bytes.len();
        if touched[offset] {
            continue;
        }
        touched[offset] = true;
        bytes[offset] ^= 0x80 | ((state >> 8) as u8 & 0x7f);
        mutations += 1;
    }
    mutations
}

#[derive(Clone, Copy, Debug)]
enum Pattern {
    Bf16Ramp,
    Bf16Wave,
    LowRankHeads,
    PageSparse,
    QuaternionChain,
    SignedZeroNanInf,
    DeltaBits,
    RepeatedTokens,
    RawNoise,
    HeadStripes,
}

impl Pattern {
    fn for_index(index: usize) -> Self {
        match index % 10 {
            0 => Self::Bf16Ramp,
            1 => Self::Bf16Wave,
            2 => Self::LowRankHeads,
            3 => Self::PageSparse,
            4 => Self::QuaternionChain,
            5 => Self::SignedZeroNanInf,
            6 => Self::DeltaBits,
            7 => Self::RepeatedTokens,
            8 => Self::RawNoise,
            _ => Self::HeadStripes,
        }
    }

    fn value(self, spec: KvSpec, token: usize, head: usize, channel: usize, state: u64) -> f32 {
        let linear = ((token * spec.heads + head) * spec.dim + channel) as u32;
        match self {
            Self::Bf16Ramp => {
                let bits = 0x3f80_0000_u32
                    .wrapping_add((token as u32) << 17)
                    .wrapping_add((head as u32) << 13)
                    .wrapping_add((channel as u32) << 9);
                f32::from_bits(bits & 0xffff_0000)
            }
            Self::Bf16Wave => {
                let wave =
                    ((token as f32 * 0.173) + (head as f32 * 0.311) + channel as f32 * 0.071).sin()
                        * 3.0;
                f32::from_bits(wave.to_bits() & 0xffff_0000)
            }
            Self::LowRankHeads => {
                let base = (token as f32 * 0.05).sin() + (channel as f32 * 0.02).cos();
                f32::from_bits((base + head as f32 * 0.000_976_562_5).to_bits() & 0xffff_8000)
            }
            Self::PageSparse => {
                if (token / 8 + head + channel / 4).is_multiple_of(5) {
                    0.0
                } else {
                    let sign = if (state & 1) == 0 { 1.0 } else { -1.0 };
                    f32::from_bits((sign * ((channel % 7) as f32 + 0.25)).to_bits() & 0xffff_0000)
                }
            }
            Self::QuaternionChain => {
                let lane = linear / 4;
                let component = linear % 4;
                let residual = match component {
                    0 => 0x0000_0010 + (head as u32 & 0x03),
                    1 => 0x0000_0001 + (token as u32 & 0x01),
                    2 => 0xffff_ffff,
                    _ => 0x0000_0000,
                };
                let bits = 0x3f80_0000_u32
                    .wrapping_add(lane.wrapping_mul(0x10))
                    .wrapping_add(residual);
                f32::from_bits(bits)
            }
            Self::SignedZeroNanInf => match linear % 8 {
                0 => 0.0,
                1 => -0.0,
                2 => f32::INFINITY,
                3 => f32::NEG_INFINITY,
                4 => f32::from_bits(0x7fc0_0000 | (linear & 0x000f_ffff)),
                5 => f32::from_bits(0xffc0_0000 | (linear & 0x000f_ffff)),
                6 => f32::MIN_POSITIVE,
                _ => -f32::MIN_POSITIVE,
            },
            Self::DeltaBits => {
                let bits = 0x3f00_0000_u32 ^ linear.rotate_left((head % 11) as u32);
                f32::from_bits(bits)
            }
            Self::RepeatedTokens => {
                let repeated_token = token % 4;
                let bits = 0x4000_0000_u32
                    .wrapping_add((repeated_token as u32) << 16)
                    .wrapping_add((head as u32) << 12)
                    .wrapping_add((channel as u32) << 4);
                f32::from_bits(bits & 0xffff_ff00)
            }
            Self::RawNoise => f32::from_bits((state as u32).wrapping_add(linear.rotate_left(7))),
            Self::HeadStripes => {
                let bits = if head.is_multiple_of(2) {
                    0x3f80_0000_u32.wrapping_add((channel as u32) << 12)
                } else {
                    0xbf80_0000_u32.wrapping_sub((token as u32) << 12)
                };
                f32::from_bits(bits)
            }
        }
    }
}

#[derive(Debug)]
struct StressSummary {
    cases: usize,
    values: usize,
    raw_bytes: usize,
    encoded_bytes: usize,
    container_bytes: usize,
    max_values: usize,
    max_encoded_bytes: usize,
    best_ratio: f64,
    worst_ratio: f64,
    encode_time: Duration,
    decode_time: Duration,
}

impl Default for StressSummary {
    fn default() -> Self {
        Self {
            cases: 0,
            values: 0,
            raw_bytes: 0,
            encoded_bytes: 0,
            container_bytes: 0,
            max_values: 0,
            max_encoded_bytes: 0,
            best_ratio: f64::INFINITY,
            worst_ratio: 0.0,
            encode_time: Duration::ZERO,
            decode_time: Duration::ZERO,
        }
    }
}

fn assert_same_bits(expected: &[f32], actual: &[f32], case_index: usize, path: &str) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "case {case_index} {path} length mismatch"
    );
    for (value_index, (expected, actual)) in expected.iter().zip(actual).enumerate() {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "case {case_index} {path} bit mismatch at value {value_index}"
        );
    }
}

fn assert_mutation_rejected(payload: &[u8], case_index: usize, path: &str) {
    let mut mutated = payload.to_vec();
    let offset = 28 + ((case_index * 17) % (mutated.len() - 28));
    mutated[offset] ^= 0x5a;
    assert!(
        decode(&mutated).is_err(),
        "case {case_index} accepted mutated {path} byte at offset {offset}"
    );
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
