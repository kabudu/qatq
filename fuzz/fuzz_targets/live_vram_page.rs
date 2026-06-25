#![no_main]

use libfuzzer_sys::fuzz_target;
use qatq::{
    KvPageDescriptor, KvPageKind, KvPageLayout, KvPageSnapshot, LiveVramLimits,
    LiveVramSchedulerPolicy, LiveVramSchedulerState, LiveVramStorage, TensorDType,
    live_vram_page_checksum, restore_live_vram_page, schedule_live_vram_page,
    try_encode_live_vram_page,
};

fuzz_target!(|data: &[u8]| {
    let mut padded;
    let data = if data.len() < 12 {
        padded = data.to_vec();
        padded.extend_from_slice(b"qatq-live-vram-seed");
        padded.as_slice()
    } else {
        data
    };

    let dtype = match data[0] % 3 {
        0 => TensorDType::F16,
        1 => TensorDType::BF16,
        _ => TensorDType::F32,
    };
    let width = dtype.element_width();
    let value_count = 1 + (u16::from_le_bytes([data[1], data[2]]) as usize % 2048);
    let raw_len = value_count.saturating_mul(width);
    let mut bytes = Vec::with_capacity(raw_len);
    for index in 0..raw_len {
        bytes.push(data[3 + (index % (data.len() - 3))]);
    }

    let token_start = u16::from_le_bytes([data[3], data[4]]) as u64;
    let token_span = 1 + (data[5] as u64 % 128);
    let next_required_token = if data[6] & 1 == 0 {
        Some(token_start + token_span + u16::from_le_bytes([data[7], data[8]]) as u64)
    } else {
        None
    };
    let descriptor = KvPageDescriptor {
        runtime_id: "fuzz-runtime".to_string(),
        runtime_commit: "fuzz-commit".to_string(),
        adapter_version: "fuzz-adapter/0".to_string(),
        model_id: "fuzz-model".to_string(),
        seq_id: format!("seq-{}", data[9] % 4),
        layer_id: data[10] as u32,
        kind: if data[11] & 1 == 0 {
            KvPageKind::Key
        } else {
            KvPageKind::Value
        },
        dtype,
        shape: vec![value_count],
        layout: match data[11] % 5 {
            0 => KvPageLayout::Contiguous,
            1 => KvPageLayout::Transposed,
            2 => KvPageLayout::Blocked,
            3 => KvPageLayout::Paged,
            _ => KvPageLayout::RuntimeSpecific,
        },
        token_start,
        token_end: token_start + token_span,
        next_required_token,
        raw_len,
        checksum: live_vram_page_checksum(&bytes),
    };
    let limits = LiveVramLimits {
        max_page_bytes: 16 * 1024,
        max_stored_bytes: 16 * 1024,
        max_shape_rank: 4,
        max_shape_elements: 4096,
        ..LiveVramLimits::default()
    };
    let snapshot = KvPageSnapshot {
        descriptor,
        bytes_le: bytes,
    };

    let policy = LiveVramSchedulerPolicy {
        hot_window_tokens: data[5] as u64,
        prefetch_window_tokens: data[6] as u64,
        max_queued_pages: 1 + (data[7] as usize % 8),
        max_cpu_stored_bytes: 16 * 1024,
        require_qatq_beats_best_general_codec: data[10] & 1 == 0,
    };
    let state = LiveVramSchedulerState {
        current_token: token_start,
        queued_pages: data[8] as usize % 8,
        cpu_stored_bytes: (data[9] as usize) * 64,
    };
    let _ = schedule_live_vram_page(&snapshot.descriptor, state, policy);

    if let Ok(encoded) = try_encode_live_vram_page(&snapshot, limits)
        && let Ok(restored) = restore_live_vram_page(&encoded.metadata, &encoded.bytes, limits)
    {
        assert_eq!(restored, snapshot.bytes_le);
    }

    if let Ok(label) = std::str::from_utf8(data) {
        let _ = LiveVramStorage::from_label(label);
    }
});
