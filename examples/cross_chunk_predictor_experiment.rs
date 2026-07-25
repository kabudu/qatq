use std::{
    hint::black_box,
    time::{Duration, Instant},
};

use qatq::{
    TensorDType, decode_qatq_exact_tensor_le, qatq_exact_strategy, try_encode_qatq_exact_tensor_le,
    try_encode_qatq_exact_tensor_le_with_stride_hint,
};

const ELEMENTS: usize = 65_536;
const ROW_WIDTH: usize = 128;
const WARMUP_ITERATIONS: usize = 8;
const MEASURED_ITERATIONS: usize = 80;

fn main() {
    println!(
        "cross-chunk predictor experiment: elements={ELEMENTS} row_width={ROW_WIDTH} iterations={MEASURED_ITERATIONS}"
    );
    for (name, words) in [
        ("repeated-token-rows", repeated_token_rows()),
        ("slowly-drifting-token-rows", slowly_drifting_token_rows()),
        ("piecewise-kv", piecewise_kv()),
        ("adjacent-smooth", adjacent_smooth()),
        ("random-bits", random_bits()),
    ] {
        measure(name, &words);
    }
}

fn measure(name: &str, words: &[u16]) {
    let mut source = Vec::with_capacity(words.len() * 2);
    for word in words {
        source.extend_from_slice(&word.to_le_bytes());
    }

    let encoded = try_encode_qatq_exact_tensor_le(&source, TensorDType::BF16).unwrap();
    let hinted =
        try_encode_qatq_exact_tensor_le_with_stride_hint(&source, TensorDType::BF16, ROW_WIDTH)
            .unwrap();
    let decoded = decode_qatq_exact_tensor_le(&encoded).unwrap();
    let hinted_decoded = decode_qatq_exact_tensor_le(&hinted).unwrap();
    assert_eq!(decoded.dtype, TensorDType::BF16);
    assert_eq!(decoded.bytes_le, source);
    assert_eq!(hinted_decoded.bytes_le, source);

    for _ in 0..WARMUP_ITERATIONS {
        black_box(try_encode_qatq_exact_tensor_le(
            black_box(&source),
            TensorDType::BF16,
        ))
        .unwrap();
        black_box(try_encode_qatq_exact_tensor_le_with_stride_hint(
            black_box(&source),
            TensorDType::BF16,
            ROW_WIDTH,
        ))
        .unwrap();
        black_box(decode_qatq_exact_tensor_le(black_box(&hinted))).unwrap();
    }

    let mut encode_elapsed = Duration::ZERO;
    let mut hinted_encode_elapsed = Duration::ZERO;
    let mut decode_elapsed = Duration::ZERO;
    for _ in 0..MEASURED_ITERATIONS {
        let start = Instant::now();
        let candidate =
            try_encode_qatq_exact_tensor_le(black_box(&source), TensorDType::BF16).unwrap();
        encode_elapsed += start.elapsed();
        assert_eq!(candidate, encoded);

        let start = Instant::now();
        let hinted_candidate = try_encode_qatq_exact_tensor_le_with_stride_hint(
            black_box(&source),
            TensorDType::BF16,
            ROW_WIDTH,
        )
        .unwrap();
        hinted_encode_elapsed += start.elapsed();
        assert_eq!(hinted_candidate, hinted);

        let start = Instant::now();
        let restored = decode_qatq_exact_tensor_le(black_box(&hinted)).unwrap();
        decode_elapsed += start.elapsed();
        assert_eq!(restored.bytes_le, source);
    }

    let values = (words.len() * MEASURED_ITERATIONS) as f64;
    println!(
        "{name}: raw={} auto_bytes={} auto_strategy={} hinted_bytes={} hinted_ratio={:.6} hinted_strategy={} auto_encode_ns/value={:.3} hinted_encode_ns/value={:.3} hinted_decode_ns/value={:.3}",
        source.len(),
        encoded.len(),
        qatq_exact_strategy(&encoded).unwrap().as_str(),
        hinted.len(),
        hinted.len() as f64 / source.len() as f64,
        qatq_exact_strategy(&hinted).unwrap().as_str(),
        encode_elapsed.as_nanos() as f64 / values,
        hinted_encode_elapsed.as_nanos() as f64 / values,
        decode_elapsed.as_nanos() as f64 / values,
    );
}

fn repeated_token_rows() -> Vec<u16> {
    (0..ELEMENTS)
        .map(|index| {
            let channel = index % ROW_WIDTH;
            0x3d00_u16.wrapping_add((channel as u16).wrapping_mul(101))
        })
        .collect()
}

fn slowly_drifting_token_rows() -> Vec<u16> {
    (0..ELEMENTS)
        .map(|index| {
            let token = index / ROW_WIDTH;
            let channel = index % ROW_WIDTH;
            0x3d00_u16
                .wrapping_add((channel as u16).wrapping_mul(101))
                .wrapping_add((token / 32) as u16)
        })
        .collect()
}

fn adjacent_smooth() -> Vec<u16> {
    (0..ELEMENTS)
        .map(|index| 0x3d00_u16.wrapping_add((index / 4) as u16))
        .collect()
}

fn piecewise_kv() -> Vec<u16> {
    (0..ELEMENTS)
        .map(|index| {
            let token = index / ROW_WIDTH;
            let channel = index % ROW_WIDTH;
            let base =
                ((channel as f32 / 19.0).sin() * 0.375) + ((token as f32 / 113.0).cos() * 0.0625);
            bf16_bits(base)
        })
        .collect()
}

fn bf16_bits(value: f32) -> u16 {
    let bits = value.to_bits();
    let rounding_bias = 0x7fff + ((bits >> 16) & 1);
    (bits.wrapping_add(rounding_bias) >> 16) as u16
}

fn random_bits() -> Vec<u16> {
    let mut state = 0x5141_5451_c0de_f00d_u64;
    (0..ELEMENTS)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u16
        })
        .collect()
}
