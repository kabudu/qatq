#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use qatq::{
    KvPageDescriptor, KvPageKey, KvPageKind, KvPageLayout, KvPageSnapshot,
    LIVE_VRAM_ADAPTER_CONTRACT_VERSION, LiveVramAdapterError, LiveVramAdapterIdentity,
    LiveVramAdapterMetrics, LiveVramCancellationStage, LiveVramGpuAllocationGranularity,
    LiveVramLimits, LiveVramOffloadOutcome, LiveVramOffloadStore, LiveVramPageEncodeResult,
    LiveVramPageEvent, LiveVramPageEventKind, LiveVramPageScheduler,
    LiveVramRestoreLatencyBudget, LiveVramRestoreStatus, LiveVramRuntimeAdapter,
    LiveVramScheduleDecision, LiveVramSchedulerState, LiveVramStorage, TensorDType,
    cancel_live_vram_offload, evaluate_live_vram_event_trace, live_vram_page_checksum,
    try_offload_live_vram_page,
    try_restore_live_vram_page_from_store_with_observed_latency,
};

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let mut cursor = Cursor::new(data);
    let limits = LiveVramLimits {
        max_page_bytes: 16 * 1024,
        max_stored_bytes: 16 * 1024,
        max_shape_rank: 4,
        max_shape_elements: 8192,
        ..LiveVramLimits::default()
    };
    let page_count = 1 + (cursor.next() as usize % 6);
    let mut pages = Vec::with_capacity(page_count);
    for index in 0..page_count {
        pages.push(make_snapshot(&mut cursor, index as u32, limits));
    }

    let mut adapter = FuzzLifecycleAdapter::new(pages.clone());
    let mut store = LiveVramOffloadStore::new_with_shadow_validation(
        limits,
        8,
        64 * 1024,
        64 * 1024,
    );
    let scheduler = AlwaysOffloadScheduler;
    let mut keys: Vec<KvPageKey> = Vec::new();
    let mut trace = Vec::<LiveVramPageEvent>::new();

    let events = 1 + (cursor.next() as usize % 96);
    for _ in 0..events {
        let page_index = cursor.next() as usize % pages.len();
        let descriptor = &pages[page_index].descriptor;
        let key = KvPageKey::from_descriptor(descriptor);
        let token = u64::from(cursor.next());
        match cursor.next() % 8 {
            0 | 1 => {
                let state = LiveVramSchedulerState {
                    current_token: token,
                    queued_pages: store.len(),
                    cpu_stored_bytes: store.metrics().cpu_stored_bytes,
                };
                let _ = try_offload_live_vram_page(
                    &mut adapter,
                    &mut store,
                    &scheduler,
                    descriptor,
                    state,
                    limits,
                )
                .map(|outcome| {
                    if let LiveVramOffloadOutcome::Offloaded { key, .. } = outcome
                        && !keys.contains(&key)
                    {
                        trace.push(live_vram_trace_event(
                            token,
                            &key,
                            LiveVramPageEventKind::Snapshot,
                            Some(descriptor.checksum),
                        ));
                        trace.push(live_vram_trace_event(
                            token,
                            &key,
                            LiveVramPageEventKind::OffloadCommitted,
                            Some(descriptor.checksum),
                        ));
                        keys.push(key);
                    }
                });
            }
            2 => {
                if let Ok(key) = store.commit_snapshot(&pages[page_index]) {
                    trace.push(live_vram_trace_event(
                        token,
                        &key,
                        LiveVramPageEventKind::Snapshot,
                        Some(descriptor.checksum),
                    ));
                    let _ = cancel_live_vram_offload(
                        &mut adapter,
                        &mut store,
                        &key,
                        LiveVramCancellationStage::BeforeRuntimeCommit,
                        limits,
                    );
                    trace.push(live_vram_trace_event(
                        token,
                        &key,
                        LiveVramPageEventKind::Cancelled,
                        Some(descriptor.checksum),
                    ));
                    keys.retain(|known| known != &key);
                }
            }
            3 => {
                if cancel_live_vram_offload(
                    &mut adapter,
                    &mut store,
                    &key,
                    LiveVramCancellationStage::AfterRuntimeCommit,
                    limits,
                )
                .is_ok()
                {
                    trace.push(live_vram_trace_event(
                        token,
                        &key,
                        LiveVramPageEventKind::Cancelled,
                        Some(descriptor.checksum),
                    ));
                }
                keys.retain(|known| known != &key);
            }
            4 | 5 => {
                if try_restore_live_vram_page_from_store_with_observed_latency(
                    &mut adapter,
                    &mut store,
                    &key,
                    limits,
                    u128::from(cursor.next()) * 1_000,
                    LiveVramRestoreLatencyBudget {
                        max_restore_ns_per_page: 64_000,
                    },
                )
                .is_ok()
                {
                    trace.push(live_vram_trace_event(
                        token,
                        &key,
                        LiveVramPageEventKind::RestoreCommitted,
                        Some(descriptor.checksum),
                    ));
                }
                keys.retain(|known| known != &key);
            }
            6 => {
                if let Ok(encoded) = qatq::try_encode_live_vram_page(&pages[page_index], limits) {
                    let mut corrupt = encoded;
                    if let Some(byte) = corrupt.bytes.get_mut(0) {
                        *byte ^= cursor.next();
                    }
                    let key = KvPageKey::from_descriptor(&corrupt.metadata.descriptor);
                    let before = store.len();
                    let result = store.commit_encoded(corrupt);
                    if result.is_err() {
                        assert_eq!(store.len(), before);
                    } else {
                        let _ = store.remove(&key);
                    }
                }
            }
            _ => {
                let _ = adapter.metrics();
                let _ = adapter.is_page_resident(descriptor);
                let _ = LiveVramStorage::from_label("qatq-live");
                trace.push(live_vram_trace_event(
                    token,
                    &key,
                    LiveVramPageEventKind::AttentionUse,
                    None,
                ));
            }
        }

        assert!(store.len() <= 8);
        assert_eq!(store.metrics().pending_pages, store.len());
        let trace_report = evaluate_live_vram_event_trace(
            &trace,
            qatq::LiveVramEventTracePolicy {
                require_all_pages_restored_at_end: false,
                ..qatq::LiveVramEventTracePolicy::default()
            },
        );
        assert_eq!(trace_report.events, trace.len());
        for page in &pages {
            let key = KvPageKey::from_descriptor(&page.descriptor);
            let resident = adapter
                .is_page_resident(&page.descriptor)
                .unwrap_or(false);
            assert_ne!(resident, store.contains_key(&key));
        }
    }
});

fn live_vram_trace_event(
    token: u64,
    key: &KvPageKey,
    kind: LiveVramPageEventKind,
    checksum: Option<u64>,
) -> LiveVramPageEvent {
    LiveVramPageEvent {
        token,
        key: key.clone(),
        kind,
        checksum,
    }
}

struct Cursor<'a> {
    data: &'a [u8],
    index: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, index: 0 }
    }

    fn next(&mut self) -> u8 {
        let value = self.data[self.index % self.data.len()];
        self.index = self.index.wrapping_add(1);
        value
    }
}

fn make_snapshot(cursor: &mut Cursor<'_>, layer_id: u32, limits: LiveVramLimits) -> KvPageSnapshot {
    let dtype = match cursor.next() % 3 {
        0 => TensorDType::F16,
        1 => TensorDType::BF16,
        _ => TensorDType::F32,
    };
    let width = dtype.element_width();
    let values = 8 + (u16::from_le_bytes([cursor.next(), cursor.next()]) as usize % 512);
    let raw_len = values.saturating_mul(width).min(limits.max_page_bytes);
    let raw_len = raw_len - (raw_len % width);
    let mut bytes = vec![0_u8; raw_len.max(width)];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = cursor.next().wrapping_add(index as u8);
    }
    let token_start = u64::from(cursor.next() % 8) * 128;
    let token_end = token_start + 128;
    let descriptor = KvPageDescriptor {
        runtime_id: "fuzz-runtime".to_string(),
        runtime_commit: "fuzz-commit".to_string(),
        adapter_version: "fuzz-adapter/0".to_string(),
        model_id: "fuzz-model".to_string(),
        seq_id: format!("seq-{}", cursor.next() % 4),
        layer_id,
        kind: if cursor.next() & 1 == 0 {
            KvPageKind::Key
        } else {
            KvPageKind::Value
        },
        dtype,
        shape: vec![bytes.len() / width],
        layout: KvPageLayout::Paged,
        token_start,
        token_end,
        next_required_token: Some(token_end + 1024),
        raw_len: bytes.len(),
        checksum: live_vram_page_checksum(&bytes),
    };
    KvPageSnapshot {
        descriptor,
        bytes_le: bytes,
    }
}

struct AlwaysOffloadScheduler;

impl LiveVramPageScheduler for AlwaysOffloadScheduler {
    fn decide(
        &self,
        _descriptor: &KvPageDescriptor,
        _state: LiveVramSchedulerState,
    ) -> LiveVramScheduleDecision {
        LiveVramScheduleDecision::Offload
    }
}

struct RuntimePage {
    snapshot: KvPageSnapshot,
    resident: bool,
}

struct FuzzLifecycleAdapter {
    pages: BTreeMap<KvPageKey, RuntimePage>,
}

impl FuzzLifecycleAdapter {
    fn new(pages: Vec<KvPageSnapshot>) -> Self {
        let pages = pages
            .into_iter()
            .map(|snapshot| {
                (
                    KvPageKey::from_descriptor(&snapshot.descriptor),
                    RuntimePage {
                        snapshot,
                        resident: true,
                    },
                )
            })
            .collect();
        Self { pages }
    }
}

impl LiveVramRuntimeAdapter for FuzzLifecycleAdapter {
    fn identity(&self) -> LiveVramAdapterIdentity {
        LiveVramAdapterIdentity {
            runtime_id: "fuzz-runtime".to_string(),
            runtime_commit: "fuzz-commit".to_string(),
            adapter_version: "fuzz-adapter/0".to_string(),
            adapter_contract_version: LIVE_VRAM_ADAPTER_CONTRACT_VERSION.to_string(),
        }
    }

    fn snapshot_page(
        &mut self,
        descriptor: &KvPageDescriptor,
        _limits: LiveVramLimits,
    ) -> Result<KvPageSnapshot, LiveVramAdapterError> {
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
            return Err(LiveVramAdapterError::CommitFailed("page is already offloaded"));
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
        let restored = qatq::restore_live_vram_page(metadata, bytes, limits)
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
            if page.resident {
                metrics.resident_pages += 1;
                metrics.current_gpu_bytes += page.snapshot.bytes_le.len();
            } else {
                metrics.offloaded_pages += 1;
            }
        }
        metrics.peak_gpu_bytes = self
            .pages
            .values()
            .map(|page| page.snapshot.bytes_le.len())
            .sum();
        Ok(metrics)
    }

    fn gpu_allocation_granularity(&self) -> LiveVramGpuAllocationGranularity {
        LiveVramGpuAllocationGranularity::PerPage
    }
}
