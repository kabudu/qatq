use std::{
    env, fs,
    hint::black_box,
    path::PathBuf,
    time::{Duration, Instant},
};

use qatq::{
    TensorDType, decode_qatq_exact_tensor_le, try_encode_qatq_exact_tensor_le_with_stride_hint,
};

const HEADER_BYTES: usize = 32;
const WARMUP: usize = 5;
const ITERATIONS: usize = 31;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Step {
    KvXor,
    KvSub,
    LayerXor,
    LayerSub,
    HeadXor(usize),
}

#[derive(Clone)]
struct Candidate {
    name: &'static str,
    steps: Vec<Step>,
    body: Vec<u8>,
}

#[derive(Clone, Copy)]
struct Config {
    rows: usize,
    stride: usize,
    dtype: TensorDType,
}

fn main() {
    let (config, label, k_bytes, v_bytes) = parse_args();
    assert_eq!(k_bytes.len(), v_bytes.len());
    assert_eq!(k_bytes.len() % (config.rows * config.stride * 2), 0);
    evaluate(&label, &k_bytes, &v_bytes, config);
}

fn parse_args() -> (Config, String, Vec<u8>, Vec<u8>) {
    let mut rows = 0;
    let mut stride = 0;
    let mut dtype = TensorDType::F16;
    let mut label = String::new();
    let mut k_path = PathBuf::new();
    let mut v_path = PathBuf::new();
    let mut args = env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .unwrap_or_else(|| panic!("missing value after {flag}"));
        match flag.as_str() {
            "--rows" => rows = value.parse().expect("rows must be an integer"),
            "--stride" => stride = value.parse().expect("stride must be an integer"),
            "--dtype" => {
                dtype = match value.as_str() {
                    "f16" => TensorDType::F16,
                    "bf16" => TensorDType::BF16,
                    _ => panic!("dtype must be f16 or bf16"),
                }
            }
            "--label" => label = value,
            "--k" => k_path = value.into(),
            "--v" => v_path = value.into(),
            _ => panic!("unknown argument {flag}"),
        }
    }
    assert!(rows > 0 && stride > 0 && !label.is_empty());
    (
        Config {
            rows,
            stride,
            dtype,
        },
        label,
        fs::read(&k_path).unwrap_or_else(|error| panic!("read {}: {error}", k_path.display())),
        fs::read(&v_path).unwrap_or_else(|error| panic!("read {}: {error}", v_path.display())),
    )
}

fn evaluate(label: &str, k_bytes: &[u8], v_bytes: &[u8], config: Config) {
    let mut source = Vec::with_capacity(k_bytes.len() + v_bytes.len());
    source.extend_from_slice(k_bytes);
    source.extend_from_slice(v_bytes);
    let production =
        try_encode_qatq_exact_tensor_le_with_stride_hint(&source, config.dtype, config.stride)
            .unwrap();
    assert_eq!(
        decode_qatq_exact_tensor_le(&production).unwrap().bytes_le,
        source
    );

    let specs = candidate_specs(config.stride);
    let candidates: Vec<Candidate> = specs
        .into_iter()
        .filter_map(|(name, steps)| encode_candidate(k_bytes, v_bytes, config, name, steps))
        .collect();
    for candidate in &candidates {
        assert_eq!(decode_candidate(candidate, k_bytes.len(), config), source);
    }

    for _ in 0..WARMUP {
        black_box(
            try_encode_qatq_exact_tensor_le_with_stride_hint(
                black_box(&source),
                config.dtype,
                config.stride,
            )
            .unwrap(),
        );
        for candidate in &candidates {
            black_box(encode_candidate(
                black_box(k_bytes),
                black_box(v_bytes),
                config,
                candidate.name,
                candidate.steps.clone(),
            ));
            black_box(decode_candidate(
                black_box(candidate),
                k_bytes.len(),
                config,
            ));
        }
    }

    let mut production_encode = Duration::ZERO;
    let mut production_decode = Duration::ZERO;
    let mut candidate_encode = vec![Duration::ZERO; candidates.len()];
    let mut candidate_decode = vec![Duration::ZERO; candidates.len()];
    for _ in 0..ITERATIONS {
        let start = Instant::now();
        black_box(
            try_encode_qatq_exact_tensor_le_with_stride_hint(
                black_box(&source),
                config.dtype,
                config.stride,
            )
            .unwrap(),
        );
        production_encode += start.elapsed();

        let start = Instant::now();
        black_box(decode_qatq_exact_tensor_le(black_box(&production)).unwrap());
        production_decode += start.elapsed();

        for (index, candidate) in candidates.iter().enumerate() {
            let start = Instant::now();
            black_box(encode_candidate(
                black_box(k_bytes),
                black_box(v_bytes),
                config,
                candidate.name,
                candidate.steps.clone(),
            ));
            candidate_encode[index] += start.elapsed();

            let start = Instant::now();
            black_box(decode_candidate(
                black_box(candidate),
                k_bytes.len(),
                config,
            ));
            candidate_decode[index] += start.elapsed();
        }
    }

    println!(
        "| dataset | transform | raw | qatq-exact | candidate | size change | exact | encode change | decode change |"
    );
    println!("| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |");
    for (index, candidate) in candidates.iter().enumerate() {
        let size = HEADER_BYTES + candidate.body.len();
        println!(
            "| {label} | {} | {} | {} | {} | {:+.2}% | yes | {:+.2}% | {:+.2}% |",
            candidate.name,
            source.len(),
            production.len(),
            size,
            percent_change(size as f64, production.len() as f64),
            percent_change(
                candidate_encode[index].as_nanos() as f64,
                production_encode.as_nanos() as f64,
            ),
            percent_change(
                candidate_decode[index].as_nanos() as f64,
                production_decode.as_nanos() as f64,
            ),
        );
    }
}

fn candidate_specs(stride: usize) -> Vec<(&'static str, Vec<Step>)> {
    let mut specs = vec![
        ("identity-control", vec![]),
        ("kv-xor", vec![Step::KvXor]),
        ("kv-sub", vec![Step::KvSub]),
        ("layer-xor", vec![Step::LayerXor]),
        ("layer-sub", vec![Step::LayerSub]),
        ("layer-xor+kv-xor", vec![Step::LayerXor, Step::KvXor]),
        ("layer-sub+kv-sub", vec![Step::LayerSub, Step::KvSub]),
    ];
    if stride.is_multiple_of(64) {
        specs.push(("head64-xor", vec![Step::HeadXor(64)]));
        specs.push((
            "head64+layer+kv-xor",
            vec![Step::HeadXor(64), Step::LayerXor, Step::KvXor],
        ));
    }
    if stride.is_multiple_of(128) && stride > 128 {
        specs.push(("head128-xor", vec![Step::HeadXor(128)]));
        specs.push((
            "head128+layer+kv-xor",
            vec![Step::HeadXor(128), Step::LayerXor, Step::KvXor],
        ));
    }
    specs
}

fn encode_candidate(
    k_bytes: &[u8],
    v_bytes: &[u8],
    config: Config,
    name: &'static str,
    steps: Vec<Step>,
) -> Option<Candidate> {
    let mut k = bytes_to_words(k_bytes);
    let mut v = bytes_to_words(v_bytes);
    for step in &steps {
        apply_forward(*step, &mut k, &mut v, config);
    }
    let mut transformed = k;
    transformed.extend_from_slice(&v);
    let planes = words_to_byte_planes(&transformed);
    Some(Candidate {
        name,
        steps,
        body: zstd::bulk::compress(&planes, 3).ok()?,
    })
}

fn decode_candidate(candidate: &Candidate, stream_byte_len: usize, config: Config) -> Vec<u8> {
    let transformed_len = stream_byte_len * 2;
    let planes = zstd::bulk::decompress(&candidate.body, transformed_len).unwrap();
    let transformed = byte_planes_to_words(&planes);
    let split = transformed.len() / 2;
    let mut k = transformed[..split].to_vec();
    let mut v = transformed[split..].to_vec();
    for step in candidate.steps.iter().rev() {
        apply_inverse(*step, &mut k, &mut v, config);
    }
    let mut bytes = words_to_bytes(&k);
    bytes.extend_from_slice(&words_to_bytes(&v));
    bytes
}

fn apply_forward(step: Step, k: &mut [u16], v: &mut [u16], config: Config) {
    match step {
        Step::KvXor => {
            for (v, k) in v.iter_mut().zip(k.iter()) {
                *v ^= *k;
            }
        }
        Step::KvSub => {
            for (v, k) in v.iter_mut().zip(k.iter()) {
                *v = v.wrapping_sub(*k);
            }
        }
        Step::LayerXor => {
            layer_forward(k, config, |current, previous| current ^ previous);
            layer_forward(v, config, |current, previous| current ^ previous);
        }
        Step::LayerSub => {
            layer_forward(k, config, u16::wrapping_sub);
            layer_forward(v, config, u16::wrapping_sub);
        }
        Step::HeadXor(width) => {
            head_forward(k, config, width);
            head_forward(v, config, width);
        }
    }
}

fn apply_inverse(step: Step, k: &mut [u16], v: &mut [u16], config: Config) {
    match step {
        Step::KvXor => {
            for (v, k) in v.iter_mut().zip(k.iter()) {
                *v ^= *k;
            }
        }
        Step::KvSub => {
            for (v, k) in v.iter_mut().zip(k.iter()) {
                *v = v.wrapping_add(*k);
            }
        }
        Step::LayerXor => {
            layer_inverse(k, config, |residual, previous| residual ^ previous);
            layer_inverse(v, config, |residual, previous| residual ^ previous);
        }
        Step::LayerSub => {
            layer_inverse(k, config, u16::wrapping_add);
            layer_inverse(v, config, u16::wrapping_add);
        }
        Step::HeadXor(width) => {
            head_inverse(k, config, width);
            head_inverse(v, config, width);
        }
    }
}

fn layer_forward(words: &mut [u16], config: Config, op: fn(u16, u16) -> u16) {
    let layer_words = config.rows * config.stride;
    let layers = words.len() / layer_words;
    for layer in (1..layers).rev() {
        for index in 0..layer_words {
            let current = layer * layer_words + index;
            words[current] = op(words[current], words[current - layer_words]);
        }
    }
}

fn layer_inverse(words: &mut [u16], config: Config, op: fn(u16, u16) -> u16) {
    let layer_words = config.rows * config.stride;
    let layers = words.len() / layer_words;
    for layer in 1..layers {
        for index in 0..layer_words {
            let current = layer * layer_words + index;
            words[current] = op(words[current], words[current - layer_words]);
        }
    }
}

fn head_forward(words: &mut [u16], config: Config, width: usize) {
    let layer_words = config.rows * config.stride;
    for layer in words.chunks_exact_mut(layer_words) {
        for row in layer.chunks_exact_mut(config.stride) {
            let heads = row.len() / width;
            for head in (1..heads).rev() {
                for lane in 0..width {
                    row[head * width + lane] ^= row[(head - 1) * width + lane];
                }
            }
        }
    }
}

fn head_inverse(words: &mut [u16], config: Config, width: usize) {
    let layer_words = config.rows * config.stride;
    for layer in words.chunks_exact_mut(layer_words) {
        for row in layer.chunks_exact_mut(config.stride) {
            let heads = row.len() / width;
            for head in 1..heads {
                for lane in 0..width {
                    row[head * width + lane] ^= row[(head - 1) * width + lane];
                }
            }
        }
    }
}

fn bytes_to_words(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect()
}

fn words_to_bytes(words: &[u16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(words.len() * 2);
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes
}

fn words_to_byte_planes(words: &[u16]) -> Vec<u8> {
    let mut bytes = vec![0; words.len() * 2];
    for (index, word) in words.iter().enumerate() {
        let [low, high] = word.to_le_bytes();
        bytes[index] = low;
        bytes[words.len() + index] = high;
    }
    bytes
}

fn byte_planes_to_words(bytes: &[u8]) -> Vec<u16> {
    let count = bytes.len() / 2;
    (0..count)
        .map(|index| u16::from_le_bytes([bytes[index], bytes[count + index]]))
        .collect()
}

fn percent_change(candidate: f64, baseline: f64) -> f64 {
    (candidate / baseline - 1.0) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_transform_roundtrips_arbitrary_words() {
        let config = Config {
            rows: 5,
            stride: 256,
            dtype: TensorDType::F16,
        };
        let count = config.rows * config.stride * 3;
        let mut state = 0x4c49_4654_5141_5451_u64;
        let mut k = Vec::with_capacity(count);
        let mut v = Vec::with_capacity(count);
        for _ in 0..count {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            k.push(state as u16);
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            v.push(state as u16);
        }
        for (_, steps) in candidate_specs(config.stride) {
            let expected_k = k.clone();
            let expected_v = v.clone();
            for step in &steps {
                apply_forward(*step, &mut k, &mut v, config);
            }
            for step in steps.iter().rev() {
                apply_inverse(*step, &mut k, &mut v, config);
            }
            assert_eq!(k, expected_k);
            assert_eq!(v, expected_v);
        }
    }
}
