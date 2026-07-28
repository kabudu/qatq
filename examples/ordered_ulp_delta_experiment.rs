use std::{
    env, fs,
    hint::black_box,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use qatq::{
    TensorDType, decode_qatq_exact_tensor_le, try_encode_qatq_exact_tensor_le_with_stride_hint,
};

const ROWS: usize = 17;
const STRIDE: usize = 128;
const LAYER_WORDS: usize = ROWS * STRIDE;
const HEADER_BYTES: usize = 24;
const WARMUP: usize = 8;
const ITERATIONS: usize = 51;

#[derive(Clone, Copy)]
enum Predictor {
    Adjacent,
    Strided,
    StridedSecondOrder,
}

impl Predictor {
    fn name(self) -> &'static str {
        match self {
            Self::Adjacent => "adjacent",
            Self::Strided => "stride-128",
            Self::StridedSecondOrder => "stride-128-second-order",
        }
    }
}

struct Candidate {
    predictor: Predictor,
    body: Vec<u8>,
}

fn main() {
    let datasets = parse_inputs();
    if datasets.is_empty() {
        eprintln!(
            "usage: ordered_ulp_delta_experiment --input-dir label:path [--input label:path]"
        );
        std::process::exit(2);
    }

    println!(
        "| dataset | predictor | raw | qatq-exact | ULP candidate | size change | exact | encode change | decode change |"
    );
    println!("| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |");
    for (label, bytes) in datasets {
        evaluate(&label, &bytes);
    }
}

fn parse_inputs() -> Vec<(String, Vec<u8>)> {
    let mut args = env::args().skip(1);
    let mut datasets = Vec::new();
    while let Some(flag) = args.next() {
        let spec = args
            .next()
            .unwrap_or_else(|| panic!("missing value after {flag}"));
        let (label, path) = spec
            .split_once(':')
            .unwrap_or_else(|| panic!("expected label:path, got {spec}"));
        let bytes = match flag.as_str() {
            "--input" => fs::read(path).unwrap(),
            "--input-dir" => read_directory(Path::new(path)),
            _ => panic!("unknown argument {flag}"),
        };
        datasets.push((label.to_owned(), bytes));
    }
    datasets
}

fn read_directory(path: &Path) -> Vec<u8> {
    let mut files: Vec<PathBuf> = fs::read_dir(path)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "f16le")
        })
        .collect();
    files.sort();
    let mut bytes = Vec::new();
    for file in files {
        let layer = fs::read(&file).unwrap();
        assert_eq!(layer.len(), LAYER_WORDS * 2);
        bytes.extend_from_slice(&layer);
    }
    bytes
}

fn evaluate(label: &str, bytes: &[u8]) {
    assert_eq!(bytes.len() % (LAYER_WORDS * 2), 0);
    let production =
        try_encode_qatq_exact_tensor_le_with_stride_hint(bytes, TensorDType::F16, STRIDE).unwrap();
    assert_eq!(
        decode_qatq_exact_tensor_le(&production).unwrap().bytes_le,
        bytes
    );

    let candidates = [
        Predictor::Adjacent,
        Predictor::Strided,
        Predictor::StridedSecondOrder,
    ]
    .map(|predictor| encode_candidate(bytes, predictor));
    for candidate in &candidates {
        assert_eq!(decode_candidate(candidate, bytes.len()), bytes);
    }

    for _ in 0..WARMUP {
        black_box(
            try_encode_qatq_exact_tensor_le_with_stride_hint(
                black_box(bytes),
                TensorDType::F16,
                STRIDE,
            )
            .unwrap(),
        );
        for candidate in &candidates {
            black_box(encode_candidate(black_box(bytes), candidate.predictor));
            black_box(decode_candidate(black_box(candidate), bytes.len()));
        }
    }

    let mut production_encode = Duration::ZERO;
    let mut production_decode = Duration::ZERO;
    let mut candidate_encode = [Duration::ZERO; 3];
    let mut candidate_decode = [Duration::ZERO; 3];
    for _ in 0..ITERATIONS {
        let start = Instant::now();
        black_box(
            try_encode_qatq_exact_tensor_le_with_stride_hint(
                black_box(bytes),
                TensorDType::F16,
                STRIDE,
            )
            .unwrap(),
        );
        production_encode += start.elapsed();

        let start = Instant::now();
        black_box(decode_qatq_exact_tensor_le(black_box(&production)).unwrap());
        production_decode += start.elapsed();

        for (index, candidate) in candidates.iter().enumerate() {
            let start = Instant::now();
            black_box(encode_candidate(black_box(bytes), candidate.predictor));
            candidate_encode[index] += start.elapsed();

            let start = Instant::now();
            black_box(decode_candidate(black_box(candidate), bytes.len()));
            candidate_decode[index] += start.elapsed();
        }
    }

    for (index, candidate) in candidates.iter().enumerate() {
        let candidate_size = HEADER_BYTES + candidate.body.len();
        println!(
            "| {label} | {} | {} | {} | {} | {:+.2}% | yes | {:+.2}% | {:+.2}% |",
            candidate.predictor.name(),
            bytes.len(),
            production.len(),
            candidate_size,
            percent_change(candidate_size as f64, production.len() as f64),
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

fn encode_candidate(bytes: &[u8], predictor: Predictor) -> Candidate {
    let words = bytes_to_words(bytes);
    let residuals = encode_residuals(&words, predictor);
    let planes = words_to_byte_planes(&residuals);
    Candidate {
        predictor,
        body: zstd::bulk::compress(&planes, 3).unwrap(),
    }
}

fn decode_candidate(candidate: &Candidate, byte_len: usize) -> Vec<u8> {
    let planes = zstd::bulk::decompress(&candidate.body, byte_len).unwrap();
    let residuals = byte_planes_to_words(&planes);
    words_to_bytes(&decode_residuals(&residuals, candidate.predictor))
}

fn encode_residuals(words: &[u16], predictor: Predictor) -> Vec<u16> {
    let mut residuals = Vec::with_capacity(words.len());
    for layer in words.chunks_exact(LAYER_WORDS) {
        for index in 0..layer.len() {
            let distance = if matches!(predictor, Predictor::Adjacent) {
                1
            } else {
                STRIDE
            };
            let prefix = if matches!(predictor, Predictor::StridedSecondOrder) {
                distance * 2
            } else {
                distance
            };
            if index < prefix {
                residuals.push(layer[index]);
            } else {
                let current = float_flip(layer[index]);
                let previous = float_flip(layer[index - distance]);
                let predicted = if matches!(predictor, Predictor::StridedSecondOrder) {
                    previous
                        .wrapping_mul(2)
                        .wrapping_sub(float_flip(layer[index - 2 * distance]))
                } else {
                    previous
                };
                residuals.push(zigzag(current.wrapping_sub(predicted) as i16));
            }
        }
    }
    residuals
}

fn decode_residuals(residuals: &[u16], predictor: Predictor) -> Vec<u16> {
    let mut words = vec![0; residuals.len()];
    for layer_index in 0..residuals.len() / LAYER_WORDS {
        let offset = layer_index * LAYER_WORDS;
        for index in 0..LAYER_WORDS {
            let distance = if matches!(predictor, Predictor::Adjacent) {
                1
            } else {
                STRIDE
            };
            let prefix = if matches!(predictor, Predictor::StridedSecondOrder) {
                distance * 2
            } else {
                distance
            };
            words[offset + index] = if index < prefix {
                residuals[offset + index]
            } else {
                let previous = float_flip(words[offset + index - distance]);
                let predicted = if matches!(predictor, Predictor::StridedSecondOrder) {
                    previous
                        .wrapping_mul(2)
                        .wrapping_sub(float_flip(words[offset + index - 2 * distance]))
                } else {
                    previous
                };
                unfloat_flip(predicted.wrapping_add(unzigzag(residuals[offset + index]) as u16))
            };
        }
    }
    words
}

fn float_flip(bits: u16) -> u16 {
    if bits & 0x8000 == 0 {
        bits ^ 0x8000
    } else {
        !bits
    }
}

fn unfloat_flip(ordered: u16) -> u16 {
    if ordered & 0x8000 == 0 {
        !ordered
    } else {
        ordered ^ 0x8000
    }
}

fn zigzag(value: i16) -> u16 {
    ((value << 1) ^ (value >> 15)) as u16
}

fn unzigzag(value: u16) -> i16 {
    ((value >> 1) as i16) ^ -((value & 1) as i16)
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
    fn float_flip_is_a_bijection_over_every_word() {
        for word in 0..=u16::MAX {
            assert_eq!(unfloat_flip(float_flip(word)), word);
        }
    }

    #[test]
    fn zigzag_is_a_bijection_over_every_word() {
        for word in 0..=u16::MAX {
            assert_eq!(unzigzag(zigzag(word as i16)) as u16, word);
        }
    }

    #[test]
    fn both_predictors_restore_arbitrary_words() {
        let mut state = 0x554c_5044_5141_5451_u64;
        let mut words = Vec::with_capacity(LAYER_WORDS * 2);
        for _ in 0..LAYER_WORDS * 2 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            words.push(state as u16);
        }
        for predictor in [
            Predictor::Adjacent,
            Predictor::Strided,
            Predictor::StridedSecondOrder,
        ] {
            let residuals = encode_residuals(&words, predictor);
            assert_eq!(decode_residuals(&residuals, predictor), words);
        }
    }
}
