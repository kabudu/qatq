use std::fmt;

const MAGIC: &[u8; 4] = b"QATQ";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 28;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecMode {
    LossyI4,
    LosslessF32,
}

impl CodecMode {
    fn id(self) -> u8 {
        match self {
            Self::LossyI4 => 1,
            Self::LosslessF32 => 2,
        }
    }

    fn from_id(id: u8) -> Result<Self, QatqError> {
        match id {
            1 => Ok(Self::LossyI4),
            2 => Ok(Self::LosslessF32),
            other => Err(QatqError::UnsupportedMode(other)),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum QatqError {
    EmptyMode,
    InvalidMagic,
    UnsupportedVersion(u8),
    UnsupportedMode(u8),
    PayloadTooShort { actual: usize, minimum: usize },
    LengthMismatch { expected: usize, actual: usize },
    InvalidScale(u32),
    ChecksumMismatch { expected: u64, actual: u64 },
    ValueCountTooLarge(usize),
}

impl fmt::Display for QatqError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyMode => write!(f, "codec mode is empty"),
            Self::InvalidMagic => write!(f, "payload magic is not QATQ"),
            Self::UnsupportedVersion(version) => write!(f, "unsupported QATQ version {version}"),
            Self::UnsupportedMode(mode) => write!(f, "unsupported QATQ mode {mode}"),
            Self::PayloadTooShort { actual, minimum } => {
                write!(
                    f,
                    "payload is too short: {actual} bytes, need at least {minimum}"
                )
            }
            Self::LengthMismatch { expected, actual } => {
                write!(
                    f,
                    "payload length mismatch: expected {expected}, got {actual}"
                )
            }
            Self::InvalidScale(bits) => write!(f, "payload scale is invalid: 0x{bits:08x}"),
            Self::ChecksumMismatch { expected, actual } => {
                write!(
                    f,
                    "checksum mismatch: expected {expected:016x}, got {actual:016x}"
                )
            }
            Self::ValueCountTooLarge(count) => write!(f, "value count is too large: {count}"),
        }
    }
}

impl std::error::Error for QatqError {}

pub fn parse_mode(mode: &str) -> Result<CodecMode, QatqError> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "" => Err(QatqError::EmptyMode),
        "lossy-i4" | "i4" | "qatq-i4" => Ok(CodecMode::LossyI4),
        "lossless-f32" | "f32" | "exact-f32" => Ok(CodecMode::LosslessF32),
        _ => Err(QatqError::UnsupportedMode(0)),
    }
}

pub fn encode(values: &[f32], mode: CodecMode) -> Vec<u8> {
    match mode {
        CodecMode::LossyI4 => encode_lossy_i4(values),
        CodecMode::LosslessF32 => encode_lossless_f32(values),
    }
}

pub fn decode(payload: &[u8]) -> Result<Vec<f32>, QatqError> {
    let header = Header::parse(payload)?;
    match header.mode {
        CodecMode::LossyI4 => decode_lossy_i4(payload),
        CodecMode::LosslessF32 => decode_lossless_f32(payload),
    }
}

pub fn encode_lossy_i4(values: &[f32]) -> Vec<u8> {
    let scale = compute_i4_scale(values);
    let checksum = checksum_f32_bits(values);
    let mut out = Vec::with_capacity(HEADER_LEN + values.len().div_ceil(2));
    write_header(&mut out, CodecMode::LossyI4, values.len(), scale, checksum);

    for chunk in values.chunks(2) {
        let first = quantize_i4_nibble(chunk[0], scale);
        let second = chunk
            .get(1)
            .map(|value| quantize_i4_nibble(*value, scale))
            .unwrap_or(0);
        out.push((first << 4) | second);
    }

    out
}

pub fn decode_lossy_i4(payload: &[u8]) -> Result<Vec<f32>, QatqError> {
    let header = Header::parse_for_mode(payload, CodecMode::LossyI4)?;
    let expected_payload_len = header.value_count.div_ceil(2);
    let packed = &payload[HEADER_LEN..];
    if packed.len() != expected_payload_len {
        return Err(QatqError::LengthMismatch {
            expected: expected_payload_len,
            actual: packed.len(),
        });
    }

    let mut values = Vec::with_capacity(header.value_count);
    for byte in packed {
        values.push(dequantize_i4_nibble(byte >> 4, header.scale));
        if values.len() < header.value_count {
            values.push(dequantize_i4_nibble(byte & 0x0f, header.scale));
        }
    }
    Ok(values)
}

pub fn encode_lossless_f32(values: &[f32]) -> Vec<u8> {
    let checksum = checksum_f32_bits(values);
    let mut out = Vec::with_capacity(HEADER_LEN + values.len() * 4);
    write_header(
        &mut out,
        CodecMode::LosslessF32,
        values.len(),
        1.0,
        checksum,
    );
    for value in values {
        out.extend_from_slice(&value.to_bits().to_be_bytes());
    }
    out
}

pub fn decode_lossless_f32(payload: &[u8]) -> Result<Vec<f32>, QatqError> {
    let header = Header::parse_for_mode(payload, CodecMode::LosslessF32)?;
    let expected_payload_len = header.value_count * 4;
    let body = &payload[HEADER_LEN..];
    if body.len() != expected_payload_len {
        return Err(QatqError::LengthMismatch {
            expected: expected_payload_len,
            actual: body.len(),
        });
    }

    let mut values = Vec::with_capacity(header.value_count);
    for chunk in body.chunks_exact(4) {
        let bits = u32::from_be_bytes(chunk.try_into().expect("chunk size checked"));
        values.push(f32::from_bits(bits));
    }

    let actual = checksum_f32_bits(&values);
    if actual != header.checksum {
        return Err(QatqError::ChecksumMismatch {
            expected: header.checksum,
            actual,
        });
    }

    Ok(values)
}

pub fn compression_ratio(encoded_len: usize, value_count: usize) -> f64 {
    if value_count == 0 {
        return 1.0;
    }
    encoded_len as f64 / (value_count * 4) as f64
}

fn compute_i4_scale(values: &[f32]) -> f32 {
    let max_abs = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .map(f32::abs)
        .fold(0.0_f32, f32::max);
    if max_abs > 0.0 && max_abs.is_finite() {
        max_abs / 7.0
    } else {
        1.0
    }
}

fn quantize_i4_nibble(value: f32, scale: f32) -> u8 {
    let scaled = if value.is_finite() {
        (value / scale).round()
    } else {
        0.0
    };
    let quantized = (scaled as i32).clamp(-7, 7);
    ((quantized + 8) as u8) & 0x0f
}

fn dequantize_i4_nibble(nibble: u8, scale: f32) -> f32 {
    let signed = ((nibble & 0x0f) as i8) - 8;
    (signed as f32) * scale
}

fn write_header(out: &mut Vec<u8>, mode: CodecMode, value_count: usize, scale: f32, checksum: u64) {
    assert!(
        value_count <= u64::MAX as usize,
        "value count exceeds portable payload header"
    );
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.push(mode.id());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&(value_count as u64).to_be_bytes());
    out.extend_from_slice(&scale.to_bits().to_be_bytes());
    out.extend_from_slice(&checksum.to_be_bytes());
}

fn checksum_f32_bits(values: &[f32]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for value in values {
        for byte in value.to_bits().to_be_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

#[derive(Debug)]
struct Header {
    mode: CodecMode,
    value_count: usize,
    scale: f32,
    checksum: u64,
}

impl Header {
    fn parse_for_mode(payload: &[u8], expected_mode: CodecMode) -> Result<Self, QatqError> {
        let header = Self::parse(payload)?;
        if header.mode != expected_mode {
            return Err(QatqError::UnsupportedMode(header.mode.id()));
        }
        Ok(header)
    }

    fn parse(payload: &[u8]) -> Result<Self, QatqError> {
        if payload.len() < HEADER_LEN {
            return Err(QatqError::PayloadTooShort {
                actual: payload.len(),
                minimum: HEADER_LEN,
            });
        }
        if &payload[0..4] != MAGIC {
            return Err(QatqError::InvalidMagic);
        }
        let version = payload[4];
        if version != VERSION {
            return Err(QatqError::UnsupportedVersion(version));
        }
        let mode = CodecMode::from_id(payload[5])?;
        let value_count_u64 = u64::from_be_bytes(payload[8..16].try_into().expect("fixed header"));
        let value_count = usize::try_from(value_count_u64)
            .map_err(|_| QatqError::ValueCountTooLarge(usize::MAX))?;
        let scale_bits = u32::from_be_bytes(payload[16..20].try_into().expect("fixed header"));
        let scale = f32::from_bits(scale_bits);
        if !scale.is_finite() || scale <= 0.0 {
            return Err(QatqError::InvalidScale(scale_bits));
        }
        let checksum = u64::from_be_bytes(payload[20..28].try_into().expect("fixed header"));
        Ok(Self {
            mode,
            value_count,
            scale,
            checksum,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lossy_i4_roundtrip_preserves_shape_and_compresses() {
        let values: Vec<f32> = (0..512)
            .map(|index| ((index as f32) * 0.125).sin() * 3.0)
            .collect();

        let encoded = encode_lossy_i4(&values);
        let decoded = decode_lossy_i4(&encoded).unwrap();

        assert_eq!(decoded.len(), values.len());
        assert!(encoded.len() < values.len() * 4);
        assert!(compression_ratio(encoded.len(), values.len()) < 0.2);
        let max_abs = values
            .iter()
            .zip(decoded.iter())
            .map(|(before, after)| (before - after).abs())
            .fold(0.0_f32, f32::max);
        assert!(max_abs < 0.25, "max_abs={max_abs}");
    }

    #[test]
    fn lossless_f32_roundtrip_preserves_bits() {
        let values = [
            0.0_f32,
            -0.0,
            1.25,
            -128.5,
            f32::INFINITY,
            f32::from_bits(0x7fc0_1234),
        ];

        let encoded = encode_lossless_f32(&values);
        let decoded = decode_lossless_f32(&encoded).unwrap();

        let before: Vec<u32> = values.iter().map(|value| value.to_bits()).collect();
        let after: Vec<u32> = decoded.iter().map(|value| value.to_bits()).collect();
        assert_eq!(after, before);
    }

    #[test]
    fn rejects_invalid_magic() {
        let mut encoded = encode_lossy_i4(&[1.0, 2.0]);
        encoded[0] = b'X';
        assert_eq!(decode(&encoded), Err(QatqError::InvalidMagic));
    }

    #[test]
    fn rejects_truncated_lossy_body() {
        let mut encoded = encode_lossy_i4(&[1.0, 2.0, 3.0, 4.0]);
        encoded.pop();
        assert_eq!(
            decode_lossy_i4(&encoded),
            Err(QatqError::LengthMismatch {
                expected: 2,
                actual: 1
            })
        );
    }

    #[test]
    fn detects_lossless_payload_corruption() {
        let mut encoded = encode_lossless_f32(&[1.0, 2.0, 3.0]);
        let last = encoded.last_mut().unwrap();
        *last ^= 0x01;
        assert!(matches!(
            decode_lossless_f32(&encoded),
            Err(QatqError::ChecksumMismatch { .. })
        ));
    }
}
