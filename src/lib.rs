use std::fmt;

mod fast_paths;

#[cfg(feature = "oracle")]
pub mod oracle;

#[cfg(feature = "geometry")]
pub mod geometry;

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
const QATQ_EXACT_STRATEGY_ADJACENT_XOR_BYTE_PLANE_ZSTD: u8 = 9;
const QATQ_EXACT_STRATEGY_STRIDED_XOR_BYTE_PLANE_ZSTD: u8 = 10;
const QATQ_EXACT_STRATEGY_BIT_SPLIT_PLANE_ZSTD: u8 = 11;
const BIT_SPLIT_TRANSFORM_IDENTITY: u8 = 0;
const BIT_SPLIT_TRANSFORM_HI16_ROT1: u8 = 1;
const BIT_SPLIT_MODE_ZERO: u8 = 0;
const BIT_SPLIT_MODE_REPEAT: u8 = 1;
const BIT_SPLIT_MODE_RAW: u8 = 2;
const BIT_SPLIT_MODE_ZSTD: u8 = 3;
const NATIVE_U16_XOR_SAMPLE_WORDS: usize = 4_096;
const NATIVE_U16_XOR_SAMPLE_BLOCKS: usize = 64;
const STRIDED_U16_XOR_CLASSIFIER_WORDS: usize = 1_024;
const STRIDED_U16_XOR_SAMPLE_WIN_PERCENT: usize = 90;
const STRIDED_U16_XOR_METADATA_LEN: usize = 4;
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
    InvalidPredictorStride(usize),
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
            Self::InvalidPredictorStride(stride) => {
                write!(f, "predictor stride is invalid: {stride}")
            }
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
    AdjacentXorBytePlaneZstd,
    StridedXorBytePlaneZstd,
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
            QATQ_EXACT_STRATEGY_ADJACENT_XOR_BYTE_PLANE_ZSTD => Ok(Self::AdjacentXorBytePlaneZstd),
            QATQ_EXACT_STRATEGY_STRIDED_XOR_BYTE_PLANE_ZSTD => Ok(Self::StridedXorBytePlaneZstd),
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
            Self::AdjacentXorBytePlaneZstd => "adjacent-xor-byte-plane-zstd",
            Self::StridedXorBytePlaneZstd => "strided-xor-byte-plane-zstd",
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
    try_encode_qatq_exact_tensor_le_with_optional_stride(bytes_le, dtype, None)
}

/// Encodes a native tensor with a caller-supplied row/chunk stride for the
/// reversible cross-chunk predictor.
///
/// The hint is expressed in tensor elements, not bytes. A conservative sample
/// classifier chooses either ordinary byte-plane Zstd or the strided predictor
/// before one full compression pass. Raw and run-coded exact fallbacks remain
/// available, but this experimental API does not exhaustively prove which Zstd
/// transform would produce the smaller complete payload.
pub fn try_encode_qatq_exact_tensor_le_with_stride_hint(
    bytes_le: &[u8],
    dtype: TensorDType,
    stride_elements: usize,
) -> Result<Vec<u8>, QatqError> {
    try_encode_qatq_exact_tensor_le_with_optional_stride(bytes_le, dtype, Some(stride_elements))
}

fn try_encode_qatq_exact_tensor_le_with_optional_stride(
    bytes_le: &[u8],
    dtype: TensorDType,
    stride_hint: Option<usize>,
) -> Result<Vec<u8>, QatqError> {
    let element_count = validate_typed_tensor_len(bytes_le, dtype)?;
    if dtype == TensorDType::F32 {
        if let Some(stride) = stride_hint {
            return Err(QatqError::InvalidPredictorStride(stride));
        }
        let values = decode_f32le_bytes(bytes_le)?;
        return try_encode_qatq_exact_with_config(&values, Phase1Config::default());
    }
    if let Some(stride) = stride_hint
        && (stride == 0 || stride >= element_count || u32::try_from(stride).is_err())
    {
        return Err(QatqError::InvalidPredictorStride(stride));
    }
    Ok(encode_qatq_exact_tensor_bytes_unchecked(
        bytes_le,
        dtype,
        element_count,
        stride_hint,
    ))
}

pub fn decode_qatq_exact_tensor_le(payload: &[u8]) -> Result<DecodedTensor, QatqError> {
    let header = Header::parse_for_mode(payload, CodecMode::QatqExact)?;
    let body = &payload[HEADER_LEN..];
    let (strategy_id, dtype) = parse_qatq_exact_prefix(body)?;
    if dtype == TensorDType::F32 {
        return Ok(DecodedTensor {
            dtype,
            bytes_le: encode_f32_bits_le(&decode_qatq_exact(payload)?),
        });
    }

    let expected_len = checked_tensor_byte_len(header.value_count, dtype)?;
    let body = &body[QATQ_EXACT_PREFIX_LEN..];
    let (bytes_le, actual) = match strategy_id {
        QATQ_EXACT_STRATEGY_RAW_BITS => decode_qatq_exact_tensor_raw_bits_le(body, expected_len)?,
        QATQ_EXACT_STRATEGY_BYTE_RLE => decode_byte_runs_to_u16_le(body, expected_len)?,
        QATQ_EXACT_STRATEGY_BYTE_PLANE_RLE => {
            decode_byte_plane_runs_to_u16_le(body, expected_len, header.value_count)?
        }
        QATQ_EXACT_STRATEGY_BYTE_PLANE_PACKED_RLE => {
            decode_byte_plane_packed_runs_to_u16_le(body, expected_len, header.value_count)?
        }
        QATQ_EXACT_STRATEGY_BYTE_PLANE_ZSTD => {
            let plane_bytes = fast_paths::zstd_decompress(body, expected_len)
                .map_err(|_| QatqError::InvalidResidualStream)?;
            if plane_bytes.len() != expected_len {
                return Err(QatqError::InvalidResidualStream);
            }
            let (p0, p1) = plane_bytes.split_at(header.value_count);
            fast_paths::untranspose_be_planes_to_u16_le_and_checksum(p0, p1, false)
        }
        QATQ_EXACT_STRATEGY_ADJACENT_XOR_BYTE_PLANE_ZSTD => {
            let residual_planes = fast_paths::zstd_decompress(body, expected_len)
                .map_err(|_| QatqError::InvalidResidualStream)?;
            decode_adjacent_xor_u16_byte_planes_to_le(
                &residual_planes,
                expected_len,
                header.value_count,
            )?
        }
        QATQ_EXACT_STRATEGY_STRIDED_XOR_BYTE_PLANE_ZSTD => {
            if body.len() < STRIDED_U16_XOR_METADATA_LEN {
                return Err(QatqError::InvalidResidualStream);
            }
            let stride =
                u32::from_be_bytes(body[..STRIDED_U16_XOR_METADATA_LEN].try_into().unwrap())
                    as usize;
            let residual_planes =
                fast_paths::zstd_decompress(&body[STRIDED_U16_XOR_METADATA_LEN..], expected_len)
                    .map_err(|_| QatqError::InvalidResidualStream)?;
            decode_strided_xor_u16_byte_planes_to_le(
                &residual_planes,
                expected_len,
                header.value_count,
                stride,
            )?
        }
        QATQ_EXACT_STRATEGY_BIT_SPLIT_PLANE_ZSTD => {
            decode_bit_split_plane_zstd_u16_le_and_checksum(
                body,
                expected_len,
                header.value_count,
                dtype,
            )?
        }
        _ => return Err(QatqError::InvalidQatqExactBody),
    };
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
    stride_hint: Option<usize>,
) -> Vec<u8> {
    let checksum = fast_paths::checksum_bytes_fast(bytes_le);
    let raw_len = bytes_le.len();
    let raw_body_len = QATQ_EXACT_PREFIX_LEN + raw_len;
    let planes_t0 = fast_paths::transpose_u16_le_to_be_planes(bytes_le);

    let mut best_strategy = QATQ_EXACT_STRATEGY_RAW_BITS;
    let mut best_payload: Option<Vec<u8>> = None;
    let mut best_payload_len = raw_len;

    let try_rle = raw_len < 512 || fast_paths::sample_has_rle_potential(&planes_t0, 20);
    if try_rle {
        if let Some(byte_rle) =
            encode_byte_runs_be_pairs_bounded(bytes_le, best_payload_len.saturating_sub(1))
        {
            best_payload_len = byte_rle.len();
            best_strategy = QATQ_EXACT_STRATEGY_BYTE_RLE;
            best_payload = Some(byte_rle);
        }
        if let Some(byte_plane) = encode_contiguous_byte_plane_runs_bounded(
            &planes_t0,
            best_payload_len.saturating_sub(1),
        ) {
            best_payload_len = byte_plane.len();
            best_strategy = QATQ_EXACT_STRATEGY_BYTE_PLANE_RLE;
            best_payload = Some(byte_plane);
        }
        if let Some(byte_plane_packed) = encode_contiguous_byte_plane_packed_runs_bounded(
            &planes_t0,
            best_payload_len.saturating_sub(1),
        ) {
            best_payload_len = byte_plane_packed.len();
            best_strategy = QATQ_EXACT_STRATEGY_BYTE_PLANE_PACKED_RLE;
            best_payload = Some(byte_plane_packed);
        }
    }

    let adjacent_xor_planes = adjacent_xor_u16_byte_planes_if_sparse_le(bytes_le);
    let strided_xor_planes = if adjacent_xor_planes.is_none() {
        stride_hint.and_then(|stride| strided_xor_u16_byte_planes_if_sparse_le(bytes_le, stride))
    } else {
        None
    };
    let use_strided_xor = strided_xor_planes
        .as_ref()
        .is_some_and(|(_, planes)| strided_xor_wins_sample_compression_le(bytes_le, planes));

    if let Some(planes) = adjacent_xor_planes.as_ref() {
        if let Some(encoded) = encode_zstd_bounded(planes, best_payload_len.saturating_sub(1)) {
            best_payload_len = encoded.len();
            best_strategy = QATQ_EXACT_STRATEGY_ADJACENT_XOR_BYTE_PLANE_ZSTD;
            best_payload = Some(encoded);
        }
    } else if use_strided_xor {
        if let Some((stride, planes)) = strided_xor_planes {
            let limit = best_payload_len.saturating_sub(1 + STRIDED_U16_XOR_METADATA_LEN);
            if let Some(encoded) = encode_zstd_bounded(&planes, limit) {
                let mut body = Vec::with_capacity(STRIDED_U16_XOR_METADATA_LEN + encoded.len());
                body.extend_from_slice(&(stride as u32).to_be_bytes());
                body.extend_from_slice(&encoded);
                best_payload_len = body.len();
                best_strategy = QATQ_EXACT_STRATEGY_STRIDED_XOR_BYTE_PLANE_ZSTD;
                best_payload = Some(body);
            }
        }
    } else if let Some(z_payload) = encode_byte_plane_zstd_flushed_bounded(
        &planes_t0,
        element_count,
        2,
        best_payload_len.saturating_sub(1),
    ) {
        best_payload_len = z_payload.len();
        best_strategy = QATQ_EXACT_STRATEGY_BYTE_PLANE_ZSTD;
        best_payload = Some(z_payload);
    }

    let best_body_len = if best_strategy == QATQ_EXACT_STRATEGY_RAW_BITS {
        raw_body_len
    } else {
        QATQ_EXACT_PREFIX_LEN + best_payload_len
    };
    let mut out = Vec::with_capacity(HEADER_LEN + best_body_len);
    write_header(&mut out, CodecMode::QatqExact, element_count, 1.0, checksum);
    write_qatq_exact_typed_prefix(&mut out, best_strategy, dtype);
    if let Some(payload) = best_payload {
        out.extend_from_slice(&payload);
    } else {
        for pair in bytes_le.chunks_exact(2) {
            out.push(pair[1]);
            out.push(pair[0]);
        }
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
    let (planes_t0, checksum) = fast_paths::transpose_f32_to_be_planes_and_checksum(values);
    let raw_len = planes_t0.len();
    let raw_body_len = QATQ_EXACT_PREFIX_LEN + raw_len;

    let mut best_strategy = QATQ_EXACT_STRATEGY_RAW_BITS;
    let mut best_payload: Option<Vec<u8>> = None;
    let mut best_payload_len = raw_len;

    if let Some(blocks) =
        encode_byte_plane_blocks_from_planes_bounded(&planes_t0, values.len(), raw_len)
        && blocks.len() < best_payload_len
    {
        best_payload_len = blocks.len();
        best_strategy = QATQ_EXACT_STRATEGY_BYTE_PLANE_BLOCKS;
        best_payload = Some(blocks);
    }
    if let Some(byte_rle) =
        encode_byte_runs_from_f32_bounded(values, best_payload_len.saturating_sub(1))
    {
        best_payload_len = byte_rle.len();
        best_strategy = QATQ_EXACT_STRATEGY_BYTE_RLE;
        best_payload = Some(byte_rle);
    }
    if let Some(byte_plane) =
        encode_contiguous_byte_plane_runs_bounded(&planes_t0, best_payload_len.saturating_sub(1))
    {
        best_payload_len = byte_plane.len();
        best_strategy = QATQ_EXACT_STRATEGY_BYTE_PLANE_RLE;
        best_payload = Some(byte_plane);
    }
    if let Some(byte_plane_packed) = encode_contiguous_byte_plane_packed_runs_bounded(
        &planes_t0,
        best_payload_len.saturating_sub(1),
    ) {
        best_payload_len = byte_plane_packed.len();
        best_strategy = QATQ_EXACT_STRATEGY_BYTE_PLANE_PACKED_RLE;
        best_payload = Some(byte_plane_packed);
    }
    for level in [3, 1] {
        if let Some(bp_flushed) = fast_paths::zstd_compress_planes_flushed_bounded(
            &planes_t0,
            values.len(),
            4,
            level,
            best_payload_len.saturating_sub(1),
        ) {
            best_payload_len = bp_flushed.len();
            best_strategy = QATQ_EXACT_STRATEGY_BYTE_PLANE_ZSTD;
            best_payload = Some(bp_flushed);
        }
        if let Some(bp_zstd) =
            encode_zstd_level_bounded(&planes_t0, level, best_payload_len.saturating_sub(1))
        {
            best_payload_len = bp_zstd.len();
            best_strategy = QATQ_EXACT_STRATEGY_BYTE_PLANE_ZSTD;
            best_payload = Some(bp_zstd);
        }
        if let Some(qc_zstd) = encode_quaternion_chain_zstd_level_bounded(
            values,
            level,
            best_payload_len.saturating_sub(1),
        ) {
            best_payload_len = qc_zstd.len();
            best_strategy = QATQ_EXACT_STRATEGY_QUATERNION_CHAIN_ZSTD;
            best_payload = Some(qc_zstd);
        }
    }
    if let Some(delta_xor) =
        encode_delta_xor_byte_plane_runs_bounded(values, best_payload_len.saturating_sub(1))
    {
        best_payload_len = delta_xor.len();
        best_strategy = QATQ_EXACT_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE;
        best_payload = Some(delta_xor);
    }

    let parts = build_phase1_parts(values, config);
    let predicted = reconstruct_phase1_values(values.len(), &parts);
    let residuals = encode_xor_residuals(values, &predicted);
    let quantized_len = parts.coord_count.div_ceil(2);
    let residual_sign_len = parts.coord_count.div_ceil(8);
    let predictor_payload_len =
        QATQ_EXACT_PREDICTOR_METADATA_LEN + quantized_len + residual_sign_len + residuals.len();
    if predictor_payload_len < best_payload_len {
        best_payload_len = predictor_payload_len;
        best_strategy = QATQ_EXACT_STRATEGY_PREDICTOR_XOR;
        best_payload = None;
    }

    let body_len = if best_strategy == QATQ_EXACT_STRATEGY_RAW_BITS {
        raw_body_len
    } else {
        QATQ_EXACT_PREFIX_LEN + best_payload_len
    };
    let mut out = Vec::with_capacity(HEADER_LEN + body_len);
    write_header(
        &mut out,
        CodecMode::QatqExact,
        values.len(),
        if best_strategy == QATQ_EXACT_STRATEGY_PREDICTOR_XOR {
            parts.scale
        } else {
            1.0
        },
        checksum,
    );
    write_qatq_exact_prefix(&mut out, best_strategy);
    if best_strategy == QATQ_EXACT_STRATEGY_PREDICTOR_XOR {
        out.extend_from_slice(&parts.seed.to_be_bytes());
        out.extend_from_slice(&parts.residual_scale.to_bits().to_be_bytes());
        pack_i4_nibbles(&parts.quantized, &mut out);
        pack_residual_signs(&parts.residual_signs, &mut out);
        out.extend_from_slice(&residuals);
    } else if let Some(payload) = best_payload {
        out.extend_from_slice(&payload);
    } else {
        for value in values {
            out.extend_from_slice(&value.to_bits().to_be_bytes());
        }
    }
    out
}

fn encode_qatq_exact_fast(values: &[f32], config: Phase1Config) -> Vec<u8> {
    if values.is_empty() {
        let mut out = Vec::with_capacity(HEADER_LEN + QATQ_EXACT_PREFIX_LEN);
        write_header(
            &mut out,
            CodecMode::QatqExact,
            0,
            1.0,
            fast_paths::checksum_f32_bits_fast(values),
        );
        write_qatq_exact_prefix(&mut out, QATQ_EXACT_STRATEGY_RAW_BITS);
        return out;
    }

    let (planes_t0, checksum) = fast_paths::transpose_f32_to_be_planes_and_checksum(values);
    let raw_len = planes_t0.len();
    let raw_body_len = QATQ_EXACT_PREFIX_LEN + raw_len;

    let mut best_strategy = QATQ_EXACT_STRATEGY_RAW_BITS;
    let mut best_payload: Option<Vec<u8>> = None;
    let mut best_payload_len = raw_len;

    if let Some(blocks) =
        encode_byte_plane_blocks_from_planes_bounded(&planes_t0, values.len(), raw_len)
        && blocks.len() < best_payload_len
    {
        best_payload_len = blocks.len();
        best_strategy = QATQ_EXACT_STRATEGY_BYTE_PLANE_BLOCKS;
        best_payload = Some(blocks);
    }

    let try_rle = values.len() <= 2_048 || fast_paths::sample_has_rle_potential(&planes_t0, 20);
    if try_rle {
        if let Some(byte_rle) =
            encode_byte_runs_from_f32_bounded(values, best_payload_len.saturating_sub(1))
        {
            best_payload_len = byte_rle.len();
            best_strategy = QATQ_EXACT_STRATEGY_BYTE_RLE;
            best_payload = Some(byte_rle);
        }
        if let Some(byte_plane) = encode_contiguous_byte_plane_runs_bounded(
            &planes_t0,
            best_payload_len.saturating_sub(1),
        ) {
            best_payload_len = byte_plane.len();
            best_strategy = QATQ_EXACT_STRATEGY_BYTE_PLANE_RLE;
            best_payload = Some(byte_plane);
        }
        if let Some(byte_plane_packed) = encode_contiguous_byte_plane_packed_runs_bounded(
            &planes_t0,
            best_payload_len.saturating_sub(1),
        ) {
            best_payload_len = byte_plane_packed.len();
            best_strategy = QATQ_EXACT_STRATEGY_BYTE_PLANE_PACKED_RLE;
            best_payload = Some(byte_plane_packed);
        }
    }

    if fast_paths::sample_has_delta_xor_rle_potential(values)
        && let Some(delta_xor) =
            encode_delta_xor_byte_plane_runs_bounded(values, best_payload_len.saturating_sub(1))
    {
        best_payload_len = delta_xor.len();
        best_strategy = QATQ_EXACT_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE;
        best_payload = Some(delta_xor);
    }

    if let Some(z_payload) = encode_byte_plane_zstd_flushed_bounded(
        &planes_t0,
        values.len(),
        4,
        best_payload_len.saturating_sub(1),
    ) {
        best_payload_len = z_payload.len();
        best_strategy = QATQ_EXACT_STRATEGY_BYTE_PLANE_ZSTD;
        best_payload = Some(z_payload);
    }

    if values.len() <= 2_048 {
        for level in [3, 1] {
            if let Some(qc_zstd) = encode_quaternion_chain_zstd_level_bounded(
                values,
                level,
                best_payload_len.saturating_sub(1),
            ) {
                best_payload_len = qc_zstd.len();
                best_strategy = QATQ_EXACT_STRATEGY_QUATERNION_CHAIN_ZSTD;
                best_payload = Some(qc_zstd);
            }
        }
    } else if quaternion_chain_wins_sample(values, &planes_t0)
        && let Some(qc_zstd) = encode_quaternion_chain_zstd_level_bounded(
            values,
            3,
            best_payload_len.saturating_sub(1),
        )
    {
        best_payload_len = qc_zstd.len();
        best_strategy = QATQ_EXACT_STRATEGY_QUATERNION_CHAIN_ZSTD;
        best_payload = Some(qc_zstd);
    }

    let mut predictor_parts: Option<PhaseParts> = None;
    let mut predictor_residuals: Option<Vec<u8>> = None;
    if values.len() <= 2_048 {
        let coord_count = phase1_coordinate_count(values.len());
        let min_predictor_len =
            QATQ_EXACT_PREDICTOR_METADATA_LEN + coord_count.div_ceil(2) + coord_count.div_ceil(8);
        if min_predictor_len < best_payload_len {
            let parts = build_phase1_parts(values, config);
            let predicted = reconstruct_phase1_values(values.len(), &parts);
            let residuals = encode_xor_residuals(values, &predicted);
            let predictor_len = min_predictor_len + residuals.len();
            if predictor_len < best_payload_len {
                best_payload_len = predictor_len;
                best_strategy = QATQ_EXACT_STRATEGY_PREDICTOR_XOR;
                best_payload = None;
                predictor_parts = Some(parts);
                predictor_residuals = Some(residuals);
            }
        }
    }

    let body_len = if best_strategy == QATQ_EXACT_STRATEGY_RAW_BITS {
        raw_body_len
    } else {
        QATQ_EXACT_PREFIX_LEN + best_payload_len
    };
    let mut out = Vec::with_capacity(HEADER_LEN + body_len);
    write_header(
        &mut out,
        CodecMode::QatqExact,
        values.len(),
        if let Some(parts) = predictor_parts.as_ref() {
            parts.scale
        } else {
            1.0
        },
        checksum,
    );
    write_qatq_exact_prefix(&mut out, best_strategy);
    if let (Some(parts), Some(residuals)) = (predictor_parts.as_ref(), predictor_residuals.as_ref())
    {
        out.extend_from_slice(&parts.seed.to_be_bytes());
        out.extend_from_slice(&parts.residual_scale.to_bits().to_be_bytes());
        pack_i4_nibbles(&parts.quantized, &mut out);
        pack_residual_signs(&parts.residual_signs, &mut out);
        out.extend_from_slice(residuals);
    } else if let Some(payload) = best_payload {
        out.extend_from_slice(&payload);
    } else {
        for value in values {
            out.extend_from_slice(&value.to_bits().to_be_bytes());
        }
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

/// Encodes opaque bytes through the exact QATC format.
///
/// Bytes are packed little-endian into opaque `u32` words. A final partial
/// word is padded with zeroes. The decoded byte length therefore belongs to
/// the surrounding protocol and must be supplied to the decode APIs.
/// The resulting bytes are identical to [`encode_qatq_exact_u32_container`]
/// after applying this canonical packing.
pub fn encode_qatq_exact_bytes_container(
    bytes: &[u8],
    max_words_per_chunk: usize,
) -> Result<Vec<u8>, QatqError> {
    let word_count = bytes.len().div_ceil(4);
    let mut words = Vec::new();
    words
        .try_reserve_exact(word_count)
        .map_err(|_| QatqError::InvalidContainer)?;
    for chunk in bytes.chunks(4) {
        let mut word = [0_u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        words.push(u32::from_le_bytes(word));
    }
    encode_qatq_exact_u32_container(&words, max_words_per_chunk)
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

fn inspect_and_index_container_with_limits<'a>(
    payload: &'a [u8],
    limits: QatcDecodeLimits,
    max_values_per_chunk: usize,
) -> Result<(&'a [u8], Vec<(usize, usize)>, QatcExactContainerMetadata), QatqError> {
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

    Ok((
        body,
        index,
        QatcExactContainerMetadata {
            encoded_bytes: payload.len(),
            total_values: header.total_values,
            chunks,
        },
    ))
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
    let (_body, _index, metadata) =
        inspect_and_index_container_with_limits(payload, limits, max_values_per_chunk)?;
    Ok(metadata)
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
    let (body, index, metadata) =
        inspect_and_index_container_with_limits(payload, limits, max_words_per_chunk)?;
    let mut words = Vec::new();
    words
        .try_reserve_exact(metadata.total_values)
        .map_err(|_| QatqError::ContainerLimitExceeded("allocation"))?;
    for (chunk_start, chunk_end) in index {
        for value in decode_qatq_exact(&body[chunk_start..chunk_end])? {
            words.push(value.to_bits());
        }
    }
    if words.len() != metadata.total_values {
        return Err(QatqError::InvalidContainer);
    }
    Ok(words)
}

/// Decodes an exact QATC container as opaque bytes.
///
/// `decoded_bytes` must come from an authenticated surrounding protocol. It
/// disambiguates the zero padding in the final packed word.
pub fn decode_qatq_exact_bytes_container(
    payload: &[u8],
    decoded_bytes: usize,
    max_words_per_chunk: usize,
) -> Result<Vec<u8>, QatqError> {
    decode_qatq_exact_bytes_container_with_limits(
        payload,
        decoded_bytes,
        QatcDecodeLimits::default(),
        max_words_per_chunk,
    )
}

/// Decodes an exact QATC container as opaque bytes under explicit aggregate
/// limits and rejects non-canonical non-zero padding.
pub fn decode_qatq_exact_bytes_container_with_limits(
    payload: &[u8],
    decoded_bytes: usize,
    limits: QatcDecodeLimits,
    max_words_per_chunk: usize,
) -> Result<Vec<u8>, QatqError> {
    preflight_qatq_exact_decoded_bytes(decoded_bytes, limits)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(decoded_bytes)
        .map_err(|_| QatqError::ContainerLimitExceeded("allocation"))?;
    for_each_qatq_exact_bytes_container_chunk_with_limits(
        payload,
        decoded_bytes,
        limits,
        max_words_per_chunk,
        |chunk| {
            bytes.extend_from_slice(chunk);
            Ok(())
        },
    )?;
    Ok(bytes)
}

/// Visits an exact QATC container as bounded chunks of opaque 32-bit words.
///
/// All container framing, aggregate limits, checksum, and canonical chunk
/// counts are validated before the first callback. Only one decoded chunk is
/// materialised at a time.
pub fn for_each_qatq_exact_u32_container_chunk(
    payload: &[u8],
    max_words_per_chunk: usize,
    visitor: impl FnMut(&[u32]) -> Result<(), QatqError>,
) -> Result<QatcExactContainerMetadata, QatqError> {
    for_each_qatq_exact_u32_container_chunk_with_limits(
        payload,
        QatcDecodeLimits::default(),
        max_words_per_chunk,
        visitor,
    )
}

/// Visits an exact QATC container as bounded chunks of opaque 32-bit words
/// under explicit aggregate decode limits.
pub fn for_each_qatq_exact_u32_container_chunk_with_limits(
    payload: &[u8],
    limits: QatcDecodeLimits,
    max_words_per_chunk: usize,
    mut visitor: impl FnMut(&[u32]) -> Result<(), QatqError>,
) -> Result<QatcExactContainerMetadata, QatqError> {
    let (body, index, metadata) =
        inspect_and_index_container_with_limits(payload, limits, max_words_per_chunk)?;
    let mut words = Vec::new();
    for (chunk_start, chunk_end) in index {
        let values = decode_qatq_exact(&body[chunk_start..chunk_end])?;
        words.clear();
        words.reserve(values.len());
        for value in values {
            words.push(value.to_bits());
        }
        visitor(&words)?;
    }
    Ok(metadata)
}

/// Visits an exact QATC container as bounded chunks of opaque bytes.
pub fn for_each_qatq_exact_bytes_container_chunk(
    payload: &[u8],
    decoded_bytes: usize,
    max_words_per_chunk: usize,
    visitor: impl FnMut(&[u8]) -> Result<(), QatqError>,
) -> Result<QatcExactContainerMetadata, QatqError> {
    for_each_qatq_exact_bytes_container_chunk_with_limits(
        payload,
        decoded_bytes,
        QatcDecodeLimits::default(),
        max_words_per_chunk,
        visitor,
    )
}

/// Visits an exact QATC container as bounded chunks of opaque bytes under
/// explicit aggregate limits.
///
/// Complete framing, decoded length, checksum, canonical chunk counts and
/// final-word padding are validated before the first callback.
pub fn for_each_qatq_exact_bytes_container_chunk_with_limits(
    payload: &[u8],
    decoded_bytes: usize,
    limits: QatcDecodeLimits,
    max_words_per_chunk: usize,
    mut visitor: impl FnMut(&[u8]) -> Result<(), QatqError>,
) -> Result<QatcExactContainerMetadata, QatqError> {
    let expected_words = preflight_qatq_exact_decoded_bytes(decoded_bytes, limits)?;
    let (body, index, metadata) =
        inspect_and_index_container_with_limits(payload, limits, max_words_per_chunk)?;
    if metadata.total_values != expected_words {
        return Err(QatqError::InvalidContainer);
    }

    let mut single_chunk_final: Option<Vec<f32>> = None;
    if let Some(&(final_start, final_end)) = index.last() {
        let final_values = decode_qatq_exact(&body[final_start..final_end])?;
        if let Some(final_val) = final_values.last() {
            let used = decoded_bytes % 4;
            if used != 0
                && final_val.to_bits().to_le_bytes()[used..]
                    .iter()
                    .any(|byte| *byte != 0)
            {
                return Err(QatqError::InvalidContainer);
            }
        }
        if index.len() == 1 {
            single_chunk_final = Some(final_values);
        }
    }

    let mut remaining = decoded_bytes;
    let mut decoded = Vec::new();
    for (pos, (chunk_start, chunk_end)) in index.into_iter().enumerate() {
        let words = if pos == 0 {
            if let Some(cached) = single_chunk_final.take() {
                cached
            } else {
                decode_qatq_exact(&body[chunk_start..chunk_end])?
            }
        } else {
            decode_qatq_exact(&body[chunk_start..chunk_end])?
        };
        let byte_count = remaining.min(words.len().saturating_mul(4));
        decoded.clear();
        decoded
            .try_reserve_exact(byte_count)
            .map_err(|_| QatqError::ContainerLimitExceeded("allocation"))?;
        for value in words {
            let take = remaining.min(4);
            decoded.extend_from_slice(&value.to_bits().to_le_bytes()[..take]);
            remaining -= take;
        }
        visitor(&decoded)?;
    }
    if remaining != 0 {
        return Err(QatqError::InvalidContainer);
    }
    Ok(metadata)
}

fn preflight_qatq_exact_decoded_bytes(
    decoded_bytes: usize,
    limits: QatcDecodeLimits,
) -> Result<usize, QatqError> {
    let expected_words = decoded_bytes
        .checked_add(3)
        .ok_or(QatqError::ContainerLimitExceeded("total values"))?
        / 4;
    if expected_words > limits.max_total_values {
        return Err(QatqError::ContainerLimitExceeded("total values"));
    }
    Ok(expected_words)
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
    let (strategy_id, dtype) = parse_qatq_exact_prefix(body)?;
    if dtype != TensorDType::F32 {
        return Err(QatqError::InvalidQatqExactBody);
    }
    let strategy_body = &body[QATQ_EXACT_PREFIX_LEN..];
    let (values, actual) = match strategy_id {
        QATQ_EXACT_STRATEGY_RAW_BITS => {
            decode_qatq_exact_raw_bits_and_checksum(strategy_body, &header)?
        }
        QATQ_EXACT_STRATEGY_BYTE_PLANE_BLOCKS => {
            let expected_len = checked_value_byte_len(header.value_count)?;
            decode_byte_plane_blocks_to_f32_and_checksum(
                strategy_body,
                expected_len,
                header.value_count,
            )?
        }
        QATQ_EXACT_STRATEGY_BYTE_PLANE_ZSTD => {
            decode_qatq_exact_byte_plane_zstd_and_checksum(strategy_body, &header)?
        }
        QATQ_EXACT_STRATEGY_QUATERNION_CHAIN_ZSTD => {
            decode_qatq_exact_quaternion_chain_zstd_and_checksum(strategy_body, &header)?
        }
        QATQ_EXACT_STRATEGY_BIT_SPLIT_PLANE_ZSTD => {
            decode_bit_split_plane_zstd_f32_and_checksum(strategy_body, header.value_count)?
        }
        QATQ_EXACT_STRATEGY_BYTE_RLE => {
            let v = decode_qatq_exact_byte_rle(strategy_body, &header)?;
            let c = checksum_f32_bits(&v);
            (v, c)
        }
        QATQ_EXACT_STRATEGY_BYTE_PLANE_RLE => {
            let v = decode_qatq_exact_byte_plane_rle(strategy_body, &header)?;
            let c = checksum_f32_bits(&v);
            (v, c)
        }
        QATQ_EXACT_STRATEGY_BYTE_PLANE_PACKED_RLE => {
            let v = decode_qatq_exact_byte_plane_packed_rle(strategy_body, &header)?;
            let c = checksum_f32_bits(&v);
            (v, c)
        }
        QATQ_EXACT_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE => {
            let v = decode_qatq_exact_delta_xor_byte_plane_rle(strategy_body, &header)?;
            let c = checksum_f32_bits(&v);
            (v, c)
        }
        QATQ_EXACT_STRATEGY_PREDICTOR_XOR => {
            let v = decode_qatq_exact_predictor_xor(strategy_body, &header)?;
            let c = checksum_f32_bits(&v);
            (v, c)
        }
        _ => return Err(QatqError::InvalidQatqExactBody),
    };

    if actual != header.checksum {
        return Err(QatqError::ChecksumMismatch {
            expected: header.checksum,
            actual,
        });
    }

    Ok(values)
}

/// Identifies the public encoding strategy for an exact payload.
/// Internal decode-only strategies return `QatqError::InvalidQatqExactBody`.
pub fn qatq_exact_strategy(payload: &[u8]) -> Result<QatqExactStrategy, QatqError> {
    let _header = Header::parse_for_mode(payload, CodecMode::QatqExact)?;
    let (strategy_id, _) = parse_qatq_exact_prefix(&payload[HEADER_LEN..])?;
    QatqExactStrategy::from_id(strategy_id)
}

fn parse_qatq_exact_prefix(body: &[u8]) -> Result<(u8, TensorDType), QatqError> {
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
    Ok((body[4], dtype))
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

fn decode_qatq_exact_raw_bits_and_checksum(
    body: &[u8],
    header: &Header,
) -> Result<(Vec<f32>, u64), QatqError> {
    let expected_len = checked_value_byte_len(header.value_count)?;
    if body.len() != expected_len {
        return Err(QatqError::LengthMismatch {
            expected: expected_len,
            actual: body.len(),
        });
    }
    Ok(fast_paths::bytes_be_to_f32_and_checksum(body))
}

fn decode_qatq_exact_tensor_raw_bits_le(
    body: &[u8],
    expected_len: usize,
) -> Result<(Vec<u8>, u64), QatqError> {
    if body.len() != expected_len {
        return Err(QatqError::LengthMismatch {
            expected: expected_len,
            actual: body.len(),
        });
    }
    Ok(fast_paths::swap_be_to_u16_le_and_checksum(body))
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

fn decode_qatq_exact_byte_plane_packed_rle(
    body: &[u8],
    header: &Header,
) -> Result<Vec<f32>, QatqError> {
    let expected_len = checked_value_byte_len(header.value_count)?;
    let words = decode_byte_plane_packed_runs_to_words(body, expected_len, header.value_count)?;
    Ok(words.into_iter().map(f32::from_bits).collect())
}

fn decode_qatq_exact_byte_plane_zstd_and_checksum(
    body: &[u8],
    header: &Header,
) -> Result<(Vec<f32>, u64), QatqError> {
    let expected_len = checked_value_byte_len(header.value_count)?;
    let plane_bytes = fast_paths::zstd_decompress(body, expected_len)
        .map_err(|_| QatqError::InvalidResidualStream)?;
    if plane_bytes.len() != expected_len {
        return Err(QatqError::InvalidResidualStream);
    }
    let n = header.value_count;
    let (p0, rest) = plane_bytes.split_at(n);
    let (p1, rest) = rest.split_at(n);
    let (p2, p3) = rest.split_at(n);
    Ok(fast_paths::untranspose_be_planes_to_f32_and_checksum(
        p0, p1, p2, p3, false,
    ))
}

fn decode_qatq_exact_quaternion_chain_zstd_and_checksum(
    body: &[u8],
    header: &Header,
) -> Result<(Vec<f32>, u64), QatqError> {
    let expected_len = checked_value_byte_len(header.value_count)?;
    let plane_bytes = fast_paths::zstd_decompress(body, expected_len)
        .map_err(|_| QatqError::InvalidResidualStream)?;
    if plane_bytes.len() != expected_len {
        return Err(QatqError::InvalidResidualStream);
    }
    let n = header.value_count;
    let (p0, rest) = plane_bytes.split_at(n);
    let (p1, rest) = rest.split_at(n);
    let (p2, p3) = rest.split_at(n);
    let mut previous = 0_u32;
    let mut values = Vec::with_capacity(n);
    for i in 0..n {
        let residual = u32::from_be_bytes([p0[i], p1[i], p2[i], p3[i]]);
        let bits = previous.wrapping_add(residual);
        values.push(f32::from_bits(bits));
        previous = bits;
    }
    let checksum = checksum_f32_bits(&values);
    Ok((values, checksum))
}

fn decode_adjacent_xor_u16_byte_planes_to_le(
    planes: &[u8],
    expected_len: usize,
    element_count: usize,
) -> Result<(Vec<u8>, u64), QatqError> {
    if planes.len() != expected_len || expected_len != element_count * 2 {
        return Err(QatqError::InvalidResidualStream);
    }
    let (p0, p1) = planes.split_at(element_count);
    let mut out = vec![0_u8; expected_len];
    let mut previous = 0_u16;
    for index in 0..element_count {
        let residual = u16::from_be_bytes([p0[index], p1[index]]);
        let word = previous ^ residual;
        let [lo, hi] = word.to_le_bytes();
        out[index * 2] = lo;
        out[index * 2 + 1] = hi;
        previous = word;
    }
    let checksum = fast_paths::checksum_bytes_fast(&out);
    Ok((out, checksum))
}

fn decode_strided_xor_u16_byte_planes_to_le(
    planes: &[u8],
    expected_len: usize,
    element_count: usize,
    stride: usize,
) -> Result<(Vec<u8>, u64), QatqError> {
    if planes.len() != expected_len
        || expected_len != element_count * 2
        || stride == 0
        || stride >= element_count
    {
        return Err(QatqError::InvalidResidualStream);
    }
    let (p0, p1) = planes.split_at(element_count);
    let mut out = vec![0_u8; expected_len];
    for index in 0..element_count {
        let residual = u16::from_be_bytes([p0[index], p1[index]]);
        let predictor = if index < stride {
            0
        } else {
            let predictor_offset = (index - stride) * 2;
            u16::from_le_bytes([out[predictor_offset], out[predictor_offset + 1]])
        };
        let [lo, hi] = (predictor ^ residual).to_le_bytes();
        out[index * 2] = lo;
        out[index * 2 + 1] = hi;
    }
    let checksum = fast_paths::checksum_bytes_fast(&out);
    Ok((out, checksum))
}

fn decode_byte_runs_to_u16_le(
    bytes: &[u8],
    expected_len: usize,
) -> Result<(Vec<u8>, u64), QatqError> {
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
            XOR_ZERO_RUN => decoded_len += len,
            XOR_RAW_RUN => {
                if offset + len > bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                for &byte in &bytes[offset..offset + len] {
                    if byte != 0 {
                        out[decoded_len ^ 1] = byte;
                    }
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
                if value != 0 {
                    for pos in decoded_len..decoded_len + len {
                        out[pos ^ 1] = value;
                    }
                }
                decoded_len += len;
            }
            _ => return Err(QatqError::InvalidResidualStream),
        }
    }
    if offset != bytes.len() {
        return Err(QatqError::InvalidResidualStream);
    }
    let checksum = fast_paths::checksum_bytes_fast(&out);
    Ok((out, checksum))
}

fn decode_byte_plane_runs_to_u16_le(
    bytes: &[u8],
    expected_len: usize,
    element_count: usize,
) -> Result<(Vec<u8>, u64), QatqError> {
    if expected_len != element_count * 2 {
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
            XOR_ZERO_RUN => decoded_len += len,
            XOR_RAW_RUN => {
                if offset + len > bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                for &byte in &bytes[offset..offset + len] {
                    if byte != 0 {
                        let dst = if decoded_len < element_count {
                            decoded_len * 2 + 1
                        } else {
                            (decoded_len - element_count) * 2
                        };
                        out[dst] = byte;
                    }
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
                if value != 0 {
                    for pos in decoded_len..decoded_len + len {
                        let dst = if pos < element_count {
                            pos * 2 + 1
                        } else {
                            (pos - element_count) * 2
                        };
                        out[dst] = value;
                    }
                }
                decoded_len += len;
            }
            _ => return Err(QatqError::InvalidResidualStream),
        }
    }
    if offset != bytes.len() {
        return Err(QatqError::InvalidResidualStream);
    }
    let checksum = fast_paths::checksum_bytes_fast(&out);
    Ok((out, checksum))
}

fn decode_byte_plane_packed_runs_to_u16_le(
    bytes: &[u8],
    expected_len: usize,
    element_count: usize,
) -> Result<(Vec<u8>, u64), QatqError> {
    if expected_len != element_count * 2 {
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
            PACKED_ZERO_RUN => decoded_len += len,
            PACKED_RAW_RUN => {
                if offset + len > bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                for &byte in &bytes[offset..offset + len] {
                    if byte != 0 {
                        let dst = if decoded_len < element_count {
                            decoded_len * 2 + 1
                        } else {
                            (decoded_len - element_count) * 2
                        };
                        out[dst] = byte;
                    }
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
                if value != 0 {
                    for pos in decoded_len..decoded_len + len {
                        let dst = if pos < element_count {
                            pos * 2 + 1
                        } else {
                            (pos - element_count) * 2
                        };
                        out[dst] = value;
                    }
                }
                decoded_len += len;
            }
            _ => return Err(QatqError::InvalidResidualStream),
        }
    }
    if offset != bytes.len() {
        return Err(QatqError::InvalidResidualStream);
    }
    let checksum = fast_paths::checksum_bytes_fast(&out);
    Ok((out, checksum))
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
    let mut plane = 0_usize;
    let mut value_index = 0_usize;
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
                value_index += len;
                while value_index >= value_count {
                    value_index -= value_count;
                    plane += 1;
                }
            }
            PACKED_RAW_RUN => {
                if offset + len > bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                for &byte in &bytes[offset..offset + len] {
                    if byte != 0 {
                        words[value_index] |= (byte as u32) << ((3 - plane) * 8);
                    }
                    value_index += 1;
                    if value_index == value_count {
                        value_index = 0;
                        plane += 1;
                    }
                }
                decoded_len += len;
                offset += len;
            }
            PACKED_REPEAT_RUN => {
                if offset >= bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                let value = bytes[offset];
                offset += 1;
                if value != 0 {
                    for _ in 0..len {
                        words[value_index] |= (value as u32) << ((3 - plane) * 8);
                        value_index += 1;
                        if value_index == value_count {
                            value_index = 0;
                            plane += 1;
                        }
                    }
                } else {
                    value_index += len;
                    while value_index >= value_count {
                        value_index -= value_count;
                        plane += 1;
                    }
                }
                decoded_len += len;
            }
            _ => return Err(QatqError::InvalidResidualStream),
        }
    }
    if offset != bytes.len() {
        return Err(QatqError::InvalidResidualStream);
    }
    Ok(words)
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
    let mut plane = 0_usize;
    let mut value_index = 0_usize;
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
                value_index += len;
                while value_index >= value_count {
                    value_index -= value_count;
                    plane += 1;
                }
            }
            XOR_RAW_RUN => {
                if offset + len > bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                for &byte in &bytes[offset..offset + len] {
                    if byte != 0 {
                        words[value_index] |= (byte as u32) << ((3 - plane) * 8);
                    }
                    value_index += 1;
                    if value_index == value_count {
                        value_index = 0;
                        plane += 1;
                    }
                }
                decoded_len += len;
                offset += len;
            }
            BYTE_REPEAT_RUN => {
                if offset >= bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                let value = bytes[offset];
                offset += 1;
                if value != 0 {
                    for _ in 0..len {
                        words[value_index] |= (value as u32) << ((3 - plane) * 8);
                        value_index += 1;
                        if value_index == value_count {
                            value_index = 0;
                            plane += 1;
                        }
                    }
                } else {
                    value_index += len;
                    while value_index >= value_count {
                        value_index -= value_count;
                        plane += 1;
                    }
                }
                decoded_len += len;
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

#[inline]
fn has_repeat_4(bytes: &[u8], index: usize) -> bool {
    let v = bytes[index];
    index + 3 < bytes.len()
        && bytes[index + 1] == v
        && bytes[index + 2] == v
        && bytes[index + 3] == v
}

#[inline]
fn has_repeat_3(bytes: &[u8], index: usize) -> bool {
    let v = bytes[index];
    index + 2 < bytes.len() && bytes[index + 1] == v && bytes[index + 2] == v
}

fn encode_contiguous_byte_plane_runs_bounded(
    plane_bytes: &[u8],
    max_encoded_len: usize,
) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < plane_bytes.len() {
        let byte = plane_bytes[index];
        if byte == 0 {
            let start = index;
            index += 1;
            while index < plane_bytes.len()
                && plane_bytes[index] == 0
                && index - start < u16::MAX as usize
            {
                index += 1;
            }
            write_xor_run_header(&mut out, XOR_ZERO_RUN, index - start);
            if out.len() > max_encoded_len {
                return None;
            }
        } else if has_repeat_4(plane_bytes, index) {
            let value = byte;
            let start = index;
            index += 4;
            while index < plane_bytes.len()
                && plane_bytes[index] == value
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
            while index < plane_bytes.len()
                && plane_bytes[index] != 0
                && !has_repeat_4(plane_bytes, index)
                && index - start < u16::MAX as usize
            {
                index += 1;
            }
            write_xor_run_header(&mut out, XOR_RAW_RUN, index - start);
            out.extend_from_slice(&plane_bytes[start..index]);
            if out.len() > max_encoded_len {
                return None;
            }
        }
    }
    Some(out)
}

fn encode_contiguous_byte_plane_packed_runs_bounded(
    plane_bytes: &[u8],
    max_encoded_len: usize,
) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < plane_bytes.len() {
        let byte = plane_bytes[index];
        if byte == 0 {
            let start = index;
            index += 1;
            while index < plane_bytes.len()
                && plane_bytes[index] == 0
                && index - start < PACKED_RUN_MAX_LEN
            {
                index += 1;
            }
            push_packed_run_header(&mut out, PACKED_ZERO_RUN, index - start);
        } else if has_repeat_3(plane_bytes, index) {
            let value = byte;
            let start = index;
            index += 3;
            while index < plane_bytes.len()
                && plane_bytes[index] == value
                && index - start < PACKED_RUN_MAX_LEN
            {
                index += 1;
            }
            push_packed_run_header(&mut out, PACKED_REPEAT_RUN, index - start);
            out.push(value);
        } else {
            let start = index;
            index += 1;
            while index < plane_bytes.len()
                && plane_bytes[index] != 0
                && !has_repeat_3(plane_bytes, index)
                && index - start < PACKED_RUN_MAX_LEN
            {
                index += 1;
            }
            push_packed_run_header(&mut out, PACKED_RAW_RUN, index - start);
            out.extend_from_slice(&plane_bytes[start..index]);
        }
        if out.len() > max_encoded_len {
            return None;
        }
    }
    Some(out)
}

fn encode_byte_runs_from_f32_bounded(values: &[f32], max_encoded_len: usize) -> Option<Vec<u8>> {
    let mut raw_be = Vec::with_capacity(values.len() * 4);
    for value in values {
        raw_be.extend_from_slice(&value.to_bits().to_be_bytes());
    }
    encode_byte_runs_bounded(&raw_be, max_encoded_len)
}

fn encode_byte_runs_be_pairs_bounded(bytes_le: &[u8], max_encoded_len: usize) -> Option<Vec<u8>> {
    let mut canonical = Vec::with_capacity(bytes_le.len());
    for pair in bytes_le.chunks_exact(2) {
        canonical.push(pair[1]);
        canonical.push(pair[0]);
    }
    encode_byte_runs_bounded(&canonical, max_encoded_len)
}

#[cfg(test)]
fn encode_byte_plane_runs_bounded(bytes: &[u8], max_encoded_len: usize) -> Option<Vec<u8>> {
    debug_assert_eq!(bytes.len() % 4, 0);
    let mut planes = Vec::with_capacity(bytes.len());
    let value_count = bytes.len() / 4;
    for plane in 0..4 {
        for value_index in 0..value_count {
            planes.push(bytes[value_index * 4 + plane]);
        }
    }
    encode_contiguous_byte_plane_runs_bounded(&planes, max_encoded_len)
}

fn encode_zstd_level_bounded(bytes: &[u8], level: i32, max_encoded_len: usize) -> Option<Vec<u8>> {
    let encoded = fast_paths::zstd_compress(bytes, level).ok()?;
    (encoded.len() <= max_encoded_len).then_some(encoded)
}

fn encode_zstd_bounded(bytes: &[u8], max_encoded_len: usize) -> Option<Vec<u8>> {
    encode_zstd_level_bounded(bytes, 3, max_encoded_len)
}

fn encode_quaternion_chain_zstd_level_bounded(
    values: &[f32],
    level: i32,
    max_encoded_len: usize,
) -> Option<Vec<u8>> {
    let transformed = encode_quaternion_chain_words(values);
    let plane_bytes = words_to_byte_planes_be(&transformed);
    let encoded = fast_paths::zstd_compress(&plane_bytes, level).ok()?;
    if encoded.len() > max_encoded_len {
        return None;
    }
    Some(encoded)
}

fn encode_byte_plane_zstd_flushed_bounded(
    planes_t0: &[u8],
    element_count: usize,
    width: usize,
    max_encoded_len: usize,
) -> Option<Vec<u8>> {
    if element_count == 0 || max_encoded_len == 0 {
        return None;
    }
    let raw_len = planes_t0.len();
    if raw_len < 1_024 {
        let mut best: Option<Vec<u8>> = None;
        let mut limit = max_encoded_len;
        for level in [3, 1] {
            if let Some(flushed) = fast_paths::zstd_compress_planes_flushed_bounded(
                planes_t0,
                element_count,
                width,
                level,
                limit,
            ) {
                limit = flushed.len().saturating_sub(1);
                best = Some(flushed);
            }
            if let Some(comp) = encode_zstd_level_bounded(planes_t0, level, limit) {
                limit = comp.len().saturating_sub(1);
                best = Some(comp);
            }
        }
        return best;
    }
    let mut best = fast_paths::zstd_compress_planes_flushed_bounded(
        planes_t0,
        element_count,
        width,
        1,
        max_encoded_len,
    );
    let best_len = best.as_ref().map_or(max_encoded_len, Vec::len);
    if raw_len <= 262_144 && best_len < raw_len * 3 / 5 {
        let l3_limit = best
            .as_ref()
            .map_or(max_encoded_len, |b| b.len().saturating_sub(1));
        if l3_limit > 0
            && let Some(l3) = fast_paths::zstd_compress_planes_flushed_bounded(
                planes_t0,
                element_count,
                width,
                3,
                l3_limit,
            )
        {
            best = Some(l3);
        }
        if let Some(current) = best.as_ref()
            && ((raw_len <= 8_192 && current.len() < raw_len / 2) || current.len() < raw_len / 8)
        {
            let mut limit = current.len().saturating_sub(1);
            for level in [3, 1] {
                if let Some(unsplit) = encode_zstd_level_bounded(planes_t0, level, limit) {
                    limit = unsplit.len().saturating_sub(1);
                    best = Some(unsplit);
                }
            }
        }
    }
    best
}

fn quaternion_chain_wins_sample(values: &[f32], planes_t0: &[u8]) -> bool {
    let n = values.len();
    if n < 256 {
        return true;
    }
    let (blocks, block_len) = if n <= 2_048 {
        (8_usize, 16_usize)
    } else {
        (16_usize, 32_usize)
    };
    let sample_n = blocks * block_len;
    let mut raw_sample = vec![0_u8; sample_n * 4];
    let mut chain_sample = vec![0_u8; sample_n * 4];
    let mut out_idx = 0_usize;
    for block in 0..blocks {
        let start = block * n / blocks;
        let end = (start + block_len).min(n);
        let mut prev = if start == 0 {
            0_u32
        } else {
            values[start - 1].to_bits()
        };
        for i in start..end {
            let bits = values[i].to_bits();
            let delta = bits.wrapping_sub(prev);
            prev = bits;
            let [d0, d1, d2, d3] = delta.to_be_bytes();
            chain_sample[out_idx] = d0;
            chain_sample[sample_n + out_idx] = d1;
            chain_sample[sample_n * 2 + out_idx] = d2;
            chain_sample[sample_n * 3 + out_idx] = d3;
            raw_sample[out_idx] = planes_t0[i];
            raw_sample[sample_n + out_idx] = planes_t0[n + i];
            raw_sample[sample_n * 2 + out_idx] = planes_t0[n * 2 + i];
            raw_sample[sample_n * 3 + out_idx] = planes_t0[n * 3 + i];
            out_idx += 1;
        }
    }
    let Ok(raw_len) = fast_paths::zstd_compress(&raw_sample, 1).map(|b| b.len()) else {
        return false;
    };
    let Ok(chain_len) = fast_paths::zstd_compress(&chain_sample, 1).map(|b| b.len()) else {
        return false;
    };
    chain_len < raw_len
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
    let n = words.len();
    let mut out = vec![0_u8; n * 4];
    let (p0, rest) = out.split_at_mut(n);
    let (p1, rest) = rest.split_at_mut(n);
    let (p2, p3) = rest.split_at_mut(n);
    for (i, &word) in words.iter().enumerate() {
        let [b0, b1, b2, b3] = word.to_be_bytes();
        p0[i] = b0;
        p1[i] = b1;
        p2[i] = b2;
        p3[i] = b3;
    }
    out
}

#[cfg(test)]
fn build_bit_split_payload_from_plane_slices(
    planes: &[&[u8]],
    modes: &[u8],
    transform: u8,
    level: i32,
    max_encoded_len: usize,
) -> Option<Vec<u8>> {
    let width = planes.len();
    let n = planes.first().map_or(0, |plane| plane.len());
    if n == 0 {
        return None;
    }
    for (p, &mode) in modes.iter().enumerate() {
        match mode {
            BIT_SPLIT_MODE_ZERO => {
                debug_assert!(planes[p].iter().all(|&b| b == 0));
            }
            BIT_SPLIT_MODE_REPEAT => {
                let first = planes[p][0];
                debug_assert!(planes[p].iter().all(|&b| b == first));
            }
            _ => {}
        }
    }
    let mut fixed_len = 1 + width;
    let mut zstd_bytes_len = 0_usize;
    for (p, &mode) in modes.iter().enumerate() {
        match mode {
            BIT_SPLIT_MODE_ZERO => {}
            BIT_SPLIT_MODE_REPEAT => fixed_len += 1,
            BIT_SPLIT_MODE_RAW => fixed_len += n,
            BIT_SPLIT_MODE_ZSTD => zstd_bytes_len += n,
            _ => unreachable!("valid bit-split mode"),
        }
        let _ = p;
    }
    if fixed_len > max_encoded_len {
        return None;
    }
    let zstd_compressed = if zstd_bytes_len > 0 {
        let mut concat = Vec::with_capacity(zstd_bytes_len);
        for (p, &mode) in modes.iter().enumerate() {
            if mode == BIT_SPLIT_MODE_ZSTD {
                concat.extend_from_slice(planes[p]);
            }
        }
        let comp = fast_paths::zstd_compress(&concat, level).ok()?;
        if fixed_len + comp.len() > max_encoded_len {
            return None;
        }
        Some(comp)
    } else {
        None
    };

    let total_len = fixed_len + zstd_compressed.as_ref().map_or(0, Vec::len);
    let mut out = Vec::with_capacity(total_len);
    out.push(transform);
    out.extend_from_slice(modes);
    for (p, &mode) in modes.iter().enumerate() {
        match mode {
            BIT_SPLIT_MODE_REPEAT => out.push(planes[p][0]),
            BIT_SPLIT_MODE_RAW => out.extend_from_slice(planes[p]),
            _ => {}
        }
    }
    if let Some(comp) = zstd_compressed {
        out.extend_from_slice(&comp);
    }
    Some(out)
}

fn parse_bit_split_header<'a>(
    body: &'a [u8],
    element_count: usize,
    width: usize,
    dtype: TensorDType,
) -> Result<(bool, [Option<&'a [u8]>; 4], [u8; 4], [u8; 4], Vec<u8>), QatqError> {
    if body.len() < 1 + width {
        return Err(QatqError::InvalidResidualStream);
    }
    let transform = body[0];
    let rot1 = match transform {
        BIT_SPLIT_TRANSFORM_IDENTITY => false,
        BIT_SPLIT_TRANSFORM_HI16_ROT1 => {
            if dtype == TensorDType::F16 {
                return Err(QatqError::InvalidQatqExactBody);
            }
            true
        }
        _ => return Err(QatqError::InvalidQatqExactBody),
    };
    let modes_slice = &body[1..1 + width];
    let mut offset = 1 + width;
    let mut zstd_plane_count = 0_usize;
    let mut modes = [0_u8; 4];
    let mut repeats = [0_u8; 4];
    let mut raw_slices: [Option<&'a [u8]>; 4] = [None; 4];

    for p in 0..width {
        let m = modes_slice[p];
        modes[p] = m;
        match m {
            BIT_SPLIT_MODE_ZERO => {}
            BIT_SPLIT_MODE_REPEAT => {
                if offset >= body.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                repeats[p] = body[offset];
                offset += 1;
            }
            BIT_SPLIT_MODE_RAW => {
                let end = offset
                    .checked_add(element_count)
                    .ok_or(QatqError::InvalidResidualStream)?;
                if end > body.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                raw_slices[p] = Some(&body[offset..end]);
                offset = end;
            }
            BIT_SPLIT_MODE_ZSTD => {
                zstd_plane_count += 1;
            }
            _ => return Err(QatqError::InvalidResidualStream),
        }
    }

    let zstd_payload = &body[offset..];
    let decompressed = if zstd_plane_count == 0 {
        if !zstd_payload.is_empty() {
            return Err(QatqError::InvalidResidualStream);
        }
        Vec::new()
    } else {
        if zstd_payload.is_empty() {
            return Err(QatqError::InvalidResidualStream);
        }
        let expected_zstd_len = zstd_plane_count
            .checked_mul(element_count)
            .ok_or(QatqError::InvalidResidualStream)?;
        let dec = fast_paths::zstd_decompress(zstd_payload, expected_zstd_len)
            .map_err(|_| QatqError::InvalidResidualStream)?;
        if dec.len() != expected_zstd_len {
            return Err(QatqError::InvalidResidualStream);
        }
        dec
    };
    Ok((rot1, raw_slices, modes, repeats, decompressed))
}

fn decode_bit_split_plane_zstd_u16_le_and_checksum(
    body: &[u8],
    expected_len: usize,
    element_count: usize,
    dtype: TensorDType,
) -> Result<(Vec<u8>, u64), QatqError> {
    if expected_len != element_count * 2 {
        return Err(QatqError::InvalidResidualStream);
    }
    let (rot1, raw_slices, modes, repeat_vals, decompressed) =
        parse_bit_split_header(body, element_count, 2, dtype)?;

    let mut zstd_slices: [Option<&[u8]>; 2] = [None; 2];
    let mut z_off = 0_usize;
    for p in 0..2 {
        if modes[p] == BIT_SPLIT_MODE_ZSTD {
            zstd_slices[p] = Some(&decompressed[z_off..z_off + element_count]);
            z_off += element_count;
        }
    }

    let direct_slice = |p: usize| -> Option<&[u8]> {
        match modes[p] {
            BIT_SPLIT_MODE_RAW => raw_slices[p],
            BIT_SPLIT_MODE_ZSTD => zstd_slices[p],
            _ => None,
        }
    };
    if let (Some(p0), Some(p1)) = (direct_slice(0), direct_slice(1)) {
        return Ok(fast_paths::untranspose_be_planes_to_u16_le_and_checksum(
            p0, p1, rot1,
        ));
    }

    let mut out = vec![0_u8; expected_len];
    for i in 0..element_count {
        let b0 = match modes[0] {
            BIT_SPLIT_MODE_ZERO => 0,
            BIT_SPLIT_MODE_REPEAT => repeat_vals[0],
            BIT_SPLIT_MODE_RAW => raw_slices[0].unwrap()[i],
            BIT_SPLIT_MODE_ZSTD => zstd_slices[0].unwrap()[i],
            _ => unreachable!(),
        };
        let b1 = match modes[1] {
            BIT_SPLIT_MODE_ZERO => 0,
            BIT_SPLIT_MODE_REPEAT => repeat_vals[1],
            BIT_SPLIT_MODE_RAW => raw_slices[1].unwrap()[i],
            BIT_SPLIT_MODE_ZSTD => zstd_slices[1].unwrap()[i],
            _ => unreachable!(),
        };
        let (lo, hi) = if rot1 {
            let [lo, hi] = u16::from_be_bytes([b0, b1]).rotate_right(1).to_le_bytes();
            (lo, hi)
        } else {
            (b1, b0)
        };
        out[i * 2] = lo;
        out[i * 2 + 1] = hi;
    }
    let checksum = fast_paths::checksum_bytes_fast(&out);
    Ok((out, checksum))
}

fn decode_bit_split_plane_zstd_f32_and_checksum(
    body: &[u8],
    value_count: usize,
) -> Result<(Vec<f32>, u64), QatqError> {
    let (rot1, raw_slices, modes, repeat_vals, decompressed) =
        parse_bit_split_header(body, value_count, 4, TensorDType::F32)?;

    let mut zstd_slices: [Option<&[u8]>; 4] = [None; 4];
    let mut z_off = 0_usize;
    for p in 0..4 {
        if modes[p] == BIT_SPLIT_MODE_ZSTD {
            zstd_slices[p] = Some(&decompressed[z_off..z_off + value_count]);
            z_off += value_count;
        }
    }

    let direct_slice = |p: usize| -> Option<&[u8]> {
        match modes[p] {
            BIT_SPLIT_MODE_RAW => raw_slices[p],
            BIT_SPLIT_MODE_ZSTD => zstd_slices[p],
            _ => None,
        }
    };

    if let (Some(p0), Some(p1), Some(p2), Some(p3)) = (
        direct_slice(0),
        direct_slice(1),
        direct_slice(2),
        direct_slice(3),
    ) {
        return Ok(fast_paths::untranspose_be_planes_to_f32_and_checksum(
            p0, p1, p2, p3, rot1,
        ));
    }

    if let (Some(p0), Some(p1)) = (direct_slice(0), direct_slice(1))
        && modes[2] == BIT_SPLIT_MODE_ZERO
        && modes[3] == BIT_SPLIT_MODE_ZERO
    {
        let mut values = Vec::with_capacity(value_count);
        let mut checksum = FNV_OFFSET;
        for i in 0..value_count {
            let (b0, b1) = if rot1 {
                ((p0[i] >> 1) | (p1[i] << 7), (p1[i] >> 1) | (p0[i] << 7))
            } else {
                (p0[i], p1[i])
            };
            checksum = checksum_two_high_bytes_update(checksum, b0, b1);
            values.push(f32::from_bits(((b0 as u32) << 24) | ((b1 as u32) << 16)));
        }
        return Ok((values, checksum));
    }

    let mut values = Vec::with_capacity(value_count);
    for i in 0..value_count {
        let p0 = match modes[0] {
            BIT_SPLIT_MODE_ZERO => 0,
            BIT_SPLIT_MODE_REPEAT => repeat_vals[0],
            BIT_SPLIT_MODE_RAW => raw_slices[0].unwrap()[i],
            BIT_SPLIT_MODE_ZSTD => zstd_slices[0].unwrap()[i],
            _ => unreachable!(),
        };
        let p1 = match modes[1] {
            BIT_SPLIT_MODE_ZERO => 0,
            BIT_SPLIT_MODE_REPEAT => repeat_vals[1],
            BIT_SPLIT_MODE_RAW => raw_slices[1].unwrap()[i],
            BIT_SPLIT_MODE_ZSTD => zstd_slices[1].unwrap()[i],
            _ => unreachable!(),
        };
        let p2 = match modes[2] {
            BIT_SPLIT_MODE_ZERO => 0,
            BIT_SPLIT_MODE_REPEAT => repeat_vals[2],
            BIT_SPLIT_MODE_RAW => raw_slices[2].unwrap()[i],
            BIT_SPLIT_MODE_ZSTD => zstd_slices[2].unwrap()[i],
            _ => unreachable!(),
        };
        let p3 = match modes[3] {
            BIT_SPLIT_MODE_ZERO => 0,
            BIT_SPLIT_MODE_REPEAT => repeat_vals[3],
            BIT_SPLIT_MODE_RAW => raw_slices[3].unwrap()[i],
            BIT_SPLIT_MODE_ZSTD => zstd_slices[3].unwrap()[i],
            _ => unreachable!(),
        };
        let (b0, b1) = if rot1 {
            ((p0 >> 1) | (p1 << 7), (p1 >> 1) | (p0 << 7))
        } else {
            (p0, p1)
        };
        values.push(f32::from_bits(u32::from_be_bytes([b0, b1, p2, p3])));
    }
    let checksum = checksum_f32_bits(&values);
    Ok((values, checksum))
}

fn push_packed_run_header(out: &mut Vec<u8>, tag: u8, len: usize) {
    debug_assert!(len > 0 && len <= PACKED_RUN_MAX_LEN);
    out.push(tag | ((len - 1) as u8));
}

fn encode_byte_plane_blocks_from_planes_bounded(
    planes_t0: &[u8],
    value_count: usize,
    max_encoded_len: usize,
) -> Option<Vec<u8>> {
    debug_assert_eq!(planes_t0.len(), value_count * 4);
    let mut stats = [fast_paths::PlaneStat::Zero; 4];
    let mut encoded_len = 0_usize;
    for plane in 0..4 {
        let slice = &planes_t0[plane * value_count..(plane + 1) * value_count];
        let stat = fast_paths::classify_plane(slice);
        stats[plane] = stat;
        encoded_len += match stat {
            fast_paths::PlaneStat::Zero => 1,
            fast_paths::PlaneStat::Repeat(_) => 2,
            fast_paths::PlaneStat::Varying => 1 + value_count,
        };
    }
    if encoded_len > max_encoded_len {
        return None;
    }
    let mut out = Vec::with_capacity(encoded_len);
    for plane in 0..4 {
        match stats[plane] {
            fast_paths::PlaneStat::Zero => out.push(BYTE_PLANE_BLOCK_ZERO),
            fast_paths::PlaneStat::Repeat(val) => {
                out.push(BYTE_PLANE_BLOCK_REPEAT);
                out.push(val);
            }
            fast_paths::PlaneStat::Varying => {
                out.push(BYTE_PLANE_BLOCK_RAW);
                out.extend_from_slice(&planes_t0[plane * value_count..(plane + 1) * value_count]);
            }
        }
    }
    Some(out)
}

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
fn encode_byte_plane_blocks_from_f32_bounded(
    values: &[f32],
    max_encoded_len: usize,
) -> (Option<Vec<u8>>, u64) {
    let (planes, checksum) = fast_paths::transpose_f32_to_be_planes_and_checksum(values);
    (
        encode_byte_plane_blocks_from_planes_bounded(&planes, values.len(), max_encoded_len),
        checksum,
    )
}

fn encode_delta_xor_byte_plane_runs_bounded(
    values: &[f32],
    max_encoded_len: usize,
) -> Option<Vec<u8>> {
    let n = values.len();
    let total_len = n.checked_mul(4)?;
    let mut deltas = vec![0_u8; total_len];
    let (d0, rest) = deltas.split_at_mut(n);
    let (d1, rest) = rest.split_at_mut(n);
    let (d2, d3) = rest.split_at_mut(n);
    let mut prev_bits = 0_u32;
    for (i, &val) in values.iter().enumerate() {
        let bits = val.to_bits();
        let delta = if i == 0 { bits } else { prev_bits ^ bits };
        prev_bits = bits;
        let [b0, b1, b2, b3] = delta.to_be_bytes();
        d0[i] = b0;
        d1[i] = b1;
        d2[i] = b2;
        d3[i] = b3;
    }
    encode_contiguous_byte_plane_runs_bounded(&deltas, max_encoded_len)
}

fn adjacent_xor_u16_byte_planes_if_sparse_le(bytes_le: &[u8]) -> Option<Vec<u8>> {
    debug_assert_eq!(bytes_le.len() % 2, 0);
    let element_count = bytes_le.len() / 2;
    if element_count == 0 {
        return None;
    }
    let sample_count = element_count.min(NATIVE_U16_XOR_SAMPLE_WORDS);
    let block_count = sample_count.min(NATIVE_U16_XOR_SAMPLE_BLOCKS);
    let words_per_block = sample_count.div_ceil(block_count);
    let mut sampled_bytes = 0;
    let mut sample_zeros = 0;
    for block in 0..block_count {
        let start = block * element_count / block_count;
        let end = (start + words_per_block).min(element_count);
        for index in start..end {
            let offset = index * 2;
            let word = u16::from_le_bytes([bytes_le[offset], bytes_le[offset + 1]]);
            let previous = if index == 0 {
                0
            } else {
                u16::from_le_bytes([bytes_le[offset - 2], bytes_le[offset - 1]])
            };
            let [high, low] = (word ^ previous).to_be_bytes();
            sample_zeros += usize::from(high == 0) + usize::from(low == 0);
            sampled_bytes += 2;
        }
    }
    if sample_zeros <= sampled_bytes / 2 {
        return None;
    }

    let mut plane_bytes = vec![0_u8; bytes_le.len()];
    let (p0, p1) = plane_bytes.split_at_mut(element_count);
    let mut zero_count = 0;
    let mut previous = 0_u16;
    for (index, chunk) in bytes_le.chunks_exact(2).enumerate() {
        let word = u16::from_le_bytes([chunk[0], chunk[1]]);
        let [high, low] = (word ^ previous).to_be_bytes();
        zero_count += usize::from(high == 0) + usize::from(low == 0);
        p0[index] = high;
        p1[index] = low;
        previous = word;
    }
    if zero_count > bytes_le.len().div_ceil(2) {
        Some(plane_bytes)
    } else {
        None
    }
}

#[cfg(test)]
fn adjacent_xor_u16_sample_is_sparse(canonical: &[u8]) -> bool {
    debug_assert_eq!(canonical.len() % 2, 0);
    let element_count = canonical.len() / 2;
    if element_count == 0 {
        return false;
    }
    let sample_count = element_count.min(NATIVE_U16_XOR_SAMPLE_WORDS);
    let block_count = sample_count.min(NATIVE_U16_XOR_SAMPLE_BLOCKS);
    let words_per_block = sample_count.div_ceil(block_count);
    let mut sampled_bytes = 0;
    let mut zero_count = 0;
    for block in 0..block_count {
        let start = block * element_count / block_count;
        let end = (start + words_per_block).min(element_count);
        for index in start..end {
            let offset = index * 2;
            let word = u16::from_be_bytes([canonical[offset], canonical[offset + 1]]);
            let previous = if index == 0 {
                0
            } else {
                u16::from_be_bytes([canonical[offset - 2], canonical[offset - 1]])
            };
            let [high, low] = (word ^ previous).to_be_bytes();
            zero_count += usize::from(high == 0) + usize::from(low == 0);
            sampled_bytes += 2;
        }
    }
    zero_count > sampled_bytes / 2
}

fn strided_xor_u16_byte_planes_if_sparse_le(
    bytes_le: &[u8],
    stride: usize,
) -> Option<(usize, Vec<u8>)> {
    debug_assert_eq!(bytes_le.len() % 2, 0);
    let element_count = bytes_le.len() / 2;
    if stride == 0 || stride >= element_count {
        return None;
    }
    let sample_count = element_count.min(STRIDED_U16_XOR_CLASSIFIER_WORDS);
    let block_count = sample_count.min(NATIVE_U16_XOR_SAMPLE_BLOCKS);
    let words_per_block = sample_count.div_ceil(block_count);
    let mut sampled_bytes = 0;
    let mut sampled_zero_count = 0;
    for block in 0..block_count {
        let start = (block * element_count / block_count).max(stride);
        let end = (start + words_per_block).min(element_count);
        for index in start..end {
            let offset = index * 2;
            let predictor_offset = (index - stride) * 2;
            let word = u16::from_le_bytes([bytes_le[offset], bytes_le[offset + 1]]);
            let predictor =
                u16::from_le_bytes([bytes_le[predictor_offset], bytes_le[predictor_offset + 1]]);
            let [high, low] = (word ^ predictor).to_be_bytes();
            sampled_zero_count += usize::from(high == 0) + usize::from(low == 0);
            sampled_bytes += 2;
        }
    }
    if sampled_bytes == 0
        || sampled_zero_count <= sampled_bytes / 2
        || sampled_zero_count == sampled_bytes
    {
        return None;
    }

    let mut plane_bytes = vec![0_u8; bytes_le.len()];
    let (p0, p1) = plane_bytes.split_at_mut(element_count);
    let mut zero_count = 0;
    for index in 0..element_count {
        let offset = index * 2;
        let word = u16::from_le_bytes([bytes_le[offset], bytes_le[offset + 1]]);
        let predictor = if index < stride {
            0
        } else {
            let predictor_offset = (index - stride) * 2;
            u16::from_le_bytes([bytes_le[predictor_offset], bytes_le[predictor_offset + 1]])
        };
        let [high, low] = (word ^ predictor).to_be_bytes();
        zero_count += usize::from(high == 0) + usize::from(low == 0);
        p0[index] = high;
        p1[index] = low;
    }
    (zero_count > bytes_le.len().div_ceil(2)).then_some((stride, plane_bytes))
}

fn strided_xor_wins_sample_compression_le(bytes_le: &[u8], residual_planes: &[u8]) -> bool {
    debug_assert_eq!(bytes_le.len(), residual_planes.len());
    debug_assert_eq!(bytes_le.len() % 2, 0);
    let nonzero_residual_bytes = residual_planes.iter().filter(|byte| **byte != 0).count();
    if nonzero_residual_bytes <= STRIDED_U16_XOR_CLASSIFIER_WORDS {
        return false;
    }
    let element_count = bytes_le.len() / 2;
    let sample_count = element_count.min(STRIDED_U16_XOR_CLASSIFIER_WORDS);
    if sample_count < 2 {
        return false;
    }

    let mut canonical_sample = vec![0_u8; sample_count * 2];
    let mut residual_sample = vec![0_u8; sample_count * 2];
    for sample_index in 0..sample_count {
        let index = sample_index * element_count / sample_count;
        canonical_sample[sample_index] = bytes_le[index * 2 + 1];
        canonical_sample[sample_count + sample_index] = bytes_le[index * 2];
        residual_sample[sample_index] = residual_planes[index];
        residual_sample[sample_count + sample_index] = residual_planes[element_count + index];
    }

    let Ok(canonical_len) = fast_paths::zstd_compress(&canonical_sample, 3).map(|b| b.len()) else {
        return false;
    };
    let Ok(residual_len) = fast_paths::zstd_compress(&residual_sample, 3).map(|b| b.len()) else {
        return false;
    };
    residual_len.saturating_mul(100)
        <= canonical_len.saturating_mul(STRIDED_U16_XOR_SAMPLE_WIN_PERCENT)
}

#[cfg(test)]
fn strided_xor_u16_byte_planes_if_sparse(
    canonical: &[u8],
    stride: usize,
) -> Option<(usize, Vec<u8>)> {
    debug_assert_eq!(canonical.len() % 2, 0);
    let (sampled_zero_count, sampled_bytes) = strided_xor_u16_sample_zero_count(canonical, stride)
        .filter(|(zero_count, sampled_bytes)| *zero_count > *sampled_bytes / 2)?;
    if sampled_zero_count == sampled_bytes {
        return None;
    }
    let element_count = canonical.len() / 2;
    let mut plane_bytes = vec![0_u8; canonical.len()];
    let mut zero_count = 0;
    for index in 0..element_count {
        let offset = index * 2;
        let word = u16::from_be_bytes([canonical[offset], canonical[offset + 1]]);
        let predictor = if index < stride {
            0
        } else {
            let predictor_offset = (index - stride) * 2;
            u16::from_be_bytes([canonical[predictor_offset], canonical[predictor_offset + 1]])
        };
        let [high, low] = (word ^ predictor).to_be_bytes();
        zero_count += usize::from(high == 0) + usize::from(low == 0);
        plane_bytes[index] = high;
        plane_bytes[element_count + index] = low;
    }
    (zero_count > canonical.len().div_ceil(2)).then_some((stride, plane_bytes))
}

#[cfg(test)]
fn strided_xor_u16_sample_zero_count(canonical: &[u8], stride: usize) -> Option<(usize, usize)> {
    let element_count = canonical.len() / 2;
    if stride == 0 || stride >= element_count {
        return None;
    }
    let sample_count = element_count.min(STRIDED_U16_XOR_CLASSIFIER_WORDS);
    let block_count = sample_count.min(NATIVE_U16_XOR_SAMPLE_BLOCKS);
    let words_per_block = sample_count.div_ceil(block_count);
    let mut sampled_bytes = 0;
    let mut zero_count = 0;
    for block in 0..block_count {
        let start = (block * element_count / block_count).max(stride);
        let end = (start + words_per_block).min(element_count);
        for index in start..end {
            let offset = index * 2;
            let predictor_offset = (index - stride) * 2;
            let word = u16::from_be_bytes([canonical[offset], canonical[offset + 1]]);
            let predictor =
                u16::from_be_bytes([canonical[predictor_offset], canonical[predictor_offset + 1]]);
            let [high, low] = (word ^ predictor).to_be_bytes();
            zero_count += usize::from(high == 0) + usize::from(low == 0);
            sampled_bytes += 2;
        }
    }
    (sampled_bytes > 0).then_some((zero_count, sampled_bytes))
}

#[cfg(test)]
fn strided_xor_wins_sample_compression(canonical: &[u8], residual_planes: &[u8]) -> bool {
    debug_assert_eq!(canonical.len(), residual_planes.len());
    debug_assert_eq!(canonical.len() % 2, 0);
    let nonzero_residual_bytes = residual_planes.iter().filter(|byte| **byte != 0).count();
    if nonzero_residual_bytes <= STRIDED_U16_XOR_CLASSIFIER_WORDS {
        return false;
    }
    let element_count = canonical.len() / 2;
    let sample_count = element_count.min(STRIDED_U16_XOR_CLASSIFIER_WORDS);
    if sample_count < 2 {
        return false;
    }

    let mut canonical_sample = vec![0_u8; sample_count * 2];
    let mut residual_sample = vec![0_u8; sample_count * 2];
    for sample_index in 0..sample_count {
        let index = sample_index * element_count / sample_count;
        canonical_sample[sample_index] = canonical[index * 2];
        canonical_sample[sample_count + sample_index] = canonical[index * 2 + 1];
        residual_sample[sample_index] = residual_planes[index];
        residual_sample[sample_count + sample_index] = residual_planes[element_count + index];
    }

    let Ok(canonical_len) = fast_paths::zstd_compress(&canonical_sample, 3).map(|b| b.len()) else {
        return false;
    };
    let Ok(residual_len) = fast_paths::zstd_compress(&residual_sample, 3).map(|b| b.len()) else {
        return false;
    };
    residual_len.saturating_mul(100)
        <= canonical_len.saturating_mul(STRIDED_U16_XOR_SAMPLE_WIN_PERCENT)
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
        } else if has_repeat_4(bytes, index) {
            let value = bytes[index];
            let start = index;
            index += 4;
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
                && !has_repeat_4(bytes, index)
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
    fast_paths::checksum_f32_bits_fast(values)
}

fn checksum_bits_update(hash: u64, bits: u32) -> u64 {
    fast_paths::fnv1a_update_u32_be(hash, bits)
}

fn checksum_four_bytes_update(hash: u64, first: u8, second: u8, third: u8, fourth: u8) -> u64 {
    fast_paths::fnv1a_update_4(hash, first, second, third, fourth)
}

fn checksum_two_high_bytes_update(hash: u64, first: u8, second: u8) -> u64 {
    fast_paths::fnv1a_update_2_with_2_zeros(hash, first, second)
}

fn container_checksum_chunk(mut hash: u64, payload: &[u8]) -> u64 {
    let len_bytes = (payload.len() as u32).to_be_bytes();
    hash = fast_paths::fnv1a_update_4(hash, len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]);
    let mut chunks = payload.chunks_exact(8);
    for chunk in &mut chunks {
        hash = fast_paths::fnv1a_update_4(hash, chunk[0], chunk[1], chunk[2], chunk[3]);
        hash = fast_paths::fnv1a_update_4(hash, chunk[4], chunk[5], chunk[6], chunk[7]);
    }
    for &byte in chunks.remainder() {
        hash = fast_paths::fnv1a_update_byte(hash, byte);
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
                | Ok(QatqExactStrategy::AdjacentXorBytePlaneZstd)
                | Ok(QatqExactStrategy::StridedXorBytePlaneZstd)
        ));
    }

    #[test]
    fn qatq_exact_typed_bf16_uses_sparse_adjacent_xor_and_preserves_native_bytes() {
        let mut bytes = Vec::new();
        for index in 0..4_096 {
            let value = -1.0 + 2.0 * index as f32 / 4_096.0;
            let bits = value.to_bits();
            let word = (bits.wrapping_add(0x7fff + ((bits >> 16) & 1)) >> 16) as u16;
            bytes.extend_from_slice(&word.to_le_bytes());
        }

        let encoded = try_encode_qatq_exact_tensor_le(&bytes, TensorDType::BF16).unwrap();
        assert_eq!(
            qatq_exact_strategy(&encoded),
            Ok(QatqExactStrategy::AdjacentXorBytePlaneZstd)
        );

        let decoded = decode_qatq_exact_tensor_le(&encoded).unwrap();
        assert_eq!(decoded.dtype, TensorDType::BF16);
        assert_eq!(decoded.bytes_le, bytes);

        let mut truncated = encoded.clone();
        truncated.pop();
        assert!(decode_qatq_exact_tensor_le(&truncated).is_err());

        let body_offset = HEADER_LEN + QATQ_EXACT_PREFIX_LEN;
        let mut residual_planes =
            zstd::bulk::decompress(&encoded[body_offset..], bytes.len()).unwrap();
        residual_planes[0] ^= 1;
        let mut corrupted = encoded[..body_offset].to_vec();
        corrupted.extend_from_slice(&zstd::bulk::compress(&residual_planes, 3).unwrap());
        assert!(matches!(
            decode_qatq_exact_tensor_le(&corrupted),
            Err(QatqError::ChecksumMismatch { .. })
        ));
    }

    #[test]
    fn qatq_exact_typed_adjacent_xor_preserves_arbitrary_u16_patterns() {
        let mut state = 0x5141_5451_c0de_f00d_u64;
        let mut bytes = Vec::new();
        for _ in 0..8_192 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            bytes.extend_from_slice(&(state as u16).to_le_bytes());
        }

        for dtype in [TensorDType::F16, TensorDType::BF16] {
            let encoded = try_encode_qatq_exact_tensor_le(&bytes, dtype).unwrap();
            let decoded = decode_qatq_exact_tensor_le(&encoded).unwrap();
            assert_eq!(decoded.dtype, dtype);
            assert_eq!(decoded.bytes_le, bytes);
        }
    }

    #[test]
    fn qatq_exact_typed_adjacent_xor_preserves_every_u16_pattern() {
        let mut bytes = Vec::with_capacity((u16::MAX as usize + 1) * 4);
        for word in 0..=u16::MAX {
            bytes.extend_from_slice(&word.to_le_bytes());
            bytes.extend_from_slice(&word.to_le_bytes());
        }

        for dtype in [TensorDType::F16, TensorDType::BF16] {
            let encoded = try_encode_qatq_exact_tensor_le(&bytes, dtype).unwrap();
            let decoded = decode_qatq_exact_tensor_le(&encoded).unwrap();
            assert_eq!(decoded.dtype, dtype);
            assert_eq!(decoded.bytes_le, bytes);
        }
    }

    #[test]
    fn native_u16_xor_sample_rejects_random_bits_and_accepts_smooth_words() {
        let mut random = Vec::new();
        let mut state = 0x5141_5451_c0de_f00d_u64;
        for _ in 0..16_384 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            random.extend_from_slice(&(state as u16).to_be_bytes());
        }
        assert!(!adjacent_xor_u16_sample_is_sparse(&random));

        let smooth: Vec<u8> = (0..16_384_u16)
            .flat_map(|index| (index / 4).to_be_bytes())
            .collect();
        assert!(adjacent_xor_u16_sample_is_sparse(&smooth));
    }

    #[test]
    fn qatq_exact_typed_bf16_uses_strided_xor_for_drifting_token_rows() {
        const STRIDE: usize = 128;
        let mut bytes = Vec::new();
        for token in 0..512_u16 {
            for channel in 0..STRIDE as u16 {
                let word = 0x3e00_u16
                    .wrapping_add(channel.wrapping_mul(101))
                    .wrapping_add(token / 32);
                bytes.extend_from_slice(&word.to_le_bytes());
            }
        }

        let automatic = try_encode_qatq_exact_tensor_le(&bytes, TensorDType::BF16).unwrap();
        let encoded =
            try_encode_qatq_exact_tensor_le_with_stride_hint(&bytes, TensorDType::BF16, STRIDE)
                .unwrap();
        assert!(matches!(
            qatq_exact_strategy(&automatic),
            Ok(QatqExactStrategy::BytePlaneZstd)
        ));
        assert_eq!(
            qatq_exact_strategy(&encoded),
            Ok(QatqExactStrategy::StridedXorBytePlaneZstd)
        );
        let decoded = decode_qatq_exact_tensor_le(&encoded).unwrap();
        assert_eq!(decoded.bytes_le, bytes);

        let encoded_f16 =
            try_encode_qatq_exact_tensor_le_with_stride_hint(&bytes, TensorDType::F16, STRIDE)
                .unwrap();
        assert_eq!(
            qatq_exact_strategy(&encoded_f16),
            Ok(QatqExactStrategy::StridedXorBytePlaneZstd)
        );
        let decoded_f16 = decode_qatq_exact_tensor_le(&encoded_f16).unwrap();
        assert_eq!(decoded_f16.dtype, TensorDType::F16);
        assert_eq!(decoded_f16.bytes_le, bytes);

        let stride_offset = HEADER_LEN + QATQ_EXACT_PREFIX_LEN;
        assert_eq!(
            u32::from_be_bytes(
                encoded[stride_offset..stride_offset + STRIDED_U16_XOR_METADATA_LEN]
                    .try_into()
                    .unwrap()
            ) as usize,
            STRIDE
        );

        let mut zero_stride = encoded.clone();
        zero_stride[stride_offset..stride_offset + STRIDED_U16_XOR_METADATA_LEN].fill(0);
        assert_eq!(
            decode_qatq_exact_tensor_le(&zero_stride),
            Err(QatqError::InvalidResidualStream)
        );

        let residual_offset = stride_offset + STRIDED_U16_XOR_METADATA_LEN;
        let mut residual_planes =
            zstd::bulk::decompress(&encoded[residual_offset..], bytes.len()).unwrap();
        residual_planes[STRIDE] ^= 1;
        let mut corrupted = encoded[..residual_offset].to_vec();
        corrupted.extend_from_slice(&zstd::bulk::compress(&residual_planes, 3).unwrap());
        assert!(matches!(
            decode_qatq_exact_tensor_le(&corrupted),
            Err(QatqError::ChecksumMismatch { .. })
        ));

        let mut truncated = encoded;
        truncated.truncate(stride_offset + STRIDED_U16_XOR_METADATA_LEN - 1);
        assert!(decode_qatq_exact_tensor_le(&truncated).is_err());
    }

    #[test]
    fn qatq_exact_typed_stride_hint_rejects_invalid_layouts() {
        let bytes = vec![0_u8; 256];
        assert_eq!(
            try_encode_qatq_exact_tensor_le_with_stride_hint(&bytes, TensorDType::BF16, 0),
            Err(QatqError::InvalidPredictorStride(0))
        );
        assert_eq!(
            try_encode_qatq_exact_tensor_le_with_stride_hint(&bytes, TensorDType::BF16, 128),
            Err(QatqError::InvalidPredictorStride(128))
        );
        assert_eq!(
            try_encode_qatq_exact_tensor_le_with_stride_hint(&bytes, TensorDType::F32, 32),
            Err(QatqError::InvalidPredictorStride(32))
        );
    }

    #[test]
    fn strided_u16_xor_sample_accepts_period_and_rejects_random_words() {
        let periodic: Vec<u8> = (0..16_384_u16)
            .flat_map(|index| (index % 128).wrapping_mul(97).to_be_bytes())
            .collect();
        let (periodic_zeros, periodic_bytes) =
            strided_xor_u16_sample_zero_count(&periodic, 128).unwrap();
        assert!(periodic_zeros > periodic_bytes / 2);

        let mut random = Vec::new();
        let mut state = 0x5141_5451_c0de_f00d_u64;
        for _ in 0..16_384 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            random.extend_from_slice(&(state as u16).to_be_bytes());
        }
        let (random_zeros, random_bytes) = strided_xor_u16_sample_zero_count(&random, 128).unwrap();
        assert!(random_zeros <= random_bytes / 2);
    }

    #[test]
    fn strided_u16_xor_compression_classifier_is_conservative() {
        const STRIDE: usize = 128;
        let repeated: Vec<u8> = (0..65_536)
            .flat_map(|index| {
                0x3d00_u16
                    .wrapping_add(((index % STRIDE) as u16).wrapping_mul(101))
                    .to_be_bytes()
            })
            .collect();
        assert!(strided_xor_u16_byte_planes_if_sparse(&repeated, STRIDE).is_none());

        let drifting: Vec<u8> = (0..65_536)
            .flat_map(|index| {
                let token = index / STRIDE;
                let channel = index % STRIDE;
                0x3d00_u16
                    .wrapping_add((channel as u16).wrapping_mul(101))
                    .wrapping_add((token / 32) as u16)
                    .to_be_bytes()
            })
            .collect();
        let (_, drifting_residual) =
            strided_xor_u16_byte_planes_if_sparse(&drifting, STRIDE).unwrap();
        assert!(strided_xor_wins_sample_compression(
            &drifting,
            &drifting_residual
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
    fn qatq_exact_selects_and_decodes_byte_plane_zstd_payload() {
        let mut state = 0x51a7_3c19_u32;
        let values: Vec<f32> = (0..4096)
            .map(|index| {
                state = lcg_next_for_test(state ^ ((index as u32).wrapping_mul(17)));
                let sign = (state >> 31) << 31;
                let exp = (0x7d_u32 + (state & 0x03)) << 23;
                let m_hi = (state & 0x0f) << 16;
                let m_mid = ((state >> 8) & 0x0f) << 8;
                let m_lo = (state >> 16) & 0x0f;
                f32::from_bits(sign | exp | m_hi | m_mid | m_lo)
            })
            .collect();
        let encoded = encode_qatq_exact(&values);
        let strategy = qatq_exact_strategy(&encoded).unwrap();
        assert_eq!(strategy, QatqExactStrategy::BytePlaneZstd);
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
    fn qatq_exact_byte_container_is_canonical_and_wire_compatible() {
        for length in 0..=17 {
            let bytes: Vec<u8> = (0..length).map(|index| (index * 31 + 7) as u8).collect();
            let words: Vec<u32> = bytes
                .chunks(4)
                .map(|chunk| {
                    let mut word = [0_u8; 4];
                    word[..chunk.len()].copy_from_slice(chunk);
                    u32::from_le_bytes(word)
                })
                .collect();

            let encoded = encode_qatq_exact_bytes_container(&bytes, 2).unwrap();
            assert_eq!(encoded, encode_qatq_exact_u32_container(&words, 2).unwrap());
            assert_eq!(
                decode_qatq_exact_bytes_container(&encoded, bytes.len(), 2).unwrap(),
                bytes
            );
        }
    }

    #[test]
    fn qatq_exact_byte_visitor_validates_before_callbacks() {
        let bytes = b"bounded byte transport";
        let encoded = encode_qatq_exact_bytes_container(bytes, 2).unwrap();
        let mut visited = Vec::new();
        let metadata =
            for_each_qatq_exact_bytes_container_chunk(&encoded, bytes.len(), 2, |chunk| {
                visited.extend_from_slice(chunk);
                Ok(())
            })
            .unwrap();
        assert_eq!(visited, bytes);
        assert_eq!(metadata.total_values, bytes.len().div_ceil(4));

        let words = decode_qatq_exact_u32_container(&encoded, 2).unwrap();
        let mut non_canonical = words;
        let last = non_canonical.last_mut().unwrap();
        *last |= 0xff00_0000;
        let non_canonical = encode_qatq_exact_u32_container(&non_canonical, 2).unwrap();
        let mut callbacks = 0;
        assert_eq!(
            for_each_qatq_exact_bytes_container_chunk(&non_canonical, bytes.len(), 2, |_| {
                callbacks += 1;
                Ok(())
            },),
            Err(QatqError::InvalidContainer)
        );
        assert_eq!(callbacks, 0);
        assert_eq!(
            decode_qatq_exact_bytes_container(&encoded, bytes.len() + 4, 2),
            Err(QatqError::InvalidContainer)
        );
    }

    #[test]
    fn qatq_exact_byte_decode_preflights_declared_length_before_allocation() {
        let encoded = encode_qatq_exact_bytes_container(b"four", 1).unwrap();
        let limits = QatcDecodeLimits {
            max_total_values: 1,
            ..QatcDecodeLimits::default()
        };
        let mut callbacks = 0;

        assert_eq!(
            decode_qatq_exact_bytes_container_with_limits(&encoded, usize::MAX, limits, 1),
            Err(QatqError::ContainerLimitExceeded("total values"))
        );
        assert_eq!(
            decode_qatq_exact_bytes_container_with_limits(&encoded, 8, limits, 1),
            Err(QatqError::ContainerLimitExceeded("total values"))
        );
        assert_eq!(
            for_each_qatq_exact_bytes_container_chunk_with_limits(&encoded, 8, limits, 1, |_| {
                callbacks += 1;
                Ok(())
            },),
            Err(QatqError::ContainerLimitExceeded("total values"))
        );
        assert_eq!(callbacks, 0);
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
    fn qatq_exact_u32_chunk_visitor_is_bounded_and_bit_exact() {
        let words = [0, 0x7fc0_0001, u32::MAX, 4, 5];
        let encoded = encode_qatq_exact_u32_container(&words, 2).unwrap();
        let mut chunk_lengths = Vec::new();
        let mut decoded = Vec::new();

        let metadata = for_each_qatq_exact_u32_container_chunk(&encoded, 2, |chunk| {
            chunk_lengths.push(chunk.len());
            decoded.extend_from_slice(chunk);
            Ok(())
        })
        .unwrap();

        assert_eq!(chunk_lengths, [2, 2, 1]);
        assert_eq!(decoded, words);
        assert_eq!(metadata.total_values, words.len());
    }

    #[test]
    fn qatq_exact_u32_chunk_visitor_validates_before_callbacks() {
        let encoded = encode_qatq_exact_u32_container(&[1, 2, 3], 2).unwrap();
        let mut callbacks = 0;

        assert_eq!(
            for_each_qatq_exact_u32_container_chunk_with_limits(
                &encoded,
                QatcDecodeLimits {
                    max_total_values: 2,
                    ..QatcDecodeLimits::default()
                },
                2,
                |_| {
                    callbacks += 1;
                    Ok(())
                },
            ),
            Err(QatqError::ContainerLimitExceeded("total values"))
        );
        assert_eq!(callbacks, 0);
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

    fn encode_f32_bits_be(values: &[f32]) -> Vec<u8> {
        let mut out = Vec::with_capacity(values.len() * 4);
        for value in values {
            out.extend_from_slice(&value.to_bits().to_be_bytes());
        }
        out
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

    fn build_bit_split_f32_payload_for_test(
        values: &[f32],
        modes: [u8; 4],
        transform: u8,
        level: i32,
    ) -> Vec<u8> {
        let (planes_t0, checksum) = fast_paths::transpose_f32_to_be_planes_and_checksum(values);
        let n = values.len();
        let (p0, rest) = planes_t0.split_at(n);
        let (p1, rest) = rest.split_at(n);
        let (p2, p3) = rest.split_at(n);
        let body = if transform == BIT_SPLIT_TRANSFORM_HI16_ROT1 {
            let mut r0 = vec![0_u8; n];
            let mut r1 = vec![0_u8; n];
            fast_paths::rot1_hi16_planes(p0, p1, &mut r0, &mut r1);
            build_bit_split_payload_from_plane_slices(
                &[&r0, &r1, p2, p3],
                &modes,
                transform,
                level,
                usize::MAX,
            )
            .unwrap()
        } else {
            build_bit_split_payload_from_plane_slices(
                &[p0, p1, p2, p3],
                &modes,
                transform,
                level,
                usize::MAX,
            )
            .unwrap()
        };
        let mut out = Vec::new();
        write_header(&mut out, CodecMode::QatqExact, n, 1.0, checksum);
        write_qatq_exact_prefix(&mut out, QATQ_EXACT_STRATEGY_BIT_SPLIT_PLANE_ZSTD);
        out.extend_from_slice(&body);
        out
    }

    fn build_bit_split_u16_payload_for_test(
        bytes_le: &[u8],
        dtype: TensorDType,
        modes: [u8; 2],
        transform: u8,
        level: i32,
    ) -> Vec<u8> {
        let n = bytes_le.len() / 2;
        let checksum = fast_paths::checksum_bytes_fast(bytes_le);
        let body = if transform == BIT_SPLIT_TRANSFORM_HI16_ROT1 {
            let mut r0 = vec![0_u8; n];
            let mut r1 = vec![0_u8; n];
            fast_paths::transpose_u16_le_rot1_planes(bytes_le, &mut r0, &mut r1);
            build_bit_split_payload_from_plane_slices(
                &[&r0, &r1],
                &modes,
                transform,
                level,
                usize::MAX,
            )
            .unwrap()
        } else {
            let planes_t0 = fast_paths::transpose_u16_le_to_be_planes(bytes_le);
            let (p0, p1) = planes_t0.split_at(n);
            build_bit_split_payload_from_plane_slices(
                &[p0, p1],
                &modes,
                transform,
                level,
                usize::MAX,
            )
            .unwrap()
        };
        let mut out = Vec::new();
        write_header(&mut out, CodecMode::QatqExact, n, 1.0, checksum);
        write_qatq_exact_typed_prefix(&mut out, QATQ_EXACT_STRATEGY_BIT_SPLIT_PLANE_ZSTD, dtype);
        out.extend_from_slice(&body);
        out
    }

    #[test]
    fn bit_split_plane_zstd_f32_f16_bf16_roundtrip_transforms_and_modes() {
        let n = 128_usize;
        // 1) F32 CRZZ under T0 and T1
        let f32_crzz: Vec<f32> = (0..n)
            .map(|i| {
                let hi = 0x3f80_u32.wrapping_add(((i * 7) % 19) as u32);
                f32::from_bits(hi << 16)
            })
            .collect();
        for transform in [BIT_SPLIT_TRANSFORM_IDENTITY, BIT_SPLIT_TRANSFORM_HI16_ROT1] {
            let payload = build_bit_split_f32_payload_for_test(
                &f32_crzz,
                [
                    BIT_SPLIT_MODE_ZSTD,
                    BIT_SPLIT_MODE_ZSTD,
                    BIT_SPLIT_MODE_ZERO,
                    BIT_SPLIT_MODE_ZERO,
                ],
                transform,
                1,
            );
            assert_eq!(
                qatq_exact_strategy(&payload),
                Err(QatqError::InvalidQatqExactBody)
            );
            assert_eq!(
                f32_bits(&decode_qatq_exact(&payload).unwrap()),
                f32_bits(&f32_crzz)
            );
        }

        // 2) F32 Z, Repeat, Raw, Zstd combinations
        let f32_mixed: Vec<f32> = (0..n)
            .map(|i| {
                let b0 = 0_u32;
                let b1 = 0x7a_u32;
                let b2 = ((i * 53 + 11) & 0xff) as u32;
                let b3 = ((i / 4) & 0xff) as u32;
                f32::from_bits((b0 << 24) | (b1 << 16) | (b2 << 8) | b3)
            })
            .collect();
        for modes in [
            [
                BIT_SPLIT_MODE_ZERO,
                BIT_SPLIT_MODE_REPEAT,
                BIT_SPLIT_MODE_RAW,
                BIT_SPLIT_MODE_ZSTD,
            ],
            [
                BIT_SPLIT_MODE_ZERO,
                BIT_SPLIT_MODE_REPEAT,
                BIT_SPLIT_MODE_RAW,
                BIT_SPLIT_MODE_RAW,
            ],
            [
                BIT_SPLIT_MODE_ZSTD,
                BIT_SPLIT_MODE_ZSTD,
                BIT_SPLIT_MODE_RAW,
                BIT_SPLIT_MODE_RAW,
            ],
        ] {
            let payload = build_bit_split_f32_payload_for_test(
                &f32_mixed,
                modes,
                BIT_SPLIT_TRANSFORM_IDENTITY,
                3,
            );
            assert_eq!(
                f32_bits(&decode_qatq_exact(&payload).unwrap()),
                f32_bits(&f32_mixed)
            );
        }

        // 3) BF16 T0 and T1 with Repeat/Raw/Zstd/Zero
        let mut bf16_le = Vec::with_capacity(n * 2);
        for i in 0..n {
            let word = (0x3c00_u16).wrapping_add((i as u16) & 0x7f);
            bf16_le.extend_from_slice(&word.to_le_bytes());
        }
        for (transform, modes) in [
            (
                BIT_SPLIT_TRANSFORM_IDENTITY,
                [BIT_SPLIT_MODE_REPEAT, BIT_SPLIT_MODE_RAW],
            ),
            (
                BIT_SPLIT_TRANSFORM_IDENTITY,
                [BIT_SPLIT_MODE_ZSTD, BIT_SPLIT_MODE_RAW],
            ),
            (
                BIT_SPLIT_TRANSFORM_HI16_ROT1,
                [BIT_SPLIT_MODE_ZSTD, BIT_SPLIT_MODE_RAW],
            ),
        ] {
            let payload = build_bit_split_u16_payload_for_test(
                &bf16_le,
                TensorDType::BF16,
                modes,
                transform,
                1,
            );
            let dec = decode_qatq_exact_tensor_le(&payload).unwrap();
            assert_eq!(dec.dtype, TensorDType::BF16);
            assert_eq!(dec.bytes_le, bf16_le);
        }

        // 4) F16 T0 with Zero/Repeat/Raw/Zstd
        let f16_zero_hi: Vec<u8> = (0..n).flat_map(|i| [(i as u8), 0_u8]).collect();
        let payload_f16 = build_bit_split_u16_payload_for_test(
            &f16_zero_hi,
            TensorDType::F16,
            [BIT_SPLIT_MODE_ZERO, BIT_SPLIT_MODE_RAW],
            BIT_SPLIT_TRANSFORM_IDENTITY,
            1,
        );
        assert_eq!(
            decode_qatq_exact_tensor_le(&payload_f16).unwrap().bytes_le,
            f16_zero_hi
        );
    }

    #[test]
    fn bit_split_plane_zstd_odd_and_neon_boundary_lengths_roundtrip() {
        let mut state = 0x1234_5678_u32;
        for len in 0..=65_usize {
            let f32_vals: Vec<f32> = (0..len)
                .map(|i| {
                    state = lcg_next_for_test(state ^ (i as u32));
                    f32::from_bits(0x3f80_0000 ^ (state & 0x00ff_ffff))
                })
                .collect();
            let enc = encode_qatq_exact(&f32_vals);
            assert_eq!(
                f32_bits(&decode_qatq_exact(&enc).unwrap()),
                f32_bits(&f32_vals)
            );

            if len > 0 {
                for transform in [BIT_SPLIT_TRANSFORM_IDENTITY, BIT_SPLIT_TRANSFORM_HI16_ROT1] {
                    let p = build_bit_split_f32_payload_for_test(
                        &f32_vals,
                        [
                            BIT_SPLIT_MODE_ZSTD,
                            BIT_SPLIT_MODE_RAW,
                            BIT_SPLIT_MODE_ZSTD,
                            BIT_SPLIT_MODE_RAW,
                        ],
                        transform,
                        1,
                    );
                    assert_eq!(
                        f32_bits(&decode_qatq_exact(&p).unwrap()),
                        f32_bits(&f32_vals)
                    );
                }
            }

            let u16_le: Vec<u8> = (0..len)
                .flat_map(|i| {
                    let w = (0x4000_u16).wrapping_add((i as u16).wrapping_mul(37));
                    w.to_le_bytes()
                })
                .collect();
            let enc_bf16 = encode_qatq_exact_tensor_le(&u16_le, TensorDType::BF16);
            assert_eq!(
                decode_qatq_exact_tensor_le(&enc_bf16).unwrap().bytes_le,
                u16_le
            );
            let enc_f16 = encode_qatq_exact_tensor_le(&u16_le, TensorDType::F16);
            assert_eq!(
                decode_qatq_exact_tensor_le(&enc_f16).unwrap().bytes_le,
                u16_le
            );
        }
    }

    #[test]
    fn bit_split_plane_zstd_large_sparse_tail_rot1_regression() {
        // Trigger `element_count > BIT_SPLIT_SAMPLE_ELEMENTS` (2048) where every sampled
        // rotated byte in p0 is identical/zero, while unsampled tail entries vary.
        let n = 4096_usize;
        let mut f32_vals = Vec::with_capacity(n);
        let mut bf16_le = Vec::with_capacity(n * 2);
        for i in 0..n {
            // Alternate sign on indices not hit by `s * n / 2048` (which hits even indices i = 2*s),
            // or place an outlier strictly at an odd tail index near the end.
            let is_sampled_index = (i % 2) == 0;
            let sign_bit_hi16 = if is_sampled_index || i < 4000 {
                0_u16
            } else {
                0x8000_u16
            };
            let hi16 = sign_bit_hi16 | 0x3f80_u16 | ((i & 0x7f) as u16);
            let low16 = ((i * 97 + 13) & 0xffff) as u16;
            f32_vals.push(f32::from_bits(((hi16 as u32) << 16) | (low16 as u32)));
            bf16_le.extend_from_slice(&hi16.to_le_bytes());
        }

        let enc_f32 = encode_qatq_exact(&f32_vals);
        assert_eq!(
            f32_bits(&decode_qatq_exact(&enc_f32).unwrap()),
            f32_bits(&f32_vals)
        );

        let enc_bf16 = encode_qatq_exact_tensor_le(&bf16_le, TensorDType::BF16);
        let dec_bf16 = decode_qatq_exact_tensor_le(&enc_bf16).unwrap();
        assert_eq!(dec_bf16.dtype, TensorDType::BF16);
        assert_eq!(dec_bf16.bytes_le, bf16_le);
        assert!(qatq_exact_strategy(&enc_f32).is_ok());
        assert!(qatq_exact_strategy(&enc_bf16).is_ok());
    }

    #[test]
    fn default_encoders_are_wire_compatible_and_never_emit_strategy_11() {
        let f32_vals: Vec<f32> = (0..4096)
            .map(|i| f32::from_bits(0x3f80_0000 ^ ((i as u32).wrapping_mul(131))))
            .collect();
        let f32_fast = encode_qatq_exact(&f32_vals);
        let f32_exh = encode_qatq_exact_exhaustive(&f32_vals);
        assert!(qatq_exact_strategy(&f32_fast).is_ok());
        assert!(qatq_exact_strategy(&f32_exh).is_ok());
        assert_eq!(
            f32_bits(&decode_qatq_exact(&f32_fast).unwrap()),
            f32_bits(&f32_vals)
        );
        assert_eq!(
            f32_bits(&decode_qatq_exact(&f32_exh).unwrap()),
            f32_bits(&f32_vals)
        );

        let u16_le: Vec<u8> = (0..4096_u16)
            .flat_map(|i| (0x3e00_u16.wrapping_add(i & 0x7f) ^ ((i & 3) << 15)).to_le_bytes())
            .collect();
        for dtype in [TensorDType::BF16, TensorDType::F16] {
            let enc = encode_qatq_exact_tensor_le(&u16_le, dtype);
            assert!(qatq_exact_strategy(&enc).is_ok());
            let dec = decode_qatq_exact_tensor_le(&enc).unwrap();
            assert_eq!(dec.dtype, dtype);
            assert_eq!(dec.bytes_le, u16_le);
        }
    }

    #[test]
    fn zstd_compress_planes_flushed_bounded_roundtrip_and_early_abort() {
        for plane_len in [1_usize, 7, 15, 16, 17, 31, 33, 65, 1024, 2048, 65536] {
            for width in [2_usize, 4] {
                let mut planes = vec![0_u8; plane_len * width];
                for p in 0..width {
                    for i in 0..plane_len {
                        planes[p * plane_len + i] = match p {
                            0 => (i & 0x07) as u8,
                            1 => ((i * 17 + 3) & 0x3f) as u8,
                            2 => 0,
                            _ => 0x5a,
                        };
                    }
                }
                let bound = if plane_len >= 65 {
                    planes.len()
                } else {
                    planes.len() + 64
                };
                for level in [1, 3] {
                    let comp = fast_paths::zstd_compress_planes_flushed_bounded(
                        &planes, plane_len, width, level, bound,
                    )
                    .expect("compressible planes must succeed");
                    let decompressed = fast_paths::zstd_decompress(&comp, planes.len()).unwrap();
                    assert_eq!(decompressed, planes);
                }
            }
        }

        // High-entropy stream exceeding small budget aborts early cleanly and does not corrupt
        // subsequent calls on the thread-local CCtx.
        let mut x = 0x1234_5678_9abc_def0_u64;
        let mut random_planes = vec![0_u8; 4096 * 2];
        for b in &mut random_planes {
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            *b = (x.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 56) as u8;
        }
        assert!(
            fast_paths::zstd_compress_planes_flushed_bounded(&random_planes, 4096, 2, 1, 512,)
                .is_none()
        );
        let follow =
            fast_paths::zstd_compress_planes_flushed_bounded(&vec![0x42_u8; 256], 128, 2, 1, 256)
                .expect("subsequent encode after early abort must succeed");
        assert_eq!(
            fast_paths::zstd_decompress(&follow, 256).unwrap(),
            vec![0x42_u8; 256]
        );
    }

    #[test]
    fn native_f16_and_bf16_all_65536_bit_patterns_and_f32_specials_roundtrip() {
        let mut all_u16_le = Vec::with_capacity(65_536 * 2);
        for raw in 0_u32..=65_535 {
            all_u16_le.extend_from_slice(&(raw as u16).to_le_bytes());
        }
        for dtype in [TensorDType::F16, TensorDType::BF16] {
            let encoded = encode_qatq_exact_tensor_le(&all_u16_le, dtype);
            let decoded = decode_qatq_exact_tensor_le(&encoded).unwrap();
            assert_eq!(decoded.dtype, dtype);
            assert_eq!(decoded.bytes_le, all_u16_le);
        }

        let mut specials = Vec::with_capacity(512);
        let patterns: [u32; 16] = [
            0x0000_0000, // +0.0
            0x8000_0000, // -0.0
            0x7f80_0000, // +Inf
            0xff80_0000, // -Inf
            0x0000_0001, // min subnormal
            0x8000_0001, // -min subnormal
            0x007f_ffff, // max subnormal
            0x7f7f_ffff, // max normal
            0x7fc0_0000, // canonical qNaN
            0xffc0_0001, // -qNaN with payload
            0x7f80_0001, // sNaN payload 1
            0x7fbf_ffff, // sNaN payload max
            0xff80_1234, // -sNaN custom payload
            0x3f80_0000, // 1.0
            0xbf80_0000, // -1.0
            0x0000_1234, // positive subnormal
        ];
        for i in 0..512 {
            let bits = patterns[i % patterns.len()] ^ ((i / patterns.len()) as u32);
            specials.push(f32::from_bits(bits));
        }
        let enc = encode_qatq_exact(&specials);
        assert_eq!(
            f32_bits(&decode_qatq_exact(&enc).unwrap()),
            f32_bits(&specials)
        );
    }

    #[test]
    fn fast_paths_neon_and_checksum_zero_skip_parity() {
        const TEST_FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
        let ref_fnv = |bytes: &[u8]| -> u64 {
            let mut h = FNV_OFFSET;
            for &b in bytes {
                h ^= b as u64;
                h = h.wrapping_mul(TEST_FNV_PRIME);
            }
            h
        };
        let mut raw_bytes = vec![0_u8; 257];
        for (i, b) in raw_bytes.iter_mut().enumerate() {
            *b = if (i >= 16 && i < 80) || (i >= 128 && i < 160) {
                0
            } else {
                ((i * 31 + 7) & 0xff) as u8
            };
        }
        assert_eq!(
            fast_paths::checksum_bytes_fast(&raw_bytes),
            ref_fnv(&raw_bytes)
        );

        for len in [0_usize, 1, 7, 15, 16, 17, 31, 32, 33, 65, 131] {
            let u16_le: Vec<u8> = (0..len)
                .flat_map(|i| {
                    ((0x8123_u16).wrapping_add((i as u16).wrapping_mul(257))).to_le_bytes()
                })
                .collect();
            let planes = fast_paths::transpose_u16_le_to_be_planes(&u16_le);
            let (p0, p1) = planes.split_at(len);
            for i in 0..len {
                assert_eq!(p0[i], u16_le[i * 2 + 1]);
                assert_eq!(p1[i], u16_le[i * 2]);
            }
            let (rec_t0, c_t0) =
                fast_paths::untranspose_be_planes_to_u16_le_and_checksum(p0, p1, false);
            assert_eq!(rec_t0, u16_le);
            assert_eq!(c_t0, ref_fnv(&u16_le));

            let mut r0 = vec![0_u8; len];
            let mut r1 = vec![0_u8; len];
            fast_paths::transpose_u16_le_rot1_planes(&u16_le, &mut r0, &mut r1);
            let (rec_t1, c_t1) =
                fast_paths::untranspose_be_planes_to_u16_le_and_checksum(&r0, &r1, true);
            assert_eq!(rec_t1, u16_le);
            assert_eq!(c_t1, ref_fnv(&u16_le));

            let f32_vals: Vec<f32> = (0..len)
                .map(|i| {
                    let bits = if i % 4 == 0 {
                        0
                    } else if i % 4 == 1 {
                        0x3f80_0000
                    } else {
                        0x4123_4567_u32.wrapping_add((i as u32).wrapping_mul(0x0101_0101))
                    };
                    f32::from_bits(bits)
                })
                .collect();
            let (f_planes, f_sum) = fast_paths::transpose_f32_to_be_planes_and_checksum(&f32_vals);
            assert_eq!(f_sum, checksum_f32_bits(&f32_vals));
            let (fp0, rest) = f_planes.split_at(len);
            let (fp1, rest) = rest.split_at(len);
            let (fp2, fp3) = rest.split_at(len);
            let (rec_f_t0, rec_sum_t0) =
                fast_paths::untranspose_be_planes_to_f32_and_checksum(fp0, fp1, fp2, fp3, false);
            assert_eq!(f32_bits(&rec_f_t0), f32_bits(&f32_vals));
            assert_eq!(rec_sum_t0, f_sum);

            let mut hr0 = vec![0_u8; len];
            let mut hr1 = vec![0_u8; len];
            fast_paths::rot1_hi16_planes(fp0, fp1, &mut hr0, &mut hr1);
            let (rec_f_t1, rec_sum_t1) =
                fast_paths::untranspose_be_planes_to_f32_and_checksum(&hr0, &hr1, fp2, fp3, true);
            assert_eq!(f32_bits(&rec_f_t1), f32_bits(&f32_vals));
            assert_eq!(rec_sum_t1, f_sum);

            let f_be = encode_f32_bits_be(&f32_vals);
            let (raw_f, raw_sum) = fast_paths::bytes_be_to_f32_and_checksum(&f_be);
            assert_eq!(f32_bits(&raw_f), f32_bits(&f32_vals));
            assert_eq!(raw_sum, f_sum);

            let u16_be: Vec<u8> = u16_le.chunks_exact(2).flat_map(|c| [c[1], c[0]]).collect();
            let (swapped_le, swapped_sum) = fast_paths::swap_be_to_u16_le_and_checksum(&u16_be);
            assert_eq!(swapped_le, u16_le);
            assert_eq!(swapped_sum, ref_fnv(&u16_le));
        }

        // Branchless high-entropy checksum path parity (>= 128 bytes / >= 64 values with no zero words)
        let dense_bytes: Vec<u8> = (0..515_usize).map(|i| ((i * 73 + 17) | 1) as u8).collect();
        assert_eq!(
            fast_paths::checksum_bytes_fast(&dense_bytes),
            ref_fnv(&dense_bytes)
        );
        let dense_f32: Vec<f32> = (0..131_u32)
            .map(|i| f32::from_bits(0x3f80_0101_u32.wrapping_add(i.wrapping_mul(0x0103_0507))))
            .collect();
        assert_eq!(
            fast_paths::checksum_f32_bits_fast(&dense_f32),
            checksum_f32_bits(&dense_f32)
        );
    }

    #[test]
    fn bit_split_plane_zstd_rejects_malformed_payloads() {
        let f32_vals: Vec<f32> = (0..64)
            .map(|i| f32::from_bits(0x3f80_0000 ^ (i as u32)))
            .collect();
        let valid_f32 = build_bit_split_f32_payload_for_test(
            &f32_vals,
            [
                BIT_SPLIT_MODE_ZSTD,
                BIT_SPLIT_MODE_ZSTD,
                BIT_SPLIT_MODE_ZERO,
                BIT_SPLIT_MODE_ZSTD,
            ],
            BIT_SPLIT_TRANSFORM_IDENTITY,
            1,
        );
        let body_start = HEADER_LEN + QATQ_EXACT_PREFIX_LEN;

        // 1) Truncated bit-split header
        let mut trunc_hdr = valid_f32[..body_start + 3].to_vec();
        assert_eq!(
            decode_qatq_exact(&trunc_hdr),
            Err(QatqError::InvalidResidualStream)
        );
        trunc_hdr.clear();

        // 2) Unknown transform tag (2)
        let mut bad_transform = valid_f32.clone();
        bad_transform[body_start] = 2;
        assert_eq!(
            decode_qatq_exact(&bad_transform),
            Err(QatqError::InvalidQatqExactBody)
        );

        // 3) Invalid mode byte (4)
        let mut bad_mode = valid_f32.clone();
        bad_mode[body_start + 1] = 4;
        assert_eq!(
            decode_qatq_exact(&bad_mode),
            Err(QatqError::InvalidResidualStream)
        );

        // 4) T1 rot1 rejected on TensorDType::F16
        let f16_le = vec![0x01_u8, 0x00_u8].repeat(16);
        let mut bad_f16 = build_bit_split_u16_payload_for_test(
            &f16_le,
            TensorDType::F16,
            [BIT_SPLIT_MODE_ZERO, BIT_SPLIT_MODE_RAW],
            BIT_SPLIT_TRANSFORM_IDENTITY,
            1,
        );
        bad_f16[body_start] = BIT_SPLIT_TRANSFORM_HI16_ROT1;
        assert_eq!(
            decode_qatq_exact_tensor_le(&bad_f16),
            Err(QatqError::InvalidQatqExactBody)
        );

        // 5) Truncated repeat byte
        let ones = vec![1.0_f32; 32];
        let valid_repeat = build_bit_split_f32_payload_for_test(
            &ones,
            [
                BIT_SPLIT_MODE_REPEAT,
                BIT_SPLIT_MODE_REPEAT,
                BIT_SPLIT_MODE_ZERO,
                BIT_SPLIT_MODE_ZERO,
            ],
            BIT_SPLIT_TRANSFORM_IDENTITY,
            1,
        );
        assert_eq!(
            decode_qatq_exact(&valid_repeat[..valid_repeat.len() - 1]),
            Err(QatqError::InvalidResidualStream)
        );

        // 6) Trailing bytes when 0 Zstd planes
        let mut trailing_no_zstd = valid_repeat.clone();
        trailing_no_zstd.push(0);
        assert_eq!(
            decode_qatq_exact(&trailing_no_zstd),
            Err(QatqError::InvalidResidualStream)
        );

        // 7) Truncated raw plane
        let raw_vals: Vec<f32> = (0..64)
            .map(|i| f32::from_bits(((i as u32) * 37) << 24))
            .collect();
        let valid_raw = build_bit_split_f32_payload_for_test(
            &raw_vals,
            [
                BIT_SPLIT_MODE_RAW,
                BIT_SPLIT_MODE_ZERO,
                BIT_SPLIT_MODE_ZERO,
                BIT_SPLIT_MODE_ZERO,
            ],
            BIT_SPLIT_TRANSFORM_IDENTITY,
            1,
        );
        assert_eq!(
            decode_qatq_exact(&valid_raw[..valid_raw.len() - 5]),
            Err(QatqError::InvalidResidualStream)
        );

        // 8) Empty Zstd payload when >= 1 Zstd plane
        let mut empty_zstd = Vec::new();
        let (_, checksum) = fast_paths::transpose_f32_to_be_planes_and_checksum(&f32_vals);
        write_header(
            &mut empty_zstd,
            CodecMode::QatqExact,
            f32_vals.len(),
            1.0,
            checksum,
        );
        write_qatq_exact_prefix(&mut empty_zstd, QATQ_EXACT_STRATEGY_BIT_SPLIT_PLANE_ZSTD);
        empty_zstd.extend_from_slice(&[
            BIT_SPLIT_TRANSFORM_IDENTITY,
            BIT_SPLIT_MODE_ZSTD,
            BIT_SPLIT_MODE_ZERO,
            BIT_SPLIT_MODE_ZERO,
            BIT_SPLIT_MODE_ZERO,
        ]);
        assert_eq!(
            decode_qatq_exact(&empty_zstd),
            Err(QatqError::InvalidResidualStream)
        );

        // 9) Corrupt Zstd payload
        let mut corrupt_zstd = empty_zstd.clone();
        corrupt_zstd.extend_from_slice(&[0xff, 0x00, 0x11, 0x22]);
        assert_eq!(
            decode_qatq_exact(&corrupt_zstd),
            Err(QatqError::InvalidResidualStream)
        );

        // 10) Checksum mismatch
        let mut bad_sum = valid_repeat.clone();
        bad_sum[HEADER_LEN - 1] ^= 0x01;
        assert!(matches!(
            decode_qatq_exact(&bad_sum),
            Err(QatqError::ChecksumMismatch { .. })
        ));
    }

    #[test]
    fn qatc_container_dedup_checksum_single_multi_truncated_and_corrupt() {
        // Multi-chunk and single-chunk f32 containers
        let values: Vec<f32> = (0..1024).map(|i| (i as f32) * 0.25 - 12.5).collect();
        for chunk_size in [1024_usize, 300_usize] {
            let container = encode_qatq_exact_container(&values, chunk_size).unwrap();
            let decoded =
                decode_qatq_exact_container_with_limits(&container, QatcDecodeLimits::default())
                    .unwrap();
            assert_eq!(f32_bits(&decoded), f32_bits(&values));

            let words = decode_qatq_exact_u32_container_with_limits(
                &container,
                QatcDecodeLimits::default(),
                chunk_size,
            )
            .unwrap();
            assert_eq!(words, f32_bits(&values));

            let mut visited = Vec::new();
            for_each_qatq_exact_u32_container_chunk_with_limits(
                &container,
                QatcDecodeLimits::default(),
                chunk_size,
                |chunk| {
                    visited.extend_from_slice(chunk);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(visited, f32_bits(&values));

            // Truncated tail rejected
            assert!(
                decode_qatq_exact_u32_container_with_limits(
                    &container[..container.len() - 1],
                    QatcDecodeLimits::default(),
                    chunk_size,
                )
                .is_err()
            );

            // Corrupted container checksum rejected BEFORE visitor callback runs
            let mut corrupt = container.clone();
            corrupt[CONTAINER_V2_HEADER_LEN - 1] ^= 0xff;
            let mut callback_invoked = false;
            let err = for_each_qatq_exact_u32_container_chunk_with_limits(
                &corrupt,
                QatcDecodeLimits::default(),
                chunk_size,
                |_chunk| {
                    callback_invoked = true;
                    Ok(())
                },
            );
            assert!(!callback_invoked);
            assert!(matches!(err, Err(QatqError::ChecksumMismatch { .. })));
        }

        // Opaque byte containers single and multi-chunk
        let opaque_bytes: Vec<u8> = (0..1_203_usize)
            .map(|i| ((i * 37 + 11) & 0xff) as u8)
            .collect();
        for chunk_words in [400_usize, 100_usize] {
            let container = encode_qatq_exact_bytes_container(&opaque_bytes, chunk_words).unwrap();
            let decoded = decode_qatq_exact_bytes_container_with_limits(
                &container,
                opaque_bytes.len(),
                QatcDecodeLimits::default(),
                chunk_words,
            )
            .unwrap();
            assert_eq!(decoded, opaque_bytes);

            let mut visited_bytes = Vec::new();
            for_each_qatq_exact_bytes_container_chunk_with_limits(
                &container,
                opaque_bytes.len(),
                QatcDecodeLimits::default(),
                chunk_words,
                |bytes| {
                    visited_bytes.extend_from_slice(bytes);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(visited_bytes, opaque_bytes);
        }
    }
}
