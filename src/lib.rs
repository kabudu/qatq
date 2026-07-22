use std::fmt;

const MAGIC: &[u8; 4] = b"QATQ";
const CONTAINER_MAGIC: &[u8; 4] = b"QATC";
const VERSION: u8 = 1;
const CONTAINER_VERSION: u8 = 2;
const HEADER_LEN: usize = 28;
const CONTAINER_V2_HEADER_LEN: usize = 32;
const CONTAINER_CHUNK_LEN: usize = 4;
const PHASE1_BODY_MAGIC: &[u8; 4] = b"P1Q4";
const TURBOQUANT_BODY_MAGIC: &[u8; 4] = b"TQ4R";
const QATQ_EXACT_BODY_MAGIC: &[u8; 4] = b"QEX1";
const PHASE1_METADATA_LEN: usize = 20;
const TURBOQUANT_METADATA_LEN: usize = 20;
const QATQ_EXACT_PREFIX_LEN: usize = 8;
const QATQ_EXACT_PREDICTOR_METADATA_LEN: usize = 12;
const DEFAULT_PHASE1_SEED: u64 = 0x5141_5451_c0de_0001;
const TURBOQUANT_QJL_MAX_PROJECTIONS: usize = 256;
const TURBOQUANT_QJL_SEED_XOR: u64 = 0x514a_4c5f_5352_4854;
const XOR_ZERO_RUN: u8 = 0;
const XOR_RAW_RUN: u8 = 1;
const BYTE_REPEAT_RUN: u8 = 2;
const QATQ_EXACT_STRATEGY_PREDICTOR_XOR: u8 = 0;
const QATQ_EXACT_STRATEGY_RAW_BITS: u8 = 1;
const QATQ_EXACT_STRATEGY_BYTE_RLE: u8 = 2;
const QATQ_EXACT_STRATEGY_BYTE_PLANE_RLE: u8 = 3;
const QATQ_EXACT_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE: u8 = 4;
const QATQ_EXACT_STRATEGY_BYTE_PLANE_BLOCKS: u8 = 5;
const QATQ_EXACT_STRATEGY_BYTE_PLANE_PACKED_RLE: u8 = 6;
const QATQ_EXACT_STRATEGY_BYTE_PLANE_ZSTD: u8 = 7;
const QATQ_EXACT_STRATEGY_QUATERNION_CHAIN_ZSTD: u8 = 8;
const BYTE_PLANE_BLOCK_ZERO: u8 = 0;
const BYTE_PLANE_BLOCK_RAW: u8 = 1;
const BYTE_PLANE_BLOCK_REPEAT: u8 = 2;
const PACKED_ZERO_RUN: u8 = 0b0000_0000;
const PACKED_RAW_RUN: u8 = 0b0100_0000;
const PACKED_REPEAT_RUN: u8 = 0b1000_0000;
const PACKED_RUN_TAG_MASK: u8 = 0b1100_0000;
const PACKED_RUN_LEN_MASK: u8 = 0b0011_1111;
const PACKED_RUN_MAX_LEN: usize = 64;
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const FNV_PRIME_SQUARED: u64 = FNV_PRIME.wrapping_mul(FNV_PRIME);
const SQRT_PI_OVER_TWO: f32 = 1.253_314_1;
pub const MAX_VALUES_PER_PAYLOAD: usize = 1 << 26;
pub const DEFAULT_MAX_QATC_VALUES: usize = 1 << 32;
pub const DEFAULT_MAX_QATC_CHUNKS: usize = 1 << 20;
pub const DEFAULT_MAX_QATC_ENCODED_BYTES: usize = usize::MAX;
pub const DEFAULT_MAX_QATC_CHUNK_BYTES: usize =
    HEADER_LEN + QATQ_EXACT_PREFIX_LEN + (MAX_VALUES_PER_PAYLOAD * 4);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecMode {
    LossyI4,
    LosslessF32,
    Phase1Q4,
    QatqExact,
    TurboQuantQ4,
}

impl CodecMode {
    fn id(self) -> u8 {
        match self {
            Self::LossyI4 => 1,
            Self::LosslessF32 => 2,
            Self::Phase1Q4 => 3,
            Self::QatqExact => 4,
            Self::TurboQuantQ4 => 5,
        }
    }

    fn from_id(id: u8) -> Result<Self, QatqError> {
        match id {
            1 => Ok(Self::LossyI4),
            2 => Ok(Self::LosslessF32),
            3 => Ok(Self::Phase1Q4),
            4 => Ok(Self::QatqExact),
            5 => Ok(Self::TurboQuantQ4),
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
    InvalidTurboQuantBody,
    InvalidQatqExactBody,
    InvalidResidualStream,
    InvalidChunkSize(usize),
    InvalidContainer,
    ContainerLimitExceeded(&'static str),
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
            Self::InvalidTurboQuantBody => write!(f, "turboquant payload body is invalid"),
            Self::InvalidQatqExactBody => write!(f, "exact payload body is invalid"),
            Self::InvalidResidualStream => write!(f, "exact residual stream is invalid"),
            Self::InvalidChunkSize(size) => write!(f, "chunk size is invalid: {size}"),
            Self::InvalidContainer => write!(f, "chunked container is invalid"),
            Self::ContainerLimitExceeded(limit) => {
                write!(f, "chunked container exceeds decode limit: {limit}")
            }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TensorDType {
    F32,
    F16,
    BF16,
}

impl TensorDType {
    pub fn element_width(self) -> usize {
        match self {
            Self::F32 => 4,
            Self::F16 | Self::BF16 => 2,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F16 => "f16",
            Self::BF16 => "bf16",
        }
    }

    fn prefix_bytes(self) -> [u8; 3] {
        match self {
            Self::F32 => [0, 0, 0],
            Self::F16 => [1, 2, 0],
            Self::BF16 => [2, 2, 0],
        }
    }

    fn from_prefix(bytes: [u8; 3]) -> Result<Self, QatqError> {
        match bytes {
            [0, 0, 0] => Ok(Self::F32),
            [1, 2, 0] => Ok(Self::F16),
            [2, 2, 0] => Ok(Self::BF16),
            _ => Err(QatqError::InvalidQatqExactBody),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedTensor {
    pub dtype: TensorDType,
    pub bytes_le: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QatcDecodeLimits {
    pub max_total_values: usize,
    pub max_chunks: usize,
    pub max_encoded_bytes: usize,
    pub max_chunk_bytes: usize,
}

/// Validated metadata for one exact payload in a QATC container.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QatcExactChunkMetadata {
    pub encoded_bytes: usize,
    pub decoded_values: usize,
}

/// Validated metadata for an exact QATC container.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QatcExactContainerMetadata {
    pub encoded_bytes: usize,
    pub total_values: usize,
    pub chunks: Vec<QatcExactChunkMetadata>,
}

impl Default for QatcDecodeLimits {
    fn default() -> Self {
        Self {
            max_total_values: DEFAULT_MAX_QATC_VALUES,
            max_chunks: DEFAULT_MAX_QATC_CHUNKS,
            max_encoded_bytes: DEFAULT_MAX_QATC_ENCODED_BYTES,
            max_chunk_bytes: DEFAULT_MAX_QATC_CHUNK_BYTES,
        }
    }
}

pub fn parse_mode(mode: &str) -> Result<CodecMode, QatqError> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "" => Err(QatqError::EmptyMode),
        "lossy-i4" | "i4" | "qatq-i4" => Ok(CodecMode::LossyI4),
        "lossless-f32" | "f32" | "exact-f32" => Ok(CodecMode::LosslessF32),
        "turboquant-q4" | "standard-turboquant-q4" | "tq-q4" => Ok(CodecMode::TurboQuantQ4),
        "phase1-q4" | "qatq-phase1" | "qatq-q4" | "quaternion-q4" => Ok(CodecMode::Phase1Q4),
        "qatq-exact" => Ok(CodecMode::QatqExact),
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
        CodecMode::TurboQuantQ4 => encode_turboquant_q4_unchecked(values, Phase1Config::default()),
        CodecMode::Phase1Q4 => encode_phase1_q4_unchecked(values, Phase1Config::default()),
        CodecMode::QatqExact => encode_qatq_exact_unchecked(values, Phase1Config::default()),
    }
}

pub fn decode(payload: &[u8]) -> Result<Vec<f32>, QatqError> {
    if payload.len() >= CONTAINER_MAGIC.len() && &payload[0..4] == CONTAINER_MAGIC {
        return decode_qatq_exact_container(payload);
    }
    let header = Header::parse(payload)?;
    match header.mode {
        CodecMode::LossyI4 => decode_lossy_i4(payload),
        CodecMode::LosslessF32 => decode_lossless_f32(payload),
        CodecMode::TurboQuantQ4 => decode_turboquant_q4(payload),
        CodecMode::Phase1Q4 => decode_phase1_q4(payload),
        CodecMode::QatqExact => decode_qatq_exact(payload),
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
pub enum QatqExactStrategy {
    PredictorXor,
    RawBits,
    ByteRle,
    BytePlaneRle,
    DeltaXorBytePlaneRle,
    BytePlaneBlocks,
    BytePlanePackedRle,
    BytePlaneZstd,
    QuaternionChainZstd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionStorage {
    QatqExact,
    RawF32LePassThrough,
}

impl ProductionStorage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::QatqExact => "qatq-exact",
            Self::RawF32LePassThrough => "raw-f32le-pass-through",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionChunkMetadata {
    pub storage: ProductionStorage,
    pub raw_f32le_len: usize,
    pub strategy: Option<QatqExactStrategy>,
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
        self.metadata.storage == ProductionStorage::QatqExact
    }

    pub fn should_pass_through(&self) -> bool {
        self.metadata.storage == ProductionStorage::RawF32LePassThrough
    }

    pub fn stored_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QatqExactEncodeDecision {
    Compressed {
        payload: Vec<u8>,
        strategy: QatqExactStrategy,
        raw_f32le_len: usize,
    },
    PassThroughRaw {
        bytes: Vec<u8>,
    },
}

impl QatqExactEncodeDecision {
    pub fn should_compress(&self) -> bool {
        matches!(self, Self::Compressed { .. })
    }

    pub fn should_pass_through(&self) -> bool {
        matches!(self, Self::PassThroughRaw { .. })
    }

    pub fn strategy(&self) -> Option<QatqExactStrategy> {
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

impl QatqExactStrategy {
    fn from_id(id: u8) -> Result<Self, QatqError> {
        match id {
            QATQ_EXACT_STRATEGY_PREDICTOR_XOR => Ok(Self::PredictorXor),
            QATQ_EXACT_STRATEGY_RAW_BITS => Ok(Self::RawBits),
            QATQ_EXACT_STRATEGY_BYTE_RLE => Ok(Self::ByteRle),
            QATQ_EXACT_STRATEGY_BYTE_PLANE_RLE => Ok(Self::BytePlaneRle),
            QATQ_EXACT_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE => Ok(Self::DeltaXorBytePlaneRle),
            QATQ_EXACT_STRATEGY_BYTE_PLANE_BLOCKS => Ok(Self::BytePlaneBlocks),
            QATQ_EXACT_STRATEGY_BYTE_PLANE_PACKED_RLE => Ok(Self::BytePlanePackedRle),
            QATQ_EXACT_STRATEGY_BYTE_PLANE_ZSTD => Ok(Self::BytePlaneZstd),
            QATQ_EXACT_STRATEGY_QUATERNION_CHAIN_ZSTD => Ok(Self::QuaternionChainZstd),
            _ => Err(QatqError::InvalidQatqExactBody),
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
            Self::BytePlanePackedRle => "byte-plane-packed-rle",
            Self::BytePlaneZstd => "byte-plane-zstd",
            Self::QuaternionChainZstd => "quaternion-chain-zstd",
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

pub fn encode_turboquant_q4(values: &[f32]) -> Vec<u8> {
    encode_turboquant_q4_with_config(values, Phase1Config::default())
}

pub fn encode_turboquant_q4_with_config(values: &[f32], config: Phase1Config) -> Vec<u8> {
    try_encode_turboquant_q4_with_config(values, config)
        .expect("value count exceeds single-payload bound; use chunked APIs")
}

pub fn try_encode_turboquant_q4_with_config(
    values: &[f32],
    config: Phase1Config,
) -> Result<Vec<u8>, QatqError> {
    validate_single_payload_value_count(values.len())?;
    Ok(encode_turboquant_q4_unchecked(values, config))
}

fn encode_turboquant_q4_unchecked(values: &[f32], config: Phase1Config) -> Vec<u8> {
    let parts = build_turboquant_parts(values, config);
    let checksum = checksum_f32_bits(values);
    let quantized_len = parts.coord_count.div_ceil(2);
    let qjl_len = parts.qjl_projection_count.div_ceil(8);
    let mut out =
        Vec::with_capacity(HEADER_LEN + TURBOQUANT_METADATA_LEN + quantized_len + qjl_len);
    write_header(
        &mut out,
        CodecMode::TurboQuantQ4,
        values.len(),
        parts.scale,
        checksum,
    );
    write_turboquant_metadata_and_payload(&mut out, &parts);
    out
}

pub fn decode_turboquant_q4(payload: &[u8]) -> Result<Vec<f32>, QatqError> {
    let parsed = parse_turboquant_payload(payload)?;
    let mut values = reconstruct_turboquant_values(&parsed, true);
    values.truncate(parsed.header.value_count);
    Ok(values)
}

pub fn estimate_turboquant_q4_inner_product(
    payload: &[u8],
    query: &[f32],
) -> Result<f32, QatqError> {
    let parsed = parse_turboquant_payload(payload)?;
    if query.len() != parsed.header.value_count {
        return Err(QatqError::LengthMismatch {
            expected: parsed.header.value_count,
            actual: query.len(),
        });
    }
    let mse_values = reconstruct_turboquant_values(&parsed, false);
    let mse_dot = dot_product(query, &mse_values[..query.len()]);
    if parsed.residual_norm == 0.0 || parsed.qjl_projection_count == 0 {
        return Ok(mse_dot);
    }

    let mut padded_query = finite_predictor_values(query);
    padded_query.resize(parsed.coord_count, 0.0);
    let mut correction_dot = 0.0_f32;
    let projected_query = qjl_project_values(&padded_query, parsed.seed);
    for (projected, sign) in projected_query
        .iter()
        .zip(parsed.qjl_signs.iter())
        .take(parsed.qjl_projection_count)
    {
        correction_dot += projected * qjl_sign_value(*sign);
    }
    Ok(mse_dot
        + (SQRT_PI_OVER_TWO / parsed.qjl_projection_count as f32)
            * parsed.residual_norm
            * correction_dot)
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

pub fn encode_qatq_exact(values: &[f32]) -> Vec<u8> {
    encode_qatq_exact_with_config(values, Phase1Config::default())
}

pub fn encode_qatq_exact_with_config(values: &[f32], config: Phase1Config) -> Vec<u8> {
    try_encode_qatq_exact_with_config(values, config)
        .expect("value count exceeds single-payload bound; use chunked APIs")
}

pub fn try_encode_qatq_exact_with_config(
    values: &[f32],
    config: Phase1Config,
) -> Result<Vec<u8>, QatqError> {
    validate_single_payload_value_count(values.len())?;
    Ok(encode_qatq_exact_unchecked(values, config))
}

pub fn encode_qatq_exact_tensor_le(bytes_le: &[u8], dtype: TensorDType) -> Vec<u8> {
    try_encode_qatq_exact_tensor_le(bytes_le, dtype)
        .expect("typed tensor exceeds single-payload bound; use chunked APIs")
}

pub fn try_encode_qatq_exact_tensor_le(
    bytes_le: &[u8],
    dtype: TensorDType,
) -> Result<Vec<u8>, QatqError> {
    let element_count = validate_typed_tensor_len(bytes_le, dtype)?;
    if dtype == TensorDType::F32 {
        let values = decode_f32le_bytes(bytes_le)?;
        return try_encode_qatq_exact_with_config(&values, Phase1Config::default());
    }
    Ok(encode_qatq_exact_tensor_bytes_unchecked(
        bytes_le,
        dtype,
        element_count,
    ))
}

pub fn decode_qatq_exact_tensor_le(payload: &[u8]) -> Result<DecodedTensor, QatqError> {
    let header = Header::parse_for_mode(payload, CodecMode::QatqExact)?;
    let body = &payload[HEADER_LEN..];
    let (strategy, dtype) = parse_qatq_exact_prefix(body)?;
    if dtype == TensorDType::F32 {
        return Ok(DecodedTensor {
            dtype,
            bytes_le: encode_f32_bits_le(&decode_qatq_exact(payload)?),
        });
    }

    let expected_len = checked_tensor_byte_len(header.value_count, dtype)?;
    let body = &body[QATQ_EXACT_PREFIX_LEN..];
    let canonical = match strategy {
        QatqExactStrategy::RawBits => decode_qatq_exact_tensor_raw_bits(body, expected_len)?,
        QatqExactStrategy::ByteRle => decode_byte_runs_to_bytes(body, expected_len)?,
        QatqExactStrategy::BytePlaneRle => {
            decode_byte_plane_runs_to_bytes_width(body, expected_len, header.value_count, dtype)?
        }
        QatqExactStrategy::BytePlanePackedRle => decode_byte_plane_packed_runs_to_bytes_width(
            body,
            expected_len,
            header.value_count,
            dtype,
        )?,
        QatqExactStrategy::BytePlaneZstd => {
            let plane_bytes = zstd::bulk::decompress(body, expected_len)
                .map_err(|_| QatqError::InvalidResidualStream)?;
            byte_plane_bytes_to_bytes(&plane_bytes, expected_len, header.value_count, dtype)?
        }
        QatqExactStrategy::PredictorXor
        | QatqExactStrategy::DeltaXorBytePlaneRle
        | QatqExactStrategy::BytePlaneBlocks
        | QatqExactStrategy::QuaternionChainZstd => return Err(QatqError::InvalidQatqExactBody),
    };
    let bytes_le = decanonicalize_le_elements(&canonical, dtype);
    let actual = checksum_bytes(&bytes_le);
    if actual != header.checksum {
        return Err(QatqError::ChecksumMismatch {
            expected: header.checksum,
            actual,
        });
    }
    Ok(DecodedTensor { dtype, bytes_le })
}

pub fn encode_qatq_exact_decision(values: &[f32]) -> QatqExactEncodeDecision {
    encode_qatq_exact_decision_with_config(values, Phase1Config::default())
}

pub fn encode_qatq_exact_decision_with_config(
    values: &[f32],
    config: Phase1Config,
) -> QatqExactEncodeDecision {
    try_encode_qatq_exact_decision_with_config(values, config)
        .expect("value count exceeds single-payload bound; use chunked APIs")
}

pub fn try_encode_qatq_exact_decision_with_config(
    values: &[f32],
    config: Phase1Config,
) -> Result<QatqExactEncodeDecision, QatqError> {
    validate_single_payload_value_count(values.len())?;
    let raw_f32le_len = checked_value_byte_len(values.len())?;
    let payload = encode_qatq_exact_unchecked(values, config);
    let strategy = qatq_exact_strategy(&payload)?;
    if strategy == QatqExactStrategy::RawBits {
        Ok(QatqExactEncodeDecision::PassThroughRaw {
            bytes: encode_f32_bits_le(values),
        })
    } else {
        Ok(QatqExactEncodeDecision::Compressed {
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
    let decision = try_encode_qatq_exact_decision_with_config(values, config)?;
    Ok(production_result_from_decision(decision))
}

pub fn production_result_from_decision(
    decision: QatqExactEncodeDecision,
) -> ProductionEncodeResult {
    match decision {
        QatqExactEncodeDecision::Compressed {
            payload,
            strategy,
            raw_f32le_len,
        } => ProductionEncodeResult {
            metadata: ProductionChunkMetadata {
                storage: ProductionStorage::QatqExact,
                raw_f32le_len,
                strategy: Some(strategy),
            },
            bytes: payload,
        },
        QatqExactEncodeDecision::PassThroughRaw { bytes } => ProductionEncodeResult {
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
        ProductionStorage::QatqExact => {
            let restored = decode_qatq_exact(bytes)?;
            let expected_len = metadata.raw_f32le_len;
            let actual_len = checked_value_byte_len(restored.len())?;
            if actual_len != expected_len {
                return Err(QatqError::LengthMismatch {
                    expected: expected_len,
                    actual: actual_len,
                });
            }
            if let Some(expected_strategy) = metadata.strategy {
                let actual_strategy = qatq_exact_strategy(bytes)?;
                if actual_strategy != expected_strategy {
                    return Err(QatqError::InvalidQatqExactBody);
                }
            }
            Ok(restored)
        }
        ProductionStorage::RawF32LePassThrough => decode_raw_f32le_pass_through(metadata, bytes),
    }
}

fn encode_qatq_exact_unchecked(values: &[f32], config: Phase1Config) -> Vec<u8> {
    encode_qatq_exact_fast(values, config)
}

fn encode_qatq_exact_tensor_bytes_unchecked(
    bytes_le: &[u8],
    dtype: TensorDType,
    element_count: usize,
) -> Vec<u8> {
    let canonical = canonicalize_le_elements(bytes_le, dtype);
    let raw_body_len = QATQ_EXACT_PREFIX_LEN + canonical.len();
    let byte_rle = encode_byte_runs_bounded(&canonical, canonical.len());
    let byte_rle_body_len = candidate_body_len(byte_rle.as_ref());
    let byte_plane =
        encode_byte_plane_runs_bounded_width(&canonical, dtype.element_width(), canonical.len());
    let byte_plane_body_len = candidate_body_len(byte_plane.as_ref());
    let byte_plane_packed = encode_byte_plane_packed_runs_bounded_width(
        &canonical,
        dtype.element_width(),
        canonical.len(),
    );
    let byte_plane_packed_body_len = candidate_body_len(byte_plane_packed.as_ref());
    let byte_plane_zstd =
        encode_byte_plane_zstd_bounded_width(&canonical, dtype.element_width(), canonical.len());
    let byte_plane_zstd_body_len = candidate_body_len(byte_plane_zstd.as_ref());
    let mut strategy = QATQ_EXACT_STRATEGY_RAW_BITS;
    let mut best_body_len = raw_body_len;
    for (candidate_strategy, candidate_len) in [
        (QATQ_EXACT_STRATEGY_BYTE_RLE, byte_rle_body_len),
        (QATQ_EXACT_STRATEGY_BYTE_PLANE_RLE, byte_plane_body_len),
        (
            QATQ_EXACT_STRATEGY_BYTE_PLANE_PACKED_RLE,
            byte_plane_packed_body_len,
        ),
        (
            QATQ_EXACT_STRATEGY_BYTE_PLANE_ZSTD,
            byte_plane_zstd_body_len,
        ),
    ] {
        if candidate_len < best_body_len {
            strategy = candidate_strategy;
            best_body_len = candidate_len;
        }
    }

    let mut out = Vec::with_capacity(HEADER_LEN + best_body_len);
    write_header(
        &mut out,
        CodecMode::QatqExact,
        element_count,
        1.0,
        checksum_bytes(bytes_le),
    );
    write_qatq_exact_typed_prefix(&mut out, strategy, dtype);
    match strategy {
        QATQ_EXACT_STRATEGY_RAW_BITS => out.extend_from_slice(&canonical),
        QATQ_EXACT_STRATEGY_BYTE_RLE => {
            out.extend_from_slice(byte_rle.as_ref().expect("selected byte-RLE candidate"))
        }
        QATQ_EXACT_STRATEGY_BYTE_PLANE_RLE => {
            out.extend_from_slice(byte_plane.as_ref().expect("selected byte-plane candidate"))
        }
        QATQ_EXACT_STRATEGY_BYTE_PLANE_PACKED_RLE => out.extend_from_slice(
            byte_plane_packed
                .as_ref()
                .expect("selected packed byte-plane candidate"),
        ),
        QATQ_EXACT_STRATEGY_BYTE_PLANE_ZSTD => out.extend_from_slice(
            byte_plane_zstd
                .as_ref()
                .expect("selected zstd byte-plane candidate"),
        ),
        _ => unreachable!("typed exact strategy set"),
    }
    out
}

pub fn encode_qatq_exact_exhaustive(values: &[f32]) -> Vec<u8> {
    encode_qatq_exact_exhaustive_with_config(values, Phase1Config::default())
}

pub fn encode_qatq_exact_exhaustive_with_config(values: &[f32], config: Phase1Config) -> Vec<u8> {
    try_encode_qatq_exact_exhaustive_with_config(values, config)
        .expect("value count exceeds single-payload bound; use chunked APIs")
}

pub fn try_encode_qatq_exact_exhaustive_with_config(
    values: &[f32],
    config: Phase1Config,
) -> Result<Vec<u8>, QatqError> {
    validate_single_payload_value_count(values.len())?;
    Ok(encode_qatq_exact_exhaustive_unchecked(values, config))
}

fn encode_qatq_exact_exhaustive_unchecked(values: &[f32], config: Phase1Config) -> Vec<u8> {
    let raw_bits = encode_f32_bits_be(values);
    let raw_body_len = QATQ_EXACT_PREFIX_LEN + raw_bits.len();
    let byte_plane_blocks = encode_byte_plane_blocks_bounded(&raw_bits, raw_bits.len());
    let byte_plane_blocks_body_len = candidate_body_len(byte_plane_blocks.as_ref());
    let byte_rle = encode_byte_runs_bounded(&raw_bits, raw_bits.len());
    let byte_rle_body_len = candidate_body_len(byte_rle.as_ref());
    let byte_plane = encode_byte_plane_runs_bounded(&raw_bits, raw_bits.len());
    let byte_plane_body_len = candidate_body_len(byte_plane.as_ref());
    let byte_plane_packed = encode_byte_plane_packed_runs_bounded(&raw_bits, raw_bits.len());
    let byte_plane_packed_body_len = candidate_body_len(byte_plane_packed.as_ref());
    let byte_plane_zstd = encode_byte_plane_zstd_bounded(&raw_bits, raw_bits.len());
    let byte_plane_zstd_body_len = candidate_body_len(byte_plane_zstd.as_ref());
    let quaternion_chain_zstd = encode_quaternion_chain_zstd_bounded(values, raw_bits.len());
    let quaternion_chain_zstd_body_len = candidate_body_len(quaternion_chain_zstd.as_ref());
    let delta_xor_byte_plane = encode_delta_xor_byte_plane_runs_bounded(values, raw_bits.len());
    let delta_xor_byte_plane_body_len = candidate_body_len(delta_xor_byte_plane.as_ref());
    let checksum = checksum_f32_bits(values);
    let parts = build_phase1_parts(values, config);
    let predicted = reconstruct_phase1_values(values.len(), &parts);
    let residuals = encode_xor_residuals(values, &predicted);
    let quantized_len = parts.coord_count.div_ceil(2);
    let residual_sign_len = parts.coord_count.div_ceil(8);
    let predictor_body_len = QATQ_EXACT_PREFIX_LEN
        + QATQ_EXACT_PREDICTOR_METADATA_LEN
        + quantized_len
        + residual_sign_len
        + residuals.len();
    let mut strategy = QATQ_EXACT_STRATEGY_RAW_BITS;
    let mut best_body_len = raw_body_len;
    for (candidate_strategy, candidate_len) in [
        (QATQ_EXACT_STRATEGY_BYTE_RLE, byte_rle_body_len),
        (QATQ_EXACT_STRATEGY_BYTE_PLANE_RLE, byte_plane_body_len),
        (
            QATQ_EXACT_STRATEGY_BYTE_PLANE_PACKED_RLE,
            byte_plane_packed_body_len,
        ),
        (
            QATQ_EXACT_STRATEGY_BYTE_PLANE_ZSTD,
            byte_plane_zstd_body_len,
        ),
        (
            QATQ_EXACT_STRATEGY_QUATERNION_CHAIN_ZSTD,
            quaternion_chain_zstd_body_len,
        ),
        (
            QATQ_EXACT_STRATEGY_BYTE_PLANE_BLOCKS,
            byte_plane_blocks_body_len,
        ),
        (
            QATQ_EXACT_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE,
            delta_xor_byte_plane_body_len,
        ),
        (QATQ_EXACT_STRATEGY_PREDICTOR_XOR, predictor_body_len),
    ] {
        if candidate_len < best_body_len {
            strategy = candidate_strategy;
            best_body_len = candidate_len;
        }
    }
    let body_len = match strategy {
        QATQ_EXACT_STRATEGY_RAW_BITS => raw_body_len,
        QATQ_EXACT_STRATEGY_BYTE_RLE => byte_rle_body_len,
        QATQ_EXACT_STRATEGY_BYTE_PLANE_RLE => byte_plane_body_len,
        QATQ_EXACT_STRATEGY_BYTE_PLANE_PACKED_RLE => byte_plane_packed_body_len,
        QATQ_EXACT_STRATEGY_BYTE_PLANE_ZSTD => byte_plane_zstd_body_len,
        QATQ_EXACT_STRATEGY_QUATERNION_CHAIN_ZSTD => quaternion_chain_zstd_body_len,
        QATQ_EXACT_STRATEGY_BYTE_PLANE_BLOCKS => byte_plane_blocks_body_len,
        QATQ_EXACT_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE => delta_xor_byte_plane_body_len,
        QATQ_EXACT_STRATEGY_PREDICTOR_XOR => predictor_body_len,
        _ => unreachable!("known strategy"),
    };
    debug_assert_eq!(body_len, best_body_len);
    let mut out = Vec::with_capacity(HEADER_LEN + body_len);
    write_header(
        &mut out,
        CodecMode::QatqExact,
        values.len(),
        if strategy == QATQ_EXACT_STRATEGY_PREDICTOR_XOR {
            parts.scale
        } else {
            1.0
        },
        checksum,
    );
    write_qatq_exact_prefix(&mut out, strategy);
    match strategy {
        QATQ_EXACT_STRATEGY_RAW_BITS => out.extend_from_slice(&raw_bits),
        QATQ_EXACT_STRATEGY_BYTE_RLE => {
            out.extend_from_slice(byte_rle.as_ref().expect("selected byte-RLE candidate"))
        }
        QATQ_EXACT_STRATEGY_BYTE_PLANE_RLE => {
            out.extend_from_slice(byte_plane.as_ref().expect("selected byte-plane candidate"))
        }
        QATQ_EXACT_STRATEGY_BYTE_PLANE_PACKED_RLE => out.extend_from_slice(
            byte_plane_packed
                .as_ref()
                .expect("selected packed byte-plane candidate"),
        ),
        QATQ_EXACT_STRATEGY_BYTE_PLANE_ZSTD => out.extend_from_slice(
            byte_plane_zstd
                .as_ref()
                .expect("selected zstd byte-plane candidate"),
        ),
        QATQ_EXACT_STRATEGY_QUATERNION_CHAIN_ZSTD => out.extend_from_slice(
            quaternion_chain_zstd
                .as_ref()
                .expect("selected quaternion-chain candidate"),
        ),
        QATQ_EXACT_STRATEGY_BYTE_PLANE_BLOCKS => out.extend_from_slice(
            byte_plane_blocks
                .as_ref()
                .expect("selected byte-plane block candidate"),
        ),
        QATQ_EXACT_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE => out.extend_from_slice(
            delta_xor_byte_plane
                .as_ref()
                .expect("selected delta-XOR byte-plane candidate"),
        ),
        QATQ_EXACT_STRATEGY_PREDICTOR_XOR => {
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

fn encode_qatq_exact_fast(values: &[f32], config: Phase1Config) -> Vec<u8> {
    let raw_bits = encode_f32_bits_be(values);
    let raw_body_len = QATQ_EXACT_PREFIX_LEN + raw_bits.len();
    let (byte_plane_blocks, checksum) =
        encode_two_high_raw_two_low_zero_blocks_bounded(values, raw_bits.len())
            .map(|(encoded, checksum)| (Some(encoded), checksum))
            .unwrap_or_else(|| encode_byte_plane_blocks_from_f32_bounded(values, raw_bits.len()));
    let byte_plane_blocks_body_len = candidate_body_len(byte_plane_blocks.as_ref());
    let byte_rle = encode_byte_runs_bounded(&raw_bits, raw_bits.len());
    let byte_rle_body_len = candidate_body_len(byte_rle.as_ref());
    let byte_plane = encode_byte_plane_runs_bounded(&raw_bits, raw_bits.len());
    let byte_plane_body_len = candidate_body_len(byte_plane.as_ref());
    let byte_plane_packed = encode_byte_plane_packed_runs_bounded(&raw_bits, raw_bits.len());
    let byte_plane_packed_body_len = candidate_body_len(byte_plane_packed.as_ref());
    let byte_plane_zstd = encode_byte_plane_zstd_bounded(&raw_bits, raw_bits.len());
    let byte_plane_zstd_body_len = candidate_body_len(byte_plane_zstd.as_ref());
    let quaternion_chain_zstd = encode_quaternion_chain_zstd_bounded(values, raw_bits.len());
    let quaternion_chain_zstd_body_len = candidate_body_len(quaternion_chain_zstd.as_ref());
    let delta_xor_byte_plane = encode_delta_xor_byte_plane_runs_bounded(values, raw_bits.len());
    let delta_xor_byte_plane_body_len = candidate_body_len(delta_xor_byte_plane.as_ref());

    let parts = build_phase1_parts(values, config);
    let predicted = reconstruct_phase1_values(values.len(), &parts);
    let residuals = encode_xor_residuals(values, &predicted);
    let quantized_len = parts.coord_count.div_ceil(2);
    let residual_sign_len = parts.coord_count.div_ceil(8);
    let predictor_body_len = QATQ_EXACT_PREFIX_LEN
        + QATQ_EXACT_PREDICTOR_METADATA_LEN
        + quantized_len
        + residual_sign_len
        + residuals.len();
    let mut strategy = QATQ_EXACT_STRATEGY_RAW_BITS;
    let mut best_body_len = raw_body_len;
    for (candidate_strategy, candidate_len) in [
        (QATQ_EXACT_STRATEGY_BYTE_RLE, byte_rle_body_len),
        (QATQ_EXACT_STRATEGY_BYTE_PLANE_RLE, byte_plane_body_len),
        (
            QATQ_EXACT_STRATEGY_BYTE_PLANE_PACKED_RLE,
            byte_plane_packed_body_len,
        ),
        (
            QATQ_EXACT_STRATEGY_BYTE_PLANE_ZSTD,
            byte_plane_zstd_body_len,
        ),
        (
            QATQ_EXACT_STRATEGY_QUATERNION_CHAIN_ZSTD,
            quaternion_chain_zstd_body_len,
        ),
        (
            QATQ_EXACT_STRATEGY_BYTE_PLANE_BLOCKS,
            byte_plane_blocks_body_len,
        ),
        (
            QATQ_EXACT_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE,
            delta_xor_byte_plane_body_len,
        ),
        (QATQ_EXACT_STRATEGY_PREDICTOR_XOR, predictor_body_len),
    ] {
        if candidate_len < best_body_len {
            strategy = candidate_strategy;
            best_body_len = candidate_len;
        }
    }
    let _ = best_body_len;
    write_qatq_exact_selected(
        values.len(),
        checksum,
        strategy,
        &raw_bits,
        byte_rle.as_ref(),
        byte_plane.as_ref(),
        byte_plane_packed.as_ref(),
        byte_plane_zstd.as_ref(),
        quaternion_chain_zstd.as_ref(),
        byte_plane_blocks.as_ref(),
        delta_xor_byte_plane.as_ref(),
        &parts,
        &residuals,
    )
}

fn write_qatq_exact_selected(
    value_count: usize,
    checksum: u64,
    strategy: u8,
    raw_bits: &[u8],
    byte_rle: Option<&Vec<u8>>,
    byte_plane: Option<&Vec<u8>>,
    byte_plane_packed: Option<&Vec<u8>>,
    byte_plane_zstd: Option<&Vec<u8>>,
    quaternion_chain_zstd: Option<&Vec<u8>>,
    byte_plane_blocks: Option<&Vec<u8>>,
    delta_xor_byte_plane: Option<&Vec<u8>>,
    parts: &PhaseParts,
    residuals: &[u8],
) -> Vec<u8> {
    let body_len = match strategy {
        QATQ_EXACT_STRATEGY_RAW_BITS => QATQ_EXACT_PREFIX_LEN + raw_bits.len(),
        QATQ_EXACT_STRATEGY_BYTE_RLE => {
            QATQ_EXACT_PREFIX_LEN + byte_rle.expect("selected byte-RLE candidate").len()
        }
        QATQ_EXACT_STRATEGY_BYTE_PLANE_RLE => {
            QATQ_EXACT_PREFIX_LEN + byte_plane.expect("selected byte-plane candidate").len()
        }
        QATQ_EXACT_STRATEGY_BYTE_PLANE_PACKED_RLE => {
            QATQ_EXACT_PREFIX_LEN
                + byte_plane_packed
                    .expect("selected packed byte-plane candidate")
                    .len()
        }
        QATQ_EXACT_STRATEGY_BYTE_PLANE_ZSTD => {
            QATQ_EXACT_PREFIX_LEN
                + byte_plane_zstd
                    .expect("selected zstd byte-plane candidate")
                    .len()
        }
        QATQ_EXACT_STRATEGY_QUATERNION_CHAIN_ZSTD => {
            QATQ_EXACT_PREFIX_LEN
                + quaternion_chain_zstd
                    .expect("selected quaternion-chain candidate")
                    .len()
        }
        QATQ_EXACT_STRATEGY_BYTE_PLANE_BLOCKS => {
            QATQ_EXACT_PREFIX_LEN
                + byte_plane_blocks
                    .expect("selected byte-plane block candidate")
                    .len()
        }
        QATQ_EXACT_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE => {
            QATQ_EXACT_PREFIX_LEN
                + delta_xor_byte_plane
                    .expect("selected delta-XOR byte-plane candidate")
                    .len()
        }
        QATQ_EXACT_STRATEGY_PREDICTOR_XOR => {
            QATQ_EXACT_PREFIX_LEN
                + QATQ_EXACT_PREDICTOR_METADATA_LEN
                + parts.coord_count.div_ceil(2)
                + parts.coord_count.div_ceil(8)
                + residuals.len()
        }
        _ => unreachable!("known strategy"),
    };
    let mut out = Vec::with_capacity(HEADER_LEN + body_len);
    write_header(
        &mut out,
        CodecMode::QatqExact,
        value_count,
        if strategy == QATQ_EXACT_STRATEGY_PREDICTOR_XOR {
            parts.scale
        } else {
            1.0
        },
        checksum,
    );
    write_qatq_exact_prefix(&mut out, strategy);
    match strategy {
        QATQ_EXACT_STRATEGY_RAW_BITS => out.extend_from_slice(raw_bits),
        QATQ_EXACT_STRATEGY_BYTE_RLE => {
            out.extend_from_slice(byte_rle.expect("selected byte-RLE candidate"))
        }
        QATQ_EXACT_STRATEGY_BYTE_PLANE_RLE => {
            out.extend_from_slice(byte_plane.expect("selected byte-plane candidate"))
        }
        QATQ_EXACT_STRATEGY_BYTE_PLANE_PACKED_RLE => {
            out.extend_from_slice(byte_plane_packed.expect("selected packed byte-plane candidate"))
        }
        QATQ_EXACT_STRATEGY_BYTE_PLANE_ZSTD => {
            out.extend_from_slice(byte_plane_zstd.expect("selected zstd byte-plane candidate"))
        }
        QATQ_EXACT_STRATEGY_QUATERNION_CHAIN_ZSTD => out
            .extend_from_slice(quaternion_chain_zstd.expect("selected quaternion-chain candidate")),
        QATQ_EXACT_STRATEGY_BYTE_PLANE_BLOCKS => {
            out.extend_from_slice(byte_plane_blocks.expect("selected byte-plane block candidate"))
        }
        QATQ_EXACT_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE => out.extend_from_slice(
            delta_xor_byte_plane.expect("selected delta-XOR byte-plane candidate"),
        ),
        QATQ_EXACT_STRATEGY_PREDICTOR_XOR => {
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

pub fn encode_qatq_exact_chunks(
    values: &[f32],
    max_values_per_chunk: usize,
) -> Result<Vec<Vec<u8>>, QatqError> {
    encode_qatq_exact_chunks_with_config(values, max_values_per_chunk, Phase1Config::default())
}

pub fn encode_qatq_exact_chunks_with_config(
    values: &[f32],
    max_values_per_chunk: usize,
    config: Phase1Config,
) -> Result<Vec<Vec<u8>>, QatqError> {
    if max_values_per_chunk == 0 || max_values_per_chunk > MAX_VALUES_PER_PAYLOAD {
        return Err(QatqError::InvalidChunkSize(max_values_per_chunk));
    }
    if values.is_empty() {
        return Ok(vec![encode_qatq_exact_with_config(values, config)]);
    }
    Ok(values
        .chunks(max_values_per_chunk)
        .map(|chunk| encode_qatq_exact_with_config(chunk, config))
        .collect())
}

pub fn decode_qatq_exact_chunks<I, B>(chunks: I) -> Result<Vec<f32>, QatqError>
where
    I: IntoIterator<Item = B>,
    B: AsRef<[u8]>,
{
    let mut values = Vec::new();
    for chunk in chunks {
        values.extend(decode_qatq_exact(chunk.as_ref())?);
    }
    Ok(values)
}

pub fn encode_qatq_exact_container(
    values: &[f32],
    max_values_per_chunk: usize,
) -> Result<Vec<u8>, QatqError> {
    encode_qatq_exact_container_with_config(values, max_values_per_chunk, Phase1Config::default())
}

/// Encodes opaque 32-bit words through the exact QATC format.
///
/// Every bit pattern is preserved, including patterns that would represent
/// NaNs if interpreted as `f32`. The resulting bytes are identical to calling
/// [`encode_qatq_exact_container`] after mapping each word with
/// [`f32::from_bits`].
pub fn encode_qatq_exact_u32_container(
    words: &[u32],
    max_words_per_chunk: usize,
) -> Result<Vec<u8>, QatqError> {
    let values: Vec<f32> = words.iter().copied().map(f32::from_bits).collect();
    encode_qatq_exact_container(&values, max_words_per_chunk)
}

pub fn encode_qatq_exact_container_with_config(
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
    write_container_header(&mut out, values.len(), chunk_count, 0);
    let mut container_checksum = FNV_OFFSET;
    if values.is_empty() {
        let payload = encode_qatq_exact_with_config(values, config);
        container_checksum = container_checksum_chunk(container_checksum, &payload);
        append_container_chunk(&mut out, &payload)?;
        patch_container_checksum(&mut out, container_checksum)?;
        return Ok(out);
    }

    for chunk_values in values.chunks(max_values_per_chunk) {
        let payload = encode_qatq_exact_with_config(chunk_values, config);
        container_checksum = container_checksum_chunk(container_checksum, &payload);
        append_container_chunk(&mut out, &payload)?;
    }
    patch_container_checksum(&mut out, container_checksum)?;
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

pub fn decode_qatq_exact_container(payload: &[u8]) -> Result<Vec<f32>, QatqError> {
    decode_qatq_exact_container_with_limits(payload, QatcDecodeLimits::default())
}

pub fn decode_qatq_exact_container_with_limits(
    payload: &[u8],
    limits: QatcDecodeLimits,
) -> Result<Vec<f32>, QatqError> {
    let (header, body, chunk_count) = container_body_and_chunk_count(payload, limits)?;
    let chunks = read_container_chunk_index(body, chunk_count, header.total_values, limits)?;
    verify_container_checksum(&header, body, &chunks)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(header.total_values)
        .map_err(|_| QatqError::ContainerLimitExceeded("allocation"))?;
    for (chunk_start, chunk_end) in chunks {
        values.extend(decode_qatq_exact(&body[chunk_start..chunk_end])?);
    }
    if values.len() != header.total_values {
        return Err(QatqError::InvalidContainer);
    }
    Ok(values)
}

/// Validates an exact QATC container and reports its canonical chunk layout
/// without decoding chunk bodies.
///
/// `max_values_per_chunk` is the chunk size declared by the surrounding
/// protocol or selected by the encoder. Requiring it here makes non-final
/// short chunks and oversized chunks reject before exact payload decoding
/// allocates their outputs.
pub fn inspect_qatq_exact_container_with_limits(
    payload: &[u8],
    limits: QatcDecodeLimits,
    max_values_per_chunk: usize,
) -> Result<QatcExactContainerMetadata, QatqError> {
    if max_values_per_chunk == 0 || max_values_per_chunk > MAX_VALUES_PER_PAYLOAD {
        return Err(QatqError::InvalidChunkSize(max_values_per_chunk));
    }
    let (header, body, chunk_count) = container_body_and_chunk_count(payload, limits)?;
    let index = read_container_chunk_index(body, chunk_count, header.total_values, limits)?;
    verify_container_checksum(&header, body, &index)?;

    let mut chunks = Vec::with_capacity(index.len());
    for (position, (chunk_start, chunk_end)) in index.iter().copied().enumerate() {
        let chunk = &body[chunk_start..chunk_end];
        let chunk_header = Header::parse_for_mode(chunk, CodecMode::QatqExact)?;
        let is_final = position + 1 == chunk_count;
        let valid_count = if header.total_values == 0 {
            chunk_count == 1 && chunk_header.value_count == 0
        } else if is_final {
            (1..=max_values_per_chunk).contains(&chunk_header.value_count)
        } else {
            chunk_header.value_count == max_values_per_chunk
        };
        if !valid_count {
            return Err(QatqError::InvalidContainer);
        }
        chunks.push(QatcExactChunkMetadata {
            encoded_bytes: chunk.len(),
            decoded_values: chunk_header.value_count,
        });
    }

    Ok(QatcExactContainerMetadata {
        encoded_bytes: payload.len(),
        total_values: header.total_values,
        chunks,
    })
}

/// Decodes an exact QATC container as opaque 32-bit words.
pub fn decode_qatq_exact_u32_container(
    payload: &[u8],
    max_words_per_chunk: usize,
) -> Result<Vec<u32>, QatqError> {
    decode_qatq_exact_u32_container_with_limits(
        payload,
        QatcDecodeLimits::default(),
        max_words_per_chunk,
    )
}

/// Decodes an exact QATC container as opaque 32-bit words after validating
/// both aggregate resource limits and the declared canonical chunk layout.
pub fn decode_qatq_exact_u32_container_with_limits(
    payload: &[u8],
    limits: QatcDecodeLimits,
    max_words_per_chunk: usize,
) -> Result<Vec<u32>, QatqError> {
    inspect_qatq_exact_container_with_limits(payload, limits, max_words_per_chunk)?;
    decode_qatq_exact_container_with_limits(payload, limits)
        .map(|values| values.into_iter().map(f32::to_bits).collect())
}

pub fn for_each_qatq_exact_container_payload(
    payload: &[u8],
    mut visitor: impl FnMut(&[u8]) -> Result<(), QatqError>,
) -> Result<(), QatqError> {
    for_each_qatq_exact_container_payload_with_limits(
        payload,
        QatcDecodeLimits::default(),
        |chunk| visitor(chunk),
    )
}

pub fn for_each_qatq_exact_container_payload_with_limits(
    payload: &[u8],
    limits: QatcDecodeLimits,
    mut visitor: impl FnMut(&[u8]) -> Result<(), QatqError>,
) -> Result<(), QatqError> {
    let (header, body, chunk_count) = container_body_and_chunk_count(payload, limits)?;
    let chunks = read_container_chunk_index(body, chunk_count, header.total_values, limits)?;
    verify_container_checksum(&header, body, &chunks)?;
    for_each_container_chunk_unchecked(body, chunk_count, |chunk| visitor(chunk))
}

pub fn decode_qatq_exact_container_payloads(payload: &[u8]) -> Result<Vec<&[u8]>, QatqError> {
    decode_qatq_exact_container_payloads_with_limits(payload, QatcDecodeLimits::default())
}

pub fn decode_qatq_exact_container_payloads_with_limits(
    payload: &[u8],
    limits: QatcDecodeLimits,
) -> Result<Vec<&[u8]>, QatqError> {
    let (header, body, chunk_count) = container_body_and_chunk_count(payload, limits)?;

    let chunks = read_container_chunk_index(body, chunk_count, header.total_values, limits)?;
    verify_container_checksum(&header, body, &chunks)?;
    Ok(chunks
        .into_iter()
        .map(|(chunk_start, chunk_end)| &body[chunk_start..chunk_end])
        .collect())
}

fn container_body_and_chunk_count(
    payload: &[u8],
    limits: QatcDecodeLimits,
) -> Result<(ContainerHeader, &[u8], usize), QatqError> {
    let header = ContainerHeader::parse(payload)?;
    if payload.len() > limits.max_encoded_bytes {
        return Err(QatqError::ContainerLimitExceeded("encoded bytes"));
    }
    if header.total_values > limits.max_total_values {
        return Err(QatqError::ContainerLimitExceeded("total values"));
    }
    let body = &payload[header.header_len..];
    let chunk_count = header.chunk_count as usize;
    if chunk_count == 0 {
        return Err(QatqError::InvalidContainer);
    }
    if chunk_count > limits.max_chunks {
        return Err(QatqError::ContainerLimitExceeded("chunks"));
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
    limits: QatcDecodeLimits,
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
        if chunk_len < HEADER_LEN + QATQ_EXACT_PREFIX_LEN {
            return Err(QatqError::InvalidContainer);
        }
        if chunk_len > limits.max_chunk_bytes {
            return Err(QatqError::ContainerLimitExceeded("chunk bytes"));
        }
        let chunk_start = len_end;
        let chunk_end = chunk_start
            .checked_add(chunk_len)
            .ok_or(QatqError::InvalidContainer)?;
        if chunk_end > body.len() {
            return Err(QatqError::InvalidContainer);
        }

        let chunk_header =
            Header::parse_for_mode(&body[chunk_start..chunk_end], CodecMode::QatqExact)?;
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

pub fn decode_qatq_exact(payload: &[u8]) -> Result<Vec<f32>, QatqError> {
    let header = Header::parse_for_mode(payload, CodecMode::QatqExact)?;
    let body = &payload[HEADER_LEN..];
    let (strategy, dtype) = parse_qatq_exact_prefix(body)?;
    if dtype != TensorDType::F32 {
        return Err(QatqError::InvalidQatqExactBody);
    }
    if strategy == QatqExactStrategy::BytePlaneBlocks {
        return decode_qatq_exact_byte_plane_blocks_checked(
            &body[QATQ_EXACT_PREFIX_LEN..],
            &header,
        );
    }
    let values = match strategy {
        QatqExactStrategy::RawBits => {
            decode_qatq_exact_raw_bits(&body[QATQ_EXACT_PREFIX_LEN..], &header)?
        }
        QatqExactStrategy::ByteRle => {
            decode_qatq_exact_byte_rle(&body[QATQ_EXACT_PREFIX_LEN..], &header)?
        }
        QatqExactStrategy::BytePlaneRle => {
            decode_qatq_exact_byte_plane_rle(&body[QATQ_EXACT_PREFIX_LEN..], &header)?
        }
        QatqExactStrategy::BytePlanePackedRle => {
            decode_qatq_exact_byte_plane_packed_rle(&body[QATQ_EXACT_PREFIX_LEN..], &header)?
        }
        QatqExactStrategy::BytePlaneZstd => {
            decode_qatq_exact_byte_plane_zstd(&body[QATQ_EXACT_PREFIX_LEN..], &header)?
        }
        QatqExactStrategy::QuaternionChainZstd => {
            decode_qatq_exact_quaternion_chain_zstd(&body[QATQ_EXACT_PREFIX_LEN..], &header)?
        }
        QatqExactStrategy::DeltaXorBytePlaneRle => {
            decode_qatq_exact_delta_xor_byte_plane_rle(&body[QATQ_EXACT_PREFIX_LEN..], &header)?
        }
        QatqExactStrategy::BytePlaneBlocks => unreachable!("byte-plane blocks returned above"),
        QatqExactStrategy::PredictorXor => {
            decode_qatq_exact_predictor_xor(&body[QATQ_EXACT_PREFIX_LEN..], &header)?
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

pub fn qatq_exact_strategy(payload: &[u8]) -> Result<QatqExactStrategy, QatqError> {
    let _header = Header::parse_for_mode(payload, CodecMode::QatqExact)?;
    parse_qatq_exact_strategy_body(&payload[HEADER_LEN..])
}

fn parse_qatq_exact_strategy_body(body: &[u8]) -> Result<QatqExactStrategy, QatqError> {
    parse_qatq_exact_prefix(body).map(|(strategy, _dtype)| strategy)
}

fn parse_qatq_exact_prefix(body: &[u8]) -> Result<(QatqExactStrategy, TensorDType), QatqError> {
    if body.len() < QATQ_EXACT_PREFIX_LEN {
        return Err(QatqError::PayloadTooShort {
            actual: body.len(),
            minimum: QATQ_EXACT_PREFIX_LEN,
        });
    }
    if &body[0..4] != QATQ_EXACT_BODY_MAGIC {
        return Err(QatqError::InvalidQatqExactBody);
    }
    let dtype = TensorDType::from_prefix(body[5..8].try_into().expect("fixed exact prefix"))?;
    Ok((QatqExactStrategy::from_id(body[4])?, dtype))
}

fn decode_qatq_exact_predictor_xor(body: &[u8], header: &Header) -> Result<Vec<f32>, QatqError> {
    let coord_count = checked_phase1_coordinate_count(header.value_count)?;
    let quantized_len = coord_count.div_ceil(2);
    let residual_sign_len = coord_count.div_ceil(8);
    let minimum_payload_len = QATQ_EXACT_PREDICTOR_METADATA_LEN + quantized_len + residual_sign_len;
    if body.len() < minimum_payload_len {
        return Err(QatqError::PayloadTooShort {
            actual: body.len(),
            minimum: minimum_payload_len,
        });
    }

    let quantized_offset = QATQ_EXACT_PREDICTOR_METADATA_LEN;
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

fn decode_qatq_exact_raw_bits(body: &[u8], header: &Header) -> Result<Vec<f32>, QatqError> {
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

fn decode_qatq_exact_tensor_raw_bits(
    body: &[u8],
    expected_len: usize,
) -> Result<Vec<u8>, QatqError> {
    if body.len() != expected_len {
        return Err(QatqError::LengthMismatch {
            expected: expected_len,
            actual: body.len(),
        });
    }
    Ok(body.to_vec())
}

fn decode_qatq_exact_byte_rle(body: &[u8], header: &Header) -> Result<Vec<f32>, QatqError> {
    let expected_len = checked_value_byte_len(header.value_count)?;
    decode_byte_runs_to_f32(body, expected_len, header.value_count)
}

fn decode_qatq_exact_byte_plane_rle(body: &[u8], header: &Header) -> Result<Vec<f32>, QatqError> {
    let expected_len = checked_value_byte_len(header.value_count)?;
    let words = decode_byte_plane_runs_to_words(body, expected_len, header.value_count)?;
    Ok(words.into_iter().map(f32::from_bits).collect())
}

fn decode_qatq_exact_delta_xor_byte_plane_rle(
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

fn decode_qatq_exact_byte_plane_blocks_checked(
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

fn decode_qatq_exact_byte_plane_packed_rle(
    body: &[u8],
    header: &Header,
) -> Result<Vec<f32>, QatqError> {
    let expected_len = checked_value_byte_len(header.value_count)?;
    let words = decode_byte_plane_packed_runs_to_words(body, expected_len, header.value_count)?;
    Ok(words.into_iter().map(f32::from_bits).collect())
}

fn decode_qatq_exact_byte_plane_zstd(body: &[u8], header: &Header) -> Result<Vec<f32>, QatqError> {
    let expected_len = checked_value_byte_len(header.value_count)?;
    let plane_bytes =
        zstd::bulk::decompress(body, expected_len).map_err(|_| QatqError::InvalidResidualStream)?;
    if plane_bytes.len() != expected_len {
        return Err(QatqError::InvalidResidualStream);
    }
    let words = byte_plane_bytes_to_words(&plane_bytes, expected_len, header.value_count)?;
    Ok(words.into_iter().map(f32::from_bits).collect())
}

fn decode_qatq_exact_quaternion_chain_zstd(
    body: &[u8],
    header: &Header,
) -> Result<Vec<f32>, QatqError> {
    let expected_len = checked_value_byte_len(header.value_count)?;
    let plane_bytes =
        zstd::bulk::decompress(body, expected_len).map_err(|_| QatqError::InvalidResidualStream)?;
    if plane_bytes.len() != expected_len {
        return Err(QatqError::InvalidResidualStream);
    }
    let residuals = byte_plane_bytes_to_words(&plane_bytes, expected_len, header.value_count)?;
    let mut previous = 0_u32;
    let mut values = Vec::with_capacity(header.value_count);
    for residual in residuals {
        let bits = previous.wrapping_add(residual);
        values.push(f32::from_bits(bits));
        previous = bits;
    }
    Ok(values)
}

fn byte_plane_bytes_to_words(
    bytes: &[u8],
    expected_len: usize,
    value_count: usize,
) -> Result<Vec<u32>, QatqError> {
    if bytes.len() != expected_len || expected_len != value_count * 4 {
        return Err(QatqError::InvalidResidualStream);
    }
    let mut words = vec![0_u32; value_count];
    for (plane_index, byte) in bytes.iter().enumerate() {
        write_plane_word_byte(&mut words, plane_index, value_count, *byte);
    }
    Ok(words)
}

fn byte_plane_bytes_to_bytes(
    bytes: &[u8],
    expected_len: usize,
    element_count: usize,
    dtype: TensorDType,
) -> Result<Vec<u8>, QatqError> {
    let width = dtype.element_width();
    if bytes.len() != expected_len || expected_len != element_count * width {
        return Err(QatqError::InvalidResidualStream);
    }
    let mut out = vec![0_u8; expected_len];
    for (plane_index, byte) in bytes.iter().enumerate() {
        write_plane_byte(&mut out, plane_index, element_count, width, *byte);
    }
    Ok(out)
}

fn decode_byte_plane_packed_runs_to_bytes_width(
    bytes: &[u8],
    expected_len: usize,
    element_count: usize,
    dtype: TensorDType,
) -> Result<Vec<u8>, QatqError> {
    let width = dtype.element_width();
    if expected_len != element_count * width {
        return Err(QatqError::InvalidResidualStream);
    }
    let mut out = vec![0_u8; expected_len];
    let mut decoded_len = 0_usize;
    let mut offset = 0_usize;
    while decoded_len < expected_len {
        if offset >= bytes.len() {
            return Err(QatqError::InvalidResidualStream);
        }
        let token = bytes[offset];
        offset += 1;
        let len = (token & PACKED_RUN_LEN_MASK) as usize + 1;
        if decoded_len + len > expected_len {
            return Err(QatqError::InvalidResidualStream);
        }
        match token & PACKED_RUN_TAG_MASK {
            PACKED_ZERO_RUN => {
                decoded_len += len;
            }
            PACKED_RAW_RUN => {
                if offset + len > bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                for byte in &bytes[offset..offset + len] {
                    write_plane_byte(&mut out, decoded_len, element_count, width, *byte);
                    decoded_len += 1;
                }
                offset += len;
            }
            PACKED_REPEAT_RUN => {
                if offset >= bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                let value = bytes[offset];
                offset += 1;
                for _ in 0..len {
                    write_plane_byte(&mut out, decoded_len, element_count, width, value);
                    decoded_len += 1;
                }
            }
            _ => return Err(QatqError::InvalidResidualStream),
        }
    }
    if offset != bytes.len() {
        return Err(QatqError::InvalidResidualStream);
    }
    Ok(out)
}

fn decode_byte_plane_packed_runs_to_words(
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
        if offset >= bytes.len() {
            return Err(QatqError::InvalidResidualStream);
        }
        let token = bytes[offset];
        offset += 1;
        let len = (token & PACKED_RUN_LEN_MASK) as usize + 1;
        if decoded_len + len > expected_len {
            return Err(QatqError::InvalidResidualStream);
        }
        match token & PACKED_RUN_TAG_MASK {
            PACKED_ZERO_RUN => {
                decoded_len += len;
            }
            PACKED_RAW_RUN => {
                if offset + len > bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                for byte in &bytes[offset..offset + len] {
                    write_plane_word_byte(&mut words, decoded_len, value_count, *byte);
                    decoded_len += 1;
                }
                offset += len;
            }
            PACKED_REPEAT_RUN => {
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

fn decode_byte_plane_runs_to_bytes_width(
    bytes: &[u8],
    expected_len: usize,
    element_count: usize,
    dtype: TensorDType,
) -> Result<Vec<u8>, QatqError> {
    let width = dtype.element_width();
    if expected_len != element_count * width {
        return Err(QatqError::InvalidResidualStream);
    }
    let mut out = vec![0_u8; expected_len];
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
                    write_plane_byte(&mut out, decoded_len, element_count, width, *byte);
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
                    write_plane_byte(&mut out, decoded_len, element_count, width, value);
                    decoded_len += 1;
                }
            }
            _ => return Err(QatqError::InvalidResidualStream),
        }
    }
    if offset != bytes.len() {
        return Err(QatqError::InvalidResidualStream);
    }
    Ok(out)
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
        [
            BytePlaneBlock::Raw { offset: first },
            BytePlaneBlock::Raw { offset: second },
            BytePlaneBlock::Zero,
            BytePlaneBlock::Zero,
        ] => {
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
        [
            BytePlaneBlock::Raw { offset: first },
            BytePlaneBlock::Raw { offset: second },
            BytePlaneBlock::Raw { offset: third },
            BytePlaneBlock::Raw { offset: fourth },
        ] => {
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

fn write_plane_byte(
    bytes: &mut [u8],
    plane_index: usize,
    element_count: usize,
    element_width: usize,
    byte: u8,
) {
    if byte == 0 {
        return;
    }
    let plane = plane_index / element_count;
    let element_index = plane_index % element_count;
    bytes[element_index * element_width + plane] = byte;
}

fn candidate_body_len(candidate: Option<&Vec<u8>>) -> usize {
    candidate
        .map(|bytes| QATQ_EXACT_PREFIX_LEN + bytes.len())
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

struct TurboQuantParts {
    seed: u64,
    scale: f32,
    residual_norm: f32,
    coord_count: usize,
    qjl_projection_count: usize,
    quantized: Vec<u8>,
    qjl_signs: Vec<bool>,
}

struct ParsedTurboQuant {
    header: Header,
    seed: u64,
    residual_norm: f32,
    coord_count: usize,
    qjl_projection_count: usize,
    quantized: Vec<u8>,
    qjl_signs: Vec<bool>,
}

fn build_turboquant_parts(values: &[f32], config: Phase1Config) -> TurboQuantParts {
    let coord_count = turboquant_coordinate_count(values.len());
    let mut padded = finite_predictor_values(values);
    padded.resize(coord_count, 0.0);
    let rotated = random_hadamard_rotate(&padded, config.seed, RotationDirection::Forward);
    let scale = compute_i4_scale(&rotated);
    let quantized = rotated
        .iter()
        .map(|value| quantize_i4_nibble(*value, scale))
        .collect::<Vec<_>>();
    let reconstructed_rotated = quantized
        .iter()
        .map(|index| dequantize_i4_nibble(*index, scale))
        .collect::<Vec<_>>();
    let reconstructed = random_hadamard_rotate(
        &reconstructed_rotated,
        config.seed,
        RotationDirection::Inverse,
    );
    let residual = padded
        .iter()
        .zip(reconstructed.iter())
        .map(|(before, after)| before - after)
        .collect::<Vec<_>>();
    let residual_norm = l2_norm(&residual);
    let qjl_projection_count = turboquant_qjl_projection_count(coord_count);
    let qjl_signs = if residual_norm == 0.0 {
        vec![true; qjl_projection_count]
    } else {
        qjl_project_values(&residual, config.seed)
            .into_iter()
            .take(qjl_projection_count)
            .map(|projected| projected >= 0.0)
            .collect()
    };

    TurboQuantParts {
        seed: config.seed,
        scale,
        residual_norm,
        coord_count,
        qjl_projection_count,
        quantized,
        qjl_signs,
    }
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

fn encode_byte_plane_runs_bounded_width(
    bytes: &[u8],
    element_width: usize,
    max_encoded_len: usize,
) -> Option<Vec<u8>> {
    debug_assert_eq!(bytes.len() % element_width, 0);
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = plane_byte_width(bytes, element_width, index);
        if byte == 0 {
            let start = index;
            index += 1;
            while index < bytes.len()
                && plane_byte_width(bytes, element_width, index) == 0
                && index - start < u16::MAX as usize
            {
                index += 1;
            }
            write_xor_run_header(&mut out, XOR_ZERO_RUN, index - start);
            if out.len() > max_encoded_len {
                return None;
            }
        } else if repeated_plane_byte_run_len_width(bytes, element_width, index) >= 4 {
            let value = byte;
            let start = index;
            index += 1;
            while index < bytes.len()
                && plane_byte_width(bytes, element_width, index) == value
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
            while index < bytes.len()
                && plane_byte_width(bytes, element_width, index) != 0
                && repeated_plane_byte_run_len_width(bytes, element_width, index) < 4
                && index - start < u16::MAX as usize
            {
                index += 1;
            }
            write_xor_run_header(&mut out, XOR_RAW_RUN, index - start);
            for plane_index in start..index {
                out.push(plane_byte_width(bytes, element_width, plane_index));
            }
            if out.len() > max_encoded_len {
                return None;
            }
        }
    }
    Some(out)
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

fn encode_byte_plane_packed_runs_bounded_width(
    bytes: &[u8],
    element_width: usize,
    max_encoded_len: usize,
) -> Option<Vec<u8>> {
    debug_assert_eq!(bytes.len() % element_width, 0);
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = plane_byte_width(bytes, element_width, index);
        if byte == 0 {
            let start = index;
            index += 1;
            while index < bytes.len()
                && plane_byte_width(bytes, element_width, index) == 0
                && index - start < PACKED_RUN_MAX_LEN
            {
                index += 1;
            }
            push_packed_run_header(&mut out, PACKED_ZERO_RUN, index - start);
        } else if repeated_plane_byte_run_len_width(bytes, element_width, index)
            .min(PACKED_RUN_MAX_LEN)
            >= 3
        {
            let value = byte;
            let start = index;
            index += 1;
            while index < bytes.len()
                && plane_byte_width(bytes, element_width, index) == value
                && index - start < PACKED_RUN_MAX_LEN
            {
                index += 1;
            }
            push_packed_run_header(&mut out, PACKED_REPEAT_RUN, index - start);
            out.push(value);
        } else {
            let start = index;
            index += 1;
            while index < bytes.len()
                && plane_byte_width(bytes, element_width, index) != 0
                && repeated_plane_byte_run_len_width(bytes, element_width, index)
                    .min(PACKED_RUN_MAX_LEN)
                    < 3
                && index - start < PACKED_RUN_MAX_LEN
            {
                index += 1;
            }
            push_packed_run_header(&mut out, PACKED_RAW_RUN, index - start);
            for plane_index in start..index {
                out.push(plane_byte_width(bytes, element_width, plane_index));
            }
        }
        if out.len() > max_encoded_len {
            return None;
        }
    }
    Some(out)
}

fn encode_byte_plane_packed_runs_bounded(bytes: &[u8], max_encoded_len: usize) -> Option<Vec<u8>> {
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
                && index - start < PACKED_RUN_MAX_LEN
            {
                index += 1;
            }
            push_packed_run_header(&mut out, PACKED_ZERO_RUN, index - start);
        } else if repeated_plane_byte_run_len(bytes, index).min(PACKED_RUN_MAX_LEN) >= 3 {
            let value = byte;
            let start = index;
            index += 1;
            while index < bytes.len()
                && plane_byte(bytes, index) == value
                && index - start < PACKED_RUN_MAX_LEN
            {
                index += 1;
            }
            push_packed_run_header(&mut out, PACKED_REPEAT_RUN, index - start);
            out.push(value);
        } else {
            let start = index;
            index += 1;
            while index < bytes.len()
                && plane_byte(bytes, index) != 0
                && repeated_plane_byte_run_len(bytes, index).min(PACKED_RUN_MAX_LEN) < 3
                && index - start < PACKED_RUN_MAX_LEN
            {
                index += 1;
            }
            push_packed_run_header(&mut out, PACKED_RAW_RUN, index - start);
            for plane_index in start..index {
                out.push(plane_byte(bytes, plane_index));
            }
        }
        if out.len() > max_encoded_len {
            return None;
        }
    }
    Some(out)
}

fn encode_byte_plane_zstd_bounded_width(
    bytes: &[u8],
    element_width: usize,
    max_encoded_len: usize,
) -> Option<Vec<u8>> {
    debug_assert_eq!(bytes.len() % element_width, 0);
    let mut plane_bytes = Vec::with_capacity(bytes.len());
    for plane_index in 0..bytes.len() {
        plane_bytes.push(plane_byte_width(bytes, element_width, plane_index));
    }
    let encoded = zstd::bulk::compress(&plane_bytes, 3).ok()?;
    if encoded.len() > max_encoded_len {
        return None;
    }
    Some(encoded)
}

fn encode_byte_plane_zstd_bounded(bytes: &[u8], max_encoded_len: usize) -> Option<Vec<u8>> {
    debug_assert_eq!(bytes.len() % 4, 0);
    let mut plane_bytes = Vec::with_capacity(bytes.len());
    for plane_index in 0..bytes.len() {
        plane_bytes.push(plane_byte(bytes, plane_index));
    }
    let encoded = zstd::bulk::compress(&plane_bytes, 3).ok()?;
    if encoded.len() > max_encoded_len {
        return None;
    }
    Some(encoded)
}

fn encode_quaternion_chain_zstd_bounded(values: &[f32], max_encoded_len: usize) -> Option<Vec<u8>> {
    let transformed = encode_quaternion_chain_words(values);
    let plane_bytes = words_to_byte_planes_be(&transformed);
    let encoded = zstd::bulk::compress(&plane_bytes, 3).ok()?;
    if encoded.len() > max_encoded_len {
        return None;
    }
    Some(encoded)
}

fn encode_quaternion_chain_words(values: &[f32]) -> Vec<u32> {
    // A reversible quaternion-lane lift: each four-coordinate lane stores the
    // exact wrapping delta from the previous lane component, carrying `d` into
    // the next lane's `a`.
    let mut previous_component = 0_u32;
    let mut out = Vec::with_capacity(values.len());
    for lane in values.chunks(4) {
        for value in lane {
            let bits = value.to_bits();
            out.push(bits.wrapping_sub(previous_component));
            previous_component = bits;
        }
    }
    out
}

fn words_to_byte_planes_be(words: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(words.len() * 4);
    for plane in 0..4 {
        for word in words {
            out.push(word.to_be_bytes()[plane]);
        }
    }
    out
}

fn push_packed_run_header(out: &mut Vec<u8>, tag: u8, len: usize) {
    debug_assert!(len > 0 && len <= PACKED_RUN_MAX_LEN);
    out.push(tag | ((len - 1) as u8));
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

fn plane_byte_width(bytes: &[u8], element_width: usize, plane_index: usize) -> u8 {
    debug_assert!(element_width > 0);
    debug_assert_eq!(bytes.len() % element_width, 0);
    let element_count = bytes.len() / element_width;
    let plane = plane_index / element_count;
    let element_index = plane_index % element_count;
    bytes[element_index * element_width + plane]
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

fn repeated_plane_byte_run_len_width(bytes: &[u8], element_width: usize, index: usize) -> usize {
    let value = plane_byte_width(bytes, element_width, index);
    let mut len = 1;
    while index + len < bytes.len()
        && plane_byte_width(bytes, element_width, index + len) == value
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

fn decode_byte_runs_to_bytes(bytes: &[u8], expected_len: usize) -> Result<Vec<u8>, QatqError> {
    let mut out = Vec::with_capacity(expected_len);
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
            XOR_ZERO_RUN => out.resize(out.len() + len, 0),
            XOR_RAW_RUN => {
                if offset + len > bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                out.extend_from_slice(&bytes[offset..offset + len]);
                offset += len;
            }
            BYTE_REPEAT_RUN => {
                if offset >= bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                let value = bytes[offset];
                offset += 1;
                out.resize(out.len() + len, value);
            }
            _ => return Err(QatqError::InvalidResidualStream),
        }
        decoded_len += len;
    }
    if offset != bytes.len() || out.len() != expected_len {
        return Err(QatqError::InvalidResidualStream);
    }
    Ok(out)
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

fn write_turboquant_metadata_and_payload(out: &mut Vec<u8>, parts: &TurboQuantParts) {
    out.extend_from_slice(TURBOQUANT_BODY_MAGIC);
    out.extend_from_slice(&parts.seed.to_be_bytes());
    out.extend_from_slice(&parts.residual_norm.to_bits().to_be_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    pack_i4_nibbles(&parts.quantized, out);
    pack_residual_signs(&parts.qjl_signs, out);
}

fn write_qatq_exact_prefix(out: &mut Vec<u8>, strategy: u8) {
    write_qatq_exact_typed_prefix(out, strategy, TensorDType::F32);
}

fn write_qatq_exact_typed_prefix(out: &mut Vec<u8>, strategy: u8, dtype: TensorDType) {
    out.extend_from_slice(QATQ_EXACT_BODY_MAGIC);
    out.push(strategy);
    out.extend_from_slice(&dtype.prefix_bytes());
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

fn parse_turboquant_payload(payload: &[u8]) -> Result<ParsedTurboQuant, QatqError> {
    let header = Header::parse_for_mode(payload, CodecMode::TurboQuantQ4)?;
    let coord_count = checked_turboquant_coordinate_count(header.value_count)?;
    let quantized_len = coord_count.div_ceil(2);
    let qjl_projection_count = turboquant_qjl_projection_count(coord_count);
    let qjl_len = qjl_projection_count.div_ceil(8);
    let expected_payload_len = TURBOQUANT_METADATA_LEN + quantized_len + qjl_len;
    let body = &payload[HEADER_LEN..];
    if body.len() != expected_payload_len {
        return Err(QatqError::LengthMismatch {
            expected: expected_payload_len,
            actual: body.len(),
        });
    }
    if &body[0..4] != TURBOQUANT_BODY_MAGIC {
        return Err(QatqError::InvalidTurboQuantBody);
    }
    let seed = u64::from_be_bytes(body[4..12].try_into().expect("fixed turboquant seed"));
    let residual_norm_bits = u32::from_be_bytes(
        body[12..16]
            .try_into()
            .expect("fixed turboquant residual norm"),
    );
    let residual_norm = f32::from_bits(residual_norm_bits);
    if !residual_norm.is_finite() || residual_norm < 0.0 {
        return Err(QatqError::InvalidResidualScale(residual_norm_bits));
    }
    if body[16..20] != [0, 0, 0, 0] {
        return Err(QatqError::InvalidTurboQuantBody);
    }
    let quantized_offset = TURBOQUANT_METADATA_LEN;
    let qjl_offset = quantized_offset + quantized_len;
    Ok(ParsedTurboQuant {
        header,
        seed,
        residual_norm,
        coord_count,
        qjl_projection_count,
        quantized: unpack_i4_nibbles(&body[quantized_offset..qjl_offset], coord_count),
        qjl_signs: unpack_residual_signs(&body[qjl_offset..], qjl_projection_count),
    })
}

fn reconstruct_turboquant_values(parsed: &ParsedTurboQuant, include_qjl: bool) -> Vec<f32> {
    let mut rotated = Vec::with_capacity(parsed.coord_count);
    for index in &parsed.quantized {
        rotated.push(dequantize_i4_nibble(*index, parsed.header.scale));
    }
    let mut values = random_hadamard_rotate(&rotated, parsed.seed, RotationDirection::Inverse);
    if include_qjl && parsed.residual_norm > 0.0 && parsed.coord_count > 0 {
        add_qjl_correction(
            &mut values,
            parsed.seed,
            parsed.residual_norm,
            &parsed.qjl_signs,
        );
    }
    values
}

fn add_qjl_correction(values: &mut [f32], seed: u64, residual_norm: f32, qjl_signs: &[bool]) {
    let dimension = values.len();
    let projection_count = qjl_signs.len();
    if dimension == 0 || projection_count == 0 {
        return;
    }
    let scale = (SQRT_PI_OVER_TWO / projection_count as f32) * residual_norm;
    let mut correction = vec![0.0_f32; dimension];
    for (index, sign) in qjl_signs.iter().enumerate() {
        correction[index] = qjl_sign_value(*sign);
    }
    let correction = qjl_unproject_values(&correction, seed);
    for (value, correction) in values.iter_mut().zip(correction.iter()) {
        *value += scale * correction;
    }
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

fn turboquant_coordinate_count(value_count: usize) -> usize {
    if value_count <= 1 {
        value_count
    } else {
        value_count.next_power_of_two()
    }
}

fn checked_turboquant_coordinate_count(value_count: usize) -> Result<usize, QatqError> {
    if value_count <= 1 {
        return Ok(value_count);
    }
    value_count
        .checked_next_power_of_two()
        .ok_or(QatqError::ValueCountTooLarge(value_count))
}

fn turboquant_qjl_projection_count(coord_count: usize) -> usize {
    coord_count.min(TURBOQUANT_QJL_MAX_PROJECTIONS)
}

fn checked_value_byte_len(value_count: usize) -> Result<usize, QatqError> {
    value_count
        .checked_mul(4)
        .ok_or(QatqError::ValueCountTooLarge(value_count))
}

fn checked_tensor_byte_len(value_count: usize, dtype: TensorDType) -> Result<usize, QatqError> {
    value_count
        .checked_mul(dtype.element_width())
        .ok_or(QatqError::ValueCountTooLarge(value_count))
}

fn validate_typed_tensor_len(bytes_le: &[u8], dtype: TensorDType) -> Result<usize, QatqError> {
    let width = dtype.element_width();
    if bytes_le.len() % width != 0 {
        return Err(QatqError::LengthMismatch {
            expected: bytes_le.len() + (width - bytes_le.len() % width),
            actual: bytes_le.len(),
        });
    }
    let element_count = bytes_le.len() / width;
    validate_single_payload_value_count(element_count)?;
    Ok(element_count)
}

fn canonicalize_le_elements(bytes_le: &[u8], dtype: TensorDType) -> Vec<u8> {
    let width = dtype.element_width();
    let mut out = Vec::with_capacity(bytes_le.len());
    for chunk in bytes_le.chunks_exact(width) {
        match dtype {
            TensorDType::F32 => out.extend_from_slice(&[chunk[3], chunk[2], chunk[1], chunk[0]]),
            TensorDType::F16 | TensorDType::BF16 => out.extend_from_slice(&[chunk[1], chunk[0]]),
        }
    }
    out
}

fn decanonicalize_le_elements(canonical: &[u8], dtype: TensorDType) -> Vec<u8> {
    let width = dtype.element_width();
    let mut out = Vec::with_capacity(canonical.len());
    for chunk in canonical.chunks_exact(width) {
        match dtype {
            TensorDType::F32 => out.extend_from_slice(&[chunk[3], chunk[2], chunk[1], chunk[0]]),
            TensorDType::F16 | TensorDType::BF16 => out.extend_from_slice(&[chunk[1], chunk[0]]),
        }
    }
    out
}

fn decode_f32le_bytes(bytes: &[u8]) -> Result<Vec<f32>, QatqError> {
    if bytes.len() % 4 != 0 {
        return Err(QatqError::LengthMismatch {
            expected: bytes.len() + (4 - bytes.len() % 4),
            actual: bytes.len(),
        });
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_bits(u32::from_le_bytes(chunk.try_into().expect("fixed f32"))))
        .collect())
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

fn random_hadamard_rotate(values: &[f32], seed: u64, direction: RotationDirection) -> Vec<f32> {
    if values.is_empty() {
        return Vec::new();
    }
    debug_assert!(values.len().is_power_of_two());
    let mut out = values.to_vec();
    match direction {
        RotationDirection::Forward => {
            apply_rademacher_signs(&mut out, seed, 0x5451_4657_445f_0001);
            walsh_hadamard_transform(&mut out);
            normalize_hadamard(&mut out);
            apply_rademacher_signs(&mut out, seed, 0x5451_4657_445f_0002);
        }
        RotationDirection::Inverse => {
            apply_rademacher_signs(&mut out, seed, 0x5451_4657_445f_0002);
            walsh_hadamard_transform(&mut out);
            normalize_hadamard(&mut out);
            apply_rademacher_signs(&mut out, seed, 0x5451_4657_445f_0001);
        }
    }
    out
}

fn apply_rademacher_signs(values: &mut [f32], seed: u64, stream: u64) {
    for (index, value) in values.iter_mut().enumerate() {
        let mixed = splitmix64(seed ^ stream ^ (index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        if mixed & 1 == 1 {
            *value = -*value;
        }
    }
}

fn walsh_hadamard_transform(values: &mut [f32]) {
    let mut width = 1;
    while width < values.len() {
        let step = width * 2;
        for start in (0..values.len()).step_by(step) {
            for offset in 0..width {
                let left = values[start + offset];
                let right = values[start + offset + width];
                values[start + offset] = left + right;
                values[start + offset + width] = left - right;
            }
        }
        width = step;
    }
}

fn normalize_hadamard(values: &mut [f32]) {
    let scale = (values.len() as f32).sqrt().recip();
    for value in values {
        *value *= scale;
    }
}

fn l2_norm(values: &[f32]) -> f32 {
    values.iter().map(|value| value * value).sum::<f32>().sqrt()
}

fn dot_product(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right.iter())
        .map(|(left, right)| left * right)
        .sum()
}

fn qjl_sign_value(value: bool) -> f32 {
    if value { 1.0 } else { -1.0 }
}

fn qjl_project_values(values: &[f32], seed: u64) -> Vec<f32> {
    random_hadamard_rotate(
        values,
        seed ^ TURBOQUANT_QJL_SEED_XOR,
        RotationDirection::Forward,
    )
}

fn qjl_unproject_values(values: &[f32], seed: u64) -> Vec<f32> {
    random_hadamard_rotate(
        values,
        seed ^ TURBOQUANT_QJL_SEED_XOR,
        RotationDirection::Inverse,
    )
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

fn write_container_header(
    out: &mut Vec<u8>,
    total_values: usize,
    chunk_count: usize,
    container_checksum: u64,
) {
    assert!(
        total_values <= u64::MAX as usize,
        "value count exceeds portable container header"
    );
    assert!(
        chunk_count <= u32::MAX as usize,
        "chunk count exceeds container header"
    );
    out.extend_from_slice(CONTAINER_MAGIC);
    out.push(CONTAINER_VERSION);
    out.push(CodecMode::QatqExact.id());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&(total_values as u64).to_be_bytes());
    out.extend_from_slice(&(chunk_count as u32).to_be_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&container_checksum.to_be_bytes());
}

fn patch_container_checksum(out: &mut [u8], container_checksum: u64) -> Result<(), QatqError> {
    if out.len() < CONTAINER_V2_HEADER_LEN {
        return Err(QatqError::InvalidContainer);
    }
    out[24..32].copy_from_slice(&container_checksum.to_be_bytes());
    Ok(())
}

fn checksum_f32_bits(values: &[f32]) -> u64 {
    let mut hash = FNV_OFFSET;
    for value in values {
        hash = checksum_bits_update(hash, value.to_bits());
    }
    hash
}

fn checksum_bytes(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
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

fn container_checksum_chunk(mut hash: u64, payload: &[u8]) -> u64 {
    for byte in (payload.len() as u32).to_be_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    for byte in payload {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn verify_container_checksum(
    header: &ContainerHeader,
    body: &[u8],
    chunks: &[(usize, usize)],
) -> Result<(), QatqError> {
    let expected = header.container_checksum;
    let mut actual = FNV_OFFSET;
    for &(chunk_start, chunk_end) in chunks {
        let len_start = chunk_start
            .checked_sub(CONTAINER_CHUNK_LEN)
            .ok_or(QatqError::InvalidContainer)?;
        let encoded_len = u32::from_be_bytes(
            body[len_start..chunk_start]
                .try_into()
                .expect("fixed length"),
        ) as usize;
        if encoded_len != chunk_end - chunk_start {
            return Err(QatqError::InvalidContainer);
        }
        actual = container_checksum_chunk(actual, &body[chunk_start..chunk_end]);
    }
    if actual != expected {
        return Err(QatqError::ChecksumMismatch { expected, actual });
    }
    Ok(())
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
    header_len: usize,
    total_values: usize,
    chunk_count: u32,
    container_checksum: u64,
}

impl ContainerHeader {
    fn parse(payload: &[u8]) -> Result<Self, QatqError> {
        if payload.len() < CONTAINER_V2_HEADER_LEN {
            return Err(QatqError::PayloadTooShort {
                actual: payload.len(),
                minimum: CONTAINER_V2_HEADER_LEN,
            });
        }
        if &payload[0..4] != CONTAINER_MAGIC {
            return Err(QatqError::InvalidMagic);
        }
        let version = payload[4];
        if version != CONTAINER_VERSION {
            return Err(QatqError::UnsupportedVersion(version));
        }
        let mode = CodecMode::from_id(payload[5])?;
        if mode != CodecMode::QatqExact {
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
            header_len: CONTAINER_V2_HEADER_LEN,
            total_values,
            chunk_count,
            container_checksum: u64::from_be_bytes(
                payload[24..32]
                    .try_into()
                    .expect("fixed container checksum"),
            ),
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

    fn lcg_next_for_test(state: u32) -> u32 {
        state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223)
    }

    #[test]
    fn parse_mode_accepts_only_qatq_exact_for_exact_product_surface() {
        assert_eq!(parse_mode("qatq-exact"), Ok(CodecMode::QatqExact));
        let old_exact_mode = ["phase", "2-lossless"].concat();
        assert_eq!(
            parse_mode(&old_exact_mode),
            Err(QatqError::UnsupportedMode(0))
        );
    }

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
    fn turboquant_q4_roundtrip_preserves_shape_and_compresses() {
        let values: Vec<f32> = (0..1025)
            .map(|index| {
                let x = index as f32;
                (x * 0.03125).sin() * 2.5 + (x * 0.0078125).cos() * 0.75
            })
            .collect();

        let encoded = encode_turboquant_q4(&values);
        let decoded = decode_turboquant_q4(&encoded).unwrap();

        assert_eq!(decoded.len(), values.len());
        assert!(encoded.len() < values.len() * 4);
        assert!(compression_ratio(encoded.len(), values.len()) < 0.7);
        let max_abs = max_abs_error(&values, &decoded);
        assert!(max_abs < 2.5, "max_abs={max_abs}");
        assert!(decoded.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn turboquant_q4_seed_is_deterministic_and_changes_payload() {
        let values: Vec<f32> = (0..128)
            .map(|index| ((index as f32) * 0.21).sin())
            .collect();
        let first = encode_turboquant_q4_with_config(&values, Phase1Config { seed: 7 });
        let second = encode_turboquant_q4_with_config(&values, Phase1Config { seed: 7 });
        let third = encode_turboquant_q4_with_config(&values, Phase1Config { seed: 8 });

        assert_eq!(first, second);
        assert_ne!(first, third);
        assert_eq!(
            decode_turboquant_q4(&first).unwrap().len(),
            decode_turboquant_q4(&third).unwrap().len()
        );
    }

    #[test]
    fn turboquant_q4_inner_product_estimator_matches_corrected_decode_dot() {
        let values: Vec<f32> = (0..64)
            .map(|index| {
                let x = index as f32;
                (x * 0.17).sin() + (x * 0.03125).cos() * 0.5
            })
            .collect();
        let query: Vec<f32> = (0..64)
            .map(|index| {
                let x = index as f32;
                (x * 0.11).cos() - (x * 0.07).sin() * 0.25
            })
            .collect();

        let encoded = encode_turboquant_q4_with_config(&values, Phase1Config { seed: 42 });
        let decoded = decode_turboquant_q4(&encoded).unwrap();
        let estimated = estimate_turboquant_q4_inner_product(&encoded, &query).unwrap();
        let decoded_dot = dot_product(&query, &decoded);

        assert!((estimated - decoded_dot).abs() < 1.0e-3);
    }

    #[test]
    fn turboquant_q4_inner_product_rejects_query_length_mismatch() {
        let encoded = encode_turboquant_q4(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(
            estimate_turboquant_q4_inner_product(&encoded, &[1.0, 2.0]),
            Err(QatqError::LengthMismatch {
                expected: 4,
                actual: 2
            })
        );
    }

    #[test]
    fn turboquant_q4_rejects_invalid_residual_norm() {
        let mut encoded = encode_turboquant_q4(&[1.0, 2.0, 3.0, 4.0]);
        let offset = HEADER_LEN + 12;
        encoded[offset..offset + 4].copy_from_slice(&f32::NAN.to_bits().to_be_bytes());

        assert_eq!(
            decode_turboquant_q4(&encoded),
            Err(QatqError::InvalidResidualScale(f32::NAN.to_bits()))
        );
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
    fn qatq_exact_roundtrip_preserves_bits() {
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

        let encoded = encode_qatq_exact(&values);
        let decoded = decode_qatq_exact(&encoded).unwrap();

        let before: Vec<u32> = values.iter().map(|value| value.to_bits()).collect();
        let after: Vec<u32> = decoded.iter().map(|value| value.to_bits()).collect();
        assert_eq!(after, before);
    }

    #[test]
    fn qatq_exact_typed_f16_roundtrip_preserves_native_bytes() {
        let mut bytes = Vec::new();
        for bits in [
            0x0000_u16, 0x8000, 0x3c00, 0xbc00, 0x7c00, 0xfc00, 0x7e01, 0x3555, 0x3555, 0x3555,
        ] {
            bytes.extend_from_slice(&bits.to_le_bytes());
        }

        let encoded = encode_qatq_exact_tensor_le(&bytes, TensorDType::F16);
        let decoded = decode_qatq_exact_tensor_le(&encoded).unwrap();

        assert_eq!(decoded.dtype, TensorDType::F16);
        assert_eq!(decoded.bytes_le, bytes);
        assert_eq!(
            decode_qatq_exact(&encoded),
            Err(QatqError::InvalidQatqExactBody)
        );
    }

    #[test]
    fn qatq_exact_typed_bf16_roundtrip_preserves_native_bytes() {
        let mut bytes = Vec::new();
        for bits in [
            0x0000_u16, 0x8000, 0x3f80, 0xbf80, 0x7f80, 0xff80, 0x7fc1, 0x3eab, 0x3eab, 0x3eab,
        ] {
            bytes.extend_from_slice(&bits.to_le_bytes());
        }

        let encoded = try_encode_qatq_exact_tensor_le(&bytes, TensorDType::BF16).unwrap();
        let decoded = decode_qatq_exact_tensor_le(&encoded).unwrap();

        assert_eq!(decoded.dtype, TensorDType::BF16);
        assert_eq!(decoded.bytes_le, bytes);
        assert!(matches!(
            qatq_exact_strategy(&encoded),
            Ok(QatqExactStrategy::RawBits)
                | Ok(QatqExactStrategy::ByteRle)
                | Ok(QatqExactStrategy::BytePlaneRle)
                | Ok(QatqExactStrategy::BytePlanePackedRle)
                | Ok(QatqExactStrategy::BytePlaneZstd)
        ));
    }

    #[test]
    fn qatq_exact_exhaustive_roundtrip_preserves_bits() {
        let values: Vec<f32> = (0..256)
            .map(|index| ((index as f32) * 0.03125).sin())
            .collect();

        let fast = encode_qatq_exact(&values);
        let exhaustive = encode_qatq_exact_exhaustive(&values);
        let decoded_fast = decode_qatq_exact(&fast).unwrap();
        let decoded_exhaustive = decode_qatq_exact(&exhaustive).unwrap();

        assert_eq!(f32_bits(&decoded_fast), f32_bits(&values));
        assert_eq!(f32_bits(&decoded_exhaustive), f32_bits(&values));
        assert!(exhaustive.len() <= fast.len());
    }

    #[test]
    fn qatq_exact_seed_is_deterministic_and_changes_payload() {
        let values: Vec<f32> = (0..128)
            .map(|index| ((index as f32) * 0.017).sin())
            .collect();
        let first = encode_qatq_exact_predictor_for_test(&values, Phase1Config { seed: 11 });
        let second = encode_qatq_exact_predictor_for_test(&values, Phase1Config { seed: 11 });
        let third = encode_qatq_exact_predictor_for_test(&values, Phase1Config { seed: 12 });

        assert_eq!(first[HEADER_LEN + 4], QATQ_EXACT_STRATEGY_PREDICTOR_XOR);
        assert_eq!(third[HEADER_LEN + 4], QATQ_EXACT_STRATEGY_PREDICTOR_XOR);
        assert_eq!(first, second);
        assert_ne!(first, third);
        assert_eq!(decode_qatq_exact(&first).unwrap(), values);
        assert_eq!(decode_qatq_exact(&third).unwrap(), values);
    }

    #[test]
    fn qatq_exact_rejects_bad_body_magic() {
        let mut encoded = encode_qatq_exact(&[1.0, 2.0, 3.0, 4.0]);
        encoded[HEADER_LEN] = b'X';

        assert_eq!(
            decode_qatq_exact(&encoded),
            Err(QatqError::InvalidQatqExactBody)
        );
    }

    #[test]
    fn qatq_exact_rejects_nonzero_reserved_prefix_bytes() {
        let mut encoded = encode_qatq_exact(&[1.0, 2.0, 3.0, 4.0]);
        encoded[HEADER_LEN + 5] = 1;

        assert_eq!(
            decode_qatq_exact(&encoded),
            Err(QatqError::InvalidQatqExactBody)
        );
    }

    #[test]
    fn qatq_exact_rejects_oversized_header_count() {
        let mut encoded = Vec::new();
        write_test_header_unchecked(
            &mut encoded,
            CodecMode::QatqExact,
            (MAX_VALUES_PER_PAYLOAD + 1) as u64,
            1.0,
            checksum_f32_bits(&[]),
        );
        write_qatq_exact_prefix(&mut encoded, QATQ_EXACT_STRATEGY_RAW_BITS);

        assert_eq!(
            decode_qatq_exact(&encoded),
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
            CodecMode::TurboQuantQ4,
            CodecMode::Phase1Q4,
            CodecMode::QatqExact,
        ];

        for mode in modes {
            let encoded = try_encode(&values, mode).unwrap();
            let decoded = decode(&encoded).unwrap();
            assert_eq!(decoded.len(), values.len());
            if matches!(mode, CodecMode::LosslessF32 | CodecMode::QatqExact) {
                assert_eq!(f32_bits(&decoded), f32_bits(&values));
            }
        }
    }

    #[test]
    fn try_encode_qatq_exact_roundtrip_preserves_bits() {
        let values = [0.0_f32, -0.0, f32::INFINITY, f32::from_bits(0x7fc0_1234)];
        let encoded = try_encode(&values, CodecMode::QatqExact).unwrap();

        assert_eq!(f32_bits(&decode(&encoded).unwrap()), f32_bits(&values));
    }

    #[test]
    fn try_encode_seeded_qatq_exact_roundtrip_preserves_bits() {
        let values = [0.25_f32, -0.5, f32::from_bits(0x7fc0_5678), 2.0];
        let encoded =
            try_encode_qatq_exact_with_config(&values, Phase1Config { seed: 17 }).unwrap();

        assert_eq!(f32_bits(&decode(&encoded).unwrap()), f32_bits(&values));
    }

    #[test]
    fn qatq_exact_rejects_truncated_residual_stream() {
        let values: Vec<f32> = (0..128)
            .map(|index| ((index as f32) * 0.017).sin())
            .collect();
        let mut encoded = encode_qatq_exact_predictor_for_test(&values, Phase1Config { seed: 1 });
        assert_eq!(encoded[HEADER_LEN + 4], QATQ_EXACT_STRATEGY_PREDICTOR_XOR);
        encoded.pop();

        assert_eq!(
            decode_qatq_exact(&encoded),
            Err(QatqError::InvalidResidualStream)
        );
    }

    #[test]
    fn qatq_exact_detects_payload_corruption() {
        let values = vec![0.0_f32; 128];
        let mut encoded = encode_qatq_exact(&values);
        assert_eq!(encoded[HEADER_LEN + 4], QATQ_EXACT_STRATEGY_BYTE_RLE);
        let last = encoded.last_mut().unwrap();
        *last ^= 0x01;

        assert!(matches!(
            decode_qatq_exact(&encoded),
            Err(QatqError::ChecksumMismatch { .. }) | Err(QatqError::InvalidResidualStream)
        ));
    }

    #[test]
    fn qatq_exact_uses_raw_bits_when_predictor_residual_is_larger() {
        let values = [
            f32::from_bits(0x0102_0304),
            f32::from_bits(0x1122_3344),
            f32::from_bits(0x5566_7788),
            f32::from_bits(0x99aa_bbcc),
        ];
        let encoded = encode_qatq_exact(&values);

        assert_eq!(encoded[HEADER_LEN + 4], QATQ_EXACT_STRATEGY_RAW_BITS);
        assert_eq!(
            f32_bits(&decode_qatq_exact(&encoded).unwrap()),
            f32_bits(&values)
        );
    }

    #[test]
    fn exact_decision_passes_through_raw_bits_as_f32le() {
        let values = [
            f32::from_bits(0x0102_0304),
            f32::from_bits(0x1122_3344),
            f32::from_bits(0x5566_7788),
            f32::from_bits(0x99aa_bbcc),
        ];
        let decision =
            try_encode_qatq_exact_decision_with_config(&values, Phase1Config::default()).unwrap();

        let expected = encode_f32_bits_le(&values);
        assert!(decision.should_pass_through());
        assert!(!decision.should_compress());
        assert_eq!(decision.strategy(), None);
        assert_eq!(decision.raw_f32le_len(), expected.len());
        assert_eq!(decision.stored_bytes(), expected.as_slice());
    }

    #[test]
    fn exact_decision_compresses_non_raw_strategy() {
        let values = vec![0.0_f32; 128];
        let decision = encode_qatq_exact_decision(&values);

        match decision {
            QatqExactEncodeDecision::Compressed {
                payload,
                strategy,
                raw_f32le_len,
            } => {
                assert_eq!(strategy, QatqExactStrategy::ByteRle);
                assert_eq!(raw_f32le_len, values.len() * 4);
                assert!(payload.len() < raw_f32le_len);
                assert_eq!(
                    f32_bits(&decode_qatq_exact(&payload).unwrap()),
                    f32_bits(&values)
                );
            }
            QatqExactEncodeDecision::PassThroughRaw { .. } => {
                panic!("compressible values should return a compressed decision")
            }
        }
    }

    #[test]
    fn production_chunk_roundtrip_restores_compressed_payload() {
        let values = vec![0.0_f32; 128];
        let encoded = try_encode_production_chunk(&values).unwrap();

        assert!(encoded.should_compress());
        assert_eq!(encoded.metadata.storage_label(), "qatq-exact");
        assert_eq!(encoded.metadata.raw_f32le_len, values.len() * 4);
        assert_eq!(encoded.metadata.strategy, Some(QatqExactStrategy::ByteRle));

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
    fn qatq_exact_uses_byte_plane_blocks_when_raw_planes_are_repetitive() {
        let values = vec![1.0_f32; 128];
        let encoded = encode_qatq_exact(&values);

        assert_eq!(
            encoded[HEADER_LEN + 4],
            QATQ_EXACT_STRATEGY_BYTE_PLANE_BLOCKS
        );
        assert!(encoded.len() < encode_lossless_f32(&values).len());
        assert_eq!(decode_qatq_exact(&encoded).unwrap(), values);
    }

    #[test]
    fn qatq_exact_uses_delta_xor_byte_plane_for_adjacent_bit_residuals() {
        let mut bits = 0x3f00_0001_u32;
        let values: Vec<f32> = (0..256)
            .map(|_| {
                bits ^= 0x0102_0304;
                f32::from_bits(bits)
            })
            .collect();
        let encoded = encode_qatq_exact(&values);

        assert_eq!(
            encoded[HEADER_LEN + 4],
            QATQ_EXACT_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE
        );
        assert!(encoded.len() < encode_lossless_f32(&values).len());
        assert_eq!(
            f32_bits(&decode_qatq_exact(&encoded).unwrap()),
            f32_bits(&values)
        );
    }

    #[test]
    fn qatq_exact_strategy_reports_selected_strategy() {
        let values = [0.0_f32; 128];
        let encoded = encode_qatq_exact(&values);

        assert_eq!(
            qatq_exact_strategy(&encoded),
            Ok(QatqExactStrategy::ByteRle)
        );
        assert_eq!(
            qatq_exact_strategy(&encode_lossless_f32(&values)),
            Err(QatqError::UnsupportedMode(2))
        );
    }

    #[test]
    fn qatq_exact_fast_accepts_compression_positive_byte_plane_zstd_candidate() {
        let mut state = 0x9e37_79b9_u32;
        let values: Vec<f32> = (0..4096)
            .map(|index| {
                state = lcg_next_for_test(state ^ index as u32);
                let mantissa = state & 0x007f_ffff;
                let exponent = 124 + (state % 6);
                let sign = (state >> 31) << 31;
                f32::from_bits(sign | (exponent << 23) | mantissa)
            })
            .collect();
        let encoded = encode_qatq_exact(&values);

        assert_eq!(encoded[HEADER_LEN + 4], QATQ_EXACT_STRATEGY_BYTE_PLANE_ZSTD);
        assert!(encoded.len() < encode_lossless_f32(&values).len());
        assert_eq!(
            f32_bits(&decode_qatq_exact(&encoded).unwrap()),
            f32_bits(&values)
        );
    }

    #[test]
    fn qatq_exact_selects_reversible_quaternion_chain_when_smaller() {
        let values: Vec<f32> = (0..4096)
            .map(|index| {
                let value = ((index as f32) * 0.03125).sin();
                f32::from_bits(value.to_bits() & 0xffff_0000)
            })
            .collect();
        let encoded = encode_qatq_exact(&values);

        assert_eq!(
            encoded[HEADER_LEN + 4],
            QATQ_EXACT_STRATEGY_QUATERNION_CHAIN_ZSTD
        );
        assert!(encoded.len() < encode_lossless_f32(&values).len());
        assert_eq!(
            f32_bits(&decode_qatq_exact(&encoded).unwrap()),
            f32_bits(&values)
        );
    }

    #[test]
    fn qatq_exact_byte_rle_compresses_repeated_nonzero_bytes() {
        let values = vec![1.0_f32; 128];
        let encoded = encode_qatq_exact(&values);

        assert_eq!(
            encoded[HEADER_LEN + 4],
            QATQ_EXACT_STRATEGY_BYTE_PLANE_BLOCKS
        );
        assert!(encoded.len() < encode_lossless_f32(&values).len());
        assert_eq!(decode_qatq_exact(&encoded).unwrap(), values);
    }

    #[test]
    fn qatq_exact_delta_xor_byte_plane_rejects_truncated_stream() {
        let mut bits = 0x3f00_0001_u32;
        let values: Vec<f32> = (0..256)
            .map(|_| {
                bits ^= 0x0102_0304;
                f32::from_bits(bits)
            })
            .collect();
        let mut encoded = encode_qatq_exact(&values);
        assert_eq!(
            encoded[HEADER_LEN + 4],
            QATQ_EXACT_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE
        );
        encoded.pop();

        assert_eq!(
            decode_qatq_exact(&encoded),
            Err(QatqError::InvalidResidualStream)
        );
    }

    #[test]
    fn qatq_exact_rejects_truncated_byte_plane_block() {
        let values = vec![1.0_f32; 128];
        let mut encoded = encode_qatq_exact(&values);
        assert_eq!(
            encoded[HEADER_LEN + 4],
            QATQ_EXACT_STRATEGY_BYTE_PLANE_BLOCKS
        );
        encoded.pop();

        assert_eq!(
            decode_qatq_exact(&encoded),
            Err(QatqError::InvalidResidualStream)
        );
    }

    #[test]
    fn qatq_exact_rejects_zero_length_byte_run() {
        let mut encoded = Vec::new();
        write_header(
            &mut encoded,
            CodecMode::QatqExact,
            1,
            1.0,
            checksum_f32_bits(&[0.0]),
        );
        write_qatq_exact_prefix(&mut encoded, QATQ_EXACT_STRATEGY_BYTE_RLE);
        encoded.extend_from_slice(&[XOR_ZERO_RUN, 0, 0]);

        assert_eq!(
            decode_qatq_exact(&encoded),
            Err(QatqError::InvalidResidualStream)
        );
    }

    #[test]
    fn qatq_exact_rejects_unknown_byte_run_token() {
        let mut encoded = Vec::new();
        write_header(
            &mut encoded,
            CodecMode::QatqExact,
            1,
            1.0,
            checksum_f32_bits(&[0.0]),
        );
        write_qatq_exact_prefix(&mut encoded, QATQ_EXACT_STRATEGY_BYTE_RLE);
        encoded.extend_from_slice(&[99, 0, 4]);

        assert_eq!(
            decode_qatq_exact(&encoded),
            Err(QatqError::InvalidResidualStream)
        );
    }

    #[test]
    fn qatq_exact_rejects_trailing_byte_run_data() {
        let mut encoded = Vec::new();
        write_header(
            &mut encoded,
            CodecMode::QatqExact,
            1,
            1.0,
            checksum_f32_bits(&[0.0]),
        );
        write_qatq_exact_prefix(&mut encoded, QATQ_EXACT_STRATEGY_BYTE_RLE);
        encoded.extend_from_slice(&[XOR_ZERO_RUN, 0, 4, 0xaa]);

        assert_eq!(
            decode_qatq_exact(&encoded),
            Err(QatqError::InvalidResidualStream)
        );
    }

    #[test]
    fn qatq_exact_chunks_roundtrip_preserves_bits() {
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
            encode_qatq_exact_chunks_with_config(&values, 64, Phase1Config { seed: 99 }).unwrap();
        let decoded = decode_qatq_exact_chunks(chunks.iter().map(Vec::as_slice)).unwrap();

        assert_eq!(chunks.len(), 5);
        assert_eq!(f32_bits(&decoded), f32_bits(&values));
    }

    #[test]
    fn qatq_exact_chunks_handle_empty_input_as_one_payload() {
        let chunks = encode_qatq_exact_chunks(&[], 64).unwrap();
        let decoded = decode_qatq_exact_chunks(chunks.iter().map(Vec::as_slice)).unwrap();

        assert_eq!(chunks.len(), 1);
        assert!(decoded.is_empty());
    }

    #[test]
    fn qatq_exact_chunks_reject_invalid_chunk_sizes() {
        assert_eq!(
            encode_qatq_exact_chunks(&[1.0], 0),
            Err(QatqError::InvalidChunkSize(0))
        );
        assert_eq!(
            encode_qatq_exact_chunks(&[1.0], MAX_VALUES_PER_PAYLOAD + 1),
            Err(QatqError::InvalidChunkSize(MAX_VALUES_PER_PAYLOAD + 1))
        );
    }

    #[test]
    fn qatq_exact_container_roundtrip_preserves_bits_through_decode() {
        let values: Vec<f32> = (0..259)
            .map(|index| match index % 53 {
                0 => -0.0,
                1 => f32::from_bits(0x7fc0_1000 | index as u32),
                _ => ((index as f32) * 0.019).cos(),
            })
            .collect();

        let encoded =
            encode_qatq_exact_container_with_config(&values, 64, Phase1Config { seed: 123 })
                .unwrap();
        let decoded = decode(&encoded).unwrap();

        assert_eq!(&encoded[0..4], CONTAINER_MAGIC);
        assert_eq!(f32_bits(&decoded), f32_bits(&values));
    }

    #[test]
    fn qatq_exact_container_handles_empty_input_as_one_chunk() {
        let encoded = encode_qatq_exact_container(&[], 64).unwrap();
        let decoded = decode_qatq_exact_container(&encoded).unwrap();

        assert_eq!(u32::from_be_bytes(encoded[16..20].try_into().unwrap()), 1);
        assert!(decoded.is_empty());
    }

    #[test]
    fn qatq_exact_container_payload_visitor_preserves_chunk_order() {
        let values: Vec<f32> = (0..10).map(|index| index as f32).collect();
        let encoded = encode_qatq_exact_container(&values, 4).unwrap();
        let mut chunk_lengths = Vec::new();
        let mut decoded = Vec::new();

        for_each_qatq_exact_container_payload(&encoded, |chunk| {
            let chunk_values = decode_qatq_exact(chunk)?;
            chunk_lengths.push(chunk_values.len());
            decoded.extend(chunk_values);
            Ok(())
        })
        .unwrap();

        assert_eq!(chunk_lengths, [4, 4, 2]);
        assert_eq!(decoded, values);
    }

    #[test]
    fn qatq_exact_container_writes_v2_checksum() {
        let encoded = encode_qatq_exact_container(&[1.0, 2.0, 3.0], 2).unwrap();

        assert_eq!(encoded[4], CONTAINER_VERSION);
        assert_ne!(&encoded[24..32], &[0_u8; 8]);
        assert_eq!(
            decode_qatq_exact_container(&encoded).unwrap(),
            [1.0, 2.0, 3.0]
        );
    }

    #[test]
    fn qatq_exact_container_rejects_v2_checksum_mismatch() {
        let mut encoded = encode_qatq_exact_container(&[1.0, 2.0, 3.0], 2).unwrap();
        let last = encoded.len() - 1;
        encoded[last] ^= 0x01;

        assert!(matches!(
            decode_qatq_exact_container(&encoded),
            Err(QatqError::ChecksumMismatch { .. })
        ));
    }

    #[test]
    fn qatq_exact_container_rejects_legacy_v1_header() {
        let mut encoded = encode_qatq_exact_container(&[1.0, 2.0], 1).unwrap();
        encoded[4] = VERSION;

        assert_eq!(
            decode_qatq_exact_container(&encoded),
            Err(QatqError::UnsupportedVersion(VERSION))
        );
    }

    #[test]
    fn qatq_exact_container_enforces_decode_limits_before_callbacks() {
        let encoded = encode_qatq_exact_container(&[1.0, 2.0, 3.0], 1).unwrap();
        let limits = QatcDecodeLimits {
            max_total_values: 2,
            ..QatcDecodeLimits::default()
        };
        let mut visited = 0;

        assert_eq!(
            decode_qatq_exact_container_with_limits(&encoded, limits),
            Err(QatqError::ContainerLimitExceeded("total values"))
        );
        assert_eq!(
            for_each_qatq_exact_container_payload_with_limits(&encoded, limits, |_| {
                visited += 1;
                Ok(())
            }),
            Err(QatqError::ContainerLimitExceeded("total values"))
        );
        assert_eq!(visited, 0);
    }

    #[test]
    fn qatq_exact_u32_container_preserves_every_bit_pattern() {
        let words = [
            0,
            0x8000_0000,
            0x7f80_0000,
            0xff80_0000,
            0x7fc0_0001,
            0x7fa0_1234,
            u32::MAX,
        ];
        let values: Vec<f32> = words.iter().copied().map(f32::from_bits).collect();

        let encoded = encode_qatq_exact_u32_container(&words, 3).unwrap();
        let legacy_encoded = encode_qatq_exact_container(&values, 3).unwrap();
        let decoded = decode_qatq_exact_u32_container(&encoded, 3).unwrap();

        assert_eq!(encoded, legacy_encoded);
        assert_eq!(decoded, words);
    }

    #[test]
    fn qatq_exact_container_inspection_reports_canonical_layout() {
        let encoded = encode_qatq_exact_u32_container(&[10, 20, 30, 40, 50], 2).unwrap();
        let metadata =
            inspect_qatq_exact_container_with_limits(&encoded, QatcDecodeLimits::default(), 2)
                .unwrap();

        assert_eq!(metadata.encoded_bytes, encoded.len());
        assert_eq!(metadata.total_values, 5);
        assert_eq!(
            metadata
                .chunks
                .iter()
                .map(|chunk| chunk.decoded_values)
                .collect::<Vec<_>>(),
            [2, 2, 1]
        );
        assert!(metadata.chunks.iter().all(|chunk| chunk.encoded_bytes > 0));
    }

    #[test]
    fn qatq_exact_container_inspection_rejects_wrong_declared_chunk_size() {
        let encoded = encode_qatq_exact_u32_container(&[10, 20, 30, 40, 50], 2).unwrap();

        assert_eq!(
            inspect_qatq_exact_container_with_limits(&encoded, QatcDecodeLimits::default(), 3,),
            Err(QatqError::InvalidContainer)
        );
        assert_eq!(
            decode_qatq_exact_u32_container(&encoded, 3),
            Err(QatqError::InvalidContainer)
        );
    }

    #[test]
    fn qatq_exact_container_inspection_handles_empty_input() {
        let encoded = encode_qatq_exact_u32_container(&[], 16).unwrap();
        let metadata =
            inspect_qatq_exact_container_with_limits(&encoded, QatcDecodeLimits::default(), 16)
                .unwrap();

        assert_eq!(metadata.total_values, 0);
        assert_eq!(metadata.chunks.len(), 1);
        assert_eq!(metadata.chunks[0].decoded_values, 0);
        assert!(
            decode_qatq_exact_u32_container(&encoded, 16)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn qatq_exact_u32_decode_enforces_limits_before_decode() {
        let encoded = encode_qatq_exact_u32_container(&[1, 2, 3], 2).unwrap();
        let limits = QatcDecodeLimits {
            max_total_values: 2,
            ..QatcDecodeLimits::default()
        };

        assert_eq!(
            decode_qatq_exact_u32_container_with_limits(&encoded, limits, 2),
            Err(QatqError::ContainerLimitExceeded("total values"))
        );
    }

    #[test]
    fn qatq_exact_container_rejects_invalid_chunk_size() {
        assert_eq!(
            encode_qatq_exact_container(&[1.0], 0),
            Err(QatqError::InvalidChunkSize(0))
        );
    }

    #[test]
    fn qatq_exact_container_rejects_zero_chunk_count() {
        let mut encoded = Vec::new();
        write_container_header(&mut encoded, 0, 0, FNV_OFFSET);

        assert_eq!(
            decode_qatq_exact_container(&encoded),
            Err(QatqError::InvalidContainer)
        );
    }

    #[test]
    fn qatq_exact_container_rejects_nonzero_reserved_bytes() {
        let mut encoded = encode_qatq_exact_container(&[1.0, 2.0], 1).unwrap();
        encoded[6] = 1;

        assert_eq!(
            decode_qatq_exact_container(&encoded),
            Err(QatqError::InvalidContainer)
        );

        encoded[6] = 0;
        encoded[20] = 1;
        assert_eq!(
            decode_qatq_exact_container(&encoded),
            Err(QatqError::InvalidContainer)
        );
    }

    #[test]
    fn qatq_exact_container_rejects_truncated_chunk_body() {
        let mut encoded = encode_qatq_exact_container(&[1.0, 2.0, 3.0], 2).unwrap();
        encoded.pop();

        assert_eq!(
            decode_qatq_exact_container(&encoded),
            Err(QatqError::InvalidContainer)
        );
    }

    #[test]
    fn qatq_exact_container_payload_visitor_validates_before_callbacks() {
        let mut encoded = encode_qatq_exact_container(&[1.0, 2.0, 3.0], 2).unwrap();
        encoded.pop();
        let mut visited = 0;

        assert_eq!(
            for_each_qatq_exact_container_payload(&encoded, |_| {
                visited += 1;
                Ok(())
            }),
            Err(QatqError::InvalidContainer)
        );
        assert_eq!(visited, 0);
    }

    #[test]
    fn qatq_exact_container_rejects_total_value_mismatch() {
        let mut encoded = encode_qatq_exact_container(&[1.0, 2.0, 3.0], 2).unwrap();
        encoded[15] = 2;

        assert_eq!(
            decode_qatq_exact_container(&encoded),
            Err(QatqError::InvalidContainer)
        );
    }

    #[test]
    fn qatq_exact_container_rejects_huge_total_before_allocation() {
        let mut encoded = encode_qatq_exact_container(&[1.0], 1).unwrap();
        encoded[8..16].copy_from_slice(&((DEFAULT_MAX_QATC_VALUES as u64) + 1).to_be_bytes());

        assert_eq!(
            decode_qatq_exact_container(&encoded),
            Err(QatqError::ContainerLimitExceeded("total values"))
        );
    }

    #[test]
    fn qatq_exact_container_rejects_trailing_data() {
        let mut encoded = encode_qatq_exact_container(&[1.0, 2.0, 3.0], 2).unwrap();
        encoded.push(0);

        assert_eq!(
            decode_qatq_exact_container(&encoded),
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

    fn encode_qatq_exact_predictor_for_test(values: &[f32], config: Phase1Config) -> Vec<u8> {
        let parts = build_phase1_parts(values, config);
        let predicted = reconstruct_phase1_values(values.len(), &parts);
        let residuals = encode_xor_residuals(values, &predicted);
        let checksum = checksum_f32_bits(values);
        let mut out = Vec::new();
        write_header(
            &mut out,
            CodecMode::QatqExact,
            values.len(),
            parts.scale,
            checksum,
        );
        write_qatq_exact_prefix(&mut out, QATQ_EXACT_STRATEGY_PREDICTOR_XOR);
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
