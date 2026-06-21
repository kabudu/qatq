use std::fmt;

const MAGIC: &[u8; 4] = b"QATQ";
const CONTAINER_MAGIC: &[u8; 4] = b"QATC";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 28;
const CONTAINER_HEADER_LEN: usize = 24;
const CONTAINER_CHUNK_LEN: usize = 4;
const PHASE1_BODY_MAGIC: &[u8; 4] = b"P1Q4";
const PHASE2_BODY_MAGIC: &[u8; 4] = b"P2L1";
const PHASE1_METADATA_LEN: usize = 20;
const PHASE2_PREFIX_LEN: usize = 8;
const PHASE2_PREDICTOR_METADATA_LEN: usize = 12;
const DEFAULT_PHASE1_SEED: u64 = 0x5141_5451_c0de_0001;
const XOR_ZERO_RUN: u8 = 0;
const XOR_RAW_RUN: u8 = 1;
const BYTE_REPEAT_RUN: u8 = 2;
const PHASE2_STRATEGY_PREDICTOR_XOR: u8 = 0;
const PHASE2_STRATEGY_RAW_BITS: u8 = 1;
const PHASE2_STRATEGY_BYTE_RLE: u8 = 2;
const PHASE2_STRATEGY_BYTE_PLANE_RLE: u8 = 3;
const PHASE2_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE: u8 = 4;
const PHASE2_STRATEGY_BYTE_PLANE_BLOCKS: u8 = 5;
const BYTE_PLANE_BLOCK_ZERO: u8 = 0;
const BYTE_PLANE_BLOCK_RAW: u8 = 1;
const BYTE_PLANE_BLOCK_REPEAT: u8 = 2;
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const FNV_PRIME_SQUARED: u64 = FNV_PRIME.wrapping_mul(FNV_PRIME);
pub const MAX_VALUES_PER_PAYLOAD: usize = 1 << 26;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecMode {
    LossyI4,
    LosslessF32,
    Phase1Q4,
    Phase2Lossless,
}

impl CodecMode {
    fn id(self) -> u8 {
        match self {
            Self::LossyI4 => 1,
            Self::LosslessF32 => 2,
            Self::Phase1Q4 => 3,
            Self::Phase2Lossless => 4,
        }
    }

    fn from_id(id: u8) -> Result<Self, QatqError> {
        match id {
            1 => Ok(Self::LossyI4),
            2 => Ok(Self::LosslessF32),
            3 => Ok(Self::Phase1Q4),
            4 => Ok(Self::Phase2Lossless),
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
    InvalidResidualScale(u32),
    InvalidHeader,
    InvalidPhase1Body,
    InvalidPhase2Body,
    InvalidResidualStream,
    InvalidChunkSize(usize),
    InvalidContainer,
    ChecksumMismatch { expected: u64, actual: u64 },
    ValueCountTooLarge(usize),
}

impl fmt::Display for QatqError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyMode => write!(f, "codec mode is empty"),
            Self::InvalidMagic => write!(f, "payload magic is not QATQ or QATC"),
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
            Self::InvalidResidualScale(bits) => {
                write!(f, "phase1 residual scale is invalid: 0x{bits:08x}")
            }
            Self::InvalidHeader => write!(f, "payload header is invalid"),
            Self::InvalidPhase1Body => write!(f, "phase1 payload body is invalid"),
            Self::InvalidPhase2Body => write!(f, "phase2 payload body is invalid"),
            Self::InvalidResidualStream => write!(f, "phase2 residual stream is invalid"),
            Self::InvalidChunkSize(size) => write!(f, "chunk size is invalid: {size}"),
            Self::InvalidContainer => write!(f, "chunked container is invalid"),
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
        "phase1-q4" | "qatq-phase1" | "qatq-q4" | "quaternion-q4" => Ok(CodecMode::Phase1Q4),
        "phase2-lossless" | "qatq-lossless" | "lossless-qatq" | "qres-lossless" => {
            Ok(CodecMode::Phase2Lossless)
        }
        _ => Err(QatqError::UnsupportedMode(0)),
    }
}

pub fn encode(values: &[f32], mode: CodecMode) -> Vec<u8> {
    try_encode(values, mode).expect("value count exceeds single-payload bound; use chunked APIs")
}

pub fn try_encode(values: &[f32], mode: CodecMode) -> Result<Vec<u8>, QatqError> {
    validate_single_payload_value_count(values.len())?;
    Ok(encode_unchecked(values, mode))
}

pub fn validate_single_payload_value_count(value_count: usize) -> Result<(), QatqError> {
    if value_count > MAX_VALUES_PER_PAYLOAD {
        return Err(QatqError::ValueCountTooLarge(value_count));
    }
    Ok(())
}

fn encode_unchecked(values: &[f32], mode: CodecMode) -> Vec<u8> {
    match mode {
        CodecMode::LossyI4 => encode_lossy_i4_unchecked(values),
        CodecMode::LosslessF32 => encode_lossless_f32_unchecked(values),
        CodecMode::Phase1Q4 => encode_phase1_q4_unchecked(values, Phase1Config::default()),
        CodecMode::Phase2Lossless => {
            encode_phase2_lossless_unchecked(values, Phase1Config::default())
        }
    }
}

pub fn decode(payload: &[u8]) -> Result<Vec<f32>, QatqError> {
    if payload.len() >= CONTAINER_MAGIC.len() && &payload[0..4] == CONTAINER_MAGIC {
        return decode_phase2_lossless_container(payload);
    }
    let header = Header::parse(payload)?;
    match header.mode {
        CodecMode::LossyI4 => decode_lossy_i4(payload),
        CodecMode::LosslessF32 => decode_lossless_f32(payload),
        CodecMode::Phase1Q4 => decode_phase1_q4(payload),
        CodecMode::Phase2Lossless => decode_phase2_lossless(payload),
    }
}

pub fn encode_lossy_i4(values: &[f32]) -> Vec<u8> {
    try_encode_lossy_i4(values).expect("value count exceeds single-payload bound; use chunked APIs")
}

pub fn try_encode_lossy_i4(values: &[f32]) -> Result<Vec<u8>, QatqError> {
    validate_single_payload_value_count(values.len())?;
    Ok(encode_lossy_i4_unchecked(values))
}

fn encode_lossy_i4_unchecked(values: &[f32]) -> Vec<u8> {
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
    try_encode_lossless_f32(values)
        .expect("value count exceeds single-payload bound; use chunked APIs")
}

pub fn try_encode_lossless_f32(values: &[f32]) -> Result<Vec<u8>, QatqError> {
    validate_single_payload_value_count(values.len())?;
    Ok(encode_lossless_f32_unchecked(values))
}

fn encode_lossless_f32_unchecked(values: &[f32]) -> Vec<u8> {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase2Strategy {
    PredictorXor,
    RawBits,
    ByteRle,
    BytePlaneRle,
    DeltaXorBytePlaneRle,
    BytePlaneBlocks,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionStorage {
    QatqPhase2,
    RawF32LePassThrough,
}

impl ProductionStorage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::QatqPhase2 => "qatq-phase2",
            Self::RawF32LePassThrough => "raw-f32le-pass-through",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionChunkMetadata {
    pub storage: ProductionStorage,
    pub raw_f32le_len: usize,
    pub strategy: Option<Phase2Strategy>,
}

impl ProductionChunkMetadata {
    pub fn storage_label(&self) -> &'static str {
        self.storage.as_str()
    }

    pub fn value_count(&self) -> Result<usize, QatqError> {
        if self.raw_f32le_len % 4 != 0 {
            return Err(QatqError::InvalidHeader);
        }
        Ok(self.raw_f32le_len / 4)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionEncodeResult {
    pub metadata: ProductionChunkMetadata,
    pub bytes: Vec<u8>,
}

impl ProductionEncodeResult {
    pub fn should_compress(&self) -> bool {
        self.metadata.storage == ProductionStorage::QatqPhase2
    }

    pub fn should_pass_through(&self) -> bool {
        self.metadata.storage == ProductionStorage::RawF32LePassThrough
    }

    pub fn stored_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Phase2EncodeDecision {
    Compressed {
        payload: Vec<u8>,
        strategy: Phase2Strategy,
        raw_f32le_len: usize,
    },
    PassThroughRaw {
        bytes: Vec<u8>,
    },
}

impl Phase2EncodeDecision {
    pub fn should_compress(&self) -> bool {
        matches!(self, Self::Compressed { .. })
    }

    pub fn should_pass_through(&self) -> bool {
        matches!(self, Self::PassThroughRaw { .. })
    }

    pub fn strategy(&self) -> Option<Phase2Strategy> {
        match self {
            Self::Compressed { strategy, .. } => Some(*strategy),
            Self::PassThroughRaw { .. } => None,
        }
    }

    pub fn stored_bytes(&self) -> &[u8] {
        match self {
            Self::Compressed { payload, .. } => payload,
            Self::PassThroughRaw { bytes } => bytes,
        }
    }

    pub fn raw_f32le_len(&self) -> usize {
        match self {
            Self::Compressed { raw_f32le_len, .. } => *raw_f32le_len,
            Self::PassThroughRaw { bytes } => bytes.len(),
        }
    }
}

impl Phase2Strategy {
    fn from_id(id: u8) -> Result<Self, QatqError> {
        match id {
            PHASE2_STRATEGY_PREDICTOR_XOR => Ok(Self::PredictorXor),
            PHASE2_STRATEGY_RAW_BITS => Ok(Self::RawBits),
            PHASE2_STRATEGY_BYTE_RLE => Ok(Self::ByteRle),
            PHASE2_STRATEGY_BYTE_PLANE_RLE => Ok(Self::BytePlaneRle),
            PHASE2_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE => Ok(Self::DeltaXorBytePlaneRle),
            PHASE2_STRATEGY_BYTE_PLANE_BLOCKS => Ok(Self::BytePlaneBlocks),
            _ => Err(QatqError::InvalidPhase2Body),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::PredictorXor => "predictor-xor",
            Self::RawBits => "raw-bits",
            Self::ByteRle => "byte-rle",
            Self::BytePlaneRle => "byte-plane-rle",
            Self::DeltaXorBytePlaneRle => "delta-xor-byte-plane-rle",
            Self::BytePlaneBlocks => "byte-plane-blocks",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Phase1Config {
    pub seed: u64,
}

impl Default for Phase1Config {
    fn default() -> Self {
        Self {
            seed: DEFAULT_PHASE1_SEED,
        }
    }
}

pub fn encode_phase1_q4(values: &[f32]) -> Vec<u8> {
    encode_phase1_q4_with_config(values, Phase1Config::default())
}

pub fn encode_phase1_q4_with_config(values: &[f32], config: Phase1Config) -> Vec<u8> {
    try_encode_phase1_q4_with_config(values, config)
        .expect("value count exceeds single-payload bound; use chunked APIs")
}

pub fn try_encode_phase1_q4_with_config(
    values: &[f32],
    config: Phase1Config,
) -> Result<Vec<u8>, QatqError> {
    validate_single_payload_value_count(values.len())?;
    Ok(encode_phase1_q4_unchecked(values, config))
}

fn encode_phase1_q4_unchecked(values: &[f32], config: Phase1Config) -> Vec<u8> {
    let parts = build_phase1_parts(values, config);
    let checksum = checksum_f32_bits(values);
    let quantized_len = parts.coord_count.div_ceil(2);
    let residual_len = parts.coord_count.div_ceil(8);
    let mut out =
        Vec::with_capacity(HEADER_LEN + PHASE1_METADATA_LEN + quantized_len + residual_len);
    write_header(
        &mut out,
        CodecMode::Phase1Q4,
        values.len(),
        parts.scale,
        checksum,
    );
    write_phase_metadata_and_payload(&mut out, PHASE1_BODY_MAGIC, &parts);
    out
}

pub fn decode_phase1_q4(payload: &[u8]) -> Result<Vec<f32>, QatqError> {
    let header = Header::parse_for_mode(payload, CodecMode::Phase1Q4)?;
    let coord_count = checked_phase1_coordinate_count(header.value_count)?;
    let quantized_len = coord_count.div_ceil(2);
    let residual_len = coord_count.div_ceil(8);
    let expected_payload_len = PHASE1_METADATA_LEN + quantized_len + residual_len;
    let body = &payload[HEADER_LEN..];
    if body.len() != expected_payload_len {
        return Err(QatqError::LengthMismatch {
            expected: expected_payload_len,
            actual: body.len(),
        });
    }
    if &body[0..4] != PHASE1_BODY_MAGIC {
        return Err(QatqError::InvalidPhase1Body);
    }
    if body[16..20] != [0, 0, 0, 0] {
        return Err(QatqError::InvalidPhase1Body);
    }

    let quantized_offset = PHASE1_METADATA_LEN;
    let residual_offset = quantized_offset + quantized_len;
    let parts = read_phase_parts(
        body,
        header.scale,
        coord_count,
        4,
        12,
        quantized_offset,
        residual_offset,
    )?;
    Ok(reconstruct_phase1_values(header.value_count, &parts))
}

pub fn encode_phase2_lossless(values: &[f32]) -> Vec<u8> {
    encode_phase2_lossless_with_config(values, Phase1Config::default())
}

pub fn encode_phase2_lossless_with_config(values: &[f32], config: Phase1Config) -> Vec<u8> {
    try_encode_phase2_lossless_with_config(values, config)
        .expect("value count exceeds single-payload bound; use chunked APIs")
}

pub fn try_encode_phase2_lossless_with_config(
    values: &[f32],
    config: Phase1Config,
) -> Result<Vec<u8>, QatqError> {
    validate_single_payload_value_count(values.len())?;
    Ok(encode_phase2_lossless_unchecked(values, config))
}

pub fn encode_phase2_lossless_decision(values: &[f32]) -> Phase2EncodeDecision {
    encode_phase2_lossless_decision_with_config(values, Phase1Config::default())
}

pub fn encode_phase2_lossless_decision_with_config(
    values: &[f32],
    config: Phase1Config,
) -> Phase2EncodeDecision {
    try_encode_phase2_lossless_decision_with_config(values, config)
        .expect("value count exceeds single-payload bound; use chunked APIs")
}

pub fn try_encode_phase2_lossless_decision_with_config(
    values: &[f32],
    config: Phase1Config,
) -> Result<Phase2EncodeDecision, QatqError> {
    validate_single_payload_value_count(values.len())?;
    let raw_f32le_len = checked_value_byte_len(values.len())?;
    let payload = encode_phase2_lossless_unchecked(values, config);
    let strategy = phase2_lossless_strategy(&payload)?;
    if strategy == Phase2Strategy::RawBits {
        Ok(Phase2EncodeDecision::PassThroughRaw {
            bytes: encode_f32_bits_le(values),
        })
    } else {
        Ok(Phase2EncodeDecision::Compressed {
            payload,
            strategy,
            raw_f32le_len,
        })
    }
}

pub fn try_encode_production_chunk(values: &[f32]) -> Result<ProductionEncodeResult, QatqError> {
    try_encode_production_chunk_with_config(values, Phase1Config::default())
}

pub fn try_encode_production_chunk_with_config(
    values: &[f32],
    config: Phase1Config,
) -> Result<ProductionEncodeResult, QatqError> {
    let decision = try_encode_phase2_lossless_decision_with_config(values, config)?;
    Ok(production_result_from_decision(decision))
}

pub fn production_result_from_decision(decision: Phase2EncodeDecision) -> ProductionEncodeResult {
    match decision {
        Phase2EncodeDecision::Compressed {
            payload,
            strategy,
            raw_f32le_len,
        } => ProductionEncodeResult {
            metadata: ProductionChunkMetadata {
                storage: ProductionStorage::QatqPhase2,
                raw_f32le_len,
                strategy: Some(strategy),
            },
            bytes: payload,
        },
        Phase2EncodeDecision::PassThroughRaw { bytes } => ProductionEncodeResult {
            metadata: ProductionChunkMetadata {
                storage: ProductionStorage::RawF32LePassThrough,
                raw_f32le_len: bytes.len(),
                strategy: None,
            },
            bytes,
        },
    }
}

pub fn restore_production_chunk(
    metadata: &ProductionChunkMetadata,
    bytes: &[u8],
) -> Result<Vec<f32>, QatqError> {
    match metadata.storage {
        ProductionStorage::QatqPhase2 => {
            let restored = decode_phase2_lossless(bytes)?;
            let expected_len = metadata.raw_f32le_len;
            let actual_len = checked_value_byte_len(restored.len())?;
            if actual_len != expected_len {
                return Err(QatqError::LengthMismatch {
                    expected: expected_len,
                    actual: actual_len,
                });
            }
            if let Some(expected_strategy) = metadata.strategy {
                let actual_strategy = phase2_lossless_strategy(bytes)?;
                if actual_strategy != expected_strategy {
                    return Err(QatqError::InvalidPhase2Body);
                }
            }
            Ok(restored)
        }
        ProductionStorage::RawF32LePassThrough => decode_raw_f32le_pass_through(metadata, bytes),
    }
}

fn encode_phase2_lossless_unchecked(values: &[f32], config: Phase1Config) -> Vec<u8> {
    encode_phase2_lossless_fast(values, config)
}

pub fn encode_phase2_lossless_exhaustive(values: &[f32]) -> Vec<u8> {
    encode_phase2_lossless_exhaustive_with_config(values, Phase1Config::default())
}

pub fn encode_phase2_lossless_exhaustive_with_config(
    values: &[f32],
    config: Phase1Config,
) -> Vec<u8> {
    try_encode_phase2_lossless_exhaustive_with_config(values, config)
        .expect("value count exceeds single-payload bound; use chunked APIs")
}

pub fn try_encode_phase2_lossless_exhaustive_with_config(
    values: &[f32],
    config: Phase1Config,
) -> Result<Vec<u8>, QatqError> {
    validate_single_payload_value_count(values.len())?;
    Ok(encode_phase2_lossless_exhaustive_unchecked(values, config))
}

fn encode_phase2_lossless_exhaustive_unchecked(values: &[f32], config: Phase1Config) -> Vec<u8> {
    let raw_bits = encode_f32_bits_be(values);
    let raw_body_len = PHASE2_PREFIX_LEN + raw_bits.len();
    let byte_plane_blocks = encode_byte_plane_blocks_bounded(&raw_bits, raw_bits.len());
    let byte_plane_blocks_body_len = candidate_body_len(byte_plane_blocks.as_ref());
    let byte_rle = encode_byte_runs_bounded(&raw_bits, raw_bits.len());
    let byte_rle_body_len = candidate_body_len(byte_rle.as_ref());
    let byte_plane = encode_byte_plane_runs_bounded(&raw_bits, raw_bits.len());
    let byte_plane_body_len = candidate_body_len(byte_plane.as_ref());
    let delta_xor_byte_plane = encode_delta_xor_byte_plane_runs_bounded(values, raw_bits.len());
    let delta_xor_byte_plane_body_len = candidate_body_len(delta_xor_byte_plane.as_ref());
    let coord_count = phase1_coordinate_count(values.len());
    let predictor_min_body_len = PHASE2_PREFIX_LEN
        + PHASE2_PREDICTOR_METADATA_LEN
        + coord_count.div_ceil(2)
        + coord_count.div_ceil(8);
    let checksum = checksum_f32_bits(values);

    if byte_plane_body_len <= raw_body_len
        && byte_plane_body_len <= byte_rle_body_len
        && byte_plane_body_len <= byte_plane_blocks_body_len
        && byte_plane_body_len <= predictor_min_body_len
        && byte_plane_body_len <= delta_xor_byte_plane_body_len
    {
        let mut out = Vec::with_capacity(HEADER_LEN + byte_plane_body_len);
        write_header(
            &mut out,
            CodecMode::Phase2Lossless,
            values.len(),
            1.0,
            checksum,
        );
        write_phase2_prefix(&mut out, PHASE2_STRATEGY_BYTE_PLANE_RLE);
        out.extend_from_slice(byte_plane.as_ref().expect("selected byte-plane candidate"));
        return out;
    }

    if byte_plane_blocks_body_len <= raw_body_len
        && byte_plane_blocks_body_len <= byte_rle_body_len
        && byte_plane_blocks_body_len <= byte_plane_body_len
        && byte_plane_blocks_body_len <= predictor_min_body_len
        && byte_plane_blocks_body_len <= delta_xor_byte_plane_body_len
    {
        return write_phase2_byte_candidate(
            values.len(),
            checksum,
            PHASE2_STRATEGY_BYTE_PLANE_BLOCKS,
            byte_plane_blocks
                .as_ref()
                .expect("selected byte-plane block candidate"),
        );
    }

    if delta_xor_byte_plane_body_len <= raw_body_len
        && delta_xor_byte_plane_body_len <= byte_rle_body_len
        && delta_xor_byte_plane_body_len <= byte_plane_blocks_body_len
        && delta_xor_byte_plane_body_len <= predictor_min_body_len
    {
        let mut out = Vec::with_capacity(HEADER_LEN + delta_xor_byte_plane_body_len);
        write_header(
            &mut out,
            CodecMode::Phase2Lossless,
            values.len(),
            1.0,
            checksum,
        );
        write_phase2_prefix(&mut out, PHASE2_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE);
        out.extend_from_slice(
            delta_xor_byte_plane
                .as_ref()
                .expect("selected delta-XOR byte-plane candidate"),
        );
        return out;
    }

    if byte_rle_body_len <= raw_body_len
        && byte_rle_body_len <= byte_plane_blocks_body_len
        && byte_rle_body_len <= predictor_min_body_len
        && byte_rle_body_len <= delta_xor_byte_plane_body_len
    {
        let mut out = Vec::with_capacity(HEADER_LEN + byte_rle_body_len);
        write_header(
            &mut out,
            CodecMode::Phase2Lossless,
            values.len(),
            1.0,
            checksum,
        );
        write_phase2_prefix(&mut out, PHASE2_STRATEGY_BYTE_RLE);
        out.extend_from_slice(byte_rle.as_ref().expect("selected byte-RLE candidate"));
        return out;
    }

    let parts = build_phase1_parts(values, config);
    let predicted = reconstruct_phase1_values(values.len(), &parts);
    let residuals = encode_xor_residuals(values, &predicted);
    let quantized_len = parts.coord_count.div_ceil(2);
    let residual_sign_len = parts.coord_count.div_ceil(8);
    let predictor_body_len = PHASE2_PREFIX_LEN
        + PHASE2_PREDICTOR_METADATA_LEN
        + quantized_len
        + residual_sign_len
        + residuals.len();
    let strategy = if raw_body_len <= byte_rle_body_len
        && raw_body_len <= predictor_body_len
        && raw_body_len <= delta_xor_byte_plane_body_len
        && raw_body_len <= byte_plane_blocks_body_len
    {
        if raw_body_len <= byte_plane_body_len {
            PHASE2_STRATEGY_RAW_BITS
        } else {
            PHASE2_STRATEGY_BYTE_PLANE_RLE
        }
    } else if byte_rle_body_len <= byte_plane_body_len
        && byte_rle_body_len <= byte_plane_blocks_body_len
        && byte_rle_body_len <= predictor_body_len
        && byte_rle_body_len <= delta_xor_byte_plane_body_len
    {
        PHASE2_STRATEGY_BYTE_RLE
    } else if byte_plane_body_len <= predictor_body_len
        && byte_plane_body_len <= delta_xor_byte_plane_body_len
        && byte_plane_body_len <= byte_plane_blocks_body_len
    {
        PHASE2_STRATEGY_BYTE_PLANE_RLE
    } else if byte_plane_blocks_body_len <= predictor_body_len
        && byte_plane_blocks_body_len <= delta_xor_byte_plane_body_len
    {
        PHASE2_STRATEGY_BYTE_PLANE_BLOCKS
    } else if delta_xor_byte_plane_body_len <= predictor_body_len {
        PHASE2_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE
    } else {
        PHASE2_STRATEGY_PREDICTOR_XOR
    };
    let body_len = match strategy {
        PHASE2_STRATEGY_RAW_BITS => raw_body_len,
        PHASE2_STRATEGY_BYTE_RLE => byte_rle_body_len,
        PHASE2_STRATEGY_BYTE_PLANE_RLE => byte_plane_body_len,
        PHASE2_STRATEGY_BYTE_PLANE_BLOCKS => byte_plane_blocks_body_len,
        PHASE2_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE => delta_xor_byte_plane_body_len,
        PHASE2_STRATEGY_PREDICTOR_XOR => predictor_body_len,
        _ => unreachable!("known strategy"),
    };
    let mut out = Vec::with_capacity(HEADER_LEN + body_len);
    write_header(
        &mut out,
        CodecMode::Phase2Lossless,
        values.len(),
        if strategy == PHASE2_STRATEGY_PREDICTOR_XOR {
            parts.scale
        } else {
            1.0
        },
        checksum,
    );
    write_phase2_prefix(&mut out, strategy);
    match strategy {
        PHASE2_STRATEGY_RAW_BITS => out.extend_from_slice(&raw_bits),
        PHASE2_STRATEGY_BYTE_RLE => {
            out.extend_from_slice(byte_rle.as_ref().expect("selected byte-RLE candidate"))
        }
        PHASE2_STRATEGY_BYTE_PLANE_RLE => {
            out.extend_from_slice(byte_plane.as_ref().expect("selected byte-plane candidate"))
        }
        PHASE2_STRATEGY_BYTE_PLANE_BLOCKS => out.extend_from_slice(
            byte_plane_blocks
                .as_ref()
                .expect("selected byte-plane block candidate"),
        ),
        PHASE2_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE => out.extend_from_slice(
            delta_xor_byte_plane
                .as_ref()
                .expect("selected delta-XOR byte-plane candidate"),
        ),
        PHASE2_STRATEGY_PREDICTOR_XOR => {
            out.extend_from_slice(&parts.seed.to_be_bytes());
            out.extend_from_slice(&parts.residual_scale.to_bits().to_be_bytes());
            pack_i4_nibbles(&parts.quantized, &mut out);
            pack_residual_signs(&parts.residual_signs, &mut out);
            out.extend_from_slice(&residuals);
        }
        _ => unreachable!("known strategy"),
    }
    out
}

fn encode_phase2_lossless_fast(values: &[f32], config: Phase1Config) -> Vec<u8> {
    let raw_bits_len = values.len() * 4;
    let raw_body_len = PHASE2_PREFIX_LEN + raw_bits_len;
    if let Some((byte_plane_blocks, checksum)) =
        encode_two_high_raw_two_low_zero_blocks_bounded(values, raw_bits_len)
    {
        return write_phase2_byte_candidate(
            values.len(),
            checksum,
            PHASE2_STRATEGY_BYTE_PLANE_BLOCKS,
            &byte_plane_blocks,
        );
    }
    let (byte_plane_blocks, checksum) =
        encode_byte_plane_blocks_from_f32_bounded(values, raw_bits_len);
    let byte_plane_blocks_body_len = candidate_body_len(byte_plane_blocks.as_ref());
    if byte_plane_blocks_body_len < raw_body_len {
        return write_phase2_byte_candidate(
            values.len(),
            checksum,
            PHASE2_STRATEGY_BYTE_PLANE_BLOCKS,
            byte_plane_blocks
                .as_ref()
                .expect("selected byte-plane block candidate"),
        );
    }
    let raw_bits = encode_f32_bits_be(values);
    let byte_rle = encode_byte_runs_bounded(&raw_bits, raw_bits.len());
    let byte_rle_body_len = candidate_body_len(byte_rle.as_ref());
    let byte_plane = encode_byte_plane_runs_bounded(&raw_bits, raw_bits.len());
    let byte_plane_body_len = candidate_body_len(byte_plane.as_ref());

    if byte_plane_body_len < raw_body_len && byte_plane_body_len <= byte_rle_body_len {
        return write_phase2_byte_candidate(
            values.len(),
            checksum,
            PHASE2_STRATEGY_BYTE_PLANE_RLE,
            byte_plane.as_ref().expect("selected byte-plane candidate"),
        );
    }
    if byte_rle_body_len < raw_body_len {
        return write_phase2_byte_candidate(
            values.len(),
            checksum,
            PHASE2_STRATEGY_BYTE_RLE,
            byte_rle.as_ref().expect("selected byte-RLE candidate"),
        );
    }

    let delta_xor_byte_plane = encode_delta_xor_byte_plane_runs_bounded(values, raw_bits.len());
    let delta_xor_byte_plane_body_len = candidate_body_len(delta_xor_byte_plane.as_ref());
    if delta_xor_byte_plane_body_len < raw_body_len {
        return write_phase2_byte_candidate(
            values.len(),
            checksum,
            PHASE2_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE,
            delta_xor_byte_plane
                .as_ref()
                .expect("selected delta-XOR byte-plane candidate"),
        );
    }

    let parts = build_phase1_parts(values, config);
    let predicted = reconstruct_phase1_values(values.len(), &parts);
    let residuals = encode_xor_residuals(values, &predicted);
    let quantized_len = parts.coord_count.div_ceil(2);
    let residual_sign_len = parts.coord_count.div_ceil(8);
    let predictor_body_len = PHASE2_PREFIX_LEN
        + PHASE2_PREDICTOR_METADATA_LEN
        + quantized_len
        + residual_sign_len
        + residuals.len();
    let strategy = if raw_body_len <= byte_rle_body_len
        && raw_body_len <= predictor_body_len
        && raw_body_len <= delta_xor_byte_plane_body_len
    {
        if raw_body_len <= byte_plane_body_len {
            PHASE2_STRATEGY_RAW_BITS
        } else {
            PHASE2_STRATEGY_BYTE_PLANE_RLE
        }
    } else if byte_rle_body_len <= byte_plane_body_len
        && byte_rle_body_len <= predictor_body_len
        && byte_rle_body_len <= delta_xor_byte_plane_body_len
    {
        PHASE2_STRATEGY_BYTE_RLE
    } else if byte_plane_body_len <= predictor_body_len
        && byte_plane_body_len <= delta_xor_byte_plane_body_len
    {
        PHASE2_STRATEGY_BYTE_PLANE_RLE
    } else if delta_xor_byte_plane_body_len <= predictor_body_len {
        PHASE2_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE
    } else {
        PHASE2_STRATEGY_PREDICTOR_XOR
    };
    write_phase2_selected(
        values.len(),
        checksum,
        strategy,
        &raw_bits,
        byte_rle.as_ref(),
        byte_plane.as_ref(),
        delta_xor_byte_plane.as_ref(),
        &parts,
        &residuals,
    )
}

fn write_phase2_byte_candidate(
    value_count: usize,
    checksum: u64,
    strategy: u8,
    bytes: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN + PHASE2_PREFIX_LEN + bytes.len());
    write_header(
        &mut out,
        CodecMode::Phase2Lossless,
        value_count,
        1.0,
        checksum,
    );
    write_phase2_prefix(&mut out, strategy);
    out.extend_from_slice(bytes);
    out
}

fn write_phase2_selected(
    value_count: usize,
    checksum: u64,
    strategy: u8,
    raw_bits: &[u8],
    byte_rle: Option<&Vec<u8>>,
    byte_plane: Option<&Vec<u8>>,
    delta_xor_byte_plane: Option<&Vec<u8>>,
    parts: &PhaseParts,
    residuals: &[u8],
) -> Vec<u8> {
    let body_len = match strategy {
        PHASE2_STRATEGY_RAW_BITS => PHASE2_PREFIX_LEN + raw_bits.len(),
        PHASE2_STRATEGY_BYTE_RLE => {
            PHASE2_PREFIX_LEN + byte_rle.expect("selected byte-RLE candidate").len()
        }
        PHASE2_STRATEGY_BYTE_PLANE_RLE => {
            PHASE2_PREFIX_LEN + byte_plane.expect("selected byte-plane candidate").len()
        }
        PHASE2_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE => {
            PHASE2_PREFIX_LEN
                + delta_xor_byte_plane
                    .expect("selected delta-XOR byte-plane candidate")
                    .len()
        }
        PHASE2_STRATEGY_PREDICTOR_XOR => {
            PHASE2_PREFIX_LEN
                + PHASE2_PREDICTOR_METADATA_LEN
                + parts.coord_count.div_ceil(2)
                + parts.coord_count.div_ceil(8)
                + residuals.len()
        }
        _ => unreachable!("known strategy"),
    };
    let mut out = Vec::with_capacity(HEADER_LEN + body_len);
    write_header(
        &mut out,
        CodecMode::Phase2Lossless,
        value_count,
        if strategy == PHASE2_STRATEGY_PREDICTOR_XOR {
            parts.scale
        } else {
            1.0
        },
        checksum,
    );
    write_phase2_prefix(&mut out, strategy);
    match strategy {
        PHASE2_STRATEGY_RAW_BITS => out.extend_from_slice(raw_bits),
        PHASE2_STRATEGY_BYTE_RLE => {
            out.extend_from_slice(byte_rle.expect("selected byte-RLE candidate"))
        }
        PHASE2_STRATEGY_BYTE_PLANE_RLE => {
            out.extend_from_slice(byte_plane.expect("selected byte-plane candidate"))
        }
        PHASE2_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE => out.extend_from_slice(
            delta_xor_byte_plane.expect("selected delta-XOR byte-plane candidate"),
        ),
        PHASE2_STRATEGY_PREDICTOR_XOR => {
            out.extend_from_slice(&parts.seed.to_be_bytes());
            out.extend_from_slice(&parts.residual_scale.to_bits().to_be_bytes());
            pack_i4_nibbles(&parts.quantized, &mut out);
            pack_residual_signs(&parts.residual_signs, &mut out);
            out.extend_from_slice(residuals);
        }
        _ => unreachable!("known strategy"),
    }
    out
}

pub fn encode_phase2_lossless_chunks(
    values: &[f32],
    max_values_per_chunk: usize,
) -> Result<Vec<Vec<u8>>, QatqError> {
    encode_phase2_lossless_chunks_with_config(values, max_values_per_chunk, Phase1Config::default())
}

pub fn encode_phase2_lossless_chunks_with_config(
    values: &[f32],
    max_values_per_chunk: usize,
    config: Phase1Config,
) -> Result<Vec<Vec<u8>>, QatqError> {
    if max_values_per_chunk == 0 || max_values_per_chunk > MAX_VALUES_PER_PAYLOAD {
        return Err(QatqError::InvalidChunkSize(max_values_per_chunk));
    }
    if values.is_empty() {
        return Ok(vec![encode_phase2_lossless_with_config(values, config)]);
    }
    Ok(values
        .chunks(max_values_per_chunk)
        .map(|chunk| encode_phase2_lossless_with_config(chunk, config))
        .collect())
}

pub fn decode_phase2_lossless_chunks<I, B>(chunks: I) -> Result<Vec<f32>, QatqError>
where
    I: IntoIterator<Item = B>,
    B: AsRef<[u8]>,
{
    let mut values = Vec::new();
    for chunk in chunks {
        values.extend(decode_phase2_lossless(chunk.as_ref())?);
    }
    Ok(values)
}

pub fn encode_phase2_lossless_container(
    values: &[f32],
    max_values_per_chunk: usize,
) -> Result<Vec<u8>, QatqError> {
    encode_phase2_lossless_container_with_config(
        values,
        max_values_per_chunk,
        Phase1Config::default(),
    )
}

pub fn encode_phase2_lossless_container_with_config(
    values: &[f32],
    max_values_per_chunk: usize,
    config: Phase1Config,
) -> Result<Vec<u8>, QatqError> {
    if max_values_per_chunk == 0 || max_values_per_chunk > MAX_VALUES_PER_PAYLOAD {
        return Err(QatqError::InvalidChunkSize(max_values_per_chunk));
    }
    let chunk_count = if values.is_empty() {
        1
    } else {
        values.len().div_ceil(max_values_per_chunk)
    };
    if chunk_count > u32::MAX as usize {
        return Err(QatqError::InvalidContainer);
    }

    let mut out = Vec::new();
    write_container_header(&mut out, values.len(), chunk_count);
    if values.is_empty() {
        let payload = encode_phase2_lossless_with_config(values, config);
        append_container_chunk(&mut out, &payload)?;
        return Ok(out);
    }

    for chunk_values in values.chunks(max_values_per_chunk) {
        let payload = encode_phase2_lossless_with_config(chunk_values, config);
        append_container_chunk(&mut out, &payload)?;
    }
    Ok(out)
}

fn append_container_chunk(out: &mut Vec<u8>, payload: &[u8]) -> Result<(), QatqError> {
    if payload.len() > u32::MAX as usize {
        return Err(QatqError::InvalidContainer);
    }
    let additional_len = CONTAINER_CHUNK_LEN
        .checked_add(payload.len())
        .ok_or(QatqError::InvalidContainer)?;
    out.try_reserve(additional_len)
        .map_err(|_| QatqError::InvalidContainer)?;
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    Ok(())
}

pub fn decode_phase2_lossless_container(payload: &[u8]) -> Result<Vec<f32>, QatqError> {
    let header = ContainerHeader::parse(payload)?;
    let mut values = Vec::with_capacity(header.total_values);
    for_each_phase2_lossless_container_payload(payload, |chunk| {
        values.extend(decode_phase2_lossless(chunk)?);
        Ok(())
    })?;
    Ok(values)
}

pub fn for_each_phase2_lossless_container_payload(
    payload: &[u8],
    mut visitor: impl FnMut(&[u8]) -> Result<(), QatqError>,
) -> Result<(), QatqError> {
    let (header, body, chunk_count) = container_body_and_chunk_count(payload)?;
    validate_container_chunk_layout(body, chunk_count, header.total_values)?;
    for_each_container_chunk_unchecked(body, chunk_count, |chunk| visitor(chunk))
}

pub fn decode_phase2_lossless_container_payloads(payload: &[u8]) -> Result<Vec<&[u8]>, QatqError> {
    let (header, body, chunk_count) = container_body_and_chunk_count(payload)?;

    let chunks = read_container_chunk_index(body, chunk_count, header.total_values)?;
    Ok(chunks
        .into_iter()
        .map(|(chunk_start, chunk_end)| &body[chunk_start..chunk_end])
        .collect())
}

fn container_body_and_chunk_count(
    payload: &[u8],
) -> Result<(ContainerHeader, &[u8], usize), QatqError> {
    let header = ContainerHeader::parse(payload)?;
    let body = &payload[CONTAINER_HEADER_LEN..];
    let chunk_count = header.chunk_count as usize;
    if chunk_count == 0 {
        return Err(QatqError::InvalidContainer);
    }
    if chunk_count > body.len() / CONTAINER_CHUNK_LEN {
        return Err(QatqError::InvalidContainer);
    }
    Ok((header, body, chunk_count))
}

fn read_container_chunk_index(
    body: &[u8],
    chunk_count: usize,
    total_values: usize,
) -> Result<Vec<(usize, usize)>, QatqError> {
    let mut offset = 0_usize;
    let mut chunks = Vec::with_capacity(chunk_count);
    let mut indexed_total = 0_usize;
    for _ in 0..chunk_count {
        let len_end = offset
            .checked_add(CONTAINER_CHUNK_LEN)
            .ok_or(QatqError::InvalidContainer)?;
        if len_end > body.len() {
            return Err(QatqError::InvalidContainer);
        }
        let chunk_len =
            u32::from_be_bytes(body[offset..len_end].try_into().expect("fixed length")) as usize;
        if chunk_len < HEADER_LEN + PHASE2_PREFIX_LEN {
            return Err(QatqError::InvalidContainer);
        }
        let chunk_start = len_end;
        let chunk_end = chunk_start
            .checked_add(chunk_len)
            .ok_or(QatqError::InvalidContainer)?;
        if chunk_end > body.len() {
            return Err(QatqError::InvalidContainer);
        }

        let chunk_header =
            Header::parse_for_mode(&body[chunk_start..chunk_end], CodecMode::Phase2Lossless)?;
        indexed_total = indexed_total
            .checked_add(chunk_header.value_count)
            .ok_or(QatqError::InvalidContainer)?;
        if indexed_total > total_values {
            return Err(QatqError::InvalidContainer);
        }
        chunks.push((chunk_start, chunk_end));
        offset = chunk_end;
    }

    if offset != body.len() || indexed_total != total_values {
        return Err(QatqError::InvalidContainer);
    }
    Ok(chunks)
}

fn validate_container_chunk_layout(
    body: &[u8],
    chunk_count: usize,
    total_values: usize,
) -> Result<(), QatqError> {
    let mut offset = 0_usize;
    let mut indexed_total = 0_usize;
    for _ in 0..chunk_count {
        let len_end = offset
            .checked_add(CONTAINER_CHUNK_LEN)
            .ok_or(QatqError::InvalidContainer)?;
        if len_end > body.len() {
            return Err(QatqError::InvalidContainer);
        }
        let chunk_len =
            u32::from_be_bytes(body[offset..len_end].try_into().expect("fixed length")) as usize;
        if chunk_len < HEADER_LEN + PHASE2_PREFIX_LEN {
            return Err(QatqError::InvalidContainer);
        }
        let chunk_start = len_end;
        let chunk_end = chunk_start
            .checked_add(chunk_len)
            .ok_or(QatqError::InvalidContainer)?;
        if chunk_end > body.len() {
            return Err(QatqError::InvalidContainer);
        }

        let chunk_header =
            Header::parse_for_mode(&body[chunk_start..chunk_end], CodecMode::Phase2Lossless)?;
        indexed_total = indexed_total
            .checked_add(chunk_header.value_count)
            .ok_or(QatqError::InvalidContainer)?;
        if indexed_total > total_values {
            return Err(QatqError::InvalidContainer);
        }
        offset = chunk_end;
    }

    if offset != body.len() || indexed_total != total_values {
        return Err(QatqError::InvalidContainer);
    }
    Ok(())
}

fn for_each_container_chunk_unchecked(
    body: &[u8],
    chunk_count: usize,
    mut visitor: impl FnMut(&[u8]) -> Result<(), QatqError>,
) -> Result<(), QatqError> {
    let mut offset = 0_usize;
    for _ in 0..chunk_count {
        let len_end = offset + CONTAINER_CHUNK_LEN;
        let chunk_len =
            u32::from_be_bytes(body[offset..len_end].try_into().expect("fixed length")) as usize;
        let chunk_start = len_end;
        let chunk_end = chunk_start + chunk_len;
        visitor(&body[chunk_start..chunk_end])?;
        offset = chunk_end;
    }
    Ok(())
}

pub fn decode_phase2_lossless(payload: &[u8]) -> Result<Vec<f32>, QatqError> {
    let header = Header::parse_for_mode(payload, CodecMode::Phase2Lossless)?;
    let body = &payload[HEADER_LEN..];
    let strategy = parse_phase2_strategy_body(body)?;
    if strategy == Phase2Strategy::BytePlaneBlocks {
        return decode_phase2_byte_plane_blocks_checked(&body[PHASE2_PREFIX_LEN..], &header);
    }
    let values = match strategy {
        Phase2Strategy::RawBits => decode_phase2_raw_bits(&body[PHASE2_PREFIX_LEN..], &header)?,
        Phase2Strategy::ByteRle => decode_phase2_byte_rle(&body[PHASE2_PREFIX_LEN..], &header)?,
        Phase2Strategy::BytePlaneRle => {
            decode_phase2_byte_plane_rle(&body[PHASE2_PREFIX_LEN..], &header)?
        }
        Phase2Strategy::DeltaXorBytePlaneRle => {
            decode_phase2_delta_xor_byte_plane_rle(&body[PHASE2_PREFIX_LEN..], &header)?
        }
        Phase2Strategy::BytePlaneBlocks => unreachable!("byte-plane blocks returned above"),
        Phase2Strategy::PredictorXor => {
            decode_phase2_predictor_xor(&body[PHASE2_PREFIX_LEN..], &header)?
        }
    };

    let actual = checksum_f32_bits(&values);
    if actual != header.checksum {
        return Err(QatqError::ChecksumMismatch {
            expected: header.checksum,
            actual,
        });
    }

    Ok(values)
}

pub fn phase2_lossless_strategy(payload: &[u8]) -> Result<Phase2Strategy, QatqError> {
    let _header = Header::parse_for_mode(payload, CodecMode::Phase2Lossless)?;
    parse_phase2_strategy_body(&payload[HEADER_LEN..])
}

fn parse_phase2_strategy_body(body: &[u8]) -> Result<Phase2Strategy, QatqError> {
    if body.len() < PHASE2_PREFIX_LEN {
        return Err(QatqError::PayloadTooShort {
            actual: body.len(),
            minimum: PHASE2_PREFIX_LEN,
        });
    }
    if &body[0..4] != PHASE2_BODY_MAGIC {
        return Err(QatqError::InvalidPhase2Body);
    }
    if body[5..8] != [0, 0, 0] {
        return Err(QatqError::InvalidPhase2Body);
    }
    Phase2Strategy::from_id(body[4])
}

fn decode_phase2_predictor_xor(body: &[u8], header: &Header) -> Result<Vec<f32>, QatqError> {
    let coord_count = checked_phase1_coordinate_count(header.value_count)?;
    let quantized_len = coord_count.div_ceil(2);
    let residual_sign_len = coord_count.div_ceil(8);
    let minimum_payload_len = PHASE2_PREDICTOR_METADATA_LEN + quantized_len + residual_sign_len;
    if body.len() < minimum_payload_len {
        return Err(QatqError::PayloadTooShort {
            actual: body.len(),
            minimum: minimum_payload_len,
        });
    }

    let quantized_offset = PHASE2_PREDICTOR_METADATA_LEN;
    let residual_sign_offset = quantized_offset + quantized_len;
    let xor_offset = residual_sign_offset + residual_sign_len;
    let parts = read_phase_parts(
        body,
        header.scale,
        coord_count,
        0,
        8,
        quantized_offset,
        residual_sign_offset,
    )?;
    let predicted = reconstruct_phase1_values(header.value_count, &parts);
    let xors = decode_xor_residuals(&body[xor_offset..], header.value_count)?;
    let mut values = Vec::with_capacity(header.value_count);
    for (predicted, xor) in predicted.iter().zip(xors.iter()) {
        values.push(f32::from_bits(predicted.to_bits() ^ xor));
    }
    Ok(values)
}

fn decode_phase2_raw_bits(body: &[u8], header: &Header) -> Result<Vec<f32>, QatqError> {
    let expected_len = checked_value_byte_len(header.value_count)?;
    if body.len() != expected_len {
        return Err(QatqError::LengthMismatch {
            expected: expected_len,
            actual: body.len(),
        });
    }
    let mut values = Vec::with_capacity(header.value_count);
    for chunk in body.chunks_exact(4) {
        let bits = u32::from_be_bytes(chunk.try_into().expect("raw f32 chunk size checked"));
        values.push(f32::from_bits(bits));
    }
    Ok(values)
}

fn decode_phase2_byte_rle(body: &[u8], header: &Header) -> Result<Vec<f32>, QatqError> {
    let expected_len = checked_value_byte_len(header.value_count)?;
    decode_byte_runs_to_f32(body, expected_len, header.value_count)
}

fn decode_phase2_byte_plane_rle(body: &[u8], header: &Header) -> Result<Vec<f32>, QatqError> {
    let expected_len = checked_value_byte_len(header.value_count)?;
    let words = decode_byte_plane_runs_to_words(body, expected_len, header.value_count)?;
    Ok(words.into_iter().map(f32::from_bits).collect())
}

fn decode_phase2_delta_xor_byte_plane_rle(
    body: &[u8],
    header: &Header,
) -> Result<Vec<f32>, QatqError> {
    let expected_len = checked_value_byte_len(header.value_count)?;
    let deltas = decode_byte_plane_runs_to_words(body, expected_len, header.value_count)?;
    let mut values = Vec::with_capacity(header.value_count);
    let mut previous_bits = 0_u32;
    for (value_index, delta) in deltas.into_iter().enumerate() {
        let bits = if value_index == 0 {
            delta
        } else {
            previous_bits ^ delta
        };
        values.push(f32::from_bits(bits));
        previous_bits = bits;
    }
    Ok(values)
}

fn decode_phase2_byte_plane_blocks_checked(
    body: &[u8],
    header: &Header,
) -> Result<Vec<f32>, QatqError> {
    let expected_len = checked_value_byte_len(header.value_count)?;
    let (values, actual) =
        decode_byte_plane_blocks_to_f32_and_checksum(body, expected_len, header.value_count)?;
    if actual != header.checksum {
        return Err(QatqError::ChecksumMismatch {
            expected: header.checksum,
            actual,
        });
    }
    Ok(values)
}

fn decode_byte_plane_runs_to_words(
    bytes: &[u8],
    expected_len: usize,
    value_count: usize,
) -> Result<Vec<u32>, QatqError> {
    if expected_len != value_count * 4 {
        return Err(QatqError::InvalidResidualStream);
    }
    let mut words = vec![0_u32; value_count];
    let mut decoded_len = 0_usize;
    let mut offset = 0_usize;
    while decoded_len < expected_len {
        if offset + 3 > bytes.len() {
            return Err(QatqError::InvalidResidualStream);
        }
        let token = bytes[offset];
        let len = u16::from_be_bytes(
            bytes[offset + 1..offset + 3]
                .try_into()
                .expect("fixed byte run length"),
        ) as usize;
        offset += 3;
        if len == 0 || decoded_len + len > expected_len {
            return Err(QatqError::InvalidResidualStream);
        }
        match token {
            XOR_ZERO_RUN => {
                decoded_len += len;
            }
            XOR_RAW_RUN => {
                if offset + len > bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                for byte in &bytes[offset..offset + len] {
                    write_plane_word_byte(&mut words, decoded_len, value_count, *byte);
                    decoded_len += 1;
                }
                offset += len;
            }
            BYTE_REPEAT_RUN => {
                if offset >= bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                let value = bytes[offset];
                offset += 1;
                for _ in 0..len {
                    write_plane_word_byte(&mut words, decoded_len, value_count, value);
                    decoded_len += 1;
                }
            }
            _ => return Err(QatqError::InvalidResidualStream),
        }
    }
    if offset != bytes.len() {
        return Err(QatqError::InvalidResidualStream);
    }
    Ok(words)
}

#[derive(Clone, Copy)]
enum BytePlaneBlock {
    Zero,
    Repeat(u8),
    Raw { offset: usize },
}

fn decode_byte_plane_blocks_to_f32_and_checksum(
    bytes: &[u8],
    expected_len: usize,
    value_count: usize,
) -> Result<(Vec<f32>, u64), QatqError> {
    let blocks = parse_byte_plane_blocks(bytes, expected_len, value_count)?;
    let mut checksum = FNV_OFFSET;
    match blocks {
        [BytePlaneBlock::Raw { offset: first }, BytePlaneBlock::Raw { offset: second }, BytePlaneBlock::Zero, BytePlaneBlock::Zero] =>
        {
            let first_plane = &bytes[first..first + value_count];
            let second_plane = &bytes[second..second + value_count];
            let mut values: Vec<f32> = Vec::with_capacity(value_count);
            let out = values.as_mut_ptr();
            let first_ptr = first_plane.as_ptr();
            let second_ptr = second_plane.as_ptr();
            for index in 0..value_count {
                // SAFETY: `first_plane` and `second_plane` are both exactly `value_count`
                // bytes long, and `index` is bounded by `0..value_count`.
                let first_byte = unsafe { *first_ptr.add(index) };
                let second_byte = unsafe { *second_ptr.add(index) };
                let bits = ((first_byte as u32) << 24) | ((second_byte as u32) << 16);
                checksum = checksum_two_high_bytes_update(checksum, first_byte, second_byte);
                // SAFETY: `values` was allocated with capacity `value_count`; each `index`
                // is written exactly once before `set_len(value_count)` below.
                unsafe { out.add(index).write(f32::from_bits(bits)) };
            }
            // SAFETY: the loop above initialized every element in `0..value_count`.
            unsafe { values.set_len(value_count) };
            Ok((values, checksum))
        }
        [BytePlaneBlock::Raw { offset: first }, BytePlaneBlock::Raw { offset: second }, BytePlaneBlock::Raw { offset: third }, BytePlaneBlock::Raw { offset: fourth }] =>
        {
            let first_plane = &bytes[first..first + value_count];
            let second_plane = &bytes[second..second + value_count];
            let third_plane = &bytes[third..third + value_count];
            let fourth_plane = &bytes[fourth..fourth + value_count];
            let mut values: Vec<f32> = Vec::with_capacity(value_count);
            let out = values.as_mut_ptr();
            let first_ptr = first_plane.as_ptr();
            let second_ptr = second_plane.as_ptr();
            let third_ptr = third_plane.as_ptr();
            let fourth_ptr = fourth_plane.as_ptr();
            for index in 0..value_count {
                // SAFETY: all four plane slices are exactly `value_count` bytes long,
                // and `index` is bounded by `0..value_count`.
                let first_byte = unsafe { *first_ptr.add(index) };
                let second_byte = unsafe { *second_ptr.add(index) };
                let third_byte = unsafe { *third_ptr.add(index) };
                let fourth_byte = unsafe { *fourth_ptr.add(index) };
                let bits = u32::from_be_bytes([first_byte, second_byte, third_byte, fourth_byte]);
                checksum = checksum_four_bytes_update(
                    checksum,
                    first_byte,
                    second_byte,
                    third_byte,
                    fourth_byte,
                );
                // SAFETY: `values` was allocated with capacity `value_count`; each `index`
                // is written exactly once before `set_len(value_count)` below.
                unsafe { out.add(index).write(f32::from_bits(bits)) };
            }
            // SAFETY: the loop above initialized every element in `0..value_count`.
            unsafe { values.set_len(value_count) };
            Ok((values, checksum))
        }
        _ => {
            let mut values = Vec::with_capacity(value_count);
            for value_index in 0..value_count {
                let mut bits = 0_u32;
                for (plane, block) in blocks.iter().enumerate() {
                    let byte = match block {
                        BytePlaneBlock::Zero => 0,
                        BytePlaneBlock::Repeat(value) => *value,
                        BytePlaneBlock::Raw { offset } => bytes[*offset + value_index],
                    };
                    bits |= (byte as u32) << ((3 - plane) * 8);
                }
                checksum = checksum_bits_update(checksum, bits);
                values.push(f32::from_bits(bits));
            }
            Ok((values, checksum))
        }
    }
}

fn parse_byte_plane_blocks(
    bytes: &[u8],
    expected_len: usize,
    value_count: usize,
) -> Result<[BytePlaneBlock; 4], QatqError> {
    if expected_len != value_count * 4 {
        return Err(QatqError::InvalidResidualStream);
    }
    let mut offset = 0_usize;
    let mut blocks = [BytePlaneBlock::Zero; 4];
    for plane in 0..4 {
        if offset >= bytes.len() {
            return Err(QatqError::InvalidResidualStream);
        }
        let tag = bytes[offset];
        offset += 1;
        match tag {
            BYTE_PLANE_BLOCK_ZERO => {
                blocks[plane] = BytePlaneBlock::Zero;
            }
            BYTE_PLANE_BLOCK_REPEAT => {
                if offset >= bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                let value = bytes[offset];
                offset += 1;
                blocks[plane] = BytePlaneBlock::Repeat(value);
            }
            BYTE_PLANE_BLOCK_RAW => {
                if offset + value_count > bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                blocks[plane] = BytePlaneBlock::Raw { offset };
                offset += value_count;
            }
            _ => return Err(QatqError::InvalidResidualStream),
        }
    }
    if offset != bytes.len() {
        return Err(QatqError::InvalidResidualStream);
    }
    Ok(blocks)
}

fn write_plane_word_byte(words: &mut [u32], plane_index: usize, value_count: usize, byte: u8) {
    if byte == 0 {
        return;
    }
    let plane = plane_index / value_count;
    let value_index = plane_index % value_count;
    let shift = (3 - plane) * 8;
    words[value_index] |= (byte as u32) << shift;
}

fn candidate_body_len(candidate: Option<&Vec<u8>>) -> usize {
    candidate
        .map(|bytes| PHASE2_PREFIX_LEN + bytes.len())
        .unwrap_or(usize::MAX)
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

struct PhaseParts {
    seed: u64,
    scale: f32,
    residual_scale: f32,
    coord_count: usize,
    quantized: Vec<u8>,
    residual_signs: Vec<bool>,
}

fn build_phase1_parts(values: &[f32], config: Phase1Config) -> PhaseParts {
    let predictor_values = finite_predictor_values(values);
    let rotated = rotate_values(&predictor_values, config.seed, RotationDirection::Forward);
    let scale = compute_i4_scale(&rotated);
    let coord_count = phase1_coordinate_count(values.len());
    let mut quantized = Vec::with_capacity(coord_count);
    let mut reconstructed_rotated = Vec::with_capacity(coord_count);

    for value in &rotated {
        let nibble = quantize_i4_nibble(*value, scale);
        quantized.push(nibble);
        reconstructed_rotated.push(dequantize_i4_nibble(nibble, scale));
    }

    let mut residual_abs_sum = 0.0_f32;
    let mut residual_signs = Vec::with_capacity(coord_count);
    for (before, after) in rotated.iter().zip(reconstructed_rotated.iter()) {
        let residual = before - after;
        residual_abs_sum += residual.abs();
        residual_signs.push(residual >= 0.0);
    }
    let residual_scale = if coord_count == 0 {
        0.0
    } else {
        residual_abs_sum / coord_count as f32
    };

    PhaseParts {
        seed: config.seed,
        scale,
        residual_scale,
        coord_count,
        quantized,
        residual_signs,
    }
}

fn finite_predictor_values(values: &[f32]) -> Vec<f32> {
    values
        .iter()
        .map(|value| if value.is_finite() { *value } else { 0.0 })
        .collect()
}

fn encode_f32_bits_be(values: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * 4);
    for value in values {
        out.extend_from_slice(&value.to_bits().to_be_bytes());
    }
    out
}

fn encode_f32_bits_le(values: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * 4);
    for value in values {
        out.extend_from_slice(&value.to_bits().to_le_bytes());
    }
    out
}

fn decode_raw_f32le_pass_through(
    metadata: &ProductionChunkMetadata,
    bytes: &[u8],
) -> Result<Vec<f32>, QatqError> {
    if bytes.len() != metadata.raw_f32le_len {
        return Err(QatqError::LengthMismatch {
            expected: metadata.raw_f32le_len,
            actual: bytes.len(),
        });
    }
    if bytes.len() % 4 != 0 {
        return Err(QatqError::InvalidHeader);
    }
    let mut values = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        values.push(f32::from_bits(u32::from_le_bytes(
            chunk.try_into().expect("chunk size checked"),
        )));
    }
    Ok(values)
}

#[cfg(test)]
fn encode_delta_xor_bits_be(values: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * 4);
    let mut previous_bits = 0_u32;
    for (index, value) in values.iter().enumerate() {
        let bits = value.to_bits();
        let delta = if index == 0 {
            bits
        } else {
            previous_bits ^ bits
        };
        out.extend_from_slice(&delta.to_be_bytes());
        previous_bits = bits;
    }
    out
}

#[cfg(test)]
fn encode_byte_planes(bytes: &[u8]) -> Vec<u8> {
    debug_assert_eq!(bytes.len() % 4, 0);
    let value_count = bytes.len() / 4;
    let mut out = Vec::with_capacity(bytes.len());
    for plane in 0..4 {
        for value_index in 0..value_count {
            out.push(bytes[value_index * 4 + plane]);
        }
    }
    out
}

fn encode_byte_plane_runs_bounded(bytes: &[u8], max_encoded_len: usize) -> Option<Vec<u8>> {
    debug_assert_eq!(bytes.len() % 4, 0);
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = plane_byte(bytes, index);
        if byte == 0 {
            let start = index;
            index += 1;
            while index < bytes.len()
                && plane_byte(bytes, index) == 0
                && index - start < u16::MAX as usize
            {
                index += 1;
            }
            write_xor_run_header(&mut out, XOR_ZERO_RUN, index - start);
            if out.len() > max_encoded_len {
                return None;
            }
        } else if repeated_plane_byte_run_len(bytes, index) >= 4 {
            let value = byte;
            let start = index;
            index += 1;
            while index < bytes.len()
                && plane_byte(bytes, index) == value
                && index - start < u16::MAX as usize
            {
                index += 1;
            }
            write_xor_run_header(&mut out, BYTE_REPEAT_RUN, index - start);
            out.push(value);
            if out.len() > max_encoded_len {
                return None;
            }
        } else {
            let start = index;
            let raw_offset = out.len() + 3;
            index += 1;
            while index < bytes.len()
                && plane_byte(bytes, index) != 0
                && repeated_plane_byte_run_len(bytes, index) < 4
                && index - start < u16::MAX as usize
            {
                index += 1;
            }
            write_xor_run_header(&mut out, XOR_RAW_RUN, index - start);
            for plane_index in start..index {
                out.push(plane_byte(bytes, plane_index));
            }
            debug_assert_eq!(out.len(), raw_offset + index - start);
            if out.len() > max_encoded_len {
                return None;
            }
        }
    }
    Some(out)
}

fn encode_byte_plane_blocks_bounded(bytes: &[u8], max_encoded_len: usize) -> Option<Vec<u8>> {
    debug_assert_eq!(bytes.len() % 4, 0);
    let value_count = bytes.len() / 4;
    let mut out = Vec::with_capacity(bytes.len().min(max_encoded_len));
    for plane in 0..4 {
        let first = if value_count == 0 { 0 } else { bytes[plane] };
        let mut all_same = true;
        for value_index in 1..value_count {
            if bytes[value_index * 4 + plane] != first {
                all_same = false;
                break;
            }
        }
        if all_same {
            if first == 0 {
                out.push(BYTE_PLANE_BLOCK_ZERO);
            } else {
                out.push(BYTE_PLANE_BLOCK_REPEAT);
                out.push(first);
            }
        } else {
            out.push(BYTE_PLANE_BLOCK_RAW);
            for value_index in 0..value_count {
                out.push(bytes[value_index * 4 + plane]);
            }
        }
        if out.len() > max_encoded_len {
            return None;
        }
    }
    Some(out)
}

fn encode_two_high_raw_two_low_zero_blocks_bounded(
    values: &[f32],
    max_encoded_len: usize,
) -> Option<(Vec<u8>, u64)> {
    let value_count = values.len();
    let encoded_len = value_count.checked_mul(2)?.checked_add(4)?;
    if value_count < 2 || encoded_len > max_encoded_len {
        return None;
    }

    let second_plane_tag = value_count + 1;
    let second_plane_start = second_plane_tag + 1;
    let low_zero_start = second_plane_start + value_count;
    let mut out = vec![0_u8; encoded_len];
    out[0] = BYTE_PLANE_BLOCK_RAW;
    out[second_plane_tag] = BYTE_PLANE_BLOCK_RAW;
    out[low_zero_start] = BYTE_PLANE_BLOCK_ZERO;
    out[low_zero_start + 1] = BYTE_PLANE_BLOCK_ZERO;

    let mut checksum = FNV_OFFSET;
    let mut first_high = [0_u8; 2];
    let mut high_same = [true; 2];
    for (value_index, value) in values.iter().enumerate() {
        let bytes = value.to_bits().to_be_bytes();
        if bytes[2] != 0 || bytes[3] != 0 {
            return None;
        }
        checksum = checksum_two_high_bytes_update(checksum, bytes[0], bytes[1]);
        if value_index == 0 {
            first_high = [bytes[0], bytes[1]];
        } else {
            high_same[0] &= bytes[0] == first_high[0];
            high_same[1] &= bytes[1] == first_high[1];
        }
        out[1 + value_index] = bytes[0];
        out[second_plane_start + value_index] = bytes[1];
    }

    if high_same[0] || high_same[1] {
        return None;
    }
    Some((out, checksum))
}

fn encode_byte_plane_blocks_from_f32_bounded(
    values: &[f32],
    max_encoded_len: usize,
) -> (Option<Vec<u8>>, u64) {
    let value_count = values.len();
    let mut checksum = FNV_OFFSET;
    let mut first = [0_u8; 4];
    let mut raw_planes: [Option<Vec<u8>>; 4] = std::array::from_fn(|_| None);

    for (value_index, value) in values.iter().enumerate() {
        let bytes = value.to_bits().to_be_bytes();
        checksum = checksum_four_bytes_update(checksum, bytes[0], bytes[1], bytes[2], bytes[3]);
        if value_index == 0 {
            first = bytes;
            continue;
        }

        for plane in 0..4 {
            if let Some(raw) = raw_planes[plane].as_mut() {
                raw.push(bytes[plane]);
            } else if bytes[plane] != first[plane] {
                let mut raw = Vec::with_capacity(value_count);
                raw.resize(value_index, first[plane]);
                raw.push(bytes[plane]);
                raw_planes[plane] = Some(raw);
            }
        }
    }

    let encoded_len = raw_planes
        .iter()
        .enumerate()
        .map(|(plane, raw)| match raw {
            Some(raw) => 1 + raw.len(),
            None if first[plane] == 0 => 1,
            None => 2,
        })
        .sum::<usize>();
    if encoded_len > max_encoded_len {
        return (None, checksum);
    }

    let mut out = Vec::with_capacity(encoded_len);
    for plane in 0..4 {
        if let Some(raw) = &raw_planes[plane] {
            debug_assert_eq!(raw.len(), value_count);
            out.push(BYTE_PLANE_BLOCK_RAW);
            out.extend_from_slice(raw);
        } else if first[plane] == 0 {
            out.push(BYTE_PLANE_BLOCK_ZERO);
        } else {
            out.push(BYTE_PLANE_BLOCK_REPEAT);
            out.push(first[plane]);
        }
    }
    (Some(out), checksum)
}

fn encode_delta_xor_byte_plane_runs_bounded(
    values: &[f32],
    max_encoded_len: usize,
) -> Option<Vec<u8>> {
    let total_len = values.len().checked_mul(4)?;
    let mut out = Vec::new();
    let mut index = 0;
    while index < total_len {
        let byte = delta_xor_plane_byte(values, index);
        if byte == 0 {
            let start = index;
            index += 1;
            while index < total_len
                && delta_xor_plane_byte(values, index) == 0
                && index - start < u16::MAX as usize
            {
                index += 1;
            }
            write_xor_run_header(&mut out, XOR_ZERO_RUN, index - start);
            if out.len() > max_encoded_len {
                return None;
            }
        } else if repeated_delta_xor_plane_byte_run_len(values, index) >= 4 {
            let value = byte;
            let start = index;
            index += 1;
            while index < total_len
                && delta_xor_plane_byte(values, index) == value
                && index - start < u16::MAX as usize
            {
                index += 1;
            }
            write_xor_run_header(&mut out, BYTE_REPEAT_RUN, index - start);
            out.push(value);
            if out.len() > max_encoded_len {
                return None;
            }
        } else {
            let start = index;
            index += 1;
            while index < total_len
                && delta_xor_plane_byte(values, index) != 0
                && repeated_delta_xor_plane_byte_run_len(values, index) < 4
                && index - start < u16::MAX as usize
            {
                index += 1;
            }
            write_xor_run_header(&mut out, XOR_RAW_RUN, index - start);
            for plane_index in start..index {
                out.push(delta_xor_plane_byte(values, plane_index));
            }
            if out.len() > max_encoded_len {
                return None;
            }
        }
    }
    Some(out)
}

fn plane_byte(bytes: &[u8], plane_index: usize) -> u8 {
    debug_assert_eq!(bytes.len() % 4, 0);
    let value_count = bytes.len() / 4;
    let plane = plane_index / value_count;
    let value_index = plane_index % value_count;
    bytes[value_index * 4 + plane]
}

fn delta_xor_plane_byte(values: &[f32], plane_index: usize) -> u8 {
    let value_count = values.len();
    debug_assert!(value_count > 0);
    let plane = plane_index / value_count;
    let value_index = plane_index % value_count;
    let bits = values[value_index].to_bits();
    let delta = if value_index == 0 {
        bits
    } else {
        values[value_index - 1].to_bits() ^ bits
    };
    delta.to_be_bytes()[plane]
}

fn repeated_plane_byte_run_len(bytes: &[u8], index: usize) -> usize {
    let value = plane_byte(bytes, index);
    let mut len = 1;
    while index + len < bytes.len()
        && plane_byte(bytes, index + len) == value
        && len < u16::MAX as usize
    {
        len += 1;
    }
    len
}

fn repeated_delta_xor_plane_byte_run_len(values: &[f32], index: usize) -> usize {
    let total_len = values.len() * 4;
    let value = delta_xor_plane_byte(values, index);
    let mut len = 1;
    while index + len < total_len
        && delta_xor_plane_byte(values, index + len) == value
        && len < u16::MAX as usize
    {
        len += 1;
    }
    len
}

fn encode_byte_runs_bounded(bytes: &[u8], max_encoded_len: usize) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == 0 {
            let start = index;
            index += 1;
            while index < bytes.len() && bytes[index] == 0 && index - start < u16::MAX as usize {
                index += 1;
            }
            write_xor_run_header(&mut out, XOR_ZERO_RUN, index - start);
            if out.len() > max_encoded_len {
                return None;
            }
        } else if repeated_byte_run_len(bytes, index) >= 4 {
            let value = bytes[index];
            let start = index;
            index += 1;
            while index < bytes.len() && bytes[index] == value && index - start < u16::MAX as usize
            {
                index += 1;
            }
            write_xor_run_header(&mut out, BYTE_REPEAT_RUN, index - start);
            out.push(value);
            if out.len() > max_encoded_len {
                return None;
            }
        } else {
            let start = index;
            index += 1;
            while index < bytes.len()
                && bytes[index] != 0
                && repeated_byte_run_len(bytes, index) < 4
                && index - start < u16::MAX as usize
            {
                index += 1;
            }
            write_xor_run_header(&mut out, XOR_RAW_RUN, index - start);
            out.extend_from_slice(&bytes[start..index]);
            if out.len() > max_encoded_len {
                return None;
            }
        }
    }
    Some(out)
}

fn decode_byte_runs_to_f32(
    bytes: &[u8],
    expected_len: usize,
    value_count: usize,
) -> Result<Vec<f32>, QatqError> {
    if expected_len != value_count * 4 {
        return Err(QatqError::InvalidResidualStream);
    }
    let mut values = Vec::with_capacity(value_count);
    let mut word = [0_u8; 4];
    let mut decoded_len = 0;
    let mut offset = 0;
    while decoded_len < expected_len {
        if offset + 3 > bytes.len() {
            return Err(QatqError::InvalidResidualStream);
        }
        let token = bytes[offset];
        let len = u16::from_be_bytes(
            bytes[offset + 1..offset + 3]
                .try_into()
                .expect("fixed byte run length"),
        ) as usize;
        offset += 3;
        if len == 0 || decoded_len + len > expected_len {
            return Err(QatqError::InvalidResidualStream);
        }
        match token {
            XOR_ZERO_RUN => {
                for _ in 0..len {
                    push_decoded_f32_byte(0, &mut word, &mut decoded_len, &mut values);
                }
            }
            XOR_RAW_RUN => {
                if offset + len > bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                for byte in &bytes[offset..offset + len] {
                    push_decoded_f32_byte(*byte, &mut word, &mut decoded_len, &mut values);
                }
                offset += len;
            }
            BYTE_REPEAT_RUN => {
                if offset >= bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                let value = bytes[offset];
                offset += 1;
                for _ in 0..len {
                    push_decoded_f32_byte(value, &mut word, &mut decoded_len, &mut values);
                }
            }
            _ => return Err(QatqError::InvalidResidualStream),
        }
    }
    if offset != bytes.len() || values.len() != value_count {
        return Err(QatqError::InvalidResidualStream);
    }
    Ok(values)
}

fn push_decoded_f32_byte(
    byte: u8,
    word: &mut [u8; 4],
    decoded_len: &mut usize,
    values: &mut Vec<f32>,
) {
    word[*decoded_len % 4] = byte;
    *decoded_len += 1;
    if *decoded_len % 4 == 0 {
        values.push(f32::from_bits(u32::from_be_bytes(*word)));
    }
}

fn repeated_byte_run_len(bytes: &[u8], index: usize) -> usize {
    let value = bytes[index];
    let mut len = 1;
    while index + len < bytes.len() && bytes[index + len] == value && len < u16::MAX as usize {
        len += 1;
    }
    len
}

fn write_phase_metadata_and_payload(out: &mut Vec<u8>, magic: &[u8; 4], parts: &PhaseParts) {
    out.extend_from_slice(magic);
    out.extend_from_slice(&parts.seed.to_be_bytes());
    out.extend_from_slice(&parts.residual_scale.to_bits().to_be_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    pack_i4_nibbles(&parts.quantized, out);
    pack_residual_signs(&parts.residual_signs, out);
}

fn write_phase2_prefix(out: &mut Vec<u8>, strategy: u8) {
    out.extend_from_slice(PHASE2_BODY_MAGIC);
    out.push(strategy);
    out.extend_from_slice(&[0, 0, 0]);
}

fn read_phase_parts(
    body: &[u8],
    scale: f32,
    coord_count: usize,
    seed_offset: usize,
    residual_scale_offset: usize,
    quantized_offset: usize,
    residual_offset: usize,
) -> Result<PhaseParts, QatqError> {
    if body.len() < residual_scale_offset + 4 || body.len() < seed_offset + 8 {
        return Err(QatqError::PayloadTooShort {
            actual: body.len(),
            minimum: residual_scale_offset + 4,
        });
    }
    let seed = u64::from_be_bytes(
        body[seed_offset..seed_offset + 8]
            .try_into()
            .expect("fixed phase seed"),
    );
    let residual_scale_bits = u32::from_be_bytes(
        body[residual_scale_offset..residual_scale_offset + 4]
            .try_into()
            .expect("fixed phase residual scale"),
    );
    let residual_scale = f32::from_bits(residual_scale_bits);
    if !residual_scale.is_finite() || residual_scale < 0.0 {
        return Err(QatqError::InvalidResidualScale(residual_scale_bits));
    }
    let quantized = unpack_i4_nibbles(&body[quantized_offset..residual_offset], coord_count);
    let residual_sign_len = coord_count.div_ceil(8);
    let residual_end = residual_offset + residual_sign_len;
    if body.len() < residual_end {
        return Err(QatqError::InvalidResidualStream);
    }
    let residual_signs = unpack_residual_signs(&body[residual_offset..residual_end], coord_count);
    Ok(PhaseParts {
        seed,
        scale,
        residual_scale,
        coord_count,
        quantized,
        residual_signs,
    })
}

fn reconstruct_phase1_values(value_count: usize, parts: &PhaseParts) -> Vec<f32> {
    let mut rotated = Vec::with_capacity(parts.coord_count);
    for (nibble, positive) in parts.quantized.iter().zip(parts.residual_signs.iter()) {
        let correction = if *positive {
            parts.residual_scale
        } else {
            -parts.residual_scale
        };
        rotated.push(dequantize_i4_nibble(*nibble, parts.scale) + correction);
    }

    let mut values = rotate_values(&rotated, parts.seed, RotationDirection::Inverse);
    values.truncate(value_count);
    values
}

fn encode_xor_residuals(values: &[f32], predicted: &[f32]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < values.len() {
        let xor = values[index].to_bits() ^ predicted[index].to_bits();
        if xor == 0 {
            let start = index;
            index += 1;
            while index < values.len()
                && values[index].to_bits() ^ predicted[index].to_bits() == 0
                && index - start < u16::MAX as usize
            {
                index += 1;
            }
            write_xor_run_header(&mut out, XOR_ZERO_RUN, index - start);
        } else {
            let start = index;
            index += 1;
            while index < values.len()
                && values[index].to_bits() ^ predicted[index].to_bits() != 0
                && index - start < u16::MAX as usize
            {
                index += 1;
            }
            write_xor_run_header(&mut out, XOR_RAW_RUN, index - start);
            for value_index in start..index {
                let xor = values[value_index].to_bits() ^ predicted[value_index].to_bits();
                out.extend_from_slice(&xor.to_be_bytes());
            }
        }
    }
    out
}

fn write_xor_run_header(out: &mut Vec<u8>, token: u8, len: usize) {
    debug_assert!(len > 0 && len <= u16::MAX as usize);
    out.push(token);
    out.extend_from_slice(&(len as u16).to_be_bytes());
}

fn decode_xor_residuals(bytes: &[u8], count: usize) -> Result<Vec<u32>, QatqError> {
    let mut out = Vec::new();
    let mut offset = 0;
    while out.len() < count {
        if offset + 3 > bytes.len() {
            return Err(QatqError::InvalidResidualStream);
        }
        let token = bytes[offset];
        let len = u16::from_be_bytes(
            bytes[offset + 1..offset + 3]
                .try_into()
                .expect("fixed run length"),
        ) as usize;
        offset += 3;
        if len == 0 || out.len() + len > count {
            return Err(QatqError::InvalidResidualStream);
        }
        match token {
            XOR_ZERO_RUN => out.resize(out.len() + len, 0),
            XOR_RAW_RUN => {
                let byte_len = len.checked_mul(4).ok_or(QatqError::InvalidResidualStream)?;
                if offset + byte_len > bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                for chunk in bytes[offset..offset + byte_len].chunks_exact(4) {
                    out.push(u32::from_be_bytes(
                        chunk.try_into().expect("raw xor chunk size checked"),
                    ));
                }
                offset += byte_len;
            }
            _ => return Err(QatqError::InvalidResidualStream),
        }
    }
    if offset != bytes.len() {
        return Err(QatqError::InvalidResidualStream);
    }
    Ok(out)
}

fn pack_i4_nibbles(values: &[u8], out: &mut Vec<u8>) {
    for chunk in values.chunks(2) {
        let first = chunk[0] & 0x0f;
        let second = chunk.get(1).copied().unwrap_or(0) & 0x0f;
        out.push((first << 4) | second);
    }
}

fn unpack_i4_nibbles(bytes: &[u8], count: usize) -> Vec<u8> {
    let mut values = Vec::with_capacity(count);
    for byte in bytes {
        values.push(byte >> 4);
        if values.len() < count {
            values.push(byte & 0x0f);
        }
    }
    values
}

fn pack_residual_signs(values: &[bool], out: &mut Vec<u8>) {
    for chunk in values.chunks(8) {
        let mut byte = 0_u8;
        for (index, value) in chunk.iter().enumerate() {
            if *value {
                byte |= 1 << index;
            }
        }
        out.push(byte);
    }
}

fn unpack_residual_signs(bytes: &[u8], count: usize) -> Vec<bool> {
    let mut values = Vec::with_capacity(count);
    for byte in bytes {
        for bit in 0..8 {
            if values.len() == count {
                break;
            }
            values.push((byte & (1 << bit)) != 0);
        }
    }
    values
}

fn phase1_coordinate_count(value_count: usize) -> usize {
    value_count.div_ceil(4) * 4
}

fn checked_phase1_coordinate_count(value_count: usize) -> Result<usize, QatqError> {
    value_count
        .checked_add(3)
        .map(|value| (value / 4) * 4)
        .ok_or(QatqError::ValueCountTooLarge(value_count))
}

fn checked_value_byte_len(value_count: usize) -> Result<usize, QatqError> {
    value_count
        .checked_mul(4)
        .ok_or(QatqError::ValueCountTooLarge(value_count))
}

#[derive(Clone, Copy, Debug)]
enum RotationDirection {
    Forward,
    Inverse,
}

#[derive(Clone, Copy, Debug)]
struct Quaternion {
    r: f32,
    i: f32,
    j: f32,
    k: f32,
}

impl Quaternion {
    fn from_slice(values: &[f32]) -> Self {
        Self {
            r: values.first().copied().unwrap_or(0.0),
            i: values.get(1).copied().unwrap_or(0.0),
            j: values.get(2).copied().unwrap_or(0.0),
            k: values.get(3).copied().unwrap_or(0.0),
        }
    }

    fn to_array(self) -> [f32; 4] {
        [self.r, self.i, self.j, self.k]
    }

    fn conjugate(self) -> Self {
        Self {
            r: self.r,
            i: -self.i,
            j: -self.j,
            k: -self.k,
        }
    }
}

fn hamilton_product(a: Quaternion, b: Quaternion) -> Quaternion {
    Quaternion {
        r: a.r * b.r - a.i * b.i - a.j * b.j - a.k * b.k,
        i: a.r * b.i + a.i * b.r + a.j * b.k - a.k * b.j,
        j: a.r * b.j - a.i * b.k + a.j * b.r + a.k * b.i,
        k: a.r * b.k + a.i * b.j - a.j * b.i + a.k * b.r,
    }
}

fn rotate_values(values: &[f32], seed: u64, direction: RotationDirection) -> Vec<f32> {
    let coord_count = phase1_coordinate_count(values.len());
    let mut rotated = Vec::with_capacity(coord_count);
    for lane in 0..coord_count.div_ceil(4) {
        let start = lane * 4;
        let end = values.len().min(start + 4);
        let input = Quaternion::from_slice(&values[start..end]);
        let rotation = deterministic_unit_quaternion(seed, lane as u64);
        let inverse = rotation.conjugate();
        let output = match direction {
            RotationDirection::Forward => {
                hamilton_product(hamilton_product(rotation, input), inverse)
            }
            RotationDirection::Inverse => {
                hamilton_product(hamilton_product(inverse, input), rotation)
            }
        };
        rotated.extend_from_slice(&output.to_array());
    }
    rotated
}

fn deterministic_unit_quaternion(seed: u64, lane: u64) -> Quaternion {
    let mut state = seed ^ lane.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0xa076_1d64_78bd_642f;
    let mut components = [0.0_f32; 4];
    let mut norm_squared = 0.0_f32;
    for component in &mut components {
        state = splitmix64(state);
        let unit = ((state >> 40) as u32) as f32 / ((1_u32 << 24) - 1) as f32;
        *component = unit.mul_add(2.0, -1.0);
        norm_squared += *component * *component;
    }
    if norm_squared <= f32::EPSILON || !norm_squared.is_finite() {
        return Quaternion {
            r: 1.0,
            i: 0.0,
            j: 0.0,
            k: 0.0,
        };
    }
    let norm = norm_squared.sqrt();
    Quaternion {
        r: components[0] / norm,
        i: components[1] / norm,
        j: components[2] / norm,
        k: components[3] / norm,
    }
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut mixed = value;
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed ^ (mixed >> 31)
}

fn write_header(out: &mut Vec<u8>, mode: CodecMode, value_count: usize, scale: f32, checksum: u64) {
    assert!(
        value_count <= u64::MAX as usize,
        "value count exceeds portable payload header"
    );
    assert!(
        value_count <= MAX_VALUES_PER_PAYLOAD,
        "value count exceeds single-payload decoder bound; use chunked APIs"
    );
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.push(mode.id());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&(value_count as u64).to_be_bytes());
    out.extend_from_slice(&scale.to_bits().to_be_bytes());
    out.extend_from_slice(&checksum.to_be_bytes());
}

fn write_container_header(out: &mut Vec<u8>, total_values: usize, chunk_count: usize) {
    assert!(
        total_values <= u64::MAX as usize,
        "value count exceeds portable container header"
    );
    assert!(
        chunk_count <= u32::MAX as usize,
        "chunk count exceeds container header"
    );
    out.extend_from_slice(CONTAINER_MAGIC);
    out.push(VERSION);
    out.push(CodecMode::Phase2Lossless.id());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&(total_values as u64).to_be_bytes());
    out.extend_from_slice(&(chunk_count as u32).to_be_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
}

fn checksum_f32_bits(values: &[f32]) -> u64 {
    let mut hash = FNV_OFFSET;
    for value in values {
        hash = checksum_bits_update(hash, value.to_bits());
    }
    hash
}

fn checksum_bits_update(hash: u64, bits: u32) -> u64 {
    let bytes = bits.to_be_bytes();
    checksum_four_bytes_update(hash, bytes[0], bytes[1], bytes[2], bytes[3])
}

fn checksum_four_bytes_update(mut hash: u64, first: u8, second: u8, third: u8, fourth: u8) -> u64 {
    hash ^= first as u64;
    hash = hash.wrapping_mul(FNV_PRIME);
    hash ^= second as u64;
    hash = hash.wrapping_mul(FNV_PRIME);
    hash ^= third as u64;
    hash = hash.wrapping_mul(FNV_PRIME);
    hash ^= fourth as u64;
    hash = hash.wrapping_mul(FNV_PRIME);
    hash
}

fn checksum_two_high_bytes_update(mut hash: u64, first: u8, second: u8) -> u64 {
    hash ^= first as u64;
    hash = hash.wrapping_mul(FNV_PRIME);
    hash ^= second as u64;
    hash = hash.wrapping_mul(FNV_PRIME);
    hash.wrapping_mul(FNV_PRIME_SQUARED)
}

#[derive(Debug)]
struct Header {
    mode: CodecMode,
    value_count: usize,
    scale: f32,
    checksum: u64,
}

#[derive(Debug)]
struct ContainerHeader {
    total_values: usize,
    chunk_count: u32,
}

impl ContainerHeader {
    fn parse(payload: &[u8]) -> Result<Self, QatqError> {
        if payload.len() < CONTAINER_HEADER_LEN {
            return Err(QatqError::PayloadTooShort {
                actual: payload.len(),
                minimum: CONTAINER_HEADER_LEN,
            });
        }
        if &payload[0..4] != CONTAINER_MAGIC {
            return Err(QatqError::InvalidMagic);
        }
        let version = payload[4];
        if version != VERSION {
            return Err(QatqError::UnsupportedVersion(version));
        }
        let mode = CodecMode::from_id(payload[5])?;
        if mode != CodecMode::Phase2Lossless {
            return Err(QatqError::UnsupportedMode(mode.id()));
        }
        if payload[6..8] != [0, 0] || payload[20..24] != [0, 0, 0, 0] {
            return Err(QatqError::InvalidContainer);
        }
        let total_values_u64 =
            u64::from_be_bytes(payload[8..16].try_into().expect("fixed container header"));
        let total_values = usize::try_from(total_values_u64)
            .map_err(|_| QatqError::ValueCountTooLarge(usize::MAX))?;
        let chunk_count =
            u32::from_be_bytes(payload[16..20].try_into().expect("fixed container header"));
        Ok(Self {
            total_values,
            chunk_count,
        })
    }
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
        if payload[6..8] != [0, 0] {
            return Err(QatqError::InvalidHeader);
        }
        let value_count_u64 = u64::from_be_bytes(payload[8..16].try_into().expect("fixed header"));
        let value_count = usize::try_from(value_count_u64)
            .map_err(|_| QatqError::ValueCountTooLarge(usize::MAX))?;
        if value_count > MAX_VALUES_PER_PAYLOAD {
            return Err(QatqError::ValueCountTooLarge(value_count));
        }
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
    fn rejects_nonzero_reserved_header_bytes() {
        let mut encoded = encode_lossy_i4(&[1.0, 2.0]);
        encoded[6] = 1;
        assert_eq!(decode(&encoded), Err(QatqError::InvalidHeader));

        encoded[6] = 0;
        encoded[7] = 1;
        assert_eq!(decode(&encoded), Err(QatqError::InvalidHeader));
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

    #[test]
    fn phase1_q4_roundtrip_preserves_shape_and_compresses() {
        let values: Vec<f32> = (0..1025)
            .map(|index| {
                let x = index as f32;
                (x * 0.03125).sin() * 2.5 + (x * 0.0078125).cos() * 0.75
            })
            .collect();

        let encoded = encode_phase1_q4(&values);
        let decoded = decode_phase1_q4(&encoded).unwrap();

        assert_eq!(decoded.len(), values.len());
        assert!(encoded.len() < values.len() * 4);
        assert!(compression_ratio(encoded.len(), values.len()) < 0.7);
        let max_abs = max_abs_error(&values, &decoded);
        assert!(max_abs < 0.6, "max_abs={max_abs}");
    }

    #[test]
    fn phase1_q4_seed_is_deterministic_and_changes_payload() {
        let values: Vec<f32> = (0..128)
            .map(|index| ((index as f32) * 0.21).sin())
            .collect();
        let first = encode_phase1_q4_with_config(&values, Phase1Config { seed: 7 });
        let second = encode_phase1_q4_with_config(&values, Phase1Config { seed: 7 });
        let third = encode_phase1_q4_with_config(&values, Phase1Config { seed: 8 });

        assert_eq!(first, second);
        assert_ne!(first, third);
        assert_eq!(
            decode_phase1_q4(&first).unwrap().len(),
            decode_phase1_q4(&third).unwrap().len()
        );
    }

    #[test]
    fn phase1_q4_handles_partial_quaternion_lane() {
        let values = [1.0_f32, -0.25, 0.5, 2.0, -3.0, 0.125];

        let encoded = encode_phase1_q4(&values);
        let decoded = decode_phase1_q4(&encoded).unwrap();

        assert_eq!(decoded.len(), values.len());
        assert!(decoded.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn phase1_q4_handles_empty_tensor() {
        let encoded = encode_phase1_q4(&[]);
        let decoded = decode_phase1_q4(&encoded).unwrap();

        assert!(decoded.is_empty());
        assert_eq!(encoded.len(), HEADER_LEN + PHASE1_METADATA_LEN);
    }

    #[test]
    fn phase1_q4_rejects_bad_body_magic() {
        let mut encoded = encode_phase1_q4(&[1.0, 2.0, 3.0, 4.0]);
        encoded[HEADER_LEN] = b'X';

        assert_eq!(
            decode_phase1_q4(&encoded),
            Err(QatqError::InvalidPhase1Body)
        );
    }

    #[test]
    fn phase1_q4_rejects_truncated_body() {
        let mut encoded = encode_phase1_q4(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        encoded.pop();

        assert_eq!(
            decode_phase1_q4(&encoded),
            Err(QatqError::LengthMismatch {
                expected: PHASE1_METADATA_LEN + 4 + 1,
                actual: PHASE1_METADATA_LEN + 4
            })
        );
    }

    #[test]
    fn phase2_lossless_roundtrip_preserves_bits() {
        let values = [
            0.0_f32,
            -0.0,
            1.25,
            -128.5,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::from_bits(0x7fc0_1234),
            f32::from_bits(0xff80_0001),
        ];

        let encoded = encode_phase2_lossless(&values);
        let decoded = decode_phase2_lossless(&encoded).unwrap();

        let before: Vec<u32> = values.iter().map(|value| value.to_bits()).collect();
        let after: Vec<u32> = decoded.iter().map(|value| value.to_bits()).collect();
        assert_eq!(after, before);
    }

    #[test]
    fn phase2_lossless_exhaustive_roundtrip_preserves_bits() {
        let values: Vec<f32> = (0..256)
            .map(|index| ((index as f32) * 0.03125).sin())
            .collect();

        let fast = encode_phase2_lossless(&values);
        let exhaustive = encode_phase2_lossless_exhaustive(&values);
        let decoded_fast = decode_phase2_lossless(&fast).unwrap();
        let decoded_exhaustive = decode_phase2_lossless(&exhaustive).unwrap();

        assert_eq!(f32_bits(&decoded_fast), f32_bits(&values));
        assert_eq!(f32_bits(&decoded_exhaustive), f32_bits(&values));
        assert!(exhaustive.len() <= fast.len());
    }

    #[test]
    fn phase2_lossless_seed_is_deterministic_and_changes_payload() {
        let values: Vec<f32> = (0..128)
            .map(|index| ((index as f32) * 0.017).sin())
            .collect();
        let first = encode_phase2_predictor_for_test(&values, Phase1Config { seed: 11 });
        let second = encode_phase2_predictor_for_test(&values, Phase1Config { seed: 11 });
        let third = encode_phase2_predictor_for_test(&values, Phase1Config { seed: 12 });

        assert_eq!(first[HEADER_LEN + 4], PHASE2_STRATEGY_PREDICTOR_XOR);
        assert_eq!(third[HEADER_LEN + 4], PHASE2_STRATEGY_PREDICTOR_XOR);
        assert_eq!(first, second);
        assert_ne!(first, third);
        assert_eq!(decode_phase2_lossless(&first).unwrap(), values);
        assert_eq!(decode_phase2_lossless(&third).unwrap(), values);
    }

    #[test]
    fn phase2_lossless_rejects_bad_body_magic() {
        let mut encoded = encode_phase2_lossless(&[1.0, 2.0, 3.0, 4.0]);
        encoded[HEADER_LEN] = b'X';

        assert_eq!(
            decode_phase2_lossless(&encoded),
            Err(QatqError::InvalidPhase2Body)
        );
    }

    #[test]
    fn phase2_lossless_rejects_nonzero_reserved_prefix_bytes() {
        let mut encoded = encode_phase2_lossless(&[1.0, 2.0, 3.0, 4.0]);
        encoded[HEADER_LEN + 5] = 1;

        assert_eq!(
            decode_phase2_lossless(&encoded),
            Err(QatqError::InvalidPhase2Body)
        );
    }

    #[test]
    fn phase2_lossless_rejects_oversized_header_count() {
        let mut encoded = Vec::new();
        write_test_header_unchecked(
            &mut encoded,
            CodecMode::Phase2Lossless,
            (MAX_VALUES_PER_PAYLOAD + 1) as u64,
            1.0,
            checksum_f32_bits(&[]),
        );
        write_phase2_prefix(&mut encoded, PHASE2_STRATEGY_RAW_BITS);

        assert_eq!(
            decode_phase2_lossless(&encoded),
            Err(QatqError::ValueCountTooLarge(MAX_VALUES_PER_PAYLOAD + 1))
        );
    }

    #[test]
    fn validate_single_payload_value_count_rejects_oversized_inputs() {
        assert_eq!(
            validate_single_payload_value_count(MAX_VALUES_PER_PAYLOAD + 1),
            Err(QatqError::ValueCountTooLarge(MAX_VALUES_PER_PAYLOAD + 1))
        );
    }

    #[test]
    fn try_encode_lossless_f32_roundtrip_preserves_bits() {
        let values = [
            0.0_f32,
            -0.0,
            f32::NEG_INFINITY,
            f32::from_bits(0x7fa0_1234),
        ];
        let encoded = try_encode_lossless_f32(&values).unwrap();

        assert_eq!(f32_bits(&decode(&encoded).unwrap()), f32_bits(&values));
    }

    #[test]
    fn try_encode_lossy_i4_roundtrip_preserves_shape() {
        let values = [-3.0_f32, -1.0, 0.0, 0.75, 2.0];
        let encoded = try_encode_lossy_i4(&values).unwrap();
        let decoded = decode_lossy_i4(&encoded).unwrap();

        assert_eq!(decoded.len(), values.len());
    }

    #[test]
    fn try_encode_dispatches_all_single_payload_modes() {
        let values = [0.25_f32, -0.5, 1.0, -2.0, 4.0];
        let modes = [
            CodecMode::LossyI4,
            CodecMode::LosslessF32,
            CodecMode::Phase1Q4,
            CodecMode::Phase2Lossless,
        ];

        for mode in modes {
            let encoded = try_encode(&values, mode).unwrap();
            let decoded = decode(&encoded).unwrap();
            assert_eq!(decoded.len(), values.len());
            if matches!(mode, CodecMode::LosslessF32 | CodecMode::Phase2Lossless) {
                assert_eq!(f32_bits(&decoded), f32_bits(&values));
            }
        }
    }

    #[test]
    fn try_encode_phase2_lossless_roundtrip_preserves_bits() {
        let values = [0.0_f32, -0.0, f32::INFINITY, f32::from_bits(0x7fc0_1234)];
        let encoded = try_encode(&values, CodecMode::Phase2Lossless).unwrap();

        assert_eq!(f32_bits(&decode(&encoded).unwrap()), f32_bits(&values));
    }

    #[test]
    fn try_encode_seeded_phase2_lossless_roundtrip_preserves_bits() {
        let values = [0.25_f32, -0.5, f32::from_bits(0x7fc0_5678), 2.0];
        let encoded =
            try_encode_phase2_lossless_with_config(&values, Phase1Config { seed: 17 }).unwrap();

        assert_eq!(f32_bits(&decode(&encoded).unwrap()), f32_bits(&values));
    }

    #[test]
    fn phase2_lossless_rejects_truncated_residual_stream() {
        let values: Vec<f32> = (0..128)
            .map(|index| ((index as f32) * 0.017).sin())
            .collect();
        let mut encoded = encode_phase2_predictor_for_test(&values, Phase1Config { seed: 1 });
        assert_eq!(encoded[HEADER_LEN + 4], PHASE2_STRATEGY_PREDICTOR_XOR);
        encoded.pop();

        assert_eq!(
            decode_phase2_lossless(&encoded),
            Err(QatqError::InvalidResidualStream)
        );
    }

    #[test]
    fn phase2_lossless_detects_payload_corruption() {
        let values = vec![0.0_f32; 128];
        let mut encoded = encode_phase2_lossless(&values);
        assert_eq!(encoded[HEADER_LEN + 4], PHASE2_STRATEGY_BYTE_PLANE_BLOCKS);
        let last = encoded.last_mut().unwrap();
        *last ^= 0x01;

        assert!(matches!(
            decode_phase2_lossless(&encoded),
            Err(QatqError::ChecksumMismatch { .. }) | Err(QatqError::InvalidResidualStream)
        ));
    }

    #[test]
    fn phase2_lossless_uses_raw_bits_when_predictor_residual_is_larger() {
        let values = [
            f32::from_bits(0x0102_0304),
            f32::from_bits(0x1122_3344),
            f32::from_bits(0x5566_7788),
            f32::from_bits(0x99aa_bbcc),
        ];
        let encoded = encode_phase2_lossless(&values);

        assert_eq!(encoded[HEADER_LEN + 4], PHASE2_STRATEGY_RAW_BITS);
        assert_eq!(
            f32_bits(&decode_phase2_lossless(&encoded).unwrap()),
            f32_bits(&values)
        );
    }

    #[test]
    fn phase2_decision_passes_through_raw_bits_as_f32le() {
        let values = [
            f32::from_bits(0x0102_0304),
            f32::from_bits(0x1122_3344),
            f32::from_bits(0x5566_7788),
            f32::from_bits(0x99aa_bbcc),
        ];
        let decision =
            try_encode_phase2_lossless_decision_with_config(&values, Phase1Config::default())
                .unwrap();

        let expected = encode_f32_bits_le(&values);
        assert!(decision.should_pass_through());
        assert!(!decision.should_compress());
        assert_eq!(decision.strategy(), None);
        assert_eq!(decision.raw_f32le_len(), expected.len());
        assert_eq!(decision.stored_bytes(), expected.as_slice());
    }

    #[test]
    fn phase2_decision_compresses_non_raw_strategy() {
        let values = vec![0.0_f32; 128];
        let decision = encode_phase2_lossless_decision(&values);

        match decision {
            Phase2EncodeDecision::Compressed {
                payload,
                strategy,
                raw_f32le_len,
            } => {
                assert_eq!(strategy, Phase2Strategy::BytePlaneBlocks);
                assert_eq!(raw_f32le_len, values.len() * 4);
                assert!(payload.len() < raw_f32le_len);
                assert_eq!(
                    f32_bits(&decode_phase2_lossless(&payload).unwrap()),
                    f32_bits(&values)
                );
            }
            Phase2EncodeDecision::PassThroughRaw { .. } => {
                panic!("compressible values should return a compressed decision")
            }
        }
    }

    #[test]
    fn production_chunk_roundtrip_restores_compressed_payload() {
        let values = vec![0.0_f32; 128];
        let encoded = try_encode_production_chunk(&values).unwrap();

        assert!(encoded.should_compress());
        assert_eq!(encoded.metadata.storage_label(), "qatq-phase2");
        assert_eq!(encoded.metadata.raw_f32le_len, values.len() * 4);
        assert_eq!(
            encoded.metadata.strategy,
            Some(Phase2Strategy::BytePlaneBlocks)
        );

        let restored = restore_production_chunk(&encoded.metadata, encoded.stored_bytes()).unwrap();
        assert_eq!(f32_bits(&restored), f32_bits(&values));
    }

    #[test]
    fn production_chunk_roundtrip_restores_pass_through_payload() {
        let values = [
            f32::from_bits(0x0102_0304),
            f32::from_bits(0x1122_3344),
            f32::from_bits(0x5566_7788),
            f32::from_bits(0x99aa_bbcc),
        ];
        let encoded = try_encode_production_chunk(&values).unwrap();

        assert!(encoded.should_pass_through());
        assert_eq!(encoded.metadata.storage_label(), "raw-f32le-pass-through");
        assert_eq!(encoded.metadata.raw_f32le_len, values.len() * 4);
        assert_eq!(encoded.metadata.strategy, None);

        let restored = restore_production_chunk(&encoded.metadata, encoded.stored_bytes()).unwrap();
        assert_eq!(f32_bits(&restored), f32_bits(&values));
    }

    #[test]
    fn production_chunk_rejects_mismatched_metadata() {
        let values = vec![0.0_f32; 128];
        let mut encoded = try_encode_production_chunk(&values).unwrap();
        encoded.metadata.raw_f32le_len += 4;

        assert_eq!(
            restore_production_chunk(&encoded.metadata, encoded.stored_bytes()),
            Err(QatqError::LengthMismatch {
                expected: values.len() * 4 + 4,
                actual: values.len() * 4
            })
        );
    }

    #[test]
    fn phase2_lossless_uses_byte_plane_blocks_when_raw_planes_are_repetitive() {
        let values = vec![0.0_f32; 128];
        let encoded = encode_phase2_lossless(&values);

        assert_eq!(encoded[HEADER_LEN + 4], PHASE2_STRATEGY_BYTE_PLANE_BLOCKS);
        assert!(encoded.len() < encode_lossless_f32(&values).len());
        assert_eq!(decode_phase2_lossless(&encoded).unwrap(), values);
    }

    #[test]
    fn phase2_lossless_uses_delta_xor_byte_plane_for_adjacent_bit_residuals() {
        let mut bits = 0x3f00_0001_u32;
        let values: Vec<f32> = (0..256)
            .map(|_| {
                bits ^= 0x0102_0304;
                f32::from_bits(bits)
            })
            .collect();
        let encoded = encode_phase2_lossless(&values);

        assert_eq!(
            encoded[HEADER_LEN + 4],
            PHASE2_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE
        );
        assert!(encoded.len() < encode_lossless_f32(&values).len());
        assert_eq!(
            f32_bits(&decode_phase2_lossless(&encoded).unwrap()),
            f32_bits(&values)
        );
    }

    #[test]
    fn phase2_lossless_strategy_reports_selected_strategy() {
        let values = [0.0_f32; 128];
        let encoded = encode_phase2_lossless(&values);

        assert_eq!(
            phase2_lossless_strategy(&encoded),
            Ok(Phase2Strategy::BytePlaneBlocks)
        );
        assert_eq!(
            phase2_lossless_strategy(&encode_lossless_f32(&values)),
            Err(QatqError::UnsupportedMode(2))
        );
    }

    #[test]
    fn phase2_lossless_fast_accepts_compression_positive_byte_plane_candidate() {
        let values: Vec<f32> = (0..512)
            .map(|index| ((index as f32) * 0.03125).sin())
            .collect();
        let encoded = encode_phase2_lossless(&values);

        assert_eq!(encoded[HEADER_LEN + 4], PHASE2_STRATEGY_BYTE_PLANE_RLE);
        assert!(encoded.len() < encode_lossless_f32(&values).len());
        assert_eq!(
            f32_bits(&decode_phase2_lossless(&encoded).unwrap()),
            f32_bits(&values)
        );
    }

    #[test]
    fn phase2_lossless_byte_rle_compresses_repeated_nonzero_bytes() {
        let values = vec![1.0_f32; 128];
        let encoded = encode_phase2_lossless(&values);

        assert_eq!(encoded[HEADER_LEN + 4], PHASE2_STRATEGY_BYTE_PLANE_BLOCKS);
        assert!(encoded.len() < encode_lossless_f32(&values).len());
        assert_eq!(decode_phase2_lossless(&encoded).unwrap(), values);
    }

    #[test]
    fn phase2_lossless_delta_xor_byte_plane_rejects_truncated_stream() {
        let mut bits = 0x3f00_0001_u32;
        let values: Vec<f32> = (0..256)
            .map(|_| {
                bits ^= 0x0102_0304;
                f32::from_bits(bits)
            })
            .collect();
        let mut encoded = encode_phase2_lossless(&values);
        assert_eq!(
            encoded[HEADER_LEN + 4],
            PHASE2_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE
        );
        encoded.pop();

        assert_eq!(
            decode_phase2_lossless(&encoded),
            Err(QatqError::InvalidResidualStream)
        );
    }

    #[test]
    fn phase2_lossless_rejects_truncated_byte_plane_block() {
        let values = vec![1.0_f32; 128];
        let mut encoded = encode_phase2_lossless(&values);
        assert_eq!(encoded[HEADER_LEN + 4], PHASE2_STRATEGY_BYTE_PLANE_BLOCKS);
        encoded.pop();

        assert_eq!(
            decode_phase2_lossless(&encoded),
            Err(QatqError::InvalidResidualStream)
        );
    }

    #[test]
    fn phase2_lossless_rejects_zero_length_byte_run() {
        let mut encoded = Vec::new();
        write_header(
            &mut encoded,
            CodecMode::Phase2Lossless,
            1,
            1.0,
            checksum_f32_bits(&[0.0]),
        );
        write_phase2_prefix(&mut encoded, PHASE2_STRATEGY_BYTE_RLE);
        encoded.extend_from_slice(&[XOR_ZERO_RUN, 0, 0]);

        assert_eq!(
            decode_phase2_lossless(&encoded),
            Err(QatqError::InvalidResidualStream)
        );
    }

    #[test]
    fn phase2_lossless_rejects_unknown_byte_run_token() {
        let mut encoded = Vec::new();
        write_header(
            &mut encoded,
            CodecMode::Phase2Lossless,
            1,
            1.0,
            checksum_f32_bits(&[0.0]),
        );
        write_phase2_prefix(&mut encoded, PHASE2_STRATEGY_BYTE_RLE);
        encoded.extend_from_slice(&[99, 0, 4]);

        assert_eq!(
            decode_phase2_lossless(&encoded),
            Err(QatqError::InvalidResidualStream)
        );
    }

    #[test]
    fn phase2_lossless_rejects_trailing_byte_run_data() {
        let mut encoded = Vec::new();
        write_header(
            &mut encoded,
            CodecMode::Phase2Lossless,
            1,
            1.0,
            checksum_f32_bits(&[0.0]),
        );
        write_phase2_prefix(&mut encoded, PHASE2_STRATEGY_BYTE_RLE);
        encoded.extend_from_slice(&[XOR_ZERO_RUN, 0, 4, 0xaa]);

        assert_eq!(
            decode_phase2_lossless(&encoded),
            Err(QatqError::InvalidResidualStream)
        );
    }

    #[test]
    fn phase2_lossless_chunks_roundtrip_preserves_bits() {
        let values: Vec<f32> = (0..257)
            .map(|index| {
                if index % 31 == 0 {
                    f32::from_bits(0x7fc0_0000 | index as u32)
                } else {
                    ((index as f32) * 0.037).sin()
                }
            })
            .collect();

        let chunks =
            encode_phase2_lossless_chunks_with_config(&values, 64, Phase1Config { seed: 99 })
                .unwrap();
        let decoded = decode_phase2_lossless_chunks(chunks.iter().map(Vec::as_slice)).unwrap();

        assert_eq!(chunks.len(), 5);
        assert_eq!(f32_bits(&decoded), f32_bits(&values));
    }

    #[test]
    fn phase2_lossless_chunks_handle_empty_input_as_one_payload() {
        let chunks = encode_phase2_lossless_chunks(&[], 64).unwrap();
        let decoded = decode_phase2_lossless_chunks(chunks.iter().map(Vec::as_slice)).unwrap();

        assert_eq!(chunks.len(), 1);
        assert!(decoded.is_empty());
    }

    #[test]
    fn phase2_lossless_chunks_reject_invalid_chunk_sizes() {
        assert_eq!(
            encode_phase2_lossless_chunks(&[1.0], 0),
            Err(QatqError::InvalidChunkSize(0))
        );
        assert_eq!(
            encode_phase2_lossless_chunks(&[1.0], MAX_VALUES_PER_PAYLOAD + 1),
            Err(QatqError::InvalidChunkSize(MAX_VALUES_PER_PAYLOAD + 1))
        );
    }

    #[test]
    fn phase2_lossless_container_roundtrip_preserves_bits_through_decode() {
        let values: Vec<f32> = (0..259)
            .map(|index| match index % 53 {
                0 => -0.0,
                1 => f32::from_bits(0x7fc0_1000 | index as u32),
                _ => ((index as f32) * 0.019).cos(),
            })
            .collect();

        let encoded =
            encode_phase2_lossless_container_with_config(&values, 64, Phase1Config { seed: 123 })
                .unwrap();
        let decoded = decode(&encoded).unwrap();

        assert_eq!(&encoded[0..4], CONTAINER_MAGIC);
        assert_eq!(f32_bits(&decoded), f32_bits(&values));
    }

    #[test]
    fn phase2_lossless_container_handles_empty_input_as_one_chunk() {
        let encoded = encode_phase2_lossless_container(&[], 64).unwrap();
        let decoded = decode_phase2_lossless_container(&encoded).unwrap();

        assert_eq!(u32::from_be_bytes(encoded[16..20].try_into().unwrap()), 1);
        assert!(decoded.is_empty());
    }

    #[test]
    fn phase2_lossless_container_payload_visitor_preserves_chunk_order() {
        let values: Vec<f32> = (0..10).map(|index| index as f32).collect();
        let encoded = encode_phase2_lossless_container(&values, 4).unwrap();
        let mut chunk_lengths = Vec::new();
        let mut decoded = Vec::new();

        for_each_phase2_lossless_container_payload(&encoded, |chunk| {
            let chunk_values = decode_phase2_lossless(chunk)?;
            chunk_lengths.push(chunk_values.len());
            decoded.extend(chunk_values);
            Ok(())
        })
        .unwrap();

        assert_eq!(chunk_lengths, [4, 4, 2]);
        assert_eq!(decoded, values);
    }

    #[test]
    fn phase2_lossless_container_rejects_invalid_chunk_size() {
        assert_eq!(
            encode_phase2_lossless_container(&[1.0], 0),
            Err(QatqError::InvalidChunkSize(0))
        );
    }

    #[test]
    fn phase2_lossless_container_rejects_zero_chunk_count() {
        let mut encoded = Vec::new();
        write_container_header(&mut encoded, 0, 0);

        assert_eq!(
            decode_phase2_lossless_container(&encoded),
            Err(QatqError::InvalidContainer)
        );
    }

    #[test]
    fn phase2_lossless_container_rejects_nonzero_reserved_bytes() {
        let mut encoded = encode_phase2_lossless_container(&[1.0, 2.0], 1).unwrap();
        encoded[6] = 1;

        assert_eq!(
            decode_phase2_lossless_container(&encoded),
            Err(QatqError::InvalidContainer)
        );

        encoded[6] = 0;
        encoded[20] = 1;
        assert_eq!(
            decode_phase2_lossless_container(&encoded),
            Err(QatqError::InvalidContainer)
        );
    }

    #[test]
    fn phase2_lossless_container_rejects_truncated_chunk_body() {
        let mut encoded = encode_phase2_lossless_container(&[1.0, 2.0, 3.0], 2).unwrap();
        encoded.pop();

        assert_eq!(
            decode_phase2_lossless_container(&encoded),
            Err(QatqError::InvalidContainer)
        );
    }

    #[test]
    fn phase2_lossless_container_payload_visitor_validates_before_callbacks() {
        let mut encoded = encode_phase2_lossless_container(&[1.0, 2.0, 3.0], 2).unwrap();
        encoded.pop();
        let mut visited = 0;

        assert_eq!(
            for_each_phase2_lossless_container_payload(&encoded, |_| {
                visited += 1;
                Ok(())
            }),
            Err(QatqError::InvalidContainer)
        );
        assert_eq!(visited, 0);
    }

    #[test]
    fn phase2_lossless_container_rejects_total_value_mismatch() {
        let mut encoded = encode_phase2_lossless_container(&[1.0, 2.0, 3.0], 2).unwrap();
        encoded[15] = 2;

        assert_eq!(
            decode_phase2_lossless_container(&encoded),
            Err(QatqError::InvalidContainer)
        );
    }

    #[test]
    fn phase2_lossless_container_rejects_trailing_data() {
        let mut encoded = encode_phase2_lossless_container(&[1.0, 2.0, 3.0], 2).unwrap();
        encoded.push(0);

        assert_eq!(
            decode_phase2_lossless_container(&encoded),
            Err(QatqError::InvalidContainer)
        );
    }

    #[test]
    fn bounded_byte_run_encoder_abandons_candidates_larger_than_limit() {
        let incompressible = [1_u8, 2, 3, 4];
        let compressible = [0_u8, 0, 0, 0];

        assert_eq!(encode_byte_runs_bounded(&incompressible, 3), None);
        assert_eq!(
            encode_byte_runs_bounded(&compressible, 3),
            Some(vec![XOR_ZERO_RUN, 0, 4])
        );
    }

    #[test]
    fn direct_byte_plane_run_encoder_matches_materialized_planes() {
        let values = [
            0.0_f32,
            -0.0,
            1.0,
            f32::from_bits(0x0102_0304),
            f32::from_bits(0x1111_1111),
            f32::from_bits(0x7fc0_1234),
        ];
        let raw = encode_f32_bits_be(&values);
        let materialized = encode_byte_runs_bounded(&encode_byte_planes(&raw), usize::MAX);
        let direct = encode_byte_plane_runs_bounded(&raw, usize::MAX);

        assert_eq!(direct, materialized);
    }

    #[test]
    fn direct_byte_plane_run_encoder_abandons_candidates_larger_than_limit() {
        let values = [
            f32::from_bits(0x0102_0304),
            f32::from_bits(0x0506_0708),
            f32::from_bits(0x090a_0b0c),
            f32::from_bits(0x0d0e_0f10),
        ];
        let raw = encode_f32_bits_be(&values);

        assert_eq!(encode_byte_plane_runs_bounded(&raw, 3), None);
    }

    #[test]
    fn direct_byte_plane_run_decoder_preserves_f32_bits() {
        let values = [
            0.0_f32,
            -0.0,
            1.0,
            f32::from_bits(0x0102_0304),
            f32::from_bits(0x1111_1111),
            f32::from_bits(0x7fc0_1234),
        ];
        let raw = encode_f32_bits_be(&values);
        let encoded = encode_byte_plane_runs_bounded(&raw, usize::MAX).unwrap();
        let decoded_words =
            decode_byte_plane_runs_to_words(&encoded, raw.len(), values.len()).unwrap();

        assert_eq!(decoded_words, f32_bits(&values));
    }

    #[test]
    fn direct_byte_plane_blocks_preserve_f32_bits() {
        let values = [
            0.0_f32,
            -0.0,
            1.0,
            f32::from_bits(0x0102_0304),
            f32::from_bits(0x1111_1111),
            f32::from_bits(0x7fc0_1234),
        ];
        let raw = encode_f32_bits_be(&values);
        let encoded = encode_byte_plane_blocks_bounded(&raw, usize::MAX).unwrap();
        let (decoded, checksum) =
            decode_byte_plane_blocks_to_f32_and_checksum(&encoded, raw.len(), values.len())
                .unwrap();

        assert_eq!(f32_bits(&decoded), f32_bits(&values));
        assert_eq!(checksum, checksum_f32_bits(&values));
    }

    #[test]
    fn direct_f32_byte_plane_blocks_matches_materialized_encoder() {
        let values = [
            0.0_f32,
            -0.0,
            1.0,
            f32::from_bits(0x0102_0304),
            f32::from_bits(0x1111_1111),
            f32::from_bits(0x7fc0_1234),
        ];
        let raw = encode_f32_bits_be(&values);
        let materialized = encode_byte_plane_blocks_bounded(&raw, usize::MAX).unwrap();
        let (direct, checksum) = encode_byte_plane_blocks_from_f32_bounded(&values, usize::MAX);

        assert_eq!(direct.as_deref(), Some(materialized.as_slice()));
        assert_eq!(checksum, checksum_f32_bits(&values));
    }

    #[test]
    fn direct_f32_byte_plane_blocks_preserves_phi_like_planes() {
        let values: Vec<f32> = (0..512)
            .map(|index| f32::from_bits(((index as u32) << 16) | 0x3f00_0000))
            .collect();
        let raw = encode_f32_bits_be(&values);
        let materialized = encode_byte_plane_blocks_bounded(&raw, usize::MAX).unwrap();
        let (direct, checksum) = encode_byte_plane_blocks_from_f32_bounded(&values, usize::MAX);
        let encoded = direct.expect("direct byte-plane-block candidate");
        let (decoded, decoded_checksum) =
            decode_byte_plane_blocks_to_f32_and_checksum(&encoded, raw.len(), values.len())
                .unwrap();

        assert_eq!(encoded, materialized);
        assert_eq!(checksum, checksum_f32_bits(&values));
        assert_eq!(decoded_checksum, checksum);
        assert_eq!(f32_bits(&decoded), f32_bits(&values));
    }

    #[test]
    fn specialized_two_high_raw_two_low_zero_encoder_matches_general_blocks() {
        let values: Vec<f32> = (0..512)
            .map(|index| f32::from_bits(((index as u32) << 24) | (((511 - index) as u32) << 16)))
            .collect();
        let raw = encode_f32_bits_be(&values);
        let materialized = encode_byte_plane_blocks_bounded(&raw, usize::MAX).unwrap();
        let (specialized, checksum) =
            encode_two_high_raw_two_low_zero_blocks_bounded(&values, usize::MAX)
                .expect("specialized byte-plane-block candidate");

        assert_eq!(specialized, materialized);
        assert_eq!(checksum, checksum_f32_bits(&values));
    }

    #[test]
    fn direct_delta_xor_byte_plane_run_encoder_matches_materialized_planes() {
        let mut bits = 0x3f00_0001_u32;
        let values: Vec<f32> = (0..64)
            .map(|index| {
                bits ^= if index % 3 == 0 {
                    0x0102_0304
                } else {
                    0x0000_0100
                };
                f32::from_bits(bits)
            })
            .collect();
        let delta_bits = encode_delta_xor_bits_be(&values);
        let materialized = encode_byte_runs_bounded(&encode_byte_planes(&delta_bits), usize::MAX);
        let direct = encode_delta_xor_byte_plane_runs_bounded(&values, usize::MAX);

        assert_eq!(direct, materialized);
    }

    #[test]
    fn direct_delta_xor_byte_plane_run_encoder_abandons_candidates_larger_than_limit() {
        let values = [
            f32::from_bits(0x0102_0304),
            f32::from_bits(0x0506_0708),
            f32::from_bits(0x090a_0b0c),
            f32::from_bits(0x0d0e_0f10),
        ];

        assert_eq!(encode_delta_xor_byte_plane_runs_bounded(&values, 3), None);
    }

    #[test]
    fn direct_byte_run_decoder_preserves_mixed_f32_bits() {
        let values = [
            0.0_f32,
            f32::from_bits(0x0102_0304),
            f32::from_bits(0x1111_1111),
        ];
        let mut raw = Vec::new();
        for value in values {
            raw.extend_from_slice(&value.to_bits().to_be_bytes());
        }
        let encoded = encode_byte_runs_bounded(&raw, usize::MAX).unwrap();
        let decoded = decode_byte_runs_to_f32(&encoded, raw.len(), values.len()).unwrap();

        assert_eq!(f32_bits(&decoded), f32_bits(&values));
    }

    #[test]
    fn direct_byte_run_decoder_rejects_trailing_data() {
        let encoded = [XOR_ZERO_RUN, 0, 4, 0xaa];

        assert_eq!(
            decode_byte_runs_to_f32(&encoded, 4, 1),
            Err(QatqError::InvalidResidualStream)
        );
    }

    fn encode_phase2_predictor_for_test(values: &[f32], config: Phase1Config) -> Vec<u8> {
        let parts = build_phase1_parts(values, config);
        let predicted = reconstruct_phase1_values(values.len(), &parts);
        let residuals = encode_xor_residuals(values, &predicted);
        let checksum = checksum_f32_bits(values);
        let mut out = Vec::new();
        write_header(
            &mut out,
            CodecMode::Phase2Lossless,
            values.len(),
            parts.scale,
            checksum,
        );
        write_phase2_prefix(&mut out, PHASE2_STRATEGY_PREDICTOR_XOR);
        out.extend_from_slice(&parts.seed.to_be_bytes());
        out.extend_from_slice(&parts.residual_scale.to_bits().to_be_bytes());
        pack_i4_nibbles(&parts.quantized, &mut out);
        pack_residual_signs(&parts.residual_signs, &mut out);
        out.extend_from_slice(&residuals);
        out
    }

    fn max_abs_error(before: &[f32], after: &[f32]) -> f32 {
        before
            .iter()
            .zip(after.iter())
            .map(|(before, after)| (before - after).abs())
            .fold(0.0_f32, f32::max)
    }

    fn f32_bits(values: &[f32]) -> Vec<u32> {
        values.iter().map(|value| value.to_bits()).collect()
    }

    fn write_test_header_unchecked(
        out: &mut Vec<u8>,
        mode: CodecMode,
        value_count: u64,
        scale: f32,
        checksum: u64,
    ) {
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.push(mode.id());
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&value_count.to_be_bytes());
        out.extend_from_slice(&scale.to_bits().to_be_bytes());
        out.extend_from_slice(&checksum.to_be_bytes());
    }
}
