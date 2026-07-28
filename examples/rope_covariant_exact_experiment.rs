use std::{
    env, fs,
    hint::black_box,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use qatq::{TensorDType, decode_qatq_exact_tensor_le, try_encode_qatq_exact_tensor_le};

const ROWS: usize = 17;
const EMBEDDING: usize = 128;
const HEAD_DIM: usize = 64;
const ROPE_BASE: f32 = 1_000_000.0;
const HEADER_BYTES: usize = 32;
const WARMUP: usize = 8;
const ITERATIONS: usize = 51;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Layout {
    Identity,
    Adjacent,
    Neox,
}

impl Layout {
    fn name(self) -> &'static str {
        match self {
            Self::Identity => "identity-control",
            Self::Adjacent => "adjacent",
            Self::Neox => "neox-half-split",
        }
    }
}

struct Dataset {
    label: String,
    bytes: Vec<u8>,
}

struct Candidate {
    layout: Layout,
    encoded_residuals: Vec<u8>,
}

fn main() {
    let datasets = parse_inputs();
    if datasets.is_empty() {
        eprintln!(
            "usage: rope_covariant_exact_experiment --input-dir label:path [--input label:path]"
        );
        std::process::exit(2);
    }

    println!(
        "| dataset | layout | raw bytes | qatq-exact bytes | candidate bytes | size change | exact | qatq enc ns/value | candidate enc change | qatq dec ns/value | candidate dec change |"
    );
    println!("| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: |");

    for dataset in datasets {
        evaluate(&dataset);
    }
}

fn parse_inputs() -> Vec<Dataset> {
    let mut args = env::args().skip(1);
    let mut datasets = Vec::new();

    while let Some(flag) = args.next() {
        let Some(spec) = args.next() else {
            panic!("missing value after {flag}");
        };
        let (label, path) = spec
            .split_once(':')
            .unwrap_or_else(|| panic!("expected label:path, got {spec}"));
        match flag.as_str() {
            "--input" => datasets.push(Dataset {
                label: label.to_owned(),
                bytes: fs::read(path).unwrap_or_else(|error| panic!("read {path}: {error}")),
            }),
            "--input-dir" => datasets.push(Dataset {
                label: label.to_owned(),
                bytes: read_directory(Path::new(path)),
            }),
            _ => panic!("unknown argument {flag}"),
        }
    }

    datasets
}

fn read_directory(path: &Path) -> Vec<u8> {
    let mut files: Vec<PathBuf> = fs::read_dir(path)
        .unwrap_or_else(|error| panic!("read directory {}: {error}", path.display()))
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
        let layer = fs::read(&file)
            .unwrap_or_else(|error| panic!("read layer {}: {error}", file.display()));
        assert_eq!(
            layer.len(),
            ROWS * EMBEDDING * 2,
            "{} does not match the documented {ROWS}x{EMBEDDING} capture shape",
            file.display()
        );
        bytes.extend_from_slice(&layer);
    }
    bytes
}

fn evaluate(dataset: &Dataset) {
    assert_eq!(dataset.bytes.len() % (ROWS * EMBEDDING * 2), 0);
    let production = try_encode_qatq_exact_tensor_le(&dataset.bytes, TensorDType::F16).unwrap();
    assert_eq!(
        decode_qatq_exact_tensor_le(&production).unwrap().bytes_le,
        dataset.bytes
    );

    let candidates = [Layout::Identity, Layout::Adjacent, Layout::Neox]
        .map(|layout| encode_candidate(&dataset.bytes, layout));
    for candidate in &candidates {
        let restored = decode_candidate(candidate, dataset.bytes.len());
        assert_eq!(restored, dataset.bytes);
    }

    for _ in 0..WARMUP {
        black_box(
            try_encode_qatq_exact_tensor_le(black_box(&dataset.bytes), TensorDType::F16).unwrap(),
        );
        for candidate in &candidates {
            black_box(encode_candidate(
                black_box(&dataset.bytes),
                candidate.layout,
            ));
        }
    }

    let mut production_encode = Duration::ZERO;
    let mut production_decode = Duration::ZERO;
    let mut candidate_encode = [Duration::ZERO; 3];
    let mut candidate_decode = [Duration::ZERO; 3];

    for _ in 0..ITERATIONS {
        let start = Instant::now();
        black_box(
            try_encode_qatq_exact_tensor_le(black_box(&dataset.bytes), TensorDType::F16).unwrap(),
        );
        production_encode += start.elapsed();

        let start = Instant::now();
        black_box(decode_qatq_exact_tensor_le(black_box(&production)).unwrap());
        production_decode += start.elapsed();

        for (index, candidate) in candidates.iter().enumerate() {
            let start = Instant::now();
            black_box(encode_candidate(
                black_box(&dataset.bytes),
                candidate.layout,
            ));
            candidate_encode[index] += start.elapsed();

            let start = Instant::now();
            black_box(decode_candidate(black_box(candidate), dataset.bytes.len()));
            candidate_decode[index] += start.elapsed();
        }
    }

    let values = (dataset.bytes.len() / 2 * ITERATIONS) as f64;
    let production_encode_ns = production_encode.as_nanos() as f64 / values;
    let production_decode_ns = production_decode.as_nanos() as f64 / values;

    for (index, candidate) in candidates.iter().enumerate() {
        let candidate_size = HEADER_BYTES + candidate.encoded_residuals.len();
        let size_change = percent_change(candidate_size as f64, production.len() as f64);
        let encode_ns = candidate_encode[index].as_nanos() as f64 / values;
        let decode_ns = candidate_decode[index].as_nanos() as f64 / values;
        println!(
            "| {} | {} | {} | {} | {} | {:+.2}% | yes | {:.3} | {:+.2}% | {:.3} | {:+.2}% |",
            dataset.label,
            candidate.layout.name(),
            dataset.bytes.len(),
            production.len(),
            candidate_size,
            size_change,
            production_encode_ns,
            percent_change(encode_ns, production_encode_ns),
            production_decode_ns,
            percent_change(decode_ns, production_decode_ns),
        );
    }
}

fn encode_candidate(bytes: &[u8], layout: Layout) -> Candidate {
    let words = bytes_to_words(bytes);
    let residuals = make_residuals(&words, layout);
    let residual_bytes = words_to_byte_planes(&residuals);
    Candidate {
        layout,
        encoded_residuals: zstd::bulk::compress(&residual_bytes, 3).unwrap(),
    }
}

fn decode_candidate(candidate: &Candidate, byte_len: usize) -> Vec<u8> {
    let residual_bytes = zstd::bulk::decompress(&candidate.encoded_residuals, byte_len).unwrap();
    let residuals = byte_planes_to_words(&residual_bytes);
    let words = restore_words(&residuals, candidate.layout);
    words_to_bytes(&words)
}

fn make_residuals(words: &[u16], layout: Layout) -> Vec<u16> {
    let mut residuals = vec![0; words.len()];
    let coefficients = rope_coefficients(layout);
    for (layer_index, layer) in words.chunks_exact(ROWS * EMBEDDING).enumerate() {
        let layer_offset = layer_index * ROWS * EMBEDDING;
        residuals[layer_offset..layer_offset + EMBEDDING].copy_from_slice(&layer[..EMBEDDING]);
        for row in 1..ROWS {
            for head in 0..EMBEDDING / HEAD_DIM {
                predict_head(
                    &layer[(row - 1) * EMBEDDING + head * HEAD_DIM..],
                    &mut residuals[layer_offset + row * EMBEDDING + head * HEAD_DIM..],
                    &layer[row * EMBEDDING + head * HEAD_DIM..],
                    layout,
                    &coefficients,
                );
            }
        }
    }
    residuals
}

fn restore_words(residuals: &[u16], layout: Layout) -> Vec<u16> {
    let mut words = vec![0; residuals.len()];
    let coefficients = rope_coefficients(layout);
    for layer_index in 0..residuals.len() / (ROWS * EMBEDDING) {
        let layer_offset = layer_index * ROWS * EMBEDDING;
        words[layer_offset..layer_offset + EMBEDDING]
            .copy_from_slice(&residuals[layer_offset..layer_offset + EMBEDDING]);
        for row in 1..ROWS {
            for head in 0..EMBEDDING / HEAD_DIM {
                let previous_start = layer_offset + (row - 1) * EMBEDDING + head * HEAD_DIM;
                let current_start = layer_offset + row * EMBEDDING + head * HEAD_DIM;
                let previous = words[previous_start..previous_start + HEAD_DIM].to_vec();
                let correction = &residuals[current_start..current_start + HEAD_DIM];
                let target = &mut words[current_start..current_start + HEAD_DIM];
                predict_head(&previous, target, correction, layout, &coefficients);
            }
        }
    }
    words
}

fn predict_head(
    previous: &[u16],
    output: &mut [u16],
    actual_or_residual: &[u16],
    layout: Layout,
    coefficients: &[(f32, f32); HEAD_DIM / 2],
) {
    for (pair, &(sin, cos)) in coefficients.iter().enumerate() {
        let (left, right) = pair_indices(pair, layout);
        let x = f16_to_f32(previous[left]);
        let y = f16_to_f32(previous[right]);
        let predicted_left = f32_to_f16(x * cos - y * sin);
        let predicted_right = f32_to_f16(x * sin + y * cos);
        output[left] = predicted_left ^ actual_or_residual[left];
        output[right] = predicted_right ^ actual_or_residual[right];
    }
}

fn rope_coefficients(layout: Layout) -> [(f32, f32); HEAD_DIM / 2] {
    std::array::from_fn(|pair| {
        if layout == Layout::Identity {
            (0.0, 1.0)
        } else {
            let frequency = ROPE_BASE.powf(-((2 * pair) as f32) / HEAD_DIM as f32);
            frequency.sin_cos()
        }
    })
}

fn pair_indices(pair: usize, layout: Layout) -> (usize, usize) {
    match layout {
        Layout::Identity | Layout::Adjacent => (pair * 2, pair * 2 + 1),
        Layout::Neox => (pair, pair + HEAD_DIM / 2),
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
    let word_count = bytes.len() / 2;
    (0..word_count)
        .map(|index| u16::from_le_bytes([bytes[index], bytes[word_count + index]]))
        .collect()
}

fn percent_change(candidate: f64, baseline: f64) -> f64 {
    (candidate / baseline - 1.0) * 100.0
}

fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits & 0x8000) as u32) << 16;
    let exponent = (bits >> 10) & 0x1f;
    let fraction = bits & 0x03ff;
    let result = match exponent {
        0 if fraction == 0 => sign,
        0 => {
            let mut fraction = fraction as u32;
            let mut exponent = 113_u32;
            while fraction & 0x0400 == 0 {
                fraction <<= 1;
                exponent -= 1;
            }
            sign | (exponent << 23) | ((fraction & 0x03ff) << 13)
        }
        0x1f => sign | 0x7f80_0000 | ((fraction as u32) << 13),
        _ => sign | (((exponent as u32) + 112) << 23) | ((fraction as u32) << 13),
    };
    f32::from_bits(result)
}

fn f32_to_f16(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32;
    let fraction = bits & 0x007f_ffff;

    if exponent == 0xff {
        return sign | 0x7c00 | if fraction == 0 { 0 } else { 0x0200 };
    }

    let half_exponent = exponent - 127 + 15;
    if half_exponent >= 0x1f {
        return sign | 0x7c00;
    }
    if half_exponent <= 0 {
        if half_exponent < -10 {
            return sign;
        }
        let mantissa = fraction | 0x0080_0000;
        let shift = (14 - half_exponent) as u32;
        let rounded = (mantissa + ((1 << (shift - 1)) - 1) + ((mantissa >> shift) & 1)) >> shift;
        return sign | rounded as u16;
    }

    let rounded = fraction + 0x0fff + ((fraction >> 13) & 1);
    if rounded & 0x0080_0000 != 0 {
        let next_exponent = half_exponent + 1;
        if next_exponent >= 0x1f {
            sign | 0x7c00
        } else {
            sign | ((next_exponent as u16) << 10)
        }
    } else {
        sign | ((half_exponent as u16) << 10) | ((rounded >> 13) as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_conversion_preserves_representative_bits() {
        for bits in [0, 1, 0x03ff, 0x0400, 0x3c00, 0x8000, 0xbc00, 0x7c00] {
            assert_eq!(f32_to_f16(f16_to_f32(bits)), bits);
        }
    }

    #[test]
    fn both_layouts_restore_every_source_bit() {
        let mut state = 0x524f_5045_5141_5451_u64;
        let mut words = Vec::with_capacity(ROWS * EMBEDDING * 2);
        for _ in 0..ROWS * EMBEDDING * 2 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            words.push(state as u16);
        }
        for layout in [Layout::Identity, Layout::Adjacent, Layout::Neox] {
            let residuals = make_residuals(&words, layout);
            assert_eq!(restore_words(&residuals, layout), words);
        }
    }

    #[test]
    fn neox_predictor_removes_a_known_rope_orbit() {
        let coefficients = rope_coefficients(Layout::Neox);
        let mut words = vec![0_u16; ROWS * EMBEDDING];
        for (index, word) in words[..EMBEDDING].iter_mut().enumerate() {
            *word = f32_to_f16((index as f32 / 11.0).sin());
        }
        for row in 1..ROWS {
            for head in 0..EMBEDDING / HEAD_DIM {
                let previous_start = (row - 1) * EMBEDDING + head * HEAD_DIM;
                let current_start = row * EMBEDDING + head * HEAD_DIM;
                let previous = words[previous_start..previous_start + HEAD_DIM].to_vec();
                let zeros = [0_u16; HEAD_DIM];
                predict_head(
                    &previous,
                    &mut words[current_start..current_start + HEAD_DIM],
                    &zeros,
                    Layout::Neox,
                    &coefficients,
                );
            }
        }

        let residuals = make_residuals(&words, Layout::Neox);
        assert!(residuals[EMBEDDING..].iter().all(|word| *word == 0));
        assert_eq!(restore_words(&residuals, Layout::Neox), words);
    }
}
