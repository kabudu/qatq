use std::{
    env, fs,
    hint::black_box,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use qatq::{
    TensorDType, decode_qatq_exact_tensor_le, try_encode_qatq_exact_tensor_le_with_stride_hint,
};

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
            Self::Strided => "strided",
            Self::StridedSecondOrder => "strided-second-order",
        }
    }
}

struct Candidate {
    predictor: Predictor,
    layer_words: usize,
    stride: usize,
    body: Vec<u8>,
}

fn main() {
    let (config, datasets) = parse_inputs();
    if datasets.is_empty() || config.rows == 0 || config.stride == 0 {
        eprintln!(
            "usage: ordered_ulp_delta_experiment --rows N --stride N --dtype f16|bf16 --input-dir label:path"
        );
        std::process::exit(2);
    }

    println!(
        "| dataset | predictor | raw | qatq-exact | ULP candidate | size change | exact | encode change | decode change |"
    );
    println!("| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |");
    for (label, bytes) in datasets {
        evaluate(&label, &bytes, config);
    }
}

#[derive(Clone, Copy)]
struct Config {
    rows: usize,
    stride: usize,
    dtype: TensorDType,
}

fn parse_inputs() -> (Config, Vec<(String, Vec<u8>)>) {
    let mut args = env::args().skip(1);
    let mut datasets = Vec::new();
    let mut rows = 0;
    let mut stride = 0;
    let mut dtype = TensorDType::F16;
    while let Some(flag) = args.next() {
        let spec = args
            .next()
            .unwrap_or_else(|| panic!("missing value after {flag}"));
        match flag.as_str() {
            "--rows" => rows = spec.parse().expect("rows must be an integer"),
            "--stride" => stride = spec.parse().expect("stride must be an integer"),
            "--dtype" => {
                dtype = match spec.as_str() {
                    "f16" => TensorDType::F16,
                    "bf16" => TensorDType::BF16,
                    _ => panic!("dtype must be f16 or bf16"),
                }
            }
            "--input" | "--input-dir" => {
                let (label, path) = spec
                    .split_once(':')
                    .unwrap_or_else(|| panic!("expected label:path, got {spec}"));
                let bytes = if flag == "--input" {
                    fs::read(path).unwrap()
                } else {
                    read_directory(Path::new(path), dtype)
                };
                datasets.push((label.to_owned(), bytes));
            }
            _ => panic!("unknown argument {flag}"),
        }
    }
    (
        Config {
            rows,
            stride,
            dtype,
        },
        datasets,
    )
}

fn read_directory(path: &Path, dtype: TensorDType) -> Vec<u8> {
    let expected_extension = match dtype {
        TensorDType::F16 => "f16le",
        TensorDType::BF16 => "bf16le",
        _ => unreachable!(),
    };
    let mut files: Vec<PathBuf> = fs::read_dir(path)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == expected_extension)
        })
        .collect();
    files.sort();
    let mut bytes = Vec::new();
    for file in files {
        bytes.extend_from_slice(&fs::read(&file).unwrap());
    }
    bytes
}

fn evaluate(label: &str, bytes: &[u8], config: Config) {
    let layer_words = config.rows * config.stride;
    assert_eq!(bytes.len() % (layer_words * 2), 0);
    let production =
        try_encode_qatq_exact_tensor_le_with_stride_hint(bytes, config.dtype, config.stride)
            .unwrap();
    assert_eq!(
        decode_qatq_exact_tensor_le(&production).unwrap().bytes_le,
        bytes
    );

    let candidates = [
        Predictor::Adjacent,
        Predictor::Strided,
        Predictor::StridedSecondOrder,
    ]
    .map(|predictor| encode_candidate(bytes, predictor, layer_words, config.stride));
    for candidate in &candidates {
        assert_eq!(decode_candidate(candidate, bytes.len()), bytes);
    }

    for _ in 0..WARMUP {
        black_box(
            try_encode_qatq_exact_tensor_le_with_stride_hint(
                black_box(bytes),
                config.dtype,
                config.stride,
            )
            .unwrap(),
        );
        for candidate in &candidates {
            black_box(encode_candidate(
                black_box(bytes),
                candidate.predictor,
                layer_words,
                config.stride,
            ));
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
                black_box(bytes),
                candidate.predictor,
                layer_words,
                config.stride,
            ));
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

fn encode_candidate(
    bytes: &[u8],
    predictor: Predictor,
    layer_words: usize,
    stride: usize,
) -> Candidate {
    let words = bytes_to_words(bytes);
    let residuals = encode_residuals(&words, predictor, layer_words, stride);
    let planes = words_to_byte_planes(&residuals);
    Candidate {
        predictor,
        layer_words,
        stride,
        body: zstd::bulk::compress(&planes, 3).unwrap(),
    }
}

fn decode_candidate(candidate: &Candidate, byte_len: usize) -> Vec<u8> {
    let planes = zstd::bulk::decompress(&candidate.body, byte_len).unwrap();
    let residuals = byte_planes_to_words(&planes);
    words_to_bytes(&decode_residuals(
        &residuals,
        candidate.predictor,
        candidate.layer_words,
        candidate.stride,
    ))
}

fn encode_residuals(
    words: &[u16],
    predictor: Predictor,
    layer_words: usize,
    stride: usize,
) -> Vec<u16> {
    let mut residuals = Vec::with_capacity(words.len());
    for layer in words.chunks_exact(layer_words) {
        for index in 0..layer.len() {
            let distance = if matches!(predictor, Predictor::Adjacent) {
                1
            } else {
                stride
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

fn decode_residuals(
    residuals: &[u16],
    predictor: Predictor,
    layer_words: usize,
    stride: usize,
) -> Vec<u16> {
    let mut words = vec![0; residuals.len()];
    for layer_index in 0..residuals.len() / layer_words {
        let offset = layer_index * layer_words;
        for index in 0..layer_words {
            let distance = if matches!(predictor, Predictor::Adjacent) {
                1
            } else {
                stride
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
        const TEST_ROWS: usize = 17;
        const TEST_STRIDE: usize = 128;
        const TEST_LAYER_WORDS: usize = TEST_ROWS * TEST_STRIDE;
        let mut state = 0x554c_5044_5141_5451_u64;
        let mut words = Vec::with_capacity(TEST_LAYER_WORDS * 2);
        for _ in 0..TEST_LAYER_WORDS * 2 {
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
            let residuals = encode_residuals(&words, predictor, TEST_LAYER_WORDS, TEST_STRIDE);
            assert_eq!(
                decode_residuals(&residuals, predictor, TEST_LAYER_WORDS, TEST_STRIDE),
                words
            );
        }
    }
}
