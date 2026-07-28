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

#[derive(Clone, Copy, Debug)]
enum ExponentCoding {
    RawFields,
    Lane0Reference,
    ModeReference,
}

impl ExponentCoding {
    fn name(self) -> &'static str {
        match self {
            Self::RawFields => "field-split-control",
            Self::Lane0Reference => "4+1-lane0-exponent",
            Self::ModeReference => "4+1-mode-exponent",
        }
    }
}

struct Format {
    exponent_bits: usize,
    mantissa_bits: usize,
}

struct Candidate {
    coding: ExponentCoding,
    body: Vec<u8>,
    uncompressed_len: usize,
    sign_len: usize,
    mantissa_len: usize,
    exponent_len: usize,
}

fn main() {
    let (label, path, dtype, stride) = parse_args();
    let bytes = fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    evaluate(&label, &bytes, dtype, stride);
}

fn parse_args() -> (String, PathBuf, TensorDType, usize) {
    let mut label = String::new();
    let mut path = PathBuf::new();
    let mut dtype = TensorDType::F16;
    let mut stride = 0;
    let mut args = env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .unwrap_or_else(|| panic!("missing value after {flag}"));
        match flag.as_str() {
            "--label" => label = value,
            "--input" => path = value.into(),
            "--stride" => stride = value.parse().expect("stride must be an integer"),
            "--dtype" => {
                dtype = match value.as_str() {
                    "f16" => TensorDType::F16,
                    "bf16" => TensorDType::BF16,
                    _ => panic!("dtype must be f16 or bf16"),
                }
            }
            _ => panic!("unknown argument {flag}"),
        }
    }
    assert!(!label.is_empty() && !path.as_os_str().is_empty() && stride > 0);
    (label, path, dtype, stride)
}

fn evaluate(label: &str, bytes: &[u8], dtype: TensorDType, stride: usize) {
    let production =
        try_encode_qatq_exact_tensor_le_with_stride_hint(bytes, dtype, stride).unwrap();
    assert_eq!(
        decode_qatq_exact_tensor_le(&production).unwrap().bytes_le,
        bytes
    );

    let candidates: Vec<Candidate> = [
        ExponentCoding::RawFields,
        ExponentCoding::Lane0Reference,
        ExponentCoding::ModeReference,
    ]
    .into_iter()
    .map(|coding| encode_candidate(bytes, dtype, coding))
    .collect();
    for candidate in &candidates {
        assert_eq!(decode_candidate(candidate, bytes.len(), dtype), bytes);
    }

    for _ in 0..WARMUP {
        black_box(
            try_encode_qatq_exact_tensor_le_with_stride_hint(black_box(bytes), dtype, stride)
                .unwrap(),
        );
        for candidate in &candidates {
            black_box(encode_candidate(black_box(bytes), dtype, candidate.coding));
            black_box(decode_candidate(black_box(candidate), bytes.len(), dtype));
        }
    }

    let mut production_encode = Duration::ZERO;
    let mut production_decode = Duration::ZERO;
    let mut candidate_encode = vec![Duration::ZERO; candidates.len()];
    let mut candidate_decode = vec![Duration::ZERO; candidates.len()];
    for _ in 0..ITERATIONS {
        let start = Instant::now();
        black_box(
            try_encode_qatq_exact_tensor_le_with_stride_hint(black_box(bytes), dtype, stride)
                .unwrap(),
        );
        production_encode += start.elapsed();

        let start = Instant::now();
        black_box(decode_qatq_exact_tensor_le(black_box(&production)).unwrap());
        production_decode += start.elapsed();

        for (index, candidate) in candidates.iter().enumerate() {
            let start = Instant::now();
            black_box(encode_candidate(black_box(bytes), dtype, candidate.coding));
            candidate_encode[index] += start.elapsed();

            let start = Instant::now();
            black_box(decode_candidate(black_box(candidate), bytes.len(), dtype));
            candidate_decode[index] += start.elapsed();
        }
    }

    println!(
        "| dataset | representation | raw | qatq-exact | candidate | size change | exact | encode change | decode change |"
    );
    println!("| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |");
    for (index, candidate) in candidates.iter().enumerate() {
        let size = HEADER_BYTES + candidate.body.len();
        println!(
            "| {label} | {} | {} | {} | {} | {:+.2}% | yes | {:+.2}% | {:+.2}% |",
            candidate.coding.name(),
            bytes.len(),
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

fn format(dtype: TensorDType) -> Format {
    match dtype {
        TensorDType::F16 => Format {
            exponent_bits: 5,
            mantissa_bits: 10,
        },
        TensorDType::BF16 => Format {
            exponent_bits: 8,
            mantissa_bits: 7,
        },
        _ => unreachable!(),
    }
}

fn encode_candidate(bytes: &[u8], dtype: TensorDType, coding: ExponentCoding) -> Candidate {
    let words = bytes_to_words(bytes);
    let format = format(dtype);
    let mut signs = BitWriter::new();
    let mut mantissas = BitWriter::new();
    let mut exponents = BitWriter::new();
    let exponent_mask = (1_u16 << format.exponent_bits) - 1;
    let mantissa_mask = (1_u16 << format.mantissa_bits) - 1;

    for word in &words {
        signs.write((word >> 15) as u32, 1);
        mantissas.write((word & mantissa_mask) as u32, format.mantissa_bits);
    }

    for group in words.chunks(4) {
        let values: Vec<u16> = group
            .iter()
            .map(|word| (word >> format.mantissa_bits) & exponent_mask)
            .collect();
        match coding {
            ExponentCoding::RawFields => {
                for exponent in values {
                    exponents.write(exponent as u32, format.exponent_bits);
                }
            }
            ExponentCoding::Lane0Reference => {
                let reference = values[0];
                exponents.write(reference as u32, format.exponent_bits);
                for exponent in values.iter().skip(1) {
                    let matches = *exponent == reference;
                    exponents.write(matches as u32, 1);
                    if !matches {
                        exponents.write(*exponent as u32, format.exponent_bits);
                    }
                }
            }
            ExponentCoding::ModeReference => {
                let reference_index = mode_index(&values);
                let reference = values[reference_index];
                exponents.write(reference_index as u32, 2);
                exponents.write(reference as u32, format.exponent_bits);
                for (index, exponent) in values.iter().enumerate() {
                    if index == reference_index {
                        continue;
                    }
                    let matches = *exponent == reference;
                    exponents.write(matches as u32, 1);
                    if !matches {
                        exponents.write(*exponent as u32, format.exponent_bits);
                    }
                }
            }
        }
    }

    let sign_bytes = signs.finish();
    let mantissa_bytes = mantissas.finish();
    let exponent_bytes = exponents.finish();
    let sign_len = sign_bytes.len();
    let mantissa_len = mantissa_bytes.len();
    let exponent_len = exponent_bytes.len();
    let mut transformed = sign_bytes;
    transformed.extend_from_slice(&mantissa_bytes);
    transformed.extend_from_slice(&exponent_bytes);
    Candidate {
        coding,
        body: zstd::bulk::compress(&transformed, 3).unwrap(),
        uncompressed_len: transformed.len(),
        sign_len,
        mantissa_len,
        exponent_len,
    }
}

fn decode_candidate(candidate: &Candidate, byte_len: usize, dtype: TensorDType) -> Vec<u8> {
    let transformed = zstd::bulk::decompress(&candidate.body, candidate.uncompressed_len).unwrap();
    assert_eq!(
        transformed.len(),
        candidate.sign_len + candidate.mantissa_len + candidate.exponent_len
    );
    let value_count = byte_len / 2;
    let format = format(dtype);
    let mut signs = BitReader::new(&transformed[..candidate.sign_len]);
    let mut mantissas = BitReader::new(
        &transformed[candidate.sign_len..candidate.sign_len + candidate.mantissa_len],
    );
    let mut exponents = BitReader::new(&transformed[candidate.sign_len + candidate.mantissa_len..]);
    let mut exponent_values = Vec::with_capacity(value_count);

    while exponent_values.len() < value_count {
        let remaining = value_count - exponent_values.len();
        let group_len = remaining.min(4);
        match candidate.coding {
            ExponentCoding::RawFields => {
                for _ in 0..group_len {
                    exponent_values.push(exponents.read(format.exponent_bits) as u16);
                }
            }
            ExponentCoding::Lane0Reference => {
                let reference = exponents.read(format.exponent_bits) as u16;
                exponent_values.push(reference);
                for _ in 1..group_len {
                    let matches = exponents.read(1) != 0;
                    exponent_values.push(if matches {
                        reference
                    } else {
                        exponents.read(format.exponent_bits) as u16
                    });
                }
            }
            ExponentCoding::ModeReference => {
                let reference_index = exponents.read(2) as usize;
                let reference = exponents.read(format.exponent_bits) as u16;
                let mut group = vec![0_u16; group_len];
                group[reference_index] = reference;
                for (index, exponent) in group.iter_mut().enumerate() {
                    if index == reference_index {
                        continue;
                    }
                    let matches = exponents.read(1) != 0;
                    *exponent = if matches {
                        reference
                    } else {
                        exponents.read(format.exponent_bits) as u16
                    };
                }
                exponent_values.extend(group);
            }
        }
    }

    let mut words = Vec::with_capacity(value_count);
    for exponent in exponent_values {
        let sign = signs.read(1) as u16;
        let mantissa = mantissas.read(format.mantissa_bits) as u16;
        words.push((sign << 15) | (exponent << format.mantissa_bits) | mantissa);
    }
    words_to_bytes(&words)
}

fn mode_index(values: &[u16]) -> usize {
    let mut best_index = 0;
    let mut best_count = 0;
    for (index, value) in values.iter().enumerate() {
        let count = values
            .iter()
            .filter(|candidate| *candidate == value)
            .count();
        if count > best_count {
            best_index = index;
            best_count = count;
        }
    }
    best_index
}

struct BitWriter {
    bytes: Vec<u8>,
    bit_len: usize,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            bit_len: 0,
        }
    }

    fn write(&mut self, value: u32, bits: usize) {
        for bit in 0..bits {
            if self.bit_len.is_multiple_of(8) {
                self.bytes.push(0);
            }
            self.bytes[self.bit_len / 8] |= (((value >> bit) & 1) as u8) << (self.bit_len % 8);
            self.bit_len += 1;
        }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

struct BitReader<'a> {
    bytes: &'a [u8],
    bit_offset: usize,
}

impl<'a> BitReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            bit_offset: 0,
        }
    }

    fn read(&mut self, bits: usize) -> u32 {
        let mut value = 0;
        for bit in 0..bits {
            value |=
                (((self.bytes[self.bit_offset / 8] >> (self.bit_offset % 8)) & 1) as u32) << bit;
            self.bit_offset += 1;
        }
        value
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

fn percent_change(candidate: f64, baseline: f64) -> f64 {
    (candidate / baseline - 1.0) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_representation_restores_all_u16_patterns() {
        let words: Vec<u16> = (0..=u16::MAX).collect();
        let bytes = words_to_bytes(&words);
        for dtype in [TensorDType::F16, TensorDType::BF16] {
            for coding in [
                ExponentCoding::RawFields,
                ExponentCoding::Lane0Reference,
                ExponentCoding::ModeReference,
            ] {
                let candidate = encode_candidate(&bytes, dtype, coding);
                assert_eq!(decode_candidate(&candidate, bytes.len(), dtype), bytes);
            }
        }
    }

    #[test]
    fn partial_four_word_groups_roundtrip() {
        for count in 1..8 {
            let words: Vec<u16> = (0..count)
                .map(|index| (index as u16).wrapping_mul(0x2345))
                .collect();
            let bytes = words_to_bytes(&words);
            for dtype in [TensorDType::F16, TensorDType::BF16] {
                for coding in [
                    ExponentCoding::RawFields,
                    ExponentCoding::Lane0Reference,
                    ExponentCoding::ModeReference,
                ] {
                    let candidate = encode_candidate(&bytes, dtype, coding);
                    assert_eq!(decode_candidate(&candidate, bytes.len(), dtype), bytes);
                }
            }
        }
    }
}
