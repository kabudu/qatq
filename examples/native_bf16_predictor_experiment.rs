use std::{
    hint::black_box,
    time::{Duration, Instant},
};

use qatq::{
    TensorDType, decode_qatq_exact_tensor_le, qatq_exact_strategy, try_encode_qatq_exact_tensor_le,
};

const ELEMENTS: usize = 65_536;
const WARMUP_ITERATIONS: usize = 8;
const MEASURED_ITERATIONS: usize = 80;

fn main() {
    println!(
        "native bf16 predictor experiment: elements={ELEMENTS} iterations={MEASURED_ITERATIONS}"
    );
    for (name, words) in [
        ("smooth-wave", smooth_wave()),
        ("slow-ramp", slow_ramp()),
        ("piecewise-kv", piecewise_kv()),
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
    let decoded = decode_qatq_exact_tensor_le(&encoded).unwrap();
    assert_eq!(decoded.dtype, TensorDType::BF16);
    assert_eq!(decoded.bytes_le, source);

    for _ in 0..WARMUP_ITERATIONS {
        black_box(try_encode_qatq_exact_tensor_le(
            black_box(&source),
            TensorDType::BF16,
        ))
        .unwrap();
        black_box(decode_qatq_exact_tensor_le(black_box(&encoded))).unwrap();
    }

    let mut encode_elapsed = Duration::ZERO;
    let mut decode_elapsed = Duration::ZERO;
    for _ in 0..MEASURED_ITERATIONS {
        let start = Instant::now();
        let candidate =
            try_encode_qatq_exact_tensor_le(black_box(&source), TensorDType::BF16).unwrap();
        encode_elapsed += start.elapsed();
        assert_eq!(candidate, encoded);

        let start = Instant::now();
        let restored = decode_qatq_exact_tensor_le(black_box(&encoded)).unwrap();
        decode_elapsed += start.elapsed();
        assert_eq!(restored.bytes_le, source);
    }

    let values = (words.len() * MEASURED_ITERATIONS) as f64;
    println!(
        "{name}: raw={} encoded={} ratio={:.6} strategy={} encode_ns/value={:.3} decode_ns/value={:.3}",
        source.len(),
        encoded.len(),
        encoded.len() as f64 / source.len() as f64,
        qatq_exact_strategy(&encoded).unwrap().as_str(),
        encode_elapsed.as_nanos() as f64 / values,
        decode_elapsed.as_nanos() as f64 / values,
    );
}

fn bf16_bits(value: f32) -> u16 {
    let bits = value.to_bits();
    let rounding_bias = 0x7fff + ((bits >> 16) & 1);
    (bits.wrapping_add(rounding_bias) >> 16) as u16
}

fn smooth_wave() -> Vec<u16> {
    (0..ELEMENTS)
        .map(|index| {
            let x = index as f32;
            bf16_bits((x / 97.0).sin() * 0.5 + (x / 997.0).cos() * 0.125)
        })
        .collect()
}

fn slow_ramp() -> Vec<u16> {
    (0..ELEMENTS)
        .map(|index| bf16_bits(-1.0 + 2.0 * index as f32 / ELEMENTS as f32))
        .collect()
}

fn piecewise_kv() -> Vec<u16> {
    (0..ELEMENTS)
        .map(|index| {
            let token = index / 128;
            let channel = index % 128;
            let base =
                ((channel as f32 / 19.0).sin() * 0.375) + ((token as f32 / 113.0).cos() * 0.0625);
            bf16_bits(base)
        })
        .collect()
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
