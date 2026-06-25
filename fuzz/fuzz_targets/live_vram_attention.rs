#![no_main]

use libfuzzer_sys::fuzz_target;
use qatq::{
    TensorDType, compare_live_vram_segment_summary_attention_reference,
    compare_live_vram_streaming_attention_reference,
    compare_live_vram_typed_streaming_attention_reference,
};

const VALUES: &[(f32, u16)] = &[
    (-4.0, 0xc400),
    (-2.0, 0xc000),
    (-1.0, 0xbc00),
    (-0.5, 0xb800),
    (-0.25, 0xb400),
    (0.0, 0x0000),
    (0.25, 0x3400),
    (0.5, 0x3800),
    (1.0, 0x3c00),
    (2.0, 0x4000),
    (4.0, 0x4400),
];

fuzz_target!(|data: &[u8]| {
    let mut padded;
    let data = if data.len() < 16 {
        padded = data.to_vec();
        padded.extend_from_slice(b"qatq-live-vram-attention-seed");
        padded.as_slice()
    } else {
        data
    };

    let head_dim = 1 + (data[0] as usize % 16);
    let value_dim = 1 + (data[1] as usize % 16);
    let tokens = 1 + (u16::from_le_bytes([data[2], data[3]]) as usize % 64);
    let max_pages = 1 + (data[4] as usize % 8);
    let dtype = match data[5] % 3 {
        0 => TensorDType::F32,
        1 => TensorDType::F16,
        _ => TensorDType::BF16,
    };

    let mut cursor = 6_usize;
    let query = value_vec(data, &mut cursor, head_dim);
    let keys_all = value_vec(data, &mut cursor, tokens * head_dim);
    let values_all = value_vec(data, &mut cursor, tokens * value_dim);
    let ranges = page_ranges(data, &mut cursor, tokens, max_pages);

    let mut key_pages = Vec::with_capacity(ranges.len());
    let mut value_pages = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        key_pages.push(keys_all[start * head_dim..end * head_dim].to_vec());
        value_pages.push(values_all[start * value_dim..end * value_dim].to_vec());
    }
    let key_refs: Vec<&[f32]> = key_pages.iter().map(Vec::as_slice).collect();
    let value_refs: Vec<&[f32]> = value_pages.iter().map(Vec::as_slice).collect();

    let segmented = compare_live_vram_streaming_attention_reference(
        &query,
        &key_refs,
        &value_refs,
        value_dim,
        1.0e-4,
    )
    .expect("finite segmented attention should compare");
    assert!(segmented.passed);
    assert!(segmented.streaming.peak_kv_value_ratio().unwrap_or(1.0) <= 1.0);

    let summary = compare_live_vram_segment_summary_attention_reference(
        &query,
        &key_refs,
        &value_refs,
        value_dim,
        1.0e-4,
    )
    .expect("finite page-summary segmented attention should compare");
    assert!(summary.passed);
    assert_attention_close(
        &summary.streaming.output,
        &segmented.streaming.output,
        1.0e-4,
    );
    assert_eq!(
        summary.streaming.peak_page_kv_values,
        segmented.streaming.peak_page_kv_values
    );

    let whole = compare_live_vram_streaming_attention_reference(
        &query,
        &[&keys_all],
        &[&values_all],
        value_dim,
        1.0e-4,
    )
    .expect("finite whole attention should compare");
    assert_attention_close(&segmented.streaming.output, &whole.streaming.output, 1.0e-4);

    let key_pages_le: Vec<Vec<u8>> = key_pages
        .iter()
        .map(|page| encode_typed_page(page, dtype))
        .collect();
    let value_pages_le: Vec<Vec<u8>> = value_pages
        .iter()
        .map(|page| encode_typed_page(page, dtype))
        .collect();
    let key_page_refs: Vec<&[u8]> = key_pages_le.iter().map(Vec::as_slice).collect();
    let value_page_refs: Vec<&[u8]> = value_pages_le.iter().map(Vec::as_slice).collect();
    let typed = compare_live_vram_typed_streaming_attention_reference(
        &query,
        &key_page_refs,
        &value_page_refs,
        dtype,
        head_dim,
        value_dim,
        1.0e-3,
    )
    .expect("finite typed segmented attention should compare");
    assert!(typed.passed);
    assert!(typed.streaming.peak_kv_value_ratio().unwrap_or(1.0) <= 1.0);
});

fn value_vec(data: &[u8], cursor: &mut usize, len: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        out.push(value_from_byte(data[*cursor % data.len()]).0);
        *cursor += 1;
    }
    out
}

fn page_ranges(
    data: &[u8],
    cursor: &mut usize,
    tokens: usize,
    max_pages: usize,
) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = 0_usize;
    while start < tokens {
        let remaining = tokens - start;
        let pages_left = max_pages.saturating_sub(ranges.len()).max(1);
        let span_cap = if pages_left == 1 {
            remaining
        } else {
            remaining.saturating_sub(pages_left - 1).max(1)
        };
        let span = 1 + (data[*cursor % data.len()] as usize % span_cap);
        *cursor += 1;
        let end = (start + span).min(tokens);
        ranges.push((start, end));
        start = end;
        if ranges.len() >= max_pages && start < tokens {
            ranges.push((start, tokens));
            break;
        }
    }
    ranges
}

fn value_from_byte(byte: u8) -> (f32, u16) {
    VALUES[byte as usize % VALUES.len()]
}

fn encode_typed_page(values: &[f32], dtype: TensorDType) -> Vec<u8> {
    let mut out = Vec::new();
    for value in values {
        match dtype {
            TensorDType::F32 => out.extend_from_slice(&value.to_le_bytes()),
            TensorDType::F16 => {
                let bits = VALUES
                    .iter()
                    .find(|(candidate, _)| candidate == value)
                    .map(|(_, bits)| *bits)
                    .expect("fuzzer values are f16 table-backed");
                out.extend_from_slice(&bits.to_le_bytes());
            }
            TensorDType::BF16 => {
                let bits = (value.to_bits() >> 16) as u16;
                out.extend_from_slice(&bits.to_le_bytes());
            }
        }
    }
    out
}

fn assert_attention_close(actual: &[f32], expected: &[f32], tolerance: f32) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected.iter()) {
        assert!((actual - expected).abs() <= tolerance);
    }
}
