use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    path::Path,
};

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
pub const LIVE_VRAM_ADAPTER_CONTRACT_VERSION: &str = "qatq-live-vram-adapter-v0";
pub const LIVE_VRAM_API_STATUS: &str = "experimental";
pub const LIVE_VRAM_PAGE_SEAL_VERSION: u8 = 1;
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
    MetadataSealMismatch,
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
            Self::MetadataSealMismatch => write!(f, "live VRAM metadata seal mismatch"),
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

    fn seal_id(self) -> u8 {
        match self {
            Self::F32 => 0,
            Self::F16 => 1,
            Self::BF16 => 2,
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
        if !self.raw_f32le_len.is_multiple_of(4) {
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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum KvPageKind {
    Key,
    Value,
}

impl KvPageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Key => "key",
            Self::Value => "value",
        }
    }

    fn seal_id(self) -> u8 {
        match self {
            Self::Key => 0,
            Self::Value => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KvPageLayout {
    Contiguous,
    Transposed,
    Blocked,
    Paged,
    RuntimeSpecific,
}

impl KvPageLayout {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Contiguous => "contiguous",
            Self::Transposed => "transposed",
            Self::Blocked => "blocked",
            Self::Paged => "paged",
            Self::RuntimeSpecific => "runtime-specific",
        }
    }

    fn seal_id(self) -> u8 {
        match self {
            Self::Contiguous => 0,
            Self::Transposed => 1,
            Self::Blocked => 2,
            Self::Paged => 3,
            Self::RuntimeSpecific => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveVramStorage {
    Qatq,
    RawTypedPassThrough,
}

impl LiveVramStorage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Qatq => "qatq-live",
            Self::RawTypedPassThrough => "raw-typed-pass-through",
        }
    }

    pub fn from_label(label: &str) -> Result<Self, QatqError> {
        match label {
            "qatq-live" => Ok(Self::Qatq),
            "raw-typed-pass-through" => Ok(Self::RawTypedPassThrough),
            _ => Err(QatqError::InvalidHeader),
        }
    }

    fn seal_id(self) -> u8 {
        match self {
            Self::Qatq => 0,
            Self::RawTypedPassThrough => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveVramGpuAllocationGranularity {
    PerPage,
    WholeTensor,
    WholeContext,
    RuntimeUnknown,
}

impl LiveVramGpuAllocationGranularity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PerPage => "per-page",
            Self::WholeTensor => "whole-tensor",
            Self::WholeContext => "whole-context",
            Self::RuntimeUnknown => "runtime-unknown",
        }
    }

    pub fn from_label(label: &str) -> Result<Self, QatqError> {
        match label {
            "per-page" => Ok(Self::PerPage),
            "whole-tensor" => Ok(Self::WholeTensor),
            "whole-context" => Ok(Self::WholeContext),
            "runtime-unknown" => Ok(Self::RuntimeUnknown),
            _ => Err(QatqError::InvalidHeader),
        }
    }

    pub fn can_reclaim_logical_pages(self) -> bool {
        self == Self::PerPage
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveVramPageResidencyGranularity {
    PerPage,
    RuntimeUnknown,
}

impl LiveVramPageResidencyGranularity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PerPage => "per-page",
            Self::RuntimeUnknown => "runtime-unknown",
        }
    }

    pub fn from_label(label: &str) -> Result<Self, QatqError> {
        match label {
            "per-page" => Ok(Self::PerPage),
            "runtime-unknown" => Ok(Self::RuntimeUnknown),
            _ => Err(QatqError::InvalidHeader),
        }
    }

    pub fn can_track_logical_pages(self) -> bool {
        self == Self::PerPage
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveVramResidencyEstimate {
    pub allocation_granularity: LiveVramGpuAllocationGranularity,
    pub gpu_context_bytes_before: usize,
    pub logical_offloaded_raw_bytes: usize,
    pub stored_cpu_bytes: usize,
    pub reclaimable_gpu_bytes: usize,
    pub gpu_context_bytes_after: usize,
}

impl LiveVramResidencyEstimate {
    pub fn gpu_saved_ratio(&self) -> Option<f64> {
        if self.gpu_context_bytes_before == 0 {
            return None;
        }
        Some(self.reclaimable_gpu_bytes as f64 / self.gpu_context_bytes_before as f64)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveVramKeepReason {
    UnknownNextUse,
    InsideHotWindow,
    InsidePrefetchWindow,
    QueueFull,
    CpuBudgetExceeded,
    CodecNotBeneficial,
}

impl LiveVramKeepReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnknownNextUse => "unknown-next-use",
            Self::InsideHotWindow => "inside-hot-window",
            Self::InsidePrefetchWindow => "inside-prefetch-window",
            Self::QueueFull => "queue-full",
            Self::CpuBudgetExceeded => "cpu-budget-exceeded",
            Self::CodecNotBeneficial => "codec-not-beneficial",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveVramScheduleDecision {
    Offload,
    KeepResident(LiveVramKeepReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveVramRestoreStatus {
    Restored,
    ChecksumFailure,
    MetadataMismatch,
    ResourceLimitRejected,
    MissingPayload,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveVramLimits {
    pub max_page_bytes: usize,
    pub max_stored_bytes: usize,
    pub max_shape_rank: usize,
    pub max_shape_elements: usize,
    pub max_runtime_id_len: usize,
    pub max_model_id_len: usize,
}

impl Default for LiveVramLimits {
    fn default() -> Self {
        Self {
            max_page_bytes: DEFAULT_MAX_QATC_CHUNK_BYTES,
            max_stored_bytes: DEFAULT_MAX_QATC_CHUNK_BYTES,
            max_shape_rank: 8,
            max_shape_elements: MAX_VALUES_PER_PAYLOAD,
            max_runtime_id_len: 128,
            max_model_id_len: 256,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveVramSchedulerPolicy {
    pub hot_window_tokens: u64,
    pub prefetch_window_tokens: u64,
    pub max_queued_pages: usize,
    pub max_cpu_stored_bytes: usize,
    pub require_qatq_beats_best_general_codec: bool,
}

impl Default for LiveVramSchedulerPolicy {
    fn default() -> Self {
        Self {
            hot_window_tokens: 128,
            prefetch_window_tokens: 32,
            max_queued_pages: 1024,
            max_cpu_stored_bytes: 512 * 1024 * 1024,
            require_qatq_beats_best_general_codec: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveVramSchedulerState {
    pub current_token: u64,
    pub queued_pages: usize,
    pub cpu_stored_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveVramAdapterIdentity {
    pub runtime_id: String,
    pub runtime_commit: String,
    pub adapter_version: String,
    pub adapter_contract_version: String,
}

impl LiveVramAdapterIdentity {
    pub fn validate(&self, limits: LiveVramLimits) -> Result<(), QatqError> {
        validate_live_vram_identifier(&self.runtime_id, limits.max_runtime_id_len)?;
        validate_live_vram_identifier(&self.runtime_commit, limits.max_runtime_id_len)?;
        validate_live_vram_identifier(&self.adapter_version, limits.max_runtime_id_len)?;
        if self.adapter_contract_version != LIVE_VRAM_ADAPTER_CONTRACT_VERSION {
            return Err(QatqError::InvalidHeader);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiveVramAdapterMetrics {
    pub resident_pages: usize,
    pub offloaded_pages: usize,
    pub pending_pages: usize,
    pub encode_failures: usize,
    pub restore_failures: usize,
    pub checksum_failures: usize,
    pub restore_stalls: usize,
    pub restore_stall_ns_total: u128,
    pub current_gpu_bytes: usize,
    pub peak_gpu_bytes: usize,
    pub cpu_stored_bytes: usize,
    pub shadow_cpu_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveVramOperatorMetrics {
    pub pages_resident_gpu: usize,
    pub pages_offloaded_cpu_raw: usize,
    pub pages_offloaded_qatq: usize,
    pub offload_bytes_raw: usize,
    pub offload_bytes_stored: usize,
    pub encode_failures_total: usize,
    pub restore_failures_total: usize,
    pub checksum_failures_total: usize,
    pub restore_stalls_total: usize,
    pub restore_stall_ns_total: u128,
    pub offload_skipped_total: usize,
    pub pass_through_total: usize,
    pub shadow_validation_bytes: usize,
}

impl LiveVramOperatorMetrics {
    pub fn from_store(store: &LiveVramOffloadStore, resident_gpu_pages: usize) -> Self {
        let metrics = store.metrics();
        let mut pages_offloaded_cpu_raw = 0_usize;
        let mut pages_offloaded_qatq = 0_usize;
        let mut offload_bytes_raw = 0_usize;
        let mut pass_through_total = 0_usize;
        for entry in store.entries.values() {
            offload_bytes_raw = offload_bytes_raw.saturating_add(entry.metadata.descriptor.raw_len);
            match entry.metadata.storage {
                LiveVramStorage::Qatq => pages_offloaded_qatq += 1,
                LiveVramStorage::RawTypedPassThrough => {
                    pages_offloaded_cpu_raw += 1;
                    pass_through_total += 1;
                }
            }
        }

        Self {
            pages_resident_gpu: resident_gpu_pages,
            pages_offloaded_cpu_raw,
            pages_offloaded_qatq,
            offload_bytes_raw,
            offload_bytes_stored: metrics.cpu_stored_bytes,
            encode_failures_total: metrics.encode_failures,
            restore_failures_total: metrics.restore_failures,
            checksum_failures_total: metrics.checksum_failures,
            restore_stalls_total: metrics.restore_stalls,
            restore_stall_ns_total: metrics.restore_stall_ns_total,
            offload_skipped_total: 0,
            pass_through_total,
            shadow_validation_bytes: metrics.shadow_cpu_bytes,
        }
    }

    pub fn with_offload_skipped_total(mut self, offload_skipped_total: usize) -> Self {
        self.offload_skipped_total = offload_skipped_total;
        self
    }

    pub fn to_prometheus_text(&self) -> String {
        let mut out = String::new();
        push_prometheus_metric(
            &mut out,
            "qatq_live_pages_resident_gpu",
            self.pages_resident_gpu,
        );
        push_prometheus_metric(
            &mut out,
            "qatq_live_pages_offloaded_cpu_raw",
            self.pages_offloaded_cpu_raw,
        );
        push_prometheus_metric(
            &mut out,
            "qatq_live_pages_offloaded_qatq",
            self.pages_offloaded_qatq,
        );
        push_prometheus_metric(
            &mut out,
            "qatq_live_offload_bytes_raw",
            self.offload_bytes_raw,
        );
        push_prometheus_metric(
            &mut out,
            "qatq_live_offload_bytes_stored",
            self.offload_bytes_stored,
        );
        push_prometheus_metric(
            &mut out,
            "qatq_live_encode_failures_total",
            self.encode_failures_total,
        );
        push_prometheus_metric(
            &mut out,
            "qatq_live_restore_failures_total",
            self.restore_failures_total,
        );
        push_prometheus_metric(
            &mut out,
            "qatq_live_checksum_failures_total",
            self.checksum_failures_total,
        );
        push_prometheus_metric(
            &mut out,
            "qatq_live_restore_stalls_total",
            self.restore_stalls_total,
        );
        push_prometheus_metric_u128(
            &mut out,
            "qatq_live_restore_stall_nanoseconds_total",
            self.restore_stall_ns_total,
        );
        push_prometheus_metric(
            &mut out,
            "qatq_live_offload_skipped_total",
            self.offload_skipped_total,
        );
        push_prometheus_metric(
            &mut out,
            "qatq_live_pass_through_total",
            self.pass_through_total,
        );
        push_prometheus_metric(
            &mut out,
            "qatq_live_shadow_validation_bytes",
            self.shadow_validation_bytes,
        );
        out
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveVramAdapterError {
    InvalidIdentity,
    SnapshotFailed(&'static str),
    CommitFailed(&'static str),
    RestoreFailed(&'static str),
    ResidencyQueryFailed(&'static str),
    MetricsUnavailable(&'static str),
}

impl fmt::Display for LiveVramAdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentity => write!(f, "live VRAM adapter identity is invalid"),
            Self::SnapshotFailed(reason) => write!(f, "live VRAM snapshot failed: {reason}"),
            Self::CommitFailed(reason) => write!(f, "live VRAM offload commit failed: {reason}"),
            Self::RestoreFailed(reason) => write!(f, "live VRAM restore failed: {reason}"),
            Self::ResidencyQueryFailed(reason) => {
                write!(f, "live VRAM residency query failed: {reason}")
            }
            Self::MetricsUnavailable(reason) => {
                write!(f, "live VRAM metrics unavailable: {reason}")
            }
        }
    }
}

impl std::error::Error for LiveVramAdapterError {}

pub trait LiveVramRuntimeAdapter {
    fn identity(&self) -> LiveVramAdapterIdentity;

    fn snapshot_page(
        &mut self,
        descriptor: &KvPageDescriptor,
        limits: LiveVramLimits,
    ) -> Result<KvPageSnapshot, LiveVramAdapterError>;

    fn commit_offload(
        &mut self,
        encoded: &LiveVramPageEncodeResult,
    ) -> Result<(), LiveVramAdapterError>;

    fn restore_committed_page(
        &mut self,
        metadata: &LiveVramPageMetadata,
        bytes: &[u8],
        limits: LiveVramLimits,
    ) -> Result<LiveVramRestoreStatus, LiveVramAdapterError>;

    fn restore_sealed_committed_page(
        &mut self,
        request: LiveVramSealedRestoreRequest<'_>,
        limits: LiveVramLimits,
    ) -> Result<LiveVramRestoreStatus, LiveVramAdapterError> {
        self.restore_committed_page(request.metadata(), request.stored_bytes(), limits)
    }

    fn is_page_resident(&self, descriptor: &KvPageDescriptor)
    -> Result<bool, LiveVramAdapterError>;

    fn metrics(&self) -> Result<LiveVramAdapterMetrics, LiveVramAdapterError>;

    fn gpu_allocation_granularity(&self) -> LiveVramGpuAllocationGranularity {
        LiveVramGpuAllocationGranularity::RuntimeUnknown
    }
}

pub trait LiveVramPageScheduler {
    fn decide(
        &self,
        descriptor: &KvPageDescriptor,
        state: LiveVramSchedulerState,
    ) -> LiveVramScheduleDecision;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixedWindowLiveVramScheduler {
    pub policy: LiveVramSchedulerPolicy,
}

impl LiveVramPageScheduler for FixedWindowLiveVramScheduler {
    fn decide(
        &self,
        descriptor: &KvPageDescriptor,
        state: LiveVramSchedulerState,
    ) -> LiveVramScheduleDecision {
        schedule_live_vram_page(descriptor, state, self.policy)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LlamaCppKvExportReplayConfig {
    pub runtime_commit: String,
    pub adapter_version: String,
    pub model_id: String,
    pub max_tensors: usize,
    /// Uniform next-use token for all replayed pages. When absent, llama.cpp
    /// token-page exports use each page's `token_end` as the next-use token so
    /// hot-window scheduling can distinguish current and future token ranges.
    pub next_required_token: Option<u64>,
}

impl LlamaCppKvExportReplayConfig {
    pub fn validate(&self, limits: LiveVramLimits) -> Result<(), QatqError> {
        validate_live_vram_identifier(&self.runtime_commit, limits.max_runtime_id_len)?;
        validate_live_vram_identifier(&self.adapter_version, limits.max_runtime_id_len)?;
        validate_live_vram_identifier(&self.model_id, limits.max_model_id_len)?;
        if self.max_tensors == 0 {
            return Err(QatqError::InvalidHeader);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LlamaCppKvTensorEntry {
    pub name: String,
    pub kind: KvPageKind,
    pub stream: u32,
    pub file: String,
    pub dtype: TensorDType,
    pub token_start: u64,
    pub token_end: u64,
    pub active_cells: usize,
    pub embedding: usize,
    pub row_bytes: usize,
    pub transposed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LlamaCppKvManifest {
    pub format: String,
    pub seq_id: i64,
    pub kv_size: usize,
    pub streams: usize,
    pub live_page_residency_granularity: Option<LiveVramPageResidencyGranularity>,
    pub gpu_allocation_granularity: Option<LiveVramGpuAllocationGranularity>,
    pub gpu_context_bytes: Option<usize>,
    pub total_context_bytes: Option<usize>,
    pub gpu_resident_tensors: Option<usize>,
    pub total_tensors: Option<usize>,
    pub gpu_page_staging_bytes: Option<usize>,
    pub gpu_page_staging_tensors: Option<usize>,
    pub tensors: Vec<LlamaCppKvTensorEntry>,
}

pub fn parse_llama_cpp_kv_manifest(text: &str) -> Result<LlamaCppKvManifest, QatqError> {
    let format = json_string_field(text, "format").ok_or(QatqError::InvalidHeader)?;
    if format != "qatq-llama-cpp-kv-v1" {
        return Err(QatqError::InvalidHeader);
    }
    let seq_id = json_i64_field(text, "seq_id").ok_or(QatqError::InvalidHeader)?;
    if seq_id < 0 {
        return Err(QatqError::InvalidHeader);
    }
    let kv_size = json_usize_field(text, "kv_size").ok_or(QatqError::InvalidHeader)?;
    let streams = json_usize_field(text, "streams").ok_or(QatqError::InvalidHeader)?;
    if kv_size == 0 || streams == 0 {
        return Err(QatqError::InvalidHeader);
    }
    let live_page_residency_granularity =
        match json_string_field(text, "live_page_residency_granularity") {
            Some(label) => Some(LiveVramPageResidencyGranularity::from_label(&label)?),
            None => None,
        };
    let gpu_allocation_granularity = match json_string_field(text, "gpu_allocation_granularity") {
        Some(label) => Some(LiveVramGpuAllocationGranularity::from_label(&label)?),
        None => None,
    };
    let gpu_context_bytes = json_usize_field(text, "gpu_context_bytes");
    let total_context_bytes = json_usize_field(text, "total_context_bytes");
    let gpu_resident_tensors = json_usize_field(text, "gpu_resident_tensors");
    let total_tensors = json_usize_field(text, "total_tensors");
    let gpu_page_staging_bytes = json_usize_field(text, "gpu_page_staging_bytes");
    let gpu_page_staging_tensors = json_usize_field(text, "gpu_page_staging_tensors");
    let has_page_staging = gpu_page_staging_bytes.is_some();
    if let (Some(gpu_context_bytes), Some(total_context_bytes)) =
        (gpu_context_bytes, total_context_bytes)
        && gpu_context_bytes > total_context_bytes
        && !matches!(
            gpu_allocation_granularity,
            Some(LiveVramGpuAllocationGranularity::PerPage)
        )
        && !has_page_staging
    {
        return Err(QatqError::InvalidHeader);
    }
    if let (Some(gpu_resident_tensors), Some(total_tensors)) = (gpu_resident_tensors, total_tensors)
        && gpu_resident_tensors > total_tensors
    {
        return Err(QatqError::InvalidHeader);
    }
    let objects = json_array_object_slices(text, "tensors").ok_or(QatqError::InvalidHeader)?;
    if objects.is_empty() {
        return Err(QatqError::InvalidHeader);
    }
    if let Some(total_tensors) = total_tensors
        && total_tensors != objects.len()
    {
        return Err(QatqError::InvalidHeader);
    }
    let mut tensors = Vec::with_capacity(objects.len());
    let mut page_keys = BTreeSet::new();
    let mut page_ranges = BTreeMap::<(String, KvPageKind, u32), Vec<(usize, usize)>>::new();
    let mut files = BTreeSet::new();
    for object in objects {
        let name = json_string_field(object, "name").ok_or(QatqError::InvalidHeader)?;
        let kind = match json_string_field(object, "kind")
            .ok_or(QatqError::InvalidHeader)?
            .as_str()
        {
            "k" => KvPageKind::Key,
            "v" => KvPageKind::Value,
            _ => return Err(QatqError::InvalidHeader),
        };
        let dtype = match json_string_field(object, "dtype")
            .ok_or(QatqError::InvalidHeader)?
            .as_str()
        {
            "f16le" => TensorDType::F16,
            "bf16le" => TensorDType::BF16,
            "f32le" => TensorDType::F32,
            _ => return Err(QatqError::InvalidHeader),
        };
        let stream = json_usize_field(object, "stream").ok_or(QatqError::InvalidHeader)?;
        let active_cells =
            json_usize_field(object, "active_cells").ok_or(QatqError::InvalidHeader)?;
        let token_start = json_usize_field(object, "token_start").unwrap_or(0);
        let token_end = json_usize_field(object, "token_end")
            .unwrap_or_else(|| token_start.saturating_add(active_cells));
        if token_start >= token_end {
            return Err(QatqError::InvalidHeader);
        }
        if token_end
            .checked_sub(token_start)
            .ok_or(QatqError::InvalidHeader)?
            != active_cells
        {
            return Err(QatqError::InvalidHeader);
        }
        if token_end > kv_size {
            return Err(QatqError::InvalidHeader);
        }
        let embedding = json_usize_field(object, "embedding").ok_or(QatqError::InvalidHeader)?;
        let row_bytes = json_usize_field(object, "row_bytes").ok_or(QatqError::InvalidHeader)?;
        if active_cells == 0 || embedding == 0 || row_bytes == 0 {
            return Err(QatqError::InvalidHeader);
        }
        let expected_row_bytes = embedding
            .checked_mul(dtype.element_width())
            .ok_or(QatqError::InvalidHeader)?;
        if row_bytes != expected_row_bytes {
            return Err(QatqError::LengthMismatch {
                expected: expected_row_bytes,
                actual: row_bytes,
            });
        }
        let stream = u32::try_from(stream).map_err(|_| QatqError::InvalidHeader)?;
        if usize::try_from(stream).map_err(|_| QatqError::InvalidHeader)? >= streams {
            return Err(QatqError::InvalidHeader);
        }
        if !page_keys.insert((name.clone(), kind, stream, token_start, token_end)) {
            return Err(QatqError::InvalidHeader);
        }
        let range_key = (name.clone(), kind, stream);
        let ranges = page_ranges.entry(range_key).or_default();
        if ranges.iter().any(|(existing_start, existing_end)| {
            token_start < *existing_end && *existing_start < token_end
        }) {
            return Err(QatqError::InvalidHeader);
        }
        ranges.push((token_start, token_end));
        let transposed = json_bool_field(object, "transposed").unwrap_or(false);
        let file = json_string_field(object, "file").ok_or(QatqError::InvalidHeader)?;
        safe_llama_cpp_manifest_file(&file)?;
        validate_llama_cpp_manifest_file_matches_tensor(
            &file,
            &name,
            kind,
            stream,
            token_start,
            token_end,
        )?;
        if !files.insert(file.clone()) {
            return Err(QatqError::InvalidHeader);
        }
        tensors.push(LlamaCppKvTensorEntry {
            name,
            kind,
            stream,
            file,
            dtype,
            token_start: token_start
                .try_into()
                .map_err(|_| QatqError::InvalidHeader)?,
            token_end: token_end.try_into().map_err(|_| QatqError::InvalidHeader)?,
            active_cells,
            embedding,
            row_bytes,
            transposed,
        });
    }
    Ok(LlamaCppKvManifest {
        format,
        seq_id,
        kv_size,
        streams,
        live_page_residency_granularity,
        gpu_allocation_granularity,
        gpu_context_bytes,
        total_context_bytes,
        gpu_resident_tensors,
        total_tensors,
        gpu_page_staging_bytes,
        gpu_page_staging_tensors,
        tensors,
    })
}

pub fn live_vram_snapshots_from_llama_cpp_export_dir(
    dir: impl AsRef<Path>,
    config: &LlamaCppKvExportReplayConfig,
    limits: LiveVramLimits,
) -> Result<Vec<KvPageSnapshot>, QatqError> {
    config.validate(limits)?;
    let dir = dir.as_ref();
    let manifest_path = dir.join("manifest.json");
    let manifest_text = fs::read_to_string(&manifest_path).map_err(|_| QatqError::InvalidHeader)?;
    let manifest = parse_llama_cpp_kv_manifest(&manifest_text)?;
    if manifest.tensors.len() > config.max_tensors {
        return Err(QatqError::ContainerLimitExceeded("llama-cpp-kv-tensors"));
    }

    let mut snapshots = Vec::with_capacity(manifest.tensors.len());
    for tensor in &manifest.tensors {
        let file = safe_llama_cpp_manifest_file(&tensor.file)?;
        let bytes = fs::read(dir.join(file)).map_err(|_| QatqError::InvalidHeader)?;
        let expected_len = tensor
            .active_cells
            .checked_mul(tensor.row_bytes)
            .ok_or(QatqError::InvalidHeader)?;
        if bytes.len() != expected_len {
            return Err(QatqError::LengthMismatch {
                expected: expected_len,
                actual: bytes.len(),
            });
        }
        let layout = if tensor.kind == KvPageKind::Value && tensor.transposed {
            KvPageLayout::Transposed
        } else {
            KvPageLayout::Contiguous
        };
        let shape = if tensor.kind == KvPageKind::Value && tensor.transposed {
            vec![tensor.embedding, tensor.active_cells]
        } else {
            vec![tensor.active_cells, tensor.embedding]
        };
        let descriptor = KvPageDescriptor {
            runtime_id: "llama.cpp".to_string(),
            runtime_commit: config.runtime_commit.clone(),
            adapter_version: config.adapter_version.clone(),
            model_id: config.model_id.clone(),
            seq_id: manifest.seq_id.to_string(),
            layer_id: layer_id_from_llama_cpp_tensor_name(&tensor.name)?,
            kind: tensor.kind,
            dtype: tensor.dtype,
            shape,
            layout,
            token_start: tensor.token_start,
            token_end: tensor.token_end,
            next_required_token: config.next_required_token.or(Some(tensor.token_end)),
            raw_len: bytes.len(),
            checksum: checksum_bytes(&bytes),
        };
        descriptor.validate(limits)?;
        snapshots.push(KvPageSnapshot {
            descriptor,
            bytes_le: bytes,
        });
    }
    Ok(snapshots)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KvPageDescriptor {
    pub runtime_id: String,
    pub runtime_commit: String,
    pub adapter_version: String,
    pub model_id: String,
    pub seq_id: String,
    pub layer_id: u32,
    pub kind: KvPageKind,
    pub dtype: TensorDType,
    pub shape: Vec<usize>,
    pub layout: KvPageLayout,
    pub token_start: u64,
    pub token_end: u64,
    pub next_required_token: Option<u64>,
    pub raw_len: usize,
    pub checksum: u64,
}

impl KvPageDescriptor {
    pub fn value_count(&self, limits: LiveVramLimits) -> Result<usize, QatqError> {
        self.validate(limits)?;
        Ok(self.raw_len / self.dtype.element_width())
    }

    pub fn validate(&self, limits: LiveVramLimits) -> Result<(), QatqError> {
        validate_live_vram_identifier(&self.runtime_id, limits.max_runtime_id_len)?;
        validate_live_vram_identifier(&self.runtime_commit, limits.max_runtime_id_len)?;
        validate_live_vram_identifier(&self.adapter_version, limits.max_runtime_id_len)?;
        validate_live_vram_identifier(&self.model_id, limits.max_model_id_len)?;
        validate_live_vram_identifier(&self.seq_id, limits.max_runtime_id_len)?;
        if self.token_start >= self.token_end {
            return Err(QatqError::InvalidHeader);
        }
        if self.raw_len == 0 || self.raw_len > limits.max_page_bytes {
            return Err(QatqError::ContainerLimitExceeded("live-vram-page-bytes"));
        }
        let width = self.dtype.element_width();
        if !self.raw_len.is_multiple_of(width) {
            return Err(QatqError::InvalidHeader);
        }
        if self.shape.is_empty() || self.shape.len() > limits.max_shape_rank {
            return Err(QatqError::InvalidHeader);
        }
        let mut elements = 1_usize;
        for dim in &self.shape {
            if *dim == 0 {
                return Err(QatqError::InvalidHeader);
            }
            elements = elements.checked_mul(*dim).ok_or(QatqError::InvalidHeader)?;
        }
        if elements > limits.max_shape_elements {
            return Err(QatqError::ValueCountTooLarge(elements));
        }
        let expected_len = elements
            .checked_mul(width)
            .ok_or(QatqError::InvalidHeader)?;
        if expected_len != self.raw_len {
            return Err(QatqError::LengthMismatch {
                expected: expected_len,
                actual: self.raw_len,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KvPageSnapshot {
    pub descriptor: KvPageDescriptor,
    pub bytes_le: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveVramPageMetadata {
    pub descriptor: KvPageDescriptor,
    pub storage: LiveVramStorage,
    pub stored_len: usize,
    pub strategy: Option<QatqExactStrategy>,
}

impl LiveVramPageMetadata {
    pub fn storage_label(&self) -> &'static str {
        self.storage.as_str()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveVramPageEncodeResult {
    pub metadata: LiveVramPageMetadata,
    pub bytes: Vec<u8>,
}

impl LiveVramPageEncodeResult {
    pub fn stored_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn should_compress(&self) -> bool {
        self.metadata.storage == LiveVramStorage::Qatq
    }

    pub fn should_pass_through(&self) -> bool {
        self.metadata.storage == LiveVramStorage::RawTypedPassThrough
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveVramPageSeal {
    pub version: u8,
    pub tag: [u8; 32],
}

pub fn seal_live_vram_page(
    metadata: &LiveVramPageMetadata,
    bytes: &[u8],
    key: &[u8; 32],
    context: &[u8],
    limits: LiveVramLimits,
) -> Result<LiveVramPageSeal, QatqError> {
    validate_live_vram_seal_inputs(metadata, bytes, limits)?;
    Ok(LiveVramPageSeal {
        version: LIVE_VRAM_PAGE_SEAL_VERSION,
        tag: compute_live_vram_page_seal_tag(metadata, bytes, key, context),
    })
}

pub fn verify_live_vram_page_seal(
    metadata: &LiveVramPageMetadata,
    bytes: &[u8],
    seal: &LiveVramPageSeal,
    key: &[u8; 32],
    context: &[u8],
    limits: LiveVramLimits,
) -> Result<(), QatqError> {
    validate_live_vram_seal_inputs(metadata, bytes, limits)?;
    if seal.version != LIVE_VRAM_PAGE_SEAL_VERSION {
        return Err(QatqError::MetadataSealMismatch);
    }
    let expected = compute_live_vram_page_seal_tag(metadata, bytes, key, context);
    if !constant_time_eq(&expected, &seal.tag) {
        return Err(QatqError::MetadataSealMismatch);
    }
    Ok(())
}

fn validate_live_vram_seal_inputs(
    metadata: &LiveVramPageMetadata,
    bytes: &[u8],
    limits: LiveVramLimits,
) -> Result<(), QatqError> {
    metadata.descriptor.validate(limits)?;
    if metadata.stored_len > limits.max_stored_bytes {
        return Err(QatqError::ContainerLimitExceeded("live-vram-stored-bytes"));
    }
    if bytes.len() != metadata.stored_len {
        return Err(QatqError::LengthMismatch {
            expected: metadata.stored_len,
            actual: bytes.len(),
        });
    }
    Ok(())
}

fn compute_live_vram_page_seal_tag(
    metadata: &LiveVramPageMetadata,
    bytes: &[u8],
    key: &[u8; 32],
    context: &[u8],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_keyed(key);
    hasher.update(b"qatq-live-vram-page-seal-v1");
    seal_update_u8(&mut hasher, LIVE_VRAM_PAGE_SEAL_VERSION);
    seal_update_bytes(&mut hasher, context);
    seal_update_descriptor(&mut hasher, &metadata.descriptor);
    seal_update_u8(&mut hasher, metadata.storage.seal_id());
    seal_update_u64(&mut hasher, metadata.stored_len as u64);
    match metadata.strategy {
        Some(strategy) => {
            seal_update_u8(&mut hasher, 1);
            seal_update_u8(&mut hasher, strategy.id());
        }
        None => seal_update_u8(&mut hasher, 0),
    }
    seal_update_bytes(&mut hasher, bytes);
    *hasher.finalize().as_bytes()
}

fn seal_update_descriptor(hasher: &mut blake3::Hasher, descriptor: &KvPageDescriptor) {
    seal_update_bytes(hasher, descriptor.runtime_id.as_bytes());
    seal_update_bytes(hasher, descriptor.runtime_commit.as_bytes());
    seal_update_bytes(hasher, descriptor.adapter_version.as_bytes());
    seal_update_bytes(hasher, descriptor.model_id.as_bytes());
    seal_update_bytes(hasher, descriptor.seq_id.as_bytes());
    seal_update_u32(hasher, descriptor.layer_id);
    seal_update_u8(hasher, descriptor.kind.seal_id());
    seal_update_u8(hasher, descriptor.dtype.seal_id());
    seal_update_u64(hasher, descriptor.shape.len() as u64);
    for dim in &descriptor.shape {
        seal_update_u64(hasher, *dim as u64);
    }
    seal_update_u8(hasher, descriptor.layout.seal_id());
    seal_update_u64(hasher, descriptor.token_start);
    seal_update_u64(hasher, descriptor.token_end);
    match descriptor.next_required_token {
        Some(token) => {
            seal_update_u8(hasher, 1);
            seal_update_u64(hasher, token);
        }
        None => seal_update_u8(hasher, 0),
    }
    seal_update_u64(hasher, descriptor.raw_len as u64);
    seal_update_u64(hasher, descriptor.checksum);
}

fn seal_update_bytes(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    seal_update_u64(hasher, bytes.len() as u64);
    hasher.update(bytes);
}

fn seal_update_u8(hasher: &mut blake3::Hasher, value: u8) {
    hasher.update(&[value]);
}

fn seal_update_u32(hasher: &mut blake3::Hasher, value: u32) {
    hasher.update(&value.to_be_bytes());
}

fn seal_update_u64(hasher: &mut blake3::Hasher, value: u64) {
    hasher.update(&value.to_be_bytes());
}

fn constant_time_eq(left: &[u8; 32], right: &[u8; 32]) -> bool {
    let mut diff = 0_u8;
    for (left, right) in left.iter().zip(right.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct KvPageKey {
    pub runtime_id: String,
    pub model_id: String,
    pub seq_id: String,
    pub layer_id: u32,
    pub kind: KvPageKind,
    pub token_start: u64,
    pub token_end: u64,
}

impl KvPageKey {
    pub fn from_descriptor(descriptor: &KvPageDescriptor) -> Self {
        Self {
            runtime_id: descriptor.runtime_id.clone(),
            model_id: descriptor.model_id.clone(),
            seq_id: descriptor.seq_id.clone(),
            layer_id: descriptor.layer_id,
            kind: descriptor.kind,
            token_start: descriptor.token_start,
            token_end: descriptor.token_end,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveVramOffloadEntry {
    pub metadata: LiveVramPageMetadata,
    pub bytes: Vec<u8>,
    pub metadata_seal: Option<LiveVramPageSeal>,
    pub shadow_bytes: Option<Vec<u8>>,
}

impl LiveVramOffloadEntry {
    pub fn stored_len(&self) -> usize {
        self.bytes.len()
    }

    pub fn shadow_len(&self) -> usize {
        self.shadow_bytes.as_ref().map(Vec::len).unwrap_or(0)
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct LiveVramPageSealPolicy {
    key: [u8; 32],
    context: Vec<u8>,
}

impl LiveVramPageSealPolicy {
    pub fn new(key: [u8; 32], context: impl Into<Vec<u8>>) -> Result<Self, QatqError> {
        let context = context.into();
        if context.is_empty() {
            return Err(QatqError::InvalidHeader);
        }
        Ok(Self { key, context })
    }

    pub fn verify_restore_request<'a>(
        &self,
        metadata: &'a LiveVramPageMetadata,
        bytes: &'a [u8],
        seal: &'a LiveVramPageSeal,
        limits: LiveVramLimits,
    ) -> Result<LiveVramSealedRestoreRequest<'a>, QatqError> {
        verify_live_vram_page_seal(metadata, bytes, seal, &self.key, &self.context, limits)?;
        Ok(LiveVramSealedRestoreRequest {
            metadata,
            bytes,
            metadata_seal: seal,
        })
    }
}

impl fmt::Debug for LiveVramPageSealPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LiveVramPageSealPolicy")
            .field("key", &"<redacted>")
            .field("context_len", &self.context.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveVramSealedRestoreRequest<'a> {
    metadata: &'a LiveVramPageMetadata,
    bytes: &'a [u8],
    metadata_seal: &'a LiveVramPageSeal,
}

impl<'a> LiveVramSealedRestoreRequest<'a> {
    pub fn metadata(&self) -> &'a LiveVramPageMetadata {
        self.metadata
    }

    pub fn stored_bytes(&self) -> &'a [u8] {
        self.bytes
    }

    pub fn metadata_seal(&self) -> &'a LiveVramPageSeal {
        self.metadata_seal
    }

    pub fn restore_bytes(&self, limits: LiveVramLimits) -> Result<Vec<u8>, QatqError> {
        restore_live_vram_page(self.metadata, self.bytes, limits)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveVramOffloadStore {
    limits: LiveVramLimits,
    max_entries: usize,
    max_cpu_stored_bytes: usize,
    keep_shadow_copy: bool,
    max_shadow_bytes: usize,
    page_seal_policy: Option<LiveVramPageSealPolicy>,
    entries: BTreeMap<KvPageKey, LiveVramOffloadEntry>,
    metrics: LiveVramAdapterMetrics,
}

impl LiveVramOffloadStore {
    pub fn new(limits: LiveVramLimits, max_entries: usize, max_cpu_stored_bytes: usize) -> Self {
        Self {
            limits,
            max_entries,
            max_cpu_stored_bytes,
            keep_shadow_copy: false,
            max_shadow_bytes: 0,
            page_seal_policy: None,
            entries: BTreeMap::new(),
            metrics: LiveVramAdapterMetrics::default(),
        }
    }

    pub fn new_with_shadow_validation(
        limits: LiveVramLimits,
        max_entries: usize,
        max_cpu_stored_bytes: usize,
        max_shadow_bytes: usize,
    ) -> Self {
        Self {
            limits,
            max_entries,
            max_cpu_stored_bytes,
            keep_shadow_copy: true,
            max_shadow_bytes,
            page_seal_policy: None,
            entries: BTreeMap::new(),
            metrics: LiveVramAdapterMetrics::default(),
        }
    }

    pub fn with_page_seal_policy(mut self, policy: LiveVramPageSealPolicy) -> LiveVramOffloadStore {
        self.page_seal_policy = Some(policy);
        self
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains_key(&self, key: &KvPageKey) -> bool {
        self.entries.contains_key(key)
    }

    pub fn entry(&self, key: &KvPageKey) -> Option<&LiveVramOffloadEntry> {
        self.entries.get(key)
    }

    pub fn metrics(&self) -> LiveVramAdapterMetrics {
        self.metrics.clone()
    }

    pub fn requires_sealed_restore_requests(&self) -> bool {
        self.page_seal_policy.is_some()
    }

    pub fn commit_snapshot(&mut self, snapshot: &KvPageSnapshot) -> Result<KvPageKey, QatqError> {
        let encoded = try_encode_live_vram_page(snapshot, self.limits)?;
        self.commit_encoded_with_shadow(encoded, self.shadow_copy_for_snapshot(snapshot)?)
    }

    pub fn commit_snapshot_for_runtime(
        &mut self,
        snapshot: &KvPageSnapshot,
    ) -> Result<(KvPageKey, LiveVramPageEncodeResult), QatqError> {
        let encoded = try_encode_live_vram_page(snapshot, self.limits)?;
        let key = self.commit_encoded_with_shadow(
            encoded.clone(),
            self.shadow_copy_for_snapshot(snapshot)?,
        )?;
        Ok((key, encoded))
    }

    pub fn commit_encoded(
        &mut self,
        encoded: LiveVramPageEncodeResult,
    ) -> Result<KvPageKey, QatqError> {
        self.commit_encoded_with_shadow(encoded, None)
    }

    fn commit_encoded_with_shadow(
        &mut self,
        encoded: LiveVramPageEncodeResult,
        shadow_bytes: Option<Vec<u8>>,
    ) -> Result<KvPageKey, QatqError> {
        encoded.metadata.descriptor.validate(self.limits)?;
        if self.entries.len() >= self.max_entries {
            self.metrics.encode_failures += 1;
            return Err(QatqError::ContainerLimitExceeded(
                "live-vram-offload-entries",
            ));
        }
        let key = KvPageKey::from_descriptor(&encoded.metadata.descriptor);
        if self.entries.contains_key(&key) {
            self.metrics.encode_failures += 1;
            return Err(QatqError::InvalidHeader);
        }
        let candidate_cpu_bytes = self
            .metrics
            .cpu_stored_bytes
            .checked_add(encoded.bytes.len())
            .ok_or(QatqError::ContainerLimitExceeded(
                "live-vram-offload-cpu-bytes",
            ))?;
        let candidate_shadow_bytes = self
            .metrics
            .shadow_cpu_bytes
            .checked_add(shadow_bytes.as_ref().map(Vec::len).unwrap_or(0))
            .ok_or(QatqError::ContainerLimitExceeded("live-vram-shadow-bytes"))?;
        if candidate_shadow_bytes > self.max_shadow_bytes {
            self.metrics.encode_failures += 1;
            return Err(QatqError::ContainerLimitExceeded("live-vram-shadow-bytes"));
        }
        if candidate_cpu_bytes > self.max_cpu_stored_bytes {
            self.metrics.encode_failures += 1;
            return Err(QatqError::ContainerLimitExceeded(
                "live-vram-offload-cpu-bytes",
            ));
        }
        let restored = match restore_live_vram_page(&encoded.metadata, &encoded.bytes, self.limits)
        {
            Ok(restored) => restored,
            Err(error) => {
                self.metrics.restore_failures += 1;
                if matches!(error, QatqError::ChecksumMismatch { .. }) {
                    self.metrics.checksum_failures += 1;
                }
                return Err(error);
            }
        };
        if let Some(shadow) = &shadow_bytes
            && restored != *shadow
        {
            self.metrics.checksum_failures += 1;
            return Err(QatqError::ChecksumMismatch {
                expected: encoded.metadata.descriptor.checksum,
                actual: checksum_bytes(&restored),
            });
        }
        if checksum_bytes(&restored) != encoded.metadata.descriptor.checksum {
            self.metrics.checksum_failures += 1;
            return Err(QatqError::ChecksumMismatch {
                expected: encoded.metadata.descriptor.checksum,
                actual: checksum_bytes(&restored),
            });
        }
        let metadata_seal = match self.create_metadata_seal(&encoded.metadata, &encoded.bytes) {
            Ok(seal) => seal,
            Err(error) => {
                self.metrics.encode_failures += 1;
                return Err(error);
            }
        };
        let raw_len = encoded.metadata.descriptor.raw_len;
        self.entries.insert(
            key.clone(),
            LiveVramOffloadEntry {
                metadata: encoded.metadata,
                bytes: encoded.bytes,
                metadata_seal,
                shadow_bytes,
            },
        );
        self.metrics.offloaded_pages += 1;
        self.metrics.pending_pages = self.entries.len();
        self.metrics.cpu_stored_bytes = candidate_cpu_bytes;
        self.metrics.shadow_cpu_bytes = candidate_shadow_bytes;
        self.metrics.current_gpu_bytes = self.metrics.current_gpu_bytes.saturating_sub(raw_len);
        Ok(key)
    }

    pub fn restore(&mut self, key: &KvPageKey) -> Result<Vec<u8>, QatqError> {
        let Some(entry) = self.entries.get(key).cloned() else {
            self.metrics.restore_failures += 1;
            return Err(QatqError::InvalidHeader);
        };
        if let Err(error) = self.verify_entry_metadata_seal(&entry) {
            self.metrics.restore_failures += 1;
            return Err(error);
        }
        match restore_live_vram_page(&entry.metadata, &entry.bytes, self.limits) {
            Ok(bytes) => {
                if let Some(shadow) = &entry.shadow_bytes
                    && bytes != *shadow
                {
                    self.metrics.checksum_failures += 1;
                    return Err(QatqError::ChecksumMismatch {
                        expected: entry.metadata.descriptor.checksum,
                        actual: checksum_bytes(&bytes),
                    });
                }
                Ok(bytes)
            }
            Err(error) => {
                self.metrics.restore_failures += 1;
                if matches!(error, QatqError::ChecksumMismatch { .. }) {
                    self.metrics.checksum_failures += 1;
                }
                Err(error)
            }
        }
    }

    pub fn sealed_restore_request<'a>(
        &'a self,
        key: &KvPageKey,
    ) -> Result<LiveVramSealedRestoreRequest<'a>, QatqError> {
        let Some(policy) = &self.page_seal_policy else {
            return Err(QatqError::MetadataSealMismatch);
        };
        let entry = self.entries.get(key).ok_or(QatqError::InvalidHeader)?;
        let seal = entry
            .metadata_seal
            .as_ref()
            .ok_or(QatqError::MetadataSealMismatch)?;
        policy.verify_restore_request(&entry.metadata, &entry.bytes, seal, self.limits)
    }

    pub fn restore_and_remove(&mut self, key: &KvPageKey) -> Result<Vec<u8>, QatqError> {
        let bytes = self.restore(key)?;
        self.remove(key);
        Ok(bytes)
    }

    pub fn remove(&mut self, key: &KvPageKey) -> Option<LiveVramOffloadEntry> {
        let removed = self.entries.remove(key)?;
        self.metrics.cpu_stored_bytes = self
            .metrics
            .cpu_stored_bytes
            .saturating_sub(removed.stored_len());
        self.metrics.shadow_cpu_bytes = self
            .metrics
            .shadow_cpu_bytes
            .saturating_sub(removed.shadow_len());
        self.metrics.pending_pages = self.entries.len();
        Some(removed)
    }

    fn shadow_copy_for_snapshot(
        &self,
        snapshot: &KvPageSnapshot,
    ) -> Result<Option<Vec<u8>>, QatqError> {
        if !self.keep_shadow_copy {
            return Ok(None);
        }
        if snapshot.bytes_le.len() > self.max_shadow_bytes {
            return Err(QatqError::ContainerLimitExceeded("live-vram-shadow-bytes"));
        }
        Ok(Some(snapshot.bytes_le.clone()))
    }

    fn create_metadata_seal(
        &self,
        metadata: &LiveVramPageMetadata,
        bytes: &[u8],
    ) -> Result<Option<LiveVramPageSeal>, QatqError> {
        let Some(policy) = &self.page_seal_policy else {
            return Ok(None);
        };
        let seal = seal_live_vram_page(metadata, bytes, &policy.key, &policy.context, self.limits)?;
        verify_live_vram_page_seal(
            metadata,
            bytes,
            &seal,
            &policy.key,
            &policy.context,
            self.limits,
        )?;
        Ok(Some(seal))
    }

    fn verify_entry_metadata_seal(&self, entry: &LiveVramOffloadEntry) -> Result<(), QatqError> {
        let Some(policy) = &self.page_seal_policy else {
            return Ok(());
        };
        let Some(seal) = &entry.metadata_seal else {
            return Err(QatqError::MetadataSealMismatch);
        };
        verify_live_vram_page_seal(
            &entry.metadata,
            &entry.bytes,
            seal,
            &policy.key,
            &policy.context,
            self.limits,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveVramOffloadOutcome {
    Offloaded {
        key: KvPageKey,
        storage: LiveVramStorage,
        stored_len: usize,
    },
    KeptResident(LiveVramKeepReason),
}

#[derive(Debug, Eq, PartialEq)]
pub enum LiveVramOffloadError {
    InvalidAdapterIdentity,
    Snapshot(LiveVramAdapterError),
    Codec(QatqError),
    RuntimeCommit(LiveVramAdapterError),
}

impl fmt::Display for LiveVramOffloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAdapterIdentity => write!(f, "live VRAM adapter identity is invalid"),
            Self::Snapshot(error) => write!(f, "live VRAM snapshot failed: {error}"),
            Self::Codec(error) => write!(f, "live VRAM codec/store failed: {error}"),
            Self::RuntimeCommit(error) => write!(f, "live VRAM runtime commit failed: {error}"),
        }
    }
}

impl std::error::Error for LiveVramOffloadError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveVramMeasuredOffloadOutcome {
    Offloaded {
        key: KvPageKey,
        storage: LiveVramStorage,
        stored_len: usize,
        gpu_bytes_before: usize,
        gpu_bytes_after: usize,
        reclaimed_gpu_bytes: usize,
    },
    KeptResident(LiveVramKeepReason),
}

#[derive(Debug, Eq, PartialEq)]
pub enum LiveVramMeasuredOffloadError {
    InvalidReclaimPolicy,
    MetricsBefore(LiveVramAdapterError),
    MetricsAfter(LiveVramAdapterError),
    Offload(LiveVramOffloadError),
    InsufficientReclaim {
        key: Box<KvPageKey>,
        gpu_bytes_before: usize,
        gpu_bytes_after: usize,
        reclaimed_gpu_bytes: usize,
        min_reclaimed_bytes: usize,
    },
    Rollback(LiveVramRestoreError),
}

impl fmt::Display for LiveVramMeasuredOffloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidReclaimPolicy => write!(f, "live VRAM reclaim policy is invalid"),
            Self::MetricsBefore(error) => {
                write!(f, "live VRAM metrics before offload failed: {error}")
            }
            Self::MetricsAfter(error) => {
                write!(f, "live VRAM metrics after offload failed: {error}")
            }
            Self::Offload(error) => write!(f, "live VRAM measured offload failed: {error}"),
            Self::InsufficientReclaim {
                reclaimed_gpu_bytes,
                min_reclaimed_bytes,
                ..
            } => write!(
                f,
                "live VRAM offload reclaimed {reclaimed_gpu_bytes} GPU bytes, below required {min_reclaimed_bytes}"
            ),
            Self::Rollback(error) => write!(f, "live VRAM reclaim rollback failed: {error}"),
        }
    }
}

impl std::error::Error for LiveVramMeasuredOffloadError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveVramRestoreOutcome {
    Restored { key: KvPageKey, restored_len: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveVramCancellationStage {
    BeforeRuntimeCommit,
    AfterRuntimeCommit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveVramCancellationOutcome {
    DroppedUncommitted { key: KvPageKey },
    RestoredCommitted { key: KvPageKey, restored_len: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveVramRestoreLatencyBudget {
    pub max_restore_ns_per_page: u128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveVramTimedRestoreOutcome {
    pub key: KvPageKey,
    pub restored_len: usize,
    pub observed_restore_ns: u128,
    pub stalled: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub enum LiveVramRestoreError {
    Codec(QatqError),
    Runtime(LiveVramAdapterError),
    RuntimeStatus(LiveVramRestoreStatus),
    RuntimeResidency(LiveVramAdapterError),
    RuntimePageNotResident,
}

impl fmt::Display for LiveVramRestoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(error) => write!(f, "live VRAM restore codec/store failed: {error}"),
            Self::Runtime(error) => write!(f, "live VRAM runtime restore failed: {error}"),
            Self::RuntimeStatus(status) => {
                write!(f, "live VRAM runtime restore returned status {status:?}")
            }
            Self::RuntimeResidency(error) => {
                write!(f, "live VRAM runtime residency check failed: {error}")
            }
            Self::RuntimePageNotResident => {
                write!(
                    f,
                    "live VRAM runtime restore did not make the page resident"
                )
            }
        }
    }
}

impl std::error::Error for LiveVramRestoreError {}

pub fn try_offload_live_vram_page<A, S>(
    adapter: &mut A,
    store: &mut LiveVramOffloadStore,
    scheduler: &S,
    descriptor: &KvPageDescriptor,
    state: LiveVramSchedulerState,
    limits: LiveVramLimits,
) -> Result<LiveVramOffloadOutcome, LiveVramOffloadError>
where
    A: LiveVramRuntimeAdapter,
    S: LiveVramPageScheduler,
{
    adapter
        .identity()
        .validate(limits)
        .map_err(|_| LiveVramOffloadError::InvalidAdapterIdentity)?;
    match scheduler.decide(descriptor, state) {
        LiveVramScheduleDecision::KeepResident(reason) => {
            return Ok(LiveVramOffloadOutcome::KeptResident(reason));
        }
        LiveVramScheduleDecision::Offload => {}
    }

    let snapshot = adapter
        .snapshot_page(descriptor, limits)
        .map_err(LiveVramOffloadError::Snapshot)?;
    let (key, encoded) = store
        .commit_snapshot_for_runtime(&snapshot)
        .map_err(LiveVramOffloadError::Codec)?;
    let storage = encoded.metadata.storage;
    let stored_len = encoded.bytes.len();
    if let Err(error) = adapter.commit_offload(&encoded) {
        store.remove(&key);
        return Err(LiveVramOffloadError::RuntimeCommit(error));
    }

    Ok(LiveVramOffloadOutcome::Offloaded {
        key,
        storage,
        stored_len,
    })
}

pub fn try_offload_live_vram_page_with_reclaim_check<A, S>(
    adapter: &mut A,
    store: &mut LiveVramOffloadStore,
    scheduler: &S,
    descriptor: &KvPageDescriptor,
    state: LiveVramSchedulerState,
    limits: LiveVramLimits,
    min_reclaimed_bytes: usize,
) -> Result<LiveVramMeasuredOffloadOutcome, LiveVramMeasuredOffloadError>
where
    A: LiveVramRuntimeAdapter,
    S: LiveVramPageScheduler,
{
    if min_reclaimed_bytes == 0 {
        return Err(LiveVramMeasuredOffloadError::InvalidReclaimPolicy);
    }
    if let LiveVramScheduleDecision::KeepResident(reason) = scheduler.decide(descriptor, state) {
        return Ok(LiveVramMeasuredOffloadOutcome::KeptResident(reason));
    }

    let metrics_before = adapter
        .metrics()
        .map_err(LiveVramMeasuredOffloadError::MetricsBefore)?;
    let offload = try_offload_live_vram_page(adapter, store, scheduler, descriptor, state, limits)
        .map_err(LiveVramMeasuredOffloadError::Offload)?;
    let (key, storage, stored_len) = match offload {
        LiveVramOffloadOutcome::Offloaded {
            key,
            storage,
            stored_len,
        } => (key, storage, stored_len),
        LiveVramOffloadOutcome::KeptResident(reason) => {
            return Ok(LiveVramMeasuredOffloadOutcome::KeptResident(reason));
        }
    };
    let metrics_after = adapter
        .metrics()
        .map_err(LiveVramMeasuredOffloadError::MetricsAfter)?;
    let reclaimed_gpu_bytes = metrics_before
        .current_gpu_bytes
        .saturating_sub(metrics_after.current_gpu_bytes);
    if reclaimed_gpu_bytes < min_reclaimed_bytes {
        cancel_live_vram_offload(
            adapter,
            store,
            &key,
            LiveVramCancellationStage::AfterRuntimeCommit,
            limits,
        )
        .map_err(LiveVramMeasuredOffloadError::Rollback)?;
        return Err(LiveVramMeasuredOffloadError::InsufficientReclaim {
            key: Box::new(key),
            gpu_bytes_before: metrics_before.current_gpu_bytes,
            gpu_bytes_after: metrics_after.current_gpu_bytes,
            reclaimed_gpu_bytes,
            min_reclaimed_bytes,
        });
    }

    Ok(LiveVramMeasuredOffloadOutcome::Offloaded {
        key,
        storage,
        stored_len,
        gpu_bytes_before: metrics_before.current_gpu_bytes,
        gpu_bytes_after: metrics_after.current_gpu_bytes,
        reclaimed_gpu_bytes,
    })
}

pub fn try_restore_live_vram_page_from_store<A>(
    adapter: &mut A,
    store: &mut LiveVramOffloadStore,
    key: &KvPageKey,
    limits: LiveVramLimits,
) -> Result<LiveVramRestoreOutcome, LiveVramRestoreError>
where
    A: LiveVramRuntimeAdapter,
{
    let entry = match store.entry(key).cloned() {
        Some(entry) => entry,
        None => {
            store.metrics.restore_failures += 1;
            return Err(LiveVramRestoreError::Codec(QatqError::InvalidHeader));
        }
    };
    let restored = store.restore(key).map_err(LiveVramRestoreError::Codec)?;
    let status_result = if store.requires_sealed_restore_requests() {
        let request = store
            .sealed_restore_request(key)
            .map_err(LiveVramRestoreError::Codec)?;
        adapter.restore_sealed_committed_page(request, limits)
    } else {
        adapter.restore_committed_page(&entry.metadata, &entry.bytes, limits)
    };
    let status = match status_result {
        Ok(status) => status,
        Err(error) => {
            store.metrics.restore_failures += 1;
            return Err(LiveVramRestoreError::Runtime(error));
        }
    };
    if status != LiveVramRestoreStatus::Restored {
        store.metrics.restore_failures += 1;
        return Err(LiveVramRestoreError::RuntimeStatus(status));
    }
    let is_resident = match adapter.is_page_resident(&entry.metadata.descriptor) {
        Ok(is_resident) => is_resident,
        Err(error) => {
            store.metrics.restore_failures += 1;
            return Err(LiveVramRestoreError::RuntimeResidency(error));
        }
    };
    if !is_resident {
        store.metrics.restore_failures += 1;
        return Err(LiveVramRestoreError::RuntimePageNotResident);
    }
    store.remove(key);
    Ok(LiveVramRestoreOutcome::Restored {
        key: key.clone(),
        restored_len: restored.len(),
    })
}

pub fn cancel_live_vram_offload<A>(
    adapter: &mut A,
    store: &mut LiveVramOffloadStore,
    key: &KvPageKey,
    stage: LiveVramCancellationStage,
    limits: LiveVramLimits,
) -> Result<LiveVramCancellationOutcome, LiveVramRestoreError>
where
    A: LiveVramRuntimeAdapter,
{
    match stage {
        LiveVramCancellationStage::BeforeRuntimeCommit => {
            if store.remove(key).is_none() {
                return Err(LiveVramRestoreError::Codec(QatqError::InvalidHeader));
            }
            Ok(LiveVramCancellationOutcome::DroppedUncommitted { key: key.clone() })
        }
        LiveVramCancellationStage::AfterRuntimeCommit => {
            let restored = try_restore_live_vram_page_from_store(adapter, store, key, limits)?;
            let LiveVramRestoreOutcome::Restored { key, restored_len } = restored;
            Ok(LiveVramCancellationOutcome::RestoredCommitted { key, restored_len })
        }
    }
}

pub fn try_restore_live_vram_page_from_store_with_observed_latency<A>(
    adapter: &mut A,
    store: &mut LiveVramOffloadStore,
    key: &KvPageKey,
    limits: LiveVramLimits,
    observed_restore_ns: u128,
    budget: LiveVramRestoreLatencyBudget,
) -> Result<LiveVramTimedRestoreOutcome, LiveVramRestoreError>
where
    A: LiveVramRuntimeAdapter,
{
    if budget.max_restore_ns_per_page == 0 {
        return Err(LiveVramRestoreError::Codec(QatqError::InvalidHeader));
    }
    let restored = try_restore_live_vram_page_from_store(adapter, store, key, limits)?;
    let LiveVramRestoreOutcome::Restored { key, restored_len } = restored;
    let stalled = observed_restore_ns > budget.max_restore_ns_per_page;
    if stalled {
        store.metrics.restore_stalls += 1;
        store.metrics.restore_stall_ns_total = store
            .metrics
            .restore_stall_ns_total
            .saturating_add(observed_restore_ns);
    }
    Ok(LiveVramTimedRestoreOutcome {
        key,
        restored_len,
        observed_restore_ns,
        stalled,
    })
}

pub fn try_restore_live_vram_page_from_store_timed<A>(
    adapter: &mut A,
    store: &mut LiveVramOffloadStore,
    key: &KvPageKey,
    limits: LiveVramLimits,
    budget: LiveVramRestoreLatencyBudget,
) -> Result<LiveVramTimedRestoreOutcome, LiveVramRestoreError>
where
    A: LiveVramRuntimeAdapter,
{
    if budget.max_restore_ns_per_page == 0 {
        return Err(LiveVramRestoreError::Codec(QatqError::InvalidHeader));
    }
    let started = std::time::Instant::now();
    let restored = try_restore_live_vram_page_from_store(adapter, store, key, limits)?;
    let observed_restore_ns = started.elapsed().as_nanos();
    let LiveVramRestoreOutcome::Restored { key, restored_len } = restored;
    let stalled = observed_restore_ns > budget.max_restore_ns_per_page;
    if stalled {
        store.metrics.restore_stalls += 1;
        store.metrics.restore_stall_ns_total = store
            .metrics
            .restore_stall_ns_total
            .saturating_add(observed_restore_ns);
    }
    Ok(LiveVramTimedRestoreOutcome {
        key,
        restored_len,
        observed_restore_ns,
        stalled,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveVramSimulationReport {
    pub total_pages: usize,
    pub compressed_pages: usize,
    pub pass_through_pages: usize,
    pub resident_pages: usize,
    pub verified_restores: usize,
    pub raw_bytes: usize,
    pub stored_cpu_bytes: usize,
    pub kept_unknown_next_use: usize,
    pub kept_hot_window: usize,
    pub kept_prefetch_window: usize,
    pub kept_queue_full: usize,
    pub kept_cpu_budget: usize,
    pub kept_codec_not_beneficial: usize,
}

impl LiveVramSimulationReport {
    pub fn stored_ratio(&self) -> Option<f64> {
        if self.raw_bytes == 0 {
            return None;
        }
        Some(self.stored_cpu_bytes as f64 / self.raw_bytes as f64)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveVramPageEvidence {
    pub page_index: usize,
    pub descriptor: KvPageDescriptor,
    pub schedule_decision: LiveVramScheduleDecision,
    pub storage: Option<LiveVramStorage>,
    pub strategy: Option<QatqExactStrategy>,
    pub metadata_seal: Option<LiveVramPageSeal>,
    pub raw_bytes: usize,
    pub qatq_candidate_bytes: usize,
    pub scheduled_stored_bytes: usize,
    pub zstd_bytes: usize,
    pub lz4_bytes: usize,
    pub verified_restore: bool,
}

impl LiveVramPageEvidence {
    pub fn qatq_beats_zstd(&self) -> bool {
        self.qatq_candidate_bytes < self.zstd_bytes
    }

    pub fn qatq_beats_lz4(&self) -> bool {
        self.qatq_candidate_bytes < self.lz4_bytes
    }

    pub fn qatq_beats_best_general_codec(&self) -> bool {
        self.qatq_candidate_bytes < self.zstd_bytes.min(self.lz4_bytes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveVramEvidenceReport {
    pub adapter_contract_version: String,
    pub total_pages: usize,
    pub offloaded_pages: usize,
    pub resident_pages: usize,
    pub compressed_pages: usize,
    pub pass_through_pages: usize,
    pub sealed_pages: usize,
    pub verified_restores: usize,
    pub raw_bytes: usize,
    pub qatq_candidate_bytes: usize,
    pub scheduled_stored_bytes: usize,
    pub zstd_bytes: usize,
    pub lz4_bytes: usize,
    pub qatq_beats_zstd_pages: usize,
    pub qatq_beats_lz4_pages: usize,
    pub qatq_beats_best_general_codec_pages: usize,
    pub pages: Vec<LiveVramPageEvidence>,
}

impl LiveVramEvidenceReport {
    pub fn scheduled_stored_ratio(&self) -> Option<f64> {
        if self.raw_bytes == 0 {
            return None;
        }
        Some(self.scheduled_stored_bytes as f64 / self.raw_bytes as f64)
    }

    pub fn qatq_candidate_ratio(&self) -> Option<f64> {
        if self.raw_bytes == 0 {
            return None;
        }
        Some(self.qatq_candidate_bytes as f64 / self.raw_bytes as f64)
    }

    pub fn zstd_ratio(&self) -> Option<f64> {
        if self.raw_bytes == 0 {
            return None;
        }
        Some(self.zstd_bytes as f64 / self.raw_bytes as f64)
    }

    pub fn lz4_ratio(&self) -> Option<f64> {
        if self.raw_bytes == 0 {
            return None;
        }
        Some(self.lz4_bytes as f64 / self.raw_bytes as f64)
    }

    pub fn to_json(&self) -> String {
        self.to_json_with_runtime_estimates(None, None)
    }

    pub fn to_json_with_residency_estimate(&self, estimate: &LiveVramResidencyEstimate) -> String {
        self.to_json_with_runtime_estimates(Some(estimate), None)
    }

    pub fn to_json_with_restore_deadline_report(
        &self,
        report: &LiveVramPrefetchDeadlineReport,
    ) -> String {
        self.to_json_with_runtime_estimates(None, Some(report))
    }

    pub fn to_json_with_runtime_estimates(
        &self,
        residency_estimate: Option<&LiveVramResidencyEstimate>,
        restore_deadline_report: Option<&LiveVramPrefetchDeadlineReport>,
    ) -> String {
        self.to_json_with_runtime_estimates_and_event_trace(
            residency_estimate,
            restore_deadline_report,
            None,
        )
    }

    pub fn to_json_with_runtime_estimates_and_event_trace(
        &self,
        residency_estimate: Option<&LiveVramResidencyEstimate>,
        restore_deadline_report: Option<&LiveVramPrefetchDeadlineReport>,
        event_trace_report: Option<&LiveVramEventTraceReport>,
    ) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        push_json_field_str(
            &mut out,
            2,
            "adapter_contract_version",
            &self.adapter_contract_version,
            true,
        );
        push_json_field_usize(&mut out, 2, "total_pages", self.total_pages, true);
        push_json_field_usize(&mut out, 2, "offloaded_pages", self.offloaded_pages, true);
        push_json_field_usize(&mut out, 2, "resident_pages", self.resident_pages, true);
        push_json_field_usize(&mut out, 2, "compressed_pages", self.compressed_pages, true);
        push_json_field_usize(
            &mut out,
            2,
            "pass_through_pages",
            self.pass_through_pages,
            true,
        );
        push_json_field_usize(&mut out, 2, "sealed_pages", self.sealed_pages, true);
        push_json_field_usize(
            &mut out,
            2,
            "verified_restores",
            self.verified_restores,
            true,
        );
        push_json_field_usize(&mut out, 2, "raw_bytes", self.raw_bytes, true);
        push_json_field_usize(
            &mut out,
            2,
            "qatq_candidate_bytes",
            self.qatq_candidate_bytes,
            true,
        );
        push_json_field_usize(
            &mut out,
            2,
            "scheduled_stored_bytes",
            self.scheduled_stored_bytes,
            true,
        );
        push_json_field_usize(&mut out, 2, "zstd_bytes", self.zstd_bytes, true);
        push_json_field_usize(&mut out, 2, "lz4_bytes", self.lz4_bytes, true);
        push_json_field_usize(
            &mut out,
            2,
            "qatq_beats_zstd_pages",
            self.qatq_beats_zstd_pages,
            true,
        );
        push_json_field_usize(
            &mut out,
            2,
            "qatq_beats_lz4_pages",
            self.qatq_beats_lz4_pages,
            true,
        );
        push_json_field_usize(
            &mut out,
            2,
            "qatq_beats_best_general_codec_pages",
            self.qatq_beats_best_general_codec_pages,
            true,
        );
        if let Some(estimate) = residency_estimate {
            push_live_vram_residency_estimate_json(&mut out, estimate, true);
        }
        if let Some(report) = restore_deadline_report {
            push_live_vram_restore_deadline_report_json(&mut out, report, true);
        }
        if let Some(report) = event_trace_report {
            push_live_vram_event_trace_report_json(&mut out, report, true);
        }
        out.push_str("  \"pages\": [\n");
        for (index, page) in self.pages.iter().enumerate() {
            push_live_vram_page_evidence_json(&mut out, page, index + 1 != self.pages.len());
        }
        out.push_str("  ]\n");
        out.push_str("}\n");
        out
    }
}

pub fn estimate_live_vram_residency_after_offload(
    report: &LiveVramEvidenceReport,
    gpu_context_bytes_before: usize,
    allocation_granularity: LiveVramGpuAllocationGranularity,
) -> LiveVramResidencyEstimate {
    let logical_offloaded_raw_bytes = report
        .pages
        .iter()
        .filter(|page| page.schedule_decision == LiveVramScheduleDecision::Offload)
        .fold(0_usize, |acc, page| acc.saturating_add(page.raw_bytes));

    let stored_cpu_bytes = report
        .pages
        .iter()
        .filter(|page| page.schedule_decision == LiveVramScheduleDecision::Offload)
        .fold(0_usize, |acc, page| {
            acc.saturating_add(page.scheduled_stored_bytes)
        });

    let reclaimable_gpu_bytes = if allocation_granularity.can_reclaim_logical_pages() {
        logical_offloaded_raw_bytes.min(gpu_context_bytes_before)
    } else {
        0
    };

    LiveVramResidencyEstimate {
        allocation_granularity,
        gpu_context_bytes_before,
        logical_offloaded_raw_bytes,
        stored_cpu_bytes,
        reclaimable_gpu_bytes,
        gpu_context_bytes_after: gpu_context_bytes_before.saturating_sub(reclaimable_gpu_bytes),
    }
}

pub fn estimate_live_vram_residency_from_runtime_allocation(
    report: &LiveVramEvidenceReport,
    total_context_bytes: usize,
    gpu_context_bytes_after: usize,
    allocation_granularity: LiveVramGpuAllocationGranularity,
) -> Result<LiveVramResidencyEstimate, QatqError> {
    if gpu_context_bytes_after > total_context_bytes {
        return Err(QatqError::InvalidHeader);
    }
    let logical_offloaded_raw_bytes = report
        .pages
        .iter()
        .filter(|page| page.schedule_decision == LiveVramScheduleDecision::Offload)
        .fold(0_usize, |acc, page| acc.saturating_add(page.raw_bytes));

    let stored_cpu_bytes = report
        .pages
        .iter()
        .filter(|page| page.schedule_decision == LiveVramScheduleDecision::Offload)
        .fold(0_usize, |acc, page| {
            acc.saturating_add(page.scheduled_stored_bytes)
        });

    let reclaimable_gpu_bytes = if matches!(
        allocation_granularity,
        LiveVramGpuAllocationGranularity::PerPage | LiveVramGpuAllocationGranularity::WholeTensor
    ) {
        total_context_bytes.saturating_sub(gpu_context_bytes_after)
    } else {
        0
    };

    Ok(LiveVramResidencyEstimate {
        allocation_granularity,
        gpu_context_bytes_before: total_context_bytes,
        logical_offloaded_raw_bytes,
        stored_cpu_bytes,
        reclaimable_gpu_bytes,
        gpu_context_bytes_after,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveVramPrefetchBudget {
    pub restore_bytes_per_token: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveVramPrefetchDeadlineReport {
    pub evaluated_pages: usize,
    pub prefetch_misses: usize,
    pub pages_without_deadline: usize,
    pub total_estimated_restore_bytes: usize,
    pub worst_deficit_bytes: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveVramStreamingAttentionReport {
    pub output: Vec<f32>,
    pub pages: usize,
    pub tokens: usize,
    pub head_dim: usize,
    pub value_dim: usize,
    pub peak_page_kv_values: usize,
    pub materialized_kv_values: usize,
}

impl LiveVramStreamingAttentionReport {
    pub fn peak_kv_value_ratio(&self) -> Option<f64> {
        if self.materialized_kv_values == 0 {
            return None;
        }
        Some(self.peak_page_kv_values as f64 / self.materialized_kv_values as f64)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveVramStreamingAttentionEquivalenceReport {
    pub streaming: LiveVramStreamingAttentionReport,
    pub materialized_output: Vec<f32>,
    pub max_abs_error: f32,
    pub max_relative_error: f32,
    pub tolerance: f32,
    pub passed: bool,
}

pub fn decode_tensor_le_bytes_to_f32(
    bytes_le: &[u8],
    dtype: TensorDType,
) -> Result<Vec<f32>, QatqError> {
    validate_typed_tensor_len(bytes_le, dtype)?;
    match dtype {
        TensorDType::F32 => decode_f32le_bytes(bytes_le),
        TensorDType::BF16 => Ok(bytes_le
            .chunks_exact(2)
            .map(|chunk| {
                let bits = u16::from_le_bytes(chunk.try_into().expect("fixed bf16"));
                f32::from_bits((bits as u32) << 16)
            })
            .collect()),
        TensorDType::F16 => Ok(bytes_le
            .chunks_exact(2)
            .map(|chunk| {
                let bits = u16::from_le_bytes(chunk.try_into().expect("fixed f16"));
                f16_bits_to_f32(bits)
            })
            .collect()),
    }
}

pub fn compare_live_vram_typed_streaming_attention_reference(
    query: &[f32],
    key_pages_le: &[&[u8]],
    value_pages_le: &[&[u8]],
    dtype: TensorDType,
    head_dim: usize,
    value_dim: usize,
    tolerance: f32,
) -> Result<LiveVramStreamingAttentionEquivalenceReport, QatqError> {
    if head_dim == 0 || query.len() != head_dim {
        return Err(QatqError::InvalidHeader);
    }
    let key_pages = decode_attention_pages_to_f32(key_pages_le, dtype, head_dim)?;
    let value_pages = decode_attention_pages_to_f32(value_pages_le, dtype, value_dim)?;
    let key_refs: Vec<&[f32]> = key_pages.iter().map(Vec::as_slice).collect();
    let value_refs: Vec<&[f32]> = value_pages.iter().map(Vec::as_slice).collect();
    compare_live_vram_streaming_attention_reference(
        query,
        &key_refs,
        &value_refs,
        value_dim,
        tolerance,
    )
}

fn decode_attention_pages_to_f32(
    pages_le: &[&[u8]],
    dtype: TensorDType,
    width_values: usize,
) -> Result<Vec<Vec<f32>>, QatqError> {
    if width_values == 0 || pages_le.is_empty() {
        return Err(QatqError::InvalidHeader);
    }
    let element_width = dtype.element_width();
    let row_bytes = width_values
        .checked_mul(element_width)
        .ok_or(QatqError::InvalidHeader)?;
    let mut decoded = Vec::with_capacity(pages_le.len());
    for page in pages_le {
        if page.is_empty() || page.len() % row_bytes != 0 {
            return Err(QatqError::InvalidHeader);
        }
        decoded.push(decode_tensor_le_bytes_to_f32(page, dtype)?);
    }
    Ok(decoded)
}

pub fn live_vram_materialized_attention_reference(
    query: &[f32],
    key_pages: &[&[f32]],
    value_pages: &[&[f32]],
    value_dim: usize,
) -> Result<Vec<f32>, QatqError> {
    validate_live_vram_attention_pages(query, key_pages, value_pages, value_dim)?;

    let head_dim = query.len();
    let scale = 1.0_f64 / (head_dim as f64).sqrt();
    let mut scores = Vec::new();
    let mut values = Vec::new();
    for (key_page, value_page) in key_pages.iter().zip(value_pages.iter()) {
        let page_tokens = key_page.len() / head_dim;
        for token_index in 0..page_tokens {
            let key_offset = token_index * head_dim;
            let value_offset = token_index * value_dim;
            let mut score = 0.0_f64;
            for dim in 0..head_dim {
                score += query[dim] as f64 * key_page[key_offset + dim] as f64;
            }
            score *= scale;
            if !score.is_finite() {
                return Err(QatqError::InvalidHeader);
            }
            scores.push(score);
            values.push(&value_page[value_offset..value_offset + value_dim]);
        }
    }

    let max_score = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !max_score.is_finite() {
        return Err(QatqError::InvalidHeader);
    }
    let mut denominator = 0.0_f64;
    let mut output = vec![0.0_f64; value_dim];
    for (score, value) in scores.iter().zip(values.iter()) {
        let weight = (*score - max_score).exp();
        denominator += weight;
        for dim in 0..value_dim {
            output[dim] += weight * value[dim] as f64;
        }
    }
    if denominator == 0.0 || !denominator.is_finite() {
        return Err(QatqError::InvalidHeader);
    }

    let mut normalized = Vec::with_capacity(value_dim);
    for value in output {
        let normalized_value = value / denominator;
        if !normalized_value.is_finite() {
            return Err(QatqError::InvalidHeader);
        }
        normalized.push(normalized_value as f32);
    }
    Ok(normalized)
}

fn f16_bits_to_f32(bits: u16) -> f32 {
    let sign = ((bits & 0x8000) as u32) << 16;
    let exponent = (bits >> 10) & 0x1f;
    let fraction = (bits & 0x03ff) as u32;

    let out_bits = match exponent {
        0 => {
            if fraction == 0 {
                sign
            } else {
                let mut frac = fraction;
                let mut exp = -14_i32;
                while (frac & 0x0400) == 0 {
                    frac <<= 1;
                    exp -= 1;
                }
                frac &= 0x03ff;
                sign | (((exp + 127) as u32) << 23) | (frac << 13)
            }
        }
        0x1f => sign | 0x7f80_0000 | (fraction << 13),
        _ => sign | (((exponent as u32) + 112) << 23) | (fraction << 13),
    };
    f32::from_bits(out_bits)
}

pub fn compare_live_vram_streaming_attention_reference(
    query: &[f32],
    key_pages: &[&[f32]],
    value_pages: &[&[f32]],
    value_dim: usize,
    tolerance: f32,
) -> Result<LiveVramStreamingAttentionEquivalenceReport, QatqError> {
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(QatqError::InvalidHeader);
    }
    let streaming =
        live_vram_streaming_attention_reference(query, key_pages, value_pages, value_dim)?;
    let materialized_output =
        live_vram_materialized_attention_reference(query, key_pages, value_pages, value_dim)?;

    let mut max_abs_error = 0.0_f32;
    let mut max_relative_error = 0.0_f32;
    for (actual, expected) in streaming.output.iter().zip(materialized_output.iter()) {
        let abs_error = (actual - expected).abs();
        let relative_denominator = expected.abs().max(f32::EPSILON);
        max_abs_error = max_abs_error.max(abs_error);
        max_relative_error = max_relative_error.max(abs_error / relative_denominator);
    }
    Ok(LiveVramStreamingAttentionEquivalenceReport {
        streaming,
        materialized_output,
        max_abs_error,
        max_relative_error,
        tolerance,
        passed: max_abs_error <= tolerance,
    })
}

pub fn compare_live_vram_segment_summary_attention_reference(
    query: &[f32],
    key_pages: &[&[f32]],
    value_pages: &[&[f32]],
    value_dim: usize,
    tolerance: f32,
) -> Result<LiveVramStreamingAttentionEquivalenceReport, QatqError> {
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(QatqError::InvalidHeader);
    }
    let streaming =
        live_vram_segment_summary_attention_reference(query, key_pages, value_pages, value_dim)?;
    let materialized_output =
        live_vram_materialized_attention_reference(query, key_pages, value_pages, value_dim)?;

    let mut max_abs_error = 0.0_f32;
    let mut max_relative_error = 0.0_f32;
    for (actual, expected) in streaming.output.iter().zip(materialized_output.iter()) {
        let abs_error = (actual - expected).abs();
        let relative_denominator = expected.abs().max(f32::EPSILON);
        max_abs_error = max_abs_error.max(abs_error);
        max_relative_error = max_relative_error.max(abs_error / relative_denominator);
    }
    Ok(LiveVramStreamingAttentionEquivalenceReport {
        streaming,
        materialized_output,
        max_abs_error,
        max_relative_error,
        tolerance,
        passed: max_abs_error <= tolerance,
    })
}

pub fn live_vram_streaming_attention_reference(
    query: &[f32],
    key_pages: &[&[f32]],
    value_pages: &[&[f32]],
    value_dim: usize,
) -> Result<LiveVramStreamingAttentionReport, QatqError> {
    let head_dim = query.len();
    validate_live_vram_attention_pages(query, key_pages, value_pages, value_dim)?;

    let scale = 1.0_f64 / (head_dim as f64).sqrt();
    let mut output = vec![0.0_f64; value_dim];
    let mut max_score = f64::NEG_INFINITY;
    let mut denominator = 0.0_f64;
    let mut tokens = 0_usize;
    let mut peak_page_kv_values = 0_usize;

    for (key_page, value_page) in key_pages.iter().zip(value_pages.iter()) {
        let page_tokens = key_page.len() / head_dim;
        peak_page_kv_values = peak_page_kv_values.max(key_page.len() + value_page.len());
        for token_index in 0..page_tokens {
            let key_offset = token_index * head_dim;
            let value_offset = token_index * value_dim;
            let mut score = 0.0_f64;
            for dim in 0..head_dim {
                score += query[dim] as f64 * key_page[key_offset + dim] as f64;
            }
            score *= scale;
            if !score.is_finite() {
                return Err(QatqError::InvalidHeader);
            }

            let next_max = max_score.max(score);
            let old_weight = if max_score.is_finite() {
                (max_score - next_max).exp()
            } else {
                0.0
            };
            let new_weight = (score - next_max).exp();
            denominator = denominator * old_weight + new_weight;
            for dim in 0..value_dim {
                output[dim] =
                    output[dim] * old_weight + new_weight * value_page[value_offset + dim] as f64;
            }
            max_score = next_max;
            tokens += 1;
        }
    }

    if tokens == 0 || denominator == 0.0 || !denominator.is_finite() {
        return Err(QatqError::InvalidHeader);
    }

    let mut normalized = Vec::with_capacity(value_dim);
    for value in output {
        let normalized_value = value / denominator;
        if !normalized_value.is_finite() {
            return Err(QatqError::InvalidHeader);
        }
        normalized.push(normalized_value as f32);
    }

    Ok(LiveVramStreamingAttentionReport {
        output: normalized,
        pages: key_pages.len(),
        tokens,
        head_dim,
        value_dim,
        peak_page_kv_values,
        materialized_kv_values: tokens * (head_dim + value_dim),
    })
}

pub fn live_vram_segment_summary_attention_reference(
    query: &[f32],
    key_pages: &[&[f32]],
    value_pages: &[&[f32]],
    value_dim: usize,
) -> Result<LiveVramStreamingAttentionReport, QatqError> {
    let head_dim = query.len();
    validate_live_vram_attention_pages(query, key_pages, value_pages, value_dim)?;

    let scale = 1.0_f64 / (head_dim as f64).sqrt();
    let mut output = vec![0.0_f64; value_dim];
    let mut max_score = f64::NEG_INFINITY;
    let mut denominator = 0.0_f64;
    let mut tokens = 0_usize;
    let mut peak_page_kv_values = 0_usize;

    for (key_page, value_page) in key_pages.iter().zip(value_pages.iter()) {
        let page_tokens = key_page.len() / head_dim;
        peak_page_kv_values = peak_page_kv_values.max(key_page.len() + value_page.len());

        let mut page_scores = Vec::with_capacity(page_tokens);
        let mut page_max = f64::NEG_INFINITY;
        for token_index in 0..page_tokens {
            let key_offset = token_index * head_dim;
            let mut score = 0.0_f64;
            for dim in 0..head_dim {
                score += query[dim] as f64 * key_page[key_offset + dim] as f64;
            }
            score *= scale;
            if !score.is_finite() {
                return Err(QatqError::InvalidHeader);
            }
            page_max = page_max.max(score);
            page_scores.push(score);
        }
        if !page_max.is_finite() {
            return Err(QatqError::InvalidHeader);
        }

        let mut page_denominator = 0.0_f64;
        let mut page_output = vec![0.0_f64; value_dim];
        for (token_index, score) in page_scores.iter().enumerate() {
            let weight = (*score - page_max).exp();
            page_denominator += weight;
            let value_offset = token_index * value_dim;
            for dim in 0..value_dim {
                page_output[dim] += weight * value_page[value_offset + dim] as f64;
            }
        }
        if page_denominator == 0.0 || !page_denominator.is_finite() {
            return Err(QatqError::InvalidHeader);
        }

        let next_max = max_score.max(page_max);
        let old_weight = if max_score.is_finite() {
            (max_score - next_max).exp()
        } else {
            0.0
        };
        let page_weight = (page_max - next_max).exp();
        denominator = denominator * old_weight + page_denominator * page_weight;
        for dim in 0..value_dim {
            output[dim] = output[dim] * old_weight + page_output[dim] * page_weight;
        }
        max_score = next_max;
        tokens += page_tokens;
    }

    if tokens == 0 || denominator == 0.0 || !denominator.is_finite() {
        return Err(QatqError::InvalidHeader);
    }

    let mut normalized = Vec::with_capacity(value_dim);
    for value in output {
        let normalized_value = value / denominator;
        if !normalized_value.is_finite() {
            return Err(QatqError::InvalidHeader);
        }
        normalized.push(normalized_value as f32);
    }

    Ok(LiveVramStreamingAttentionReport {
        output: normalized,
        pages: key_pages.len(),
        tokens,
        head_dim,
        value_dim,
        peak_page_kv_values,
        materialized_kv_values: tokens * (head_dim + value_dim),
    })
}

fn validate_live_vram_attention_pages(
    query: &[f32],
    key_pages: &[&[f32]],
    value_pages: &[&[f32]],
    value_dim: usize,
) -> Result<(), QatqError> {
    let head_dim = query.len();
    if head_dim == 0
        || value_dim == 0
        || key_pages.is_empty()
        || key_pages.len() != value_pages.len()
        || !query.iter().all(|value| value.is_finite())
    {
        return Err(QatqError::InvalidHeader);
    }
    for (key_page, value_page) in key_pages.iter().zip(value_pages.iter()) {
        if key_page.is_empty()
            || key_page.len() % head_dim != 0
            || !key_page.iter().all(|value| value.is_finite())
            || !value_page.iter().all(|value| value.is_finite())
        {
            return Err(QatqError::InvalidHeader);
        }
        let page_tokens = key_page.len() / head_dim;
        if value_page.len() != page_tokens * value_dim {
            return Err(QatqError::InvalidHeader);
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveVramPageEventKind {
    Snapshot,
    OffloadCommitted,
    RestoreCommitted,
    AttentionUse,
    CancelledBeforeRuntimeCommit,
    CancelledAfterRuntimeCommit,
    /// Legacy alias for after-runtime-commit cancellation.
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveVramPageEvent {
    pub token: u64,
    pub key: KvPageKey,
    pub kind: LiveVramPageEventKind,
    pub checksum: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveVramEventTracePolicy {
    pub require_monotonic_tokens: bool,
    pub require_known_pages: bool,
    pub require_restore_checksums: bool,
    pub require_all_pages_restored_at_end: bool,
}

impl Default for LiveVramEventTracePolicy {
    fn default() -> Self {
        Self {
            require_monotonic_tokens: true,
            require_known_pages: true,
            require_restore_checksums: true,
            require_all_pages_restored_at_end: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveVramEventTraceFailure {
    NoEvents,
    NonMonotonicToken {
        index: usize,
        previous_token: u64,
        token: u64,
    },
    UnknownPage {
        index: usize,
        key: Box<KvPageKey>,
    },
    TracePageNotInEvidence {
        index: usize,
        key: Box<KvPageKey>,
    },
    EvidenceOffloadMissingFromTrace {
        key: Box<KvPageKey>,
    },
    EvidenceResidentPageOffloadedInTrace {
        key: Box<KvPageKey>,
    },
    DuplicateOffload {
        index: usize,
        key: Box<KvPageKey>,
    },
    RestoreWithoutOffload {
        index: usize,
        key: Box<KvPageKey>,
    },
    MissingRestoreChecksum {
        index: usize,
        key: Box<KvPageKey>,
    },
    RestoreChecksumMismatch {
        index: usize,
        key: Box<KvPageKey>,
        expected: u64,
        actual: u64,
    },
    AttentionUseWhileOffloaded {
        index: usize,
        key: Box<KvPageKey>,
        token: u64,
    },
    CancellationBeforeRuntimeCommitAfterOffload {
        index: usize,
        key: Box<KvPageKey>,
    },
    CancellationAfterRuntimeCommitWithoutOffload {
        index: usize,
        key: Box<KvPageKey>,
    },
    MissingCancellationChecksum {
        index: usize,
        key: Box<KvPageKey>,
    },
    CancellationChecksumMismatch {
        index: usize,
        key: Box<KvPageKey>,
        expected: u64,
        actual: u64,
    },
    OffloadedPagesRemaining {
        count: usize,
    },
}

impl fmt::Display for LiveVramEventTraceFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoEvents => write!(f, "live VRAM trace contains no events"),
            Self::NonMonotonicToken {
                previous_token,
                token,
                ..
            } => write!(
                f,
                "live VRAM trace token {token} went backwards from {previous_token}"
            ),
            Self::UnknownPage { .. } => write!(f, "live VRAM trace referenced an unknown page"),
            Self::TracePageNotInEvidence { .. } => {
                write!(
                    f,
                    "live VRAM trace referenced a page outside the evidence report"
                )
            }
            Self::EvidenceOffloadMissingFromTrace { .. } => {
                write!(
                    f,
                    "live VRAM trace did not cover an offloaded evidence page"
                )
            }
            Self::EvidenceResidentPageOffloadedInTrace { .. } => {
                write!(
                    f,
                    "live VRAM trace offloaded a page that evidence kept resident"
                )
            }
            Self::DuplicateOffload { .. } => {
                write!(f, "live VRAM trace offloaded an already offloaded page")
            }
            Self::RestoreWithoutOffload { .. } => {
                write!(f, "live VRAM trace restored a page that was not offloaded")
            }
            Self::MissingRestoreChecksum { .. } => {
                write!(f, "live VRAM trace restore event is missing a checksum")
            }
            Self::RestoreChecksumMismatch {
                expected, actual, ..
            } => write!(
                f,
                "live VRAM trace restore checksum {actual:016x} did not match {expected:016x}"
            ),
            Self::AttentionUseWhileOffloaded { token, .. } => write!(
                f,
                "live VRAM trace consumed an offloaded page at token {token}"
            ),
            Self::CancellationBeforeRuntimeCommitAfterOffload { .. } => write!(
                f,
                "live VRAM trace recorded before-runtime-commit cancellation after runtime offload"
            ),
            Self::CancellationAfterRuntimeCommitWithoutOffload { .. } => write!(
                f,
                "live VRAM trace recorded after-runtime-commit cancellation without an offloaded page"
            ),
            Self::MissingCancellationChecksum { .. } => {
                write!(
                    f,
                    "live VRAM trace after-runtime-commit cancellation is missing a checksum"
                )
            }
            Self::CancellationChecksumMismatch {
                expected, actual, ..
            } => write!(
                f,
                "live VRAM trace cancellation checksum {actual:016x} did not match {expected:016x}"
            ),
            Self::OffloadedPagesRemaining { count } => {
                write!(f, "live VRAM trace ended with {count} offloaded pages")
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveVramEventTraceReport {
    pub events: usize,
    pub snapshots: usize,
    pub offloads: usize,
    pub restores: usize,
    pub attention_uses: usize,
    pub cancellations: usize,
    pub peak_offloaded_pages: usize,
    pub offloaded_pages_at_end: usize,
    pub failures: Vec<LiveVramEventTraceFailure>,
}

impl LiveVramEventTraceReport {
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LiveVramProofGate {
    pub min_gpu_saved_ratio: f64,
    pub max_restore_deadline_misses: usize,
    pub require_all_restores_verified: bool,
    pub require_aggregate_qatq_beats_best_general_codec: bool,
    pub require_all_pages_beat_best_general_codec: bool,
    pub require_all_offloaded_pages_compressed: bool,
    pub require_page_granular_reclaim: bool,
}

impl Default for LiveVramProofGate {
    fn default() -> Self {
        Self {
            min_gpu_saved_ratio: 0.10,
            max_restore_deadline_misses: 0,
            require_all_restores_verified: true,
            require_aggregate_qatq_beats_best_general_codec: false,
            require_all_pages_beat_best_general_codec: true,
            require_all_offloaded_pages_compressed: true,
            require_page_granular_reclaim: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LiveVramProofGateFailure {
    NoPages,
    NoOffloadedPages,
    RestoresNotFullyVerified {
        verified: usize,
        total: usize,
    },
    MissingResidencyEstimate,
    AllocationGranularityCannotReclaimPages {
        allocation_granularity: LiveVramGpuAllocationGranularity,
    },
    ReclaimableGpuBytesZero,
    GpuSavedRatioBelowThreshold {
        actual: Option<f64>,
        required: f64,
    },
    MissingRestoreDeadlineReport,
    RestoreDeadlineMissesExceeded {
        actual: usize,
        max_allowed: usize,
    },
    PagesWithoutRestoreDeadline {
        count: usize,
    },
    QatqDidNotBeatBestGeneralCodecOnAllPages {
        actual: usize,
        total: usize,
    },
    QatqDidNotBeatBestGeneralCodecInAggregate {
        qatq_bytes: usize,
        best_general_bytes: usize,
    },
    OffloadedPagesNotCompressed {
        offloaded: usize,
        compressed: usize,
    },
}

impl fmt::Display for LiveVramProofGateFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPages => write!(f, "no live VRAM pages were evaluated"),
            Self::NoOffloadedPages => write!(f, "no live VRAM pages were scheduled for offload"),
            Self::RestoresNotFullyVerified { verified, total } => {
                write!(
                    f,
                    "verified restores {verified}/{total} did not cover every page"
                )
            }
            Self::MissingResidencyEstimate => {
                write!(f, "missing allocator-aware residency estimate")
            }
            Self::AllocationGranularityCannotReclaimPages {
                allocation_granularity,
            } => write!(
                f,
                "allocation granularity {} cannot prove page-level GPU reclaim",
                allocation_granularity.as_str()
            ),
            Self::ReclaimableGpuBytesZero => write!(f, "reclaimable GPU bytes is zero"),
            Self::GpuSavedRatioBelowThreshold { actual, required } => match actual {
                Some(actual) => write!(
                    f,
                    "GPU saved ratio {actual:.6} is below required {required:.6}"
                ),
                None => write!(f, "GPU saved ratio is unavailable"),
            },
            Self::MissingRestoreDeadlineReport => {
                write!(f, "missing restore-deadline report")
            }
            Self::RestoreDeadlineMissesExceeded {
                actual,
                max_allowed,
            } => write!(
                f,
                "restore-deadline misses {actual} exceed allowed {max_allowed}"
            ),
            Self::PagesWithoutRestoreDeadline { count } => {
                write!(f, "{count} offloaded pages do not have restore deadlines")
            }
            Self::QatqDidNotBeatBestGeneralCodecOnAllPages { actual, total } => write!(
                f,
                "QATQ beat the best general codec on {actual}/{total} offloaded pages"
            ),
            Self::QatqDidNotBeatBestGeneralCodecInAggregate {
                qatq_bytes,
                best_general_bytes,
            } => write!(
                f,
                "QATQ aggregate bytes {qatq_bytes} did not beat best general codec bytes {best_general_bytes}"
            ),
            Self::OffloadedPagesNotCompressed {
                offloaded,
                compressed,
            } => write!(
                f,
                "compressed pages {compressed} do not cover all {offloaded} offloaded pages"
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveVramProofGateReport {
    pub failures: Vec<LiveVramProofGateFailure>,
}

impl LiveVramProofGateReport {
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveVramLivePagingProofReport {
    pub proof_gate: LiveVramProofGateReport,
    pub event_trace: LiveVramEventTraceReport,
}

impl LiveVramLivePagingProofReport {
    pub fn passed(&self) -> bool {
        self.proof_gate.passed() && self.event_trace.passed()
    }
}

pub fn evaluate_live_vram_prefetch_deadlines(
    evidence: &LiveVramEvidenceReport,
    current_token: u64,
    budget: LiveVramPrefetchBudget,
) -> Result<LiveVramPrefetchDeadlineReport, QatqError> {
    if budget.restore_bytes_per_token == 0 {
        return Err(QatqError::InvalidHeader);
    }
    let mut report = LiveVramPrefetchDeadlineReport {
        evaluated_pages: 0,
        prefetch_misses: 0,
        pages_without_deadline: 0,
        total_estimated_restore_bytes: 0,
        worst_deficit_bytes: 0,
    };

    for page in &evidence.pages {
        if page.schedule_decision != LiveVramScheduleDecision::Offload {
            continue;
        }
        let Some(next_required_token) = page.descriptor.next_required_token else {
            report.pages_without_deadline += 1;
            continue;
        };
        report.evaluated_pages += 1;
        let estimated_restore_bytes = page
            .scheduled_stored_bytes
            .checked_add(page.raw_bytes)
            .ok_or(QatqError::ContainerLimitExceeded(
                "live-vram-restore-estimate",
            ))?;
        report.total_estimated_restore_bytes = report
            .total_estimated_restore_bytes
            .checked_add(estimated_restore_bytes)
            .ok_or(QatqError::ContainerLimitExceeded(
                "live-vram-total-restore-estimate",
            ))?;
        let lead_tokens = next_required_token.saturating_sub(current_token);
        let restore_budget = (lead_tokens as usize)
            .checked_mul(budget.restore_bytes_per_token)
            .ok_or(QatqError::ContainerLimitExceeded(
                "live-vram-restore-budget",
            ))?;
        if estimated_restore_bytes > restore_budget {
            report.prefetch_misses += 1;
            report.worst_deficit_bytes = report
                .worst_deficit_bytes
                .max(estimated_restore_bytes - restore_budget);
        }
    }

    Ok(report)
}

pub fn evaluate_live_vram_event_trace(
    events: &[LiveVramPageEvent],
    policy: LiveVramEventTracePolicy,
) -> LiveVramEventTraceReport {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct PageTraceState {
        resident: bool,
        checksum: Option<u64>,
    }

    let mut report = LiveVramEventTraceReport {
        events: events.len(),
        snapshots: 0,
        offloads: 0,
        restores: 0,
        attention_uses: 0,
        cancellations: 0,
        peak_offloaded_pages: 0,
        offloaded_pages_at_end: 0,
        failures: Vec::new(),
    };
    if events.is_empty() {
        report.failures.push(LiveVramEventTraceFailure::NoEvents);
        return report;
    }

    let mut pages = BTreeMap::<KvPageKey, PageTraceState>::new();
    let mut offloaded = BTreeSet::<KvPageKey>::new();
    let mut previous_token = events[0].token;

    for (index, event) in events.iter().enumerate() {
        if policy.require_monotonic_tokens && index > 0 && event.token < previous_token {
            report
                .failures
                .push(LiveVramEventTraceFailure::NonMonotonicToken {
                    index,
                    previous_token,
                    token: event.token,
                });
        }
        previous_token = event.token;

        match event.kind {
            LiveVramPageEventKind::Snapshot => {
                report.snapshots += 1;
                pages.insert(
                    event.key.clone(),
                    PageTraceState {
                        resident: true,
                        checksum: event.checksum,
                    },
                );
            }
            LiveVramPageEventKind::OffloadCommitted => {
                report.offloads += 1;
                let Some(state) = pages.get_mut(&event.key) else {
                    if policy.require_known_pages {
                        report
                            .failures
                            .push(LiveVramEventTraceFailure::UnknownPage {
                                index,
                                key: Box::new(event.key.clone()),
                            });
                    }
                    pages.insert(
                        event.key.clone(),
                        PageTraceState {
                            resident: false,
                            checksum: event.checksum,
                        },
                    );
                    offloaded.insert(event.key.clone());
                    report.peak_offloaded_pages = report.peak_offloaded_pages.max(offloaded.len());
                    continue;
                };
                if !state.resident {
                    report
                        .failures
                        .push(LiveVramEventTraceFailure::DuplicateOffload {
                            index,
                            key: Box::new(event.key.clone()),
                        });
                }
                if let Some(checksum) = event.checksum {
                    state.checksum = Some(checksum);
                }
                state.resident = false;
                offloaded.insert(event.key.clone());
                report.peak_offloaded_pages = report.peak_offloaded_pages.max(offloaded.len());
            }
            LiveVramPageEventKind::RestoreCommitted => {
                report.restores += 1;
                let Some(state) = pages.get_mut(&event.key) else {
                    if policy.require_known_pages {
                        report
                            .failures
                            .push(LiveVramEventTraceFailure::UnknownPage {
                                index,
                                key: Box::new(event.key.clone()),
                            });
                    }
                    continue;
                };
                if state.resident {
                    report
                        .failures
                        .push(LiveVramEventTraceFailure::RestoreWithoutOffload {
                            index,
                            key: Box::new(event.key.clone()),
                        });
                }
                if policy.require_restore_checksums {
                    match (state.checksum, event.checksum) {
                        (Some(expected), Some(actual)) if expected != actual => {
                            report.failures.push(
                                LiveVramEventTraceFailure::RestoreChecksumMismatch {
                                    index,
                                    key: Box::new(event.key.clone()),
                                    expected,
                                    actual,
                                },
                            );
                        }
                        (_, None) => {
                            report.failures.push(
                                LiveVramEventTraceFailure::MissingRestoreChecksum {
                                    index,
                                    key: Box::new(event.key.clone()),
                                },
                            );
                        }
                        (None, Some(actual)) => {
                            state.checksum = Some(actual);
                        }
                        _ => {}
                    }
                }
                state.resident = true;
                offloaded.remove(&event.key);
            }
            LiveVramPageEventKind::AttentionUse => {
                report.attention_uses += 1;
                match pages.get(&event.key) {
                    Some(state) if !state.resident => {
                        report.failures.push(
                            LiveVramEventTraceFailure::AttentionUseWhileOffloaded {
                                index,
                                key: Box::new(event.key.clone()),
                                token: event.token,
                            },
                        );
                    }
                    Some(_) => {}
                    None if policy.require_known_pages => {
                        report
                            .failures
                            .push(LiveVramEventTraceFailure::UnknownPage {
                                index,
                                key: Box::new(event.key.clone()),
                            });
                    }
                    None => {}
                }
            }
            LiveVramPageEventKind::CancelledBeforeRuntimeCommit => {
                report.cancellations += 1;
                match pages.get_mut(&event.key) {
                    Some(state) => {
                        if !state.resident {
                            report.failures.push(
                                LiveVramEventTraceFailure::CancellationBeforeRuntimeCommitAfterOffload {
                                    index,
                                    key: Box::new(event.key.clone()),
                                },
                            );
                        }
                    }
                    None if policy.require_known_pages => {
                        report
                            .failures
                            .push(LiveVramEventTraceFailure::UnknownPage {
                                index,
                                key: Box::new(event.key.clone()),
                            });
                    }
                    None => {}
                }
            }
            LiveVramPageEventKind::Cancelled
            | LiveVramPageEventKind::CancelledAfterRuntimeCommit => {
                report.cancellations += 1;
                match pages.get_mut(&event.key) {
                    Some(state) => {
                        if state.resident {
                            report.failures.push(
                                LiveVramEventTraceFailure::CancellationAfterRuntimeCommitWithoutOffload {
                                    index,
                                    key: Box::new(event.key.clone()),
                                },
                            );
                        }
                        if policy.require_restore_checksums {
                            match (state.checksum, event.checksum) {
                                (Some(expected), Some(actual)) if expected != actual => {
                                    report.failures.push(
                                        LiveVramEventTraceFailure::CancellationChecksumMismatch {
                                            index,
                                            key: Box::new(event.key.clone()),
                                            expected,
                                            actual,
                                        },
                                    );
                                }
                                (_, None) => {
                                    report.failures.push(
                                        LiveVramEventTraceFailure::MissingCancellationChecksum {
                                            index,
                                            key: Box::new(event.key.clone()),
                                        },
                                    );
                                }
                                (None, Some(actual)) => {
                                    state.checksum = Some(actual);
                                }
                                _ => {}
                            }
                        }
                        state.resident = true;
                        offloaded.remove(&event.key);
                    }
                    None if policy.require_known_pages => {
                        report
                            .failures
                            .push(LiveVramEventTraceFailure::UnknownPage {
                                index,
                                key: Box::new(event.key.clone()),
                            });
                    }
                    None => {}
                }
            }
        }
    }

    report.offloaded_pages_at_end = offloaded.len();
    if policy.require_all_pages_restored_at_end && !offloaded.is_empty() {
        report
            .failures
            .push(LiveVramEventTraceFailure::OffloadedPagesRemaining {
                count: offloaded.len(),
            });
    }

    report
}

pub fn evaluate_live_vram_proof_gate(
    evidence: &LiveVramEvidenceReport,
    residency_estimate: Option<&LiveVramResidencyEstimate>,
    restore_deadline_report: Option<&LiveVramPrefetchDeadlineReport>,
    gate: LiveVramProofGate,
) -> Result<LiveVramProofGateReport, QatqError> {
    if !gate.min_gpu_saved_ratio.is_finite() || !(0.0..=1.0).contains(&gate.min_gpu_saved_ratio) {
        return Err(QatqError::InvalidHeader);
    }

    let mut failures = Vec::new();
    if evidence.total_pages == 0 {
        failures.push(LiveVramProofGateFailure::NoPages);
    }
    if evidence.offloaded_pages == 0 {
        failures.push(LiveVramProofGateFailure::NoOffloadedPages);
    }
    if gate.require_all_restores_verified && evidence.verified_restores != evidence.total_pages {
        failures.push(LiveVramProofGateFailure::RestoresNotFullyVerified {
            verified: evidence.verified_restores,
            total: evidence.total_pages,
        });
    }

    match residency_estimate {
        Some(estimate) => {
            if gate.require_page_granular_reclaim
                && !estimate.allocation_granularity.can_reclaim_logical_pages()
            {
                failures.push(
                    LiveVramProofGateFailure::AllocationGranularityCannotReclaimPages {
                        allocation_granularity: estimate.allocation_granularity,
                    },
                );
            }
            if estimate.reclaimable_gpu_bytes == 0 {
                failures.push(LiveVramProofGateFailure::ReclaimableGpuBytesZero);
            }
            let actual = estimate.gpu_saved_ratio();
            if actual.is_none_or(|actual| actual < gate.min_gpu_saved_ratio) {
                failures.push(LiveVramProofGateFailure::GpuSavedRatioBelowThreshold {
                    actual,
                    required: gate.min_gpu_saved_ratio,
                });
            }
        }
        None => failures.push(LiveVramProofGateFailure::MissingResidencyEstimate),
    }

    match restore_deadline_report {
        Some(report) => {
            if report.prefetch_misses > gate.max_restore_deadline_misses {
                failures.push(LiveVramProofGateFailure::RestoreDeadlineMissesExceeded {
                    actual: report.prefetch_misses,
                    max_allowed: gate.max_restore_deadline_misses,
                });
            }
            if report.pages_without_deadline > 0 {
                failures.push(LiveVramProofGateFailure::PagesWithoutRestoreDeadline {
                    count: report.pages_without_deadline,
                });
            }
        }
        None => failures.push(LiveVramProofGateFailure::MissingRestoreDeadlineReport),
    }

    let offloaded_pages_beating_best_general_codec = evidence
        .pages
        .iter()
        .filter(|page| page.schedule_decision == LiveVramScheduleDecision::Offload)
        .filter(|page| page.qatq_beats_best_general_codec())
        .count();
    if gate.require_aggregate_qatq_beats_best_general_codec {
        let best_general_bytes = evidence.zstd_bytes.min(evidence.lz4_bytes);
        if evidence.qatq_candidate_bytes >= best_general_bytes {
            failures.push(
                LiveVramProofGateFailure::QatqDidNotBeatBestGeneralCodecInAggregate {
                    qatq_bytes: evidence.qatq_candidate_bytes,
                    best_general_bytes,
                },
            );
        }
    }
    if gate.require_all_pages_beat_best_general_codec
        && offloaded_pages_beating_best_general_codec != evidence.offloaded_pages
    {
        failures.push(
            LiveVramProofGateFailure::QatqDidNotBeatBestGeneralCodecOnAllPages {
                actual: offloaded_pages_beating_best_general_codec,
                total: evidence.offloaded_pages,
            },
        );
    }
    if gate.require_all_offloaded_pages_compressed
        && evidence.compressed_pages < evidence.offloaded_pages
    {
        failures.push(LiveVramProofGateFailure::OffloadedPagesNotCompressed {
            offloaded: evidence.offloaded_pages,
            compressed: evidence.compressed_pages,
        });
    }

    Ok(LiveVramProofGateReport { failures })
}

pub fn evaluate_live_vram_live_paging_proof_gate(
    evidence: &LiveVramEvidenceReport,
    residency_estimate: Option<&LiveVramResidencyEstimate>,
    restore_deadline_report: Option<&LiveVramPrefetchDeadlineReport>,
    events: &[LiveVramPageEvent],
    gate: LiveVramProofGate,
    trace_policy: LiveVramEventTracePolicy,
) -> Result<LiveVramLivePagingProofReport, QatqError> {
    let mut event_trace = evaluate_live_vram_event_trace(events, trace_policy);
    let evidence_keys = evidence
        .pages
        .iter()
        .map(|page| KvPageKey::from_descriptor(&page.descriptor))
        .collect::<BTreeSet<_>>();
    let traced_offloads = events
        .iter()
        .filter(|event| event.kind == LiveVramPageEventKind::OffloadCommitted)
        .map(|event| event.key.clone())
        .collect::<BTreeSet<_>>();
    for (index, event) in events.iter().enumerate() {
        if !evidence_keys.contains(&event.key) {
            event_trace
                .failures
                .push(LiveVramEventTraceFailure::TracePageNotInEvidence {
                    index,
                    key: Box::new(event.key.clone()),
                });
        }
    }
    for page in &evidence.pages {
        if page.schedule_decision == LiveVramScheduleDecision::Offload {
            let key = KvPageKey::from_descriptor(&page.descriptor);
            if !traced_offloads.contains(&key) {
                event_trace.failures.push(
                    LiveVramEventTraceFailure::EvidenceOffloadMissingFromTrace {
                        key: Box::new(key),
                    },
                );
            }
        } else {
            let key = KvPageKey::from_descriptor(&page.descriptor);
            if traced_offloads.contains(&key) {
                event_trace.failures.push(
                    LiveVramEventTraceFailure::EvidenceResidentPageOffloadedInTrace {
                        key: Box::new(key),
                    },
                );
            }
        }
    }
    Ok(LiveVramLivePagingProofReport {
        proof_gate: evaluate_live_vram_proof_gate(
            evidence,
            residency_estimate,
            restore_deadline_report,
            gate,
        )?,
        event_trace,
    })
}

pub fn live_vram_page_checksum(bytes_le: &[u8]) -> u64 {
    checksum_bytes(bytes_le)
}

pub fn schedule_live_vram_page(
    descriptor: &KvPageDescriptor,
    state: LiveVramSchedulerState,
    policy: LiveVramSchedulerPolicy,
) -> LiveVramScheduleDecision {
    if state.queued_pages >= policy.max_queued_pages {
        return LiveVramScheduleDecision::KeepResident(LiveVramKeepReason::QueueFull);
    }
    if state.cpu_stored_bytes >= policy.max_cpu_stored_bytes {
        return LiveVramScheduleDecision::KeepResident(LiveVramKeepReason::CpuBudgetExceeded);
    }
    let Some(next_required_token) = descriptor.next_required_token else {
        return LiveVramScheduleDecision::KeepResident(LiveVramKeepReason::UnknownNextUse);
    };
    let hot_until = state.current_token.saturating_add(policy.hot_window_tokens);
    if next_required_token <= hot_until {
        return LiveVramScheduleDecision::KeepResident(LiveVramKeepReason::InsideHotWindow);
    }
    let prefetch_until = hot_until.saturating_add(policy.prefetch_window_tokens);
    if next_required_token <= prefetch_until {
        return LiveVramScheduleDecision::KeepResident(LiveVramKeepReason::InsidePrefetchWindow);
    }
    LiveVramScheduleDecision::Offload
}

pub fn try_encode_live_vram_page(
    snapshot: &KvPageSnapshot,
    limits: LiveVramLimits,
) -> Result<LiveVramPageEncodeResult, QatqError> {
    validate_live_vram_snapshot(snapshot, limits)?;
    let payload = try_encode_qatq_exact_tensor_le(&snapshot.bytes_le, snapshot.descriptor.dtype)?;
    let strategy = qatq_exact_strategy(&payload)?;
    let (storage, bytes, strategy) = if payload.len() < snapshot.bytes_le.len() {
        (LiveVramStorage::Qatq, payload, Some(strategy))
    } else {
        (
            LiveVramStorage::RawTypedPassThrough,
            snapshot.bytes_le.clone(),
            None,
        )
    };
    if bytes.len() > limits.max_stored_bytes {
        return Err(QatqError::ContainerLimitExceeded("live-vram-stored-bytes"));
    }
    Ok(LiveVramPageEncodeResult {
        metadata: LiveVramPageMetadata {
            descriptor: snapshot.descriptor.clone(),
            storage,
            stored_len: bytes.len(),
            strategy,
        },
        bytes,
    })
}

pub fn restore_live_vram_page(
    metadata: &LiveVramPageMetadata,
    bytes: &[u8],
    limits: LiveVramLimits,
) -> Result<Vec<u8>, QatqError> {
    metadata.descriptor.validate(limits)?;
    if metadata.stored_len > limits.max_stored_bytes {
        return Err(QatqError::ContainerLimitExceeded("live-vram-stored-bytes"));
    }
    if bytes.len() != metadata.stored_len {
        return Err(QatqError::LengthMismatch {
            expected: metadata.stored_len,
            actual: bytes.len(),
        });
    }

    let restored = match metadata.storage {
        LiveVramStorage::Qatq => {
            let decoded = decode_qatq_exact_tensor_le(bytes)?;
            if decoded.dtype != metadata.descriptor.dtype {
                return Err(QatqError::InvalidHeader);
            }
            if let Some(expected_strategy) = metadata.strategy {
                let actual_strategy = qatq_exact_strategy(bytes)?;
                if actual_strategy != expected_strategy {
                    return Err(QatqError::InvalidQatqExactBody);
                }
            }
            decoded.bytes_le
        }
        LiveVramStorage::RawTypedPassThrough => bytes.to_vec(),
    };

    if restored.len() != metadata.descriptor.raw_len {
        return Err(QatqError::LengthMismatch {
            expected: metadata.descriptor.raw_len,
            actual: restored.len(),
        });
    }
    let actual = checksum_bytes(&restored);
    if actual != metadata.descriptor.checksum {
        return Err(QatqError::ChecksumMismatch {
            expected: metadata.descriptor.checksum,
            actual,
        });
    }
    Ok(restored)
}

pub fn simulate_live_vram_reduction(
    snapshots: &[KvPageSnapshot],
    state: LiveVramSchedulerState,
    policy: LiveVramSchedulerPolicy,
    limits: LiveVramLimits,
) -> Result<LiveVramSimulationReport, QatqError> {
    let mut state = state;
    let mut report = LiveVramSimulationReport {
        total_pages: 0,
        compressed_pages: 0,
        pass_through_pages: 0,
        resident_pages: 0,
        verified_restores: 0,
        raw_bytes: 0,
        stored_cpu_bytes: state.cpu_stored_bytes,
        kept_unknown_next_use: 0,
        kept_hot_window: 0,
        kept_prefetch_window: 0,
        kept_queue_full: 0,
        kept_cpu_budget: 0,
        kept_codec_not_beneficial: 0,
    };

    for snapshot in snapshots {
        validate_live_vram_snapshot(snapshot, limits)?;
        report.total_pages += 1;
        report.raw_bytes = report
            .raw_bytes
            .checked_add(snapshot.descriptor.raw_len)
            .ok_or(QatqError::ContainerLimitExceeded("live-vram-raw-bytes"))?;

        match schedule_live_vram_page(&snapshot.descriptor, state, policy) {
            LiveVramScheduleDecision::KeepResident(reason) => {
                record_live_vram_keep(&mut report, reason);
            }
            LiveVramScheduleDecision::Offload => {
                let encoded = try_encode_live_vram_page(snapshot, limits)?;
                let candidate_cpu_bytes =
                    state
                        .cpu_stored_bytes
                        .checked_add(encoded.bytes.len())
                        .ok_or(QatqError::ContainerLimitExceeded("live-vram-cpu-budget"))?;
                if candidate_cpu_bytes > policy.max_cpu_stored_bytes {
                    record_live_vram_keep(&mut report, LiveVramKeepReason::CpuBudgetExceeded);
                    continue;
                }
                let restored = restore_live_vram_page(&encoded.metadata, &encoded.bytes, limits)?;
                if restored != snapshot.bytes_le {
                    return Err(QatqError::ChecksumMismatch {
                        expected: snapshot.descriptor.checksum,
                        actual: checksum_bytes(&restored),
                    });
                }
                if encoded.should_compress() {
                    report.compressed_pages += 1;
                } else {
                    report.pass_through_pages += 1;
                }
                report.verified_restores += 1;
                state.queued_pages = state
                    .queued_pages
                    .checked_add(1)
                    .ok_or(QatqError::ContainerLimitExceeded("live-vram-queue"))?;
                state.cpu_stored_bytes = candidate_cpu_bytes;
                report.stored_cpu_bytes = candidate_cpu_bytes;
            }
        }
    }

    Ok(report)
}

pub fn build_live_vram_evidence_report(
    snapshots: &[KvPageSnapshot],
    state: LiveVramSchedulerState,
    policy: LiveVramSchedulerPolicy,
    limits: LiveVramLimits,
) -> Result<LiveVramEvidenceReport, QatqError> {
    build_live_vram_evidence_report_inner(snapshots, state, policy, limits, None, &[])
}

pub fn build_live_vram_evidence_report_with_page_seals(
    snapshots: &[KvPageSnapshot],
    state: LiveVramSchedulerState,
    policy: LiveVramSchedulerPolicy,
    limits: LiveVramLimits,
    seal_key: &[u8; 32],
    seal_context: &[u8],
) -> Result<LiveVramEvidenceReport, QatqError> {
    build_live_vram_evidence_report_inner(
        snapshots,
        state,
        policy,
        limits,
        Some(seal_key),
        seal_context,
    )
}

fn build_live_vram_evidence_report_inner(
    snapshots: &[KvPageSnapshot],
    state: LiveVramSchedulerState,
    policy: LiveVramSchedulerPolicy,
    limits: LiveVramLimits,
    seal_key: Option<&[u8; 32]>,
    seal_context: &[u8],
) -> Result<LiveVramEvidenceReport, QatqError> {
    let mut state = state;
    let mut report = LiveVramEvidenceReport {
        adapter_contract_version: LIVE_VRAM_ADAPTER_CONTRACT_VERSION.to_string(),
        total_pages: 0,
        offloaded_pages: 0,
        resident_pages: 0,
        compressed_pages: 0,
        pass_through_pages: 0,
        sealed_pages: 0,
        verified_restores: 0,
        raw_bytes: 0,
        qatq_candidate_bytes: 0,
        scheduled_stored_bytes: state.cpu_stored_bytes,
        zstd_bytes: 0,
        lz4_bytes: 0,
        qatq_beats_zstd_pages: 0,
        qatq_beats_lz4_pages: 0,
        qatq_beats_best_general_codec_pages: 0,
        pages: Vec::with_capacity(snapshots.len()),
    };

    for (page_index, snapshot) in snapshots.iter().enumerate() {
        validate_live_vram_snapshot(snapshot, limits)?;
        let encoded = try_encode_live_vram_page(snapshot, limits)?;
        let restored = restore_live_vram_page(&encoded.metadata, &encoded.bytes, limits)?;
        if restored != snapshot.bytes_le {
            return Err(QatqError::ChecksumMismatch {
                expected: snapshot.descriptor.checksum,
                actual: checksum_bytes(&restored),
            });
        }
        let zstd_bytes = zstd::bulk::compress(&snapshot.bytes_le, 3)
            .map_err(|_| QatqError::InvalidResidualStream)?
            .len();
        let lz4_bytes = lz4_flex::compress_prepend_size(&snapshot.bytes_le).len();

        let mut schedule_decision = schedule_live_vram_page(&snapshot.descriptor, state, policy);
        let mut scheduled_stored_bytes = 0_usize;
        let mut storage = None;
        let mut strategy = None;
        let mut metadata_seal = None;
        if schedule_decision == LiveVramScheduleDecision::Offload {
            if !encoded.should_compress()
                || (policy.require_qatq_beats_best_general_codec
                    && encoded.bytes.len() >= zstd_bytes.min(lz4_bytes))
            {
                schedule_decision =
                    LiveVramScheduleDecision::KeepResident(LiveVramKeepReason::CodecNotBeneficial);
            }
        }
        if schedule_decision == LiveVramScheduleDecision::Offload {
            let candidate_cpu_bytes = state
                .cpu_stored_bytes
                .checked_add(encoded.bytes.len())
                .ok_or(QatqError::ContainerLimitExceeded("live-vram-cpu-budget"))?;
            if candidate_cpu_bytes > policy.max_cpu_stored_bytes {
                schedule_decision =
                    LiveVramScheduleDecision::KeepResident(LiveVramKeepReason::CpuBudgetExceeded);
            } else {
                scheduled_stored_bytes = encoded.bytes.len();
                storage = Some(encoded.metadata.storage);
                strategy = encoded.metadata.strategy;
                if let Some(key) = seal_key {
                    let seal = seal_live_vram_page(
                        &encoded.metadata,
                        encoded.stored_bytes(),
                        key,
                        seal_context,
                        limits,
                    )?;
                    verify_live_vram_page_seal(
                        &encoded.metadata,
                        encoded.stored_bytes(),
                        &seal,
                        key,
                        seal_context,
                        limits,
                    )?;
                    metadata_seal = Some(seal);
                }
                state.queued_pages = state
                    .queued_pages
                    .checked_add(1)
                    .ok_or(QatqError::ContainerLimitExceeded("live-vram-queue"))?;
                state.cpu_stored_bytes = candidate_cpu_bytes;
            }
        }

        let page = LiveVramPageEvidence {
            page_index,
            descriptor: snapshot.descriptor.clone(),
            schedule_decision,
            storage,
            strategy,
            metadata_seal,
            raw_bytes: snapshot.descriptor.raw_len,
            qatq_candidate_bytes: encoded.bytes.len(),
            scheduled_stored_bytes,
            zstd_bytes,
            lz4_bytes,
            verified_restore: true,
        };

        report.total_pages += 1;
        report.raw_bytes = report
            .raw_bytes
            .checked_add(page.raw_bytes)
            .ok_or(QatqError::ContainerLimitExceeded("live-vram-raw-bytes"))?;
        report.qatq_candidate_bytes = report
            .qatq_candidate_bytes
            .checked_add(page.qatq_candidate_bytes)
            .ok_or(QatqError::ContainerLimitExceeded(
                "live-vram-qatq-candidates",
            ))?;
        report.zstd_bytes = report
            .zstd_bytes
            .checked_add(page.zstd_bytes)
            .ok_or(QatqError::ContainerLimitExceeded("live-vram-zstd-baseline"))?;
        report.lz4_bytes = report
            .lz4_bytes
            .checked_add(page.lz4_bytes)
            .ok_or(QatqError::ContainerLimitExceeded("live-vram-lz4-baseline"))?;
        report.scheduled_stored_bytes = state.cpu_stored_bytes;
        if page.verified_restore {
            report.verified_restores += 1;
        }
        if page.qatq_beats_zstd() {
            report.qatq_beats_zstd_pages += 1;
        }
        if page.qatq_beats_lz4() {
            report.qatq_beats_lz4_pages += 1;
        }
        if page.qatq_beats_best_general_codec() {
            report.qatq_beats_best_general_codec_pages += 1;
        }
        match page.schedule_decision {
            LiveVramScheduleDecision::Offload => {
                report.offloaded_pages += 1;
                match page.storage {
                    Some(LiveVramStorage::Qatq) => report.compressed_pages += 1,
                    Some(LiveVramStorage::RawTypedPassThrough) => report.pass_through_pages += 1,
                    None => return Err(QatqError::InvalidHeader),
                }
                if page.metadata_seal.is_some() {
                    report.sealed_pages += 1;
                }
            }
            LiveVramScheduleDecision::KeepResident(_) => report.resident_pages += 1,
        }
        report.pages.push(page);
    }

    Ok(report)
}

fn validate_live_vram_snapshot(
    snapshot: &KvPageSnapshot,
    limits: LiveVramLimits,
) -> Result<(), QatqError> {
    snapshot.descriptor.validate(limits)?;
    if snapshot.bytes_le.len() != snapshot.descriptor.raw_len {
        return Err(QatqError::LengthMismatch {
            expected: snapshot.descriptor.raw_len,
            actual: snapshot.bytes_le.len(),
        });
    }
    let actual = checksum_bytes(&snapshot.bytes_le);
    if actual != snapshot.descriptor.checksum {
        return Err(QatqError::ChecksumMismatch {
            expected: snapshot.descriptor.checksum,
            actual,
        });
    }
    Ok(())
}

fn validate_live_vram_identifier(value: &str, max_len: usize) -> Result<(), QatqError> {
    if value.is_empty() || value.len() > max_len {
        return Err(QatqError::InvalidHeader);
    }
    if !value
        .bytes()
        .all(|byte| matches!(byte, b' '..=b'~') && byte != b'\\')
    {
        return Err(QatqError::InvalidHeader);
    }
    Ok(())
}

fn record_live_vram_keep(report: &mut LiveVramSimulationReport, reason: LiveVramKeepReason) {
    report.resident_pages += 1;
    match reason {
        LiveVramKeepReason::UnknownNextUse => report.kept_unknown_next_use += 1,
        LiveVramKeepReason::InsideHotWindow => report.kept_hot_window += 1,
        LiveVramKeepReason::InsidePrefetchWindow => report.kept_prefetch_window += 1,
        LiveVramKeepReason::QueueFull => report.kept_queue_full += 1,
        LiveVramKeepReason::CpuBudgetExceeded => report.kept_cpu_budget += 1,
        LiveVramKeepReason::CodecNotBeneficial => report.kept_codec_not_beneficial += 1,
    }
}

fn safe_llama_cpp_manifest_file(file: &str) -> Result<&str, QatqError> {
    if file.is_empty()
        || file.contains('/')
        || file.contains('\\')
        || file.contains("..")
        || !file.bytes().all(|byte| matches!(byte, b'!'..=b'~'))
    {
        return Err(QatqError::InvalidHeader);
    }
    Ok(file)
}

fn validate_llama_cpp_manifest_file_matches_tensor(
    file: &str,
    name: &str,
    kind: KvPageKind,
    stream: u32,
    token_start: usize,
    token_end: usize,
) -> Result<(), QatqError> {
    let layer_id = layer_id_from_llama_cpp_tensor_name(name)?;
    let kind_label = match kind {
        KvPageKind::Key => "k",
        KvPageKind::Value => "v",
    };
    let stem = file.split('.').next().ok_or(QatqError::InvalidHeader)?;
    let expected_prefix = format!("cache_{kind_label}_l{layer_id}_s{stream}");
    if !stem.starts_with(&expected_prefix) {
        return Err(QatqError::InvalidHeader);
    }
    if let Some((_prefix, token_range)) = stem.split_once("_t") {
        let Some((start, end)) = token_range.split_once('_') else {
            return Err(QatqError::InvalidHeader);
        };
        if start
            .parse::<usize>()
            .map_err(|_| QatqError::InvalidHeader)?
            != token_start
            || end.parse::<usize>().map_err(|_| QatqError::InvalidHeader)? != token_end
        {
            return Err(QatqError::InvalidHeader);
        }
    }
    Ok(())
}

fn layer_id_from_llama_cpp_tensor_name(name: &str) -> Result<u32, QatqError> {
    let Some(offset) = name.find("_l") else {
        return Err(QatqError::InvalidHeader);
    };
    let digits = name[offset + 2..]
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(QatqError::InvalidHeader);
    }
    digits.parse().map_err(|_| QatqError::InvalidHeader)
}

fn json_string_field(text: &str, key: &str) -> Option<String> {
    let start = json_field_value_start(text, key)?;
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
    json_parse_string(text, start).map(|(value, _end)| value)
}

fn json_i64_field(text: &str, key: &str) -> Option<i64> {
    let start = json_field_value_start(text, key)?;
    let token = json_number_token(text, start)?;
    token.parse().ok()
}

fn json_usize_field(text: &str, key: &str) -> Option<usize> {
    let start = json_field_value_start(text, key)?;
    let token = json_number_token(text, start)?;
    token.parse().ok()
}

fn json_bool_field(text: &str, key: &str) -> Option<bool> {
    let start = json_field_value_start(text, key)?;
    if text[start..].starts_with("true") {
        Some(true)
    } else if text[start..].starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn json_field_value_start(text: &str, key: &str) -> Option<usize> {
    let pattern = format!("\"{key}\"");
    let key_pos = text.find(&pattern)?;
    let after_key = key_pos.checked_add(pattern.len())?;
    let colon_offset = text[after_key..].find(':')?;
    let mut index = after_key + colon_offset + 1;
    while matches!(
        text.as_bytes().get(index),
        Some(b' ' | b'\n' | b'\r' | b'\t')
    ) {
        index += 1;
    }
    Some(index)
}

fn json_number_token(text: &str, start: usize) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut end = start;
    if bytes.get(end) == Some(&b'-') {
        end += 1;
    }
    let digit_start = end;
    while matches!(bytes.get(end), Some(b'0'..=b'9')) {
        end += 1;
    }
    if end == digit_start {
        return None;
    }
    Some(&text[start..end])
}

fn json_parse_string(text: &str, start: usize) -> Option<(String, usize)> {
    let mut out = String::new();
    let mut chars = text[start..].char_indices();
    let (_, first) = chars.next()?;
    if first != '"' {
        return None;
    }
    let mut escaped = false;
    for (offset, ch) in chars {
        if escaped {
            match ch {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                '/' => out.push('/'),
                'b' => out.push('\u{08}'),
                'f' => out.push('\u{0c}'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                _ => return None,
            }
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some((out, start + offset + ch.len_utf8()));
        } else if ch.is_control() {
            return None;
        } else {
            out.push(ch);
        }
    }
    None
}

fn json_array_object_slices<'a>(text: &'a str, key: &str) -> Option<Vec<&'a str>> {
    let mut index = json_field_value_start(text, key)?;
    let bytes = text.as_bytes();
    if bytes.get(index) != Some(&b'[') {
        return None;
    }
    index += 1;
    let mut objects = Vec::new();
    loop {
        while matches!(bytes.get(index), Some(b' ' | b'\n' | b'\r' | b'\t' | b',')) {
            index += 1;
        }
        match bytes.get(index) {
            Some(b']') => return Some(objects),
            Some(b'{') => {
                let start = index;
                let mut depth = 0_usize;
                let mut in_string = false;
                let mut escaped = false;
                while let Some(byte) = bytes.get(index) {
                    if in_string {
                        if escaped {
                            escaped = false;
                        } else if *byte == b'\\' {
                            escaped = true;
                        } else if *byte == b'"' {
                            in_string = false;
                        }
                    } else if *byte == b'"' {
                        in_string = true;
                    } else if *byte == b'{' {
                        depth += 1;
                    } else if *byte == b'}' {
                        depth = depth.checked_sub(1)?;
                        if depth == 0 {
                            let end = index + 1;
                            objects.push(&text[start..end]);
                            index = end;
                            break;
                        }
                    }
                    index += 1;
                }
                if depth != 0 {
                    return None;
                }
            }
            _ => return None,
        }
    }
}

fn push_live_vram_page_evidence_json(
    out: &mut String,
    page: &LiveVramPageEvidence,
    trailing_comma: bool,
) {
    out.push_str("    {\n");
    push_json_field_usize(out, 6, "page_index", page.page_index, true);
    push_json_field_usize(out, 6, "layer_id", page.descriptor.layer_id as usize, true);
    push_json_field_str(out, 6, "kind", page.descriptor.kind.as_str(), true);
    push_json_field_str(out, 6, "dtype", page.descriptor.dtype.as_str(), true);
    push_json_field_str(out, 6, "layout", page.descriptor.layout.as_str(), true);
    push_json_field_str(out, 6, "runtime_id", &page.descriptor.runtime_id, true);
    push_json_field_str(
        out,
        6,
        "runtime_commit",
        &page.descriptor.runtime_commit,
        true,
    );
    push_json_field_str(
        out,
        6,
        "adapter_version",
        &page.descriptor.adapter_version,
        true,
    );
    push_json_field_str(out, 6, "model_id", &page.descriptor.model_id, true);
    push_json_field_str(out, 6, "seq_id", &page.descriptor.seq_id, true);
    push_json_field_usize(
        out,
        6,
        "token_start",
        page.descriptor.token_start as usize,
        true,
    );
    push_json_field_usize(
        out,
        6,
        "token_end",
        page.descriptor.token_end as usize,
        true,
    );
    push_json_field_str(
        out,
        6,
        "schedule",
        schedule_decision_label(page.schedule_decision),
        true,
    );
    push_json_field_str(
        out,
        6,
        "keep_reason",
        keep_reason_label(page.schedule_decision),
        true,
    );
    push_json_field_str(
        out,
        6,
        "storage",
        page.storage
            .map(LiveVramStorage::as_str)
            .unwrap_or("resident"),
        true,
    );
    push_json_field_str(
        out,
        6,
        "strategy",
        page.strategy
            .map(QatqExactStrategy::as_str)
            .unwrap_or("none"),
        true,
    );
    push_json_field_usize(out, 6, "raw_bytes", page.raw_bytes, true);
    push_json_field_usize(
        out,
        6,
        "qatq_candidate_bytes",
        page.qatq_candidate_bytes,
        true,
    );
    push_json_field_usize(
        out,
        6,
        "scheduled_stored_bytes",
        page.scheduled_stored_bytes,
        true,
    );
    push_json_field_usize(out, 6, "zstd_bytes", page.zstd_bytes, true);
    push_json_field_usize(out, 6, "lz4_bytes", page.lz4_bytes, true);
    push_live_vram_metadata_seal_json(out, page.metadata_seal.as_ref(), true);
    push_json_field_bool(out, 6, "verified_restore", page.verified_restore, false);
    out.push_str("    }");
    if trailing_comma {
        out.push(',');
    }
    out.push('\n');
}

fn push_live_vram_metadata_seal_json(
    out: &mut String,
    seal: Option<&LiveVramPageSeal>,
    trailing_comma: bool,
) {
    out.push_str("      \"metadata_seal\": ");
    match seal {
        Some(seal) => {
            out.push_str("{\"version\": ");
            out.push_str(&seal.version.to_string());
            out.push_str(", \"tag\": \"");
            push_hex_bytes(out, &seal.tag);
            out.push_str("\"}");
        }
        None => out.push_str("null"),
    }
    if trailing_comma {
        out.push(',');
    }
    out.push('\n');
}

fn push_hex_bytes(out: &mut String, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
}

fn push_live_vram_residency_estimate_json(
    out: &mut String,
    estimate: &LiveVramResidencyEstimate,
    trailing_comma: bool,
) {
    push_json_indent(out, 2);
    out.push_str("\"residency_estimate\": {\n");
    push_json_field_str(
        out,
        4,
        "allocation_granularity",
        estimate.allocation_granularity.as_str(),
        true,
    );
    push_json_field_usize(
        out,
        4,
        "gpu_context_bytes_before",
        estimate.gpu_context_bytes_before,
        true,
    );
    push_json_field_usize(
        out,
        4,
        "logical_offloaded_raw_bytes",
        estimate.logical_offloaded_raw_bytes,
        true,
    );
    push_json_field_usize(out, 4, "stored_cpu_bytes", estimate.stored_cpu_bytes, true);
    push_json_field_usize(
        out,
        4,
        "reclaimable_gpu_bytes",
        estimate.reclaimable_gpu_bytes,
        true,
    );
    push_json_field_usize(
        out,
        4,
        "gpu_context_bytes_after",
        estimate.gpu_context_bytes_after,
        false,
    );
    push_json_indent(out, 2);
    out.push('}');
    push_json_trailing(out, trailing_comma);
}

fn push_live_vram_restore_deadline_report_json(
    out: &mut String,
    report: &LiveVramPrefetchDeadlineReport,
    trailing_comma: bool,
) {
    push_json_indent(out, 2);
    out.push_str("\"restore_deadline_report\": {\n");
    push_json_field_usize(out, 4, "evaluated_pages", report.evaluated_pages, true);
    push_json_field_usize(out, 4, "prefetch_misses", report.prefetch_misses, true);
    push_json_field_usize(
        out,
        4,
        "pages_without_deadline",
        report.pages_without_deadline,
        true,
    );
    push_json_field_usize(
        out,
        4,
        "total_estimated_restore_bytes",
        report.total_estimated_restore_bytes,
        true,
    );
    push_json_field_usize(
        out,
        4,
        "worst_deficit_bytes",
        report.worst_deficit_bytes,
        false,
    );
    push_json_indent(out, 2);
    out.push('}');
    push_json_trailing(out, trailing_comma);
}

fn push_live_vram_event_trace_report_json(
    out: &mut String,
    report: &LiveVramEventTraceReport,
    trailing_comma: bool,
) {
    push_json_indent(out, 2);
    out.push_str("\"event_trace_report\": {\n");
    push_json_field_bool(out, 4, "passed", report.passed(), true);
    push_json_field_usize(out, 4, "events", report.events, true);
    push_json_field_usize(out, 4, "snapshots", report.snapshots, true);
    push_json_field_usize(out, 4, "offloads", report.offloads, true);
    push_json_field_usize(out, 4, "restores", report.restores, true);
    push_json_field_usize(out, 4, "attention_uses", report.attention_uses, true);
    push_json_field_usize(out, 4, "cancellations", report.cancellations, true);
    push_json_field_usize(
        out,
        4,
        "peak_offloaded_pages",
        report.peak_offloaded_pages,
        true,
    );
    push_json_field_usize(
        out,
        4,
        "offloaded_pages_at_end",
        report.offloaded_pages_at_end,
        true,
    );
    push_json_indent(out, 4);
    out.push_str("\"failures\": [");
    for (index, failure) in report.failures.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push('"');
        push_json_escaped(out, &failure.to_string());
        out.push('"');
    }
    out.push_str("]\n");
    push_json_indent(out, 2);
    out.push('}');
    push_json_trailing(out, trailing_comma);
}

fn schedule_decision_label(decision: LiveVramScheduleDecision) -> &'static str {
    match decision {
        LiveVramScheduleDecision::Offload => "offload",
        LiveVramScheduleDecision::KeepResident(_) => "keep-resident",
    }
}

fn keep_reason_label(decision: LiveVramScheduleDecision) -> &'static str {
    match decision {
        LiveVramScheduleDecision::Offload => "none",
        LiveVramScheduleDecision::KeepResident(reason) => reason.as_str(),
    }
}

fn push_json_field_str(
    out: &mut String,
    indent: usize,
    name: &str,
    value: &str,
    trailing_comma: bool,
) {
    push_json_indent(out, indent);
    out.push('"');
    out.push_str(name);
    out.push_str("\": \"");
    push_json_escaped(out, value);
    out.push('"');
    push_json_trailing(out, trailing_comma);
}

fn push_json_field_usize(
    out: &mut String,
    indent: usize,
    name: &str,
    value: usize,
    trailing_comma: bool,
) {
    push_json_indent(out, indent);
    out.push('"');
    out.push_str(name);
    out.push_str("\": ");
    out.push_str(&value.to_string());
    push_json_trailing(out, trailing_comma);
}

fn push_json_field_bool(
    out: &mut String,
    indent: usize,
    name: &str,
    value: bool,
    trailing_comma: bool,
) {
    push_json_indent(out, indent);
    out.push('"');
    out.push_str(name);
    out.push_str("\": ");
    out.push_str(if value { "true" } else { "false" });
    push_json_trailing(out, trailing_comma);
}

fn push_json_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push(' ');
    }
}

fn push_json_trailing(out: &mut String, trailing_comma: bool) {
    if trailing_comma {
        out.push(',');
    }
    out.push('\n');
}

fn push_json_escaped(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            ch if ch.is_control() => {
                out.push_str("\\u");
                out.push_str(&format!("{:04x}", ch as u32));
            }
            ch => out.push(ch),
        }
    }
}

fn push_prometheus_metric(out: &mut String, name: &str, value: usize) {
    out.push_str(name);
    out.push(' ');
    out.push_str(&value.to_string());
    out.push('\n');
}

fn push_prometheus_metric_u128(out: &mut String, name: &str, value: u128) {
    out.push_str(name);
    out.push(' ');
    out.push_str(&value.to_string());
    out.push('\n');
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
    fn id(self) -> u8 {
        match self {
            Self::PredictorXor => QATQ_EXACT_STRATEGY_PREDICTOR_XOR,
            Self::RawBits => QATQ_EXACT_STRATEGY_RAW_BITS,
            Self::ByteRle => QATQ_EXACT_STRATEGY_BYTE_RLE,
            Self::BytePlaneRle => QATQ_EXACT_STRATEGY_BYTE_PLANE_RLE,
            Self::DeltaXorBytePlaneRle => QATQ_EXACT_STRATEGY_DELTA_XOR_BYTE_PLANE_RLE,
            Self::BytePlaneBlocks => QATQ_EXACT_STRATEGY_BYTE_PLANE_BLOCKS,
            Self::BytePlanePackedRle => QATQ_EXACT_STRATEGY_BYTE_PLANE_PACKED_RLE,
            Self::BytePlaneZstd => QATQ_EXACT_STRATEGY_BYTE_PLANE_ZSTD,
            Self::QuaternionChainZstd => QATQ_EXACT_STRATEGY_QUATERNION_CHAIN_ZSTD,
        }
    }

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
    validate_qatq_exact_header_scale(&header, strategy)?;
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

#[allow(clippy::too_many_arguments)]
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
    validate_qatq_exact_header_scale(&header, strategy)?;
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

fn validate_qatq_exact_header_scale(
    header: &Header,
    strategy: QatqExactStrategy,
) -> Result<(), QatqError> {
    if strategy != QatqExactStrategy::PredictorXor && header.scale.to_bits() != 1.0_f32.to_bits() {
        return Err(QatqError::InvalidScale(header.scale.to_bits()));
    }
    Ok(())
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
    for block in &mut blocks {
        if offset >= bytes.len() {
            return Err(QatqError::InvalidResidualStream);
        }
        let tag = bytes[offset];
        offset += 1;
        match tag {
            BYTE_PLANE_BLOCK_ZERO => {
                *block = BytePlaneBlock::Zero;
            }
            BYTE_PLANE_BLOCK_REPEAT => {
                if offset >= bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                let value = bytes[offset];
                offset += 1;
                *block = BytePlaneBlock::Repeat(value);
            }
            BYTE_PLANE_BLOCK_RAW => {
                if offset + value_count > bytes.len() {
                    return Err(QatqError::InvalidResidualStream);
                }
                *block = BytePlaneBlock::Raw { offset };
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
    if !bytes.len().is_multiple_of(4) {
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
    if (*decoded_len).is_multiple_of(4) {
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
    if !bytes_le.len().is_multiple_of(width) {
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
    if !bytes.len().is_multiple_of(4) {
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
    fn qatq_exact_typed_rejects_mutated_unused_scale_header() {
        let bytes = vec![0_u8; 512];
        let mut encoded = encode_qatq_exact_tensor_le(&bytes, TensorDType::F16);
        assert!(matches!(
            qatq_exact_strategy(&encoded),
            Ok(QatqExactStrategy::ByteRle)
        ));

        encoded[17] ^= 0x40;

        assert_eq!(
            decode_qatq_exact_tensor_le(&encoded),
            Err(QatqError::InvalidScale(u32::from_be_bytes(
                encoded[16..20].try_into().unwrap()
            )))
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
    fn qatq_exact_rejects_mutated_unused_scale_header_for_non_predictor() {
        let values = vec![0.0_f32; 128];
        let mut encoded = encode_qatq_exact(&values);
        assert!(matches!(
            qatq_exact_strategy(&encoded),
            Ok(QatqExactStrategy::ByteRle)
        ));

        encoded[17] ^= 0x40;

        assert_eq!(
            decode_qatq_exact(&encoded),
            Err(QatqError::InvalidScale(u32::from_be_bytes(
                encoded[16..20].try_into().unwrap()
            )))
        );
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
    fn live_vram_page_roundtrip_restores_compressed_bf16_page() {
        let bytes = repeated_u16_bytes(0x3f80, 512);
        let snapshot = live_vram_snapshot(bytes, TensorDType::BF16, vec![8, 64], Some(10_000));

        let encoded = try_encode_live_vram_page(&snapshot, LiveVramLimits::default()).unwrap();
        assert!(encoded.should_compress());
        assert_eq!(encoded.metadata.storage_label(), "qatq-live");
        assert!(encoded.bytes.len() < snapshot.bytes_le.len());

        let restored = restore_live_vram_page(
            &encoded.metadata,
            encoded.stored_bytes(),
            LiveVramLimits::default(),
        )
        .unwrap();
        assert_eq!(restored, snapshot.bytes_le);
    }

    #[test]
    fn live_vram_page_uses_raw_pass_through_when_typed_page_does_not_shrink() {
        let bytes = deterministic_bytes(4096);
        let snapshot = live_vram_snapshot(bytes, TensorDType::F16, vec![2048], Some(10_000));

        let encoded = try_encode_live_vram_page(&snapshot, LiveVramLimits::default()).unwrap();
        assert!(encoded.should_pass_through());
        assert_eq!(encoded.metadata.storage_label(), "raw-typed-pass-through");

        let restored = restore_live_vram_page(
            &encoded.metadata,
            encoded.stored_bytes(),
            LiveVramLimits::default(),
        )
        .unwrap();
        assert_eq!(restored, snapshot.bytes_le);
    }

    #[test]
    fn live_vram_scheduler_keeps_pages_that_are_unsafe_to_evict() {
        let snapshot = live_vram_snapshot(vec![0; 256], TensorDType::F16, vec![128], None);
        let state = LiveVramSchedulerState {
            current_token: 512,
            queued_pages: 0,
            cpu_stored_bytes: 0,
        };
        let policy = LiveVramSchedulerPolicy {
            hot_window_tokens: 128,
            prefetch_window_tokens: 32,
            max_queued_pages: 4,
            max_cpu_stored_bytes: 4096,
            require_qatq_beats_best_general_codec: true,
        };

        assert_eq!(
            schedule_live_vram_page(&snapshot.descriptor, state, policy),
            LiveVramScheduleDecision::KeepResident(LiveVramKeepReason::UnknownNextUse)
        );

        let mut hot = snapshot.descriptor.clone();
        hot.next_required_token = Some(640);
        assert_eq!(
            schedule_live_vram_page(&hot, state, policy),
            LiveVramScheduleDecision::KeepResident(LiveVramKeepReason::InsideHotWindow)
        );

        let mut cold = hot;
        cold.next_required_token = Some(641);
        assert_eq!(
            schedule_live_vram_page(&cold, state, policy),
            LiveVramScheduleDecision::KeepResident(LiveVramKeepReason::InsidePrefetchWindow)
        );

        let mut cold_after_prefetch = cold;
        cold_after_prefetch.next_required_token = Some(673);
        assert_eq!(
            schedule_live_vram_page(&cold_after_prefetch, state, policy),
            LiveVramScheduleDecision::Offload
        );

        assert_eq!(
            schedule_live_vram_page(
                &cold_after_prefetch,
                LiveVramSchedulerState {
                    queued_pages: 4,
                    ..state
                },
                policy
            ),
            LiveVramScheduleDecision::KeepResident(LiveVramKeepReason::QueueFull)
        );
    }

    #[test]
    fn live_vram_page_rejects_checksum_and_shape_mismatches() {
        let mut checksum_bad =
            live_vram_snapshot(vec![0; 256], TensorDType::F16, vec![128], Some(10_000));
        checksum_bad.descriptor.checksum ^= 1;
        assert!(matches!(
            try_encode_live_vram_page(&checksum_bad, LiveVramLimits::default()),
            Err(QatqError::ChecksumMismatch { .. })
        ));

        let shape_bad = live_vram_snapshot(vec![0; 256], TensorDType::F16, vec![64], Some(10_000));
        assert_eq!(
            try_encode_live_vram_page(&shape_bad, LiveVramLimits::default()),
            Err(QatqError::LengthMismatch {
                expected: 128,
                actual: 256
            })
        );
    }

    #[test]
    fn live_vram_restore_rejects_metadata_tampering() {
        let snapshot = live_vram_snapshot(vec![0; 256], TensorDType::F16, vec![128], Some(10_000));
        let mut encoded = try_encode_live_vram_page(&snapshot, LiveVramLimits::default()).unwrap();
        encoded.metadata.descriptor.dtype = TensorDType::BF16;

        assert!(matches!(
            restore_live_vram_page(
                &encoded.metadata,
                encoded.stored_bytes(),
                LiveVramLimits::default()
            ),
            Err(QatqError::ChecksumMismatch { .. }) | Err(QatqError::InvalidHeader)
        ));
    }

    #[test]
    fn live_vram_page_seal_rejects_metadata_payload_context_and_key_tampering() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(10_000),
        );
        let encoded = try_encode_live_vram_page(&snapshot, LiveVramLimits::default()).unwrap();
        assert!(encoded.should_compress());
        let key = [0x51_u8; 32];
        let context = b"runtime=session-1;tenant=alpha";
        let seal = seal_live_vram_page(
            &encoded.metadata,
            encoded.stored_bytes(),
            &key,
            context,
            LiveVramLimits::default(),
        )
        .unwrap();

        verify_live_vram_page_seal(
            &encoded.metadata,
            encoded.stored_bytes(),
            &seal,
            &key,
            context,
            LiveVramLimits::default(),
        )
        .unwrap();

        let mut tampered_bytes = encoded.bytes.clone();
        let last = tampered_bytes.last_mut().unwrap();
        *last ^= 0x01;
        assert_eq!(
            verify_live_vram_page_seal(
                &encoded.metadata,
                &tampered_bytes,
                &seal,
                &key,
                context,
                LiveVramLimits::default(),
            ),
            Err(QatqError::MetadataSealMismatch)
        );

        let mut tampered_metadata = encoded.metadata.clone();
        tampered_metadata.descriptor.seq_id.push_str("-forged");
        assert_eq!(
            verify_live_vram_page_seal(
                &tampered_metadata,
                encoded.stored_bytes(),
                &seal,
                &key,
                context,
                LiveVramLimits::default(),
            ),
            Err(QatqError::MetadataSealMismatch)
        );

        let mut tampered_metadata = encoded.metadata.clone();
        tampered_metadata.descriptor.layer_id += 1;
        assert_eq!(
            verify_live_vram_page_seal(
                &tampered_metadata,
                encoded.stored_bytes(),
                &seal,
                &key,
                context,
                LiveVramLimits::default(),
            ),
            Err(QatqError::MetadataSealMismatch)
        );

        let mut tampered_metadata = encoded.metadata.clone();
        tampered_metadata.descriptor.token_end += 1;
        assert_eq!(
            verify_live_vram_page_seal(
                &tampered_metadata,
                encoded.stored_bytes(),
                &seal,
                &key,
                context,
                LiveVramLimits::default(),
            ),
            Err(QatqError::MetadataSealMismatch)
        );

        let mut tampered_metadata = encoded.metadata.clone();
        tampered_metadata.strategy = None;
        assert_eq!(
            verify_live_vram_page_seal(
                &tampered_metadata,
                encoded.stored_bytes(),
                &seal,
                &key,
                context,
                LiveVramLimits::default(),
            ),
            Err(QatqError::MetadataSealMismatch)
        );

        assert_eq!(
            verify_live_vram_page_seal(
                &encoded.metadata,
                encoded.stored_bytes(),
                &seal,
                &key,
                b"runtime=session-2;tenant=alpha",
                LiveVramLimits::default(),
            ),
            Err(QatqError::MetadataSealMismatch)
        );

        let wrong_key = [0x52_u8; 32];
        assert_eq!(
            verify_live_vram_page_seal(
                &encoded.metadata,
                encoded.stored_bytes(),
                &seal,
                &wrong_key,
                context,
                LiveVramLimits::default(),
            ),
            Err(QatqError::MetadataSealMismatch)
        );

        let mut wrong_version = seal;
        wrong_version.version += 1;
        assert_eq!(
            verify_live_vram_page_seal(
                &encoded.metadata,
                encoded.stored_bytes(),
                &wrong_version,
                &key,
                context,
                LiveVramLimits::default(),
            ),
            Err(QatqError::MetadataSealMismatch)
        );
    }

    #[test]
    fn live_vram_page_seal_covers_raw_pass_through_storage_metadata() {
        let snapshot = live_vram_snapshot(
            deterministic_bytes(4096),
            TensorDType::F16,
            vec![2048],
            Some(10_000),
        );
        let encoded = try_encode_live_vram_page(&snapshot, LiveVramLimits::default()).unwrap();
        assert!(encoded.should_pass_through());
        let key = [0x99_u8; 32];
        let context = b"runtime=session-raw;tenant=beta";
        let seal = seal_live_vram_page(
            &encoded.metadata,
            encoded.stored_bytes(),
            &key,
            context,
            LiveVramLimits::default(),
        )
        .unwrap();

        verify_live_vram_page_seal(
            &encoded.metadata,
            encoded.stored_bytes(),
            &seal,
            &key,
            context,
            LiveVramLimits::default(),
        )
        .unwrap();

        let mut tampered_metadata = encoded.metadata.clone();
        tampered_metadata.storage = LiveVramStorage::Qatq;
        assert_eq!(
            verify_live_vram_page_seal(
                &tampered_metadata,
                encoded.stored_bytes(),
                &seal,
                &key,
                context,
                LiveVramLimits::default(),
            ),
            Err(QatqError::MetadataSealMismatch)
        );
    }

    #[test]
    fn live_vram_sealed_restore_request_verifies_before_adapter_upload() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(10_000),
        );
        let store = {
            let mut store =
                LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 8 * 1024 * 1024)
                    .with_page_seal_policy(live_vram_test_seal_policy());
            let key = store.commit_snapshot(&snapshot).unwrap();
            let request = store.sealed_restore_request(&key).unwrap();

            assert_eq!(request.metadata().descriptor, snapshot.descriptor);
            assert_eq!(request.metadata_seal().version, LIVE_VRAM_PAGE_SEAL_VERSION);
            assert_eq!(
                request.restore_bytes(LiveVramLimits::default()).unwrap(),
                snapshot.bytes_le
            );
            store
        };
        assert_eq!(store.metrics().restore_failures, 0);
    }

    #[test]
    fn live_vram_sealed_restore_request_rejects_tamper_and_cross_context_replay() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(10_000),
        );
        let mut store = LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 8 * 1024 * 1024)
            .with_page_seal_policy(live_vram_test_seal_policy());
        let key = store.commit_snapshot(&snapshot).unwrap();
        let entry = store.entry(&key).unwrap().clone();
        let seal = entry.metadata_seal.as_ref().unwrap();

        let wrong_context =
            LiveVramPageSealPolicy::new([0xA5; 32], b"qatq-live-vram-test-other-session".to_vec())
                .unwrap();
        assert_eq!(
            wrong_context.verify_restore_request(
                &entry.metadata,
                &entry.bytes,
                seal,
                LiveVramLimits::default(),
            ),
            Err(QatqError::MetadataSealMismatch)
        );

        store.entries.get_mut(&key).unwrap().bytes[0] ^= 0x01;
        assert_eq!(
            store.sealed_restore_request(&key),
            Err(QatqError::MetadataSealMismatch)
        );
    }

    #[test]
    fn live_vram_simulation_accounts_budget_and_verified_restores() {
        let snapshots = vec![
            live_vram_snapshot(
                repeated_u16_bytes(0x3f80, 512),
                TensorDType::BF16,
                vec![512],
                Some(5000),
            ),
            live_vram_snapshot(
                deterministic_bytes(1024),
                TensorDType::F16,
                vec![512],
                Some(5000),
            ),
            live_vram_snapshot(
                repeated_u16_bytes(0x4100, 512),
                TensorDType::BF16,
                vec![512],
                None,
            ),
        ];
        let report = simulate_live_vram_reduction(
            &snapshots,
            LiveVramSchedulerState {
                current_token: 0,
                queued_pages: 0,
                cpu_stored_bytes: 0,
            },
            LiveVramSchedulerPolicy {
                hot_window_tokens: 128,
                prefetch_window_tokens: 32,
                max_queued_pages: 16,
                max_cpu_stored_bytes: 256,
                require_qatq_beats_best_general_codec: true,
            },
            LiveVramLimits::default(),
        )
        .unwrap();

        assert_eq!(report.total_pages, 3);
        assert!(report.verified_restores >= 1);
        assert!(report.compressed_pages >= 1);
        assert!(report.kept_unknown_next_use >= 1);
        assert!(report.kept_cpu_budget >= 1);
        assert!(report.stored_ratio().unwrap() < 1.0);
    }

    #[test]
    fn live_vram_simulation_stresses_many_kv_pages() {
        let mut snapshots = Vec::new();
        for page in 0..1024 {
            let dtype = if page % 3 == 0 {
                TensorDType::BF16
            } else {
                TensorDType::F16
            };
            let bytes = if page % 7 == 0 {
                deterministic_bytes(1024)
            } else {
                repeated_u16_bytes(0x3c00_u16.wrapping_add(page as u16), 512)
            };
            snapshots.push(live_vram_snapshot(bytes, dtype, vec![8, 64], Some(20_000)));
        }

        let report = simulate_live_vram_reduction(
            &snapshots,
            LiveVramSchedulerState {
                current_token: 0,
                queued_pages: 0,
                cpu_stored_bytes: 0,
            },
            LiveVramSchedulerPolicy {
                hot_window_tokens: 128,
                prefetch_window_tokens: 32,
                max_queued_pages: 2048,
                max_cpu_stored_bytes: 8 * 1024 * 1024,
                require_qatq_beats_best_general_codec: true,
            },
            LiveVramLimits::default(),
        )
        .unwrap();

        assert_eq!(report.total_pages, 1024);
        assert_eq!(report.resident_pages, 0);
        assert_eq!(
            report.verified_restores,
            report.compressed_pages + report.pass_through_pages
        );
        assert!(report.compressed_pages > report.pass_through_pages);
        assert!(report.stored_ratio().unwrap() < 0.5);
    }

    #[test]
    fn live_vram_adapter_contract_supports_safe_snapshot_commit_restore() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(10_000),
        );
        let mut adapter = TestLiveVramAdapter {
            snapshot: snapshot.clone(),
            committed: None,
            metrics: LiveVramAdapterMetrics {
                resident_pages: 1,
                current_gpu_bytes: snapshot.bytes_le.len(),
                peak_gpu_bytes: snapshot.bytes_le.len(),
                ..LiveVramAdapterMetrics::default()
            },
            fail_commit: false,
        };

        adapter
            .identity()
            .validate(LiveVramLimits::default())
            .unwrap();
        let captured = adapter
            .snapshot_page(&snapshot.descriptor, LiveVramLimits::default())
            .unwrap();
        let encoded = try_encode_live_vram_page(&captured, LiveVramLimits::default()).unwrap();
        adapter.commit_offload(&encoded).unwrap();
        assert!(!adapter.is_page_resident(&snapshot.descriptor).unwrap());
        assert_eq!(adapter.metrics().unwrap().offloaded_pages, 1);

        let status = adapter
            .restore_committed_page(
                &encoded.metadata,
                encoded.stored_bytes(),
                LiveVramLimits::default(),
            )
            .unwrap();
        assert_eq!(status, LiveVramRestoreStatus::Restored);
        assert!(adapter.is_page_resident(&snapshot.descriptor).unwrap());

        let mut stale_identity = adapter.identity();
        stale_identity.adapter_contract_version = "old-contract".to_string();
        assert_eq!(
            stale_identity.validate(LiveVramLimits::default()),
            Err(QatqError::InvalidHeader)
        );
    }

    #[test]
    fn live_vram_evidence_report_compares_same_pages_against_general_codecs() {
        let snapshots = vec![
            live_vram_snapshot(
                repeated_u16_bytes(0x3f80, 512),
                TensorDType::BF16,
                vec![512],
                Some(5000),
            ),
            live_vram_snapshot(
                deterministic_bytes(1024),
                TensorDType::F16,
                vec![512],
                Some(5000),
            ),
            live_vram_snapshot(
                repeated_u16_bytes(0x4000, 512),
                TensorDType::BF16,
                vec![512],
                None,
            ),
        ];

        let report = build_live_vram_evidence_report(
            &snapshots,
            LiveVramSchedulerState {
                current_token: 0,
                queued_pages: 0,
                cpu_stored_bytes: 0,
            },
            LiveVramSchedulerPolicy {
                hot_window_tokens: 128,
                prefetch_window_tokens: 32,
                max_queued_pages: 16,
                max_cpu_stored_bytes: 8 * 1024 * 1024,
                require_qatq_beats_best_general_codec: true,
            },
            LiveVramLimits::default(),
        )
        .unwrap();

        assert_eq!(
            report.adapter_contract_version,
            LIVE_VRAM_ADAPTER_CONTRACT_VERSION
        );
        assert_eq!(report.total_pages, 3);
        assert_eq!(report.offloaded_pages, 0);
        assert_eq!(report.resident_pages, 3);
        assert_eq!(report.verified_restores, 3);
        assert_eq!(report.compressed_pages, 0);
        assert_eq!(report.pass_through_pages, 0);
        assert_eq!(report.raw_bytes, 3 * 1024);
        assert!(report.qatq_candidate_ratio().unwrap() < 1.0);
        assert!(report.zstd_ratio().unwrap() > 0.0);
        assert!(report.lz4_ratio().unwrap() > 0.0);
        assert!(report.qatq_beats_lz4_pages >= 1);
        assert!(report.pages.iter().any(|page| {
            page.schedule_decision
                == LiveVramScheduleDecision::KeepResident(LiveVramKeepReason::CodecNotBeneficial)
        }));

        let json = report.to_json();
        assert!(json.contains("\"adapter_contract_version\": \"qatq-live-vram-adapter-v0\""));
        assert!(json.contains("\"pages\": ["));
        assert!(json.contains("\"schedule\": \"keep-resident\""));
        assert!(json.contains("\"keep_reason\": \"codec-not-beneficial\""));
        assert!(json.contains("\"verified_restore\": true"));
    }

    #[test]
    fn live_vram_evidence_keeps_codec_negative_pages_resident() {
        let mut snapshot = live_vram_snapshot(
            deterministic_bytes(1024),
            TensorDType::F16,
            vec![512],
            Some(4096),
        );
        snapshot.descriptor.token_start = 1024;
        snapshot.descriptor.token_end = 1280;
        let key = KvPageKey::from_descriptor(&snapshot.descriptor);
        let checksum = snapshot.descriptor.checksum;

        let evidence = build_live_vram_evidence_report(
            &[snapshot],
            LiveVramSchedulerState {
                current_token: 0,
                queued_pages: 0,
                cpu_stored_bytes: 0,
            },
            LiveVramSchedulerPolicy {
                hot_window_tokens: 0,
                prefetch_window_tokens: 0,
                max_queued_pages: 16,
                max_cpu_stored_bytes: 8 * 1024 * 1024,
                require_qatq_beats_best_general_codec: true,
            },
            LiveVramLimits::default(),
        )
        .unwrap();

        assert_eq!(evidence.offloaded_pages, 0);
        assert_eq!(evidence.resident_pages, 1);
        assert_eq!(
            evidence.pages[0].schedule_decision,
            LiveVramScheduleDecision::KeepResident(LiveVramKeepReason::CodecNotBeneficial)
        );

        let residency = estimate_live_vram_residency_after_offload(
            &evidence,
            evidence.raw_bytes,
            LiveVramGpuAllocationGranularity::PerPage,
        );
        let deadlines = evaluate_live_vram_prefetch_deadlines(
            &evidence,
            0,
            LiveVramPrefetchBudget {
                restore_bytes_per_token: 1 << 20,
            },
        )
        .unwrap();
        let trace = vec![
            live_vram_event(0, &key, LiveVramPageEventKind::Snapshot, Some(checksum)),
            live_vram_event(
                1,
                &key,
                LiveVramPageEventKind::OffloadCommitted,
                Some(checksum),
            ),
            live_vram_event(
                2,
                &key,
                LiveVramPageEventKind::RestoreCommitted,
                Some(checksum),
            ),
            live_vram_event(3, &key, LiveVramPageEventKind::AttentionUse, None),
        ];

        let proof = evaluate_live_vram_live_paging_proof_gate(
            &evidence,
            Some(&residency),
            Some(&deadlines),
            &trace,
            LiveVramProofGate {
                min_gpu_saved_ratio: 0.0,
                ..LiveVramProofGate::default()
            },
            LiveVramEventTracePolicy::default(),
        )
        .unwrap();

        assert!(!proof.passed());
        assert!(proof.event_trace.failures.iter().any(|failure| matches!(
            failure,
            LiveVramEventTraceFailure::EvidenceResidentPageOffloadedInTrace { key: failed_key }
                if **failed_key == key
        )));
    }

    #[test]
    fn live_vram_evidence_can_use_aggregate_codec_policy_for_tail_pages() {
        let snapshot = qatq_smaller_than_raw_but_not_best_general_snapshot();

        let strict = build_live_vram_evidence_report(
            std::slice::from_ref(&snapshot),
            LiveVramSchedulerState {
                current_token: 0,
                queued_pages: 0,
                cpu_stored_bytes: 0,
            },
            LiveVramSchedulerPolicy {
                hot_window_tokens: 0,
                prefetch_window_tokens: 0,
                max_queued_pages: 16,
                max_cpu_stored_bytes: 8 * 1024 * 1024,
                require_qatq_beats_best_general_codec: true,
            },
            LiveVramLimits::default(),
        )
        .unwrap();
        assert_eq!(strict.offloaded_pages, 0);
        assert_eq!(
            strict.pages[0].schedule_decision,
            LiveVramScheduleDecision::KeepResident(LiveVramKeepReason::CodecNotBeneficial)
        );

        let aggregate = build_live_vram_evidence_report(
            std::slice::from_ref(&snapshot),
            LiveVramSchedulerState {
                current_token: 0,
                queued_pages: 0,
                cpu_stored_bytes: 0,
            },
            LiveVramSchedulerPolicy {
                hot_window_tokens: 0,
                prefetch_window_tokens: 32,
                max_queued_pages: 16,
                max_cpu_stored_bytes: 8 * 1024 * 1024,
                require_qatq_beats_best_general_codec: false,
            },
            LiveVramLimits::default(),
        )
        .unwrap();
        assert_eq!(aggregate.offloaded_pages, 1);
        assert_eq!(aggregate.compressed_pages, 1);
        assert_eq!(aggregate.pass_through_pages, 0);
        assert_eq!(
            aggregate.pages[0].schedule_decision,
            LiveVramScheduleDecision::Offload
        );
        assert!(aggregate.pages[0].qatq_candidate_bytes < aggregate.pages[0].raw_bytes);
        assert!(!aggregate.pages[0].qatq_beats_best_general_codec());

        let residency = estimate_live_vram_residency_after_offload(
            &aggregate,
            aggregate.raw_bytes,
            LiveVramGpuAllocationGranularity::PerPage,
        );
        let deadlines = evaluate_live_vram_prefetch_deadlines(
            &aggregate,
            0,
            LiveVramPrefetchBudget {
                restore_bytes_per_token: 1 << 20,
            },
        )
        .unwrap();
        let strict_proof = evaluate_live_vram_proof_gate(
            &aggregate,
            Some(&residency),
            Some(&deadlines),
            LiveVramProofGate {
                min_gpu_saved_ratio: 0.0,
                ..LiveVramProofGate::default()
            },
        )
        .unwrap();
        assert!(!strict_proof.passed());

        let aggregate_proof = evaluate_live_vram_proof_gate(
            &aggregate,
            Some(&residency),
            Some(&deadlines),
            LiveVramProofGate {
                min_gpu_saved_ratio: 0.0,
                require_aggregate_qatq_beats_best_general_codec: true,
                require_all_pages_beat_best_general_codec: false,
                ..LiveVramProofGate::default()
            },
        )
        .unwrap();
        assert!(!aggregate_proof.passed());
        assert!(aggregate_proof.failures.iter().any(|failure| matches!(
            failure,
            LiveVramProofGateFailure::QatqDidNotBeatBestGeneralCodecInAggregate { .. }
        )));

        let mut positive = compression_positive_live_vram_snapshot();
        positive.descriptor.kind = KvPageKind::Value;
        positive.descriptor.token_start = 1280;
        positive.descriptor.token_end = 1280 + positive.descriptor.shape[0] as u64;
        positive.descriptor.next_required_token = Some(8192);
        let mixed = build_live_vram_evidence_report(
            &[snapshot, positive],
            LiveVramSchedulerState {
                current_token: 0,
                queued_pages: 0,
                cpu_stored_bytes: 0,
            },
            LiveVramSchedulerPolicy {
                hot_window_tokens: 0,
                prefetch_window_tokens: 32,
                max_queued_pages: 16,
                max_cpu_stored_bytes: 8 * 1024 * 1024,
                require_qatq_beats_best_general_codec: false,
            },
            LiveVramLimits::default(),
        )
        .unwrap();
        assert_eq!(mixed.offloaded_pages, 2);
        assert!(mixed.qatq_candidate_bytes < mixed.zstd_bytes.min(mixed.lz4_bytes));
        let mixed_residency = estimate_live_vram_residency_after_offload(
            &mixed,
            mixed.raw_bytes,
            LiveVramGpuAllocationGranularity::PerPage,
        );
        let mixed_deadlines = evaluate_live_vram_prefetch_deadlines(
            &mixed,
            0,
            LiveVramPrefetchBudget {
                restore_bytes_per_token: 1 << 20,
            },
        )
        .unwrap();
        let mixed_proof = evaluate_live_vram_proof_gate(
            &mixed,
            Some(&mixed_residency),
            Some(&mixed_deadlines),
            LiveVramProofGate {
                min_gpu_saved_ratio: 0.0,
                require_aggregate_qatq_beats_best_general_codec: true,
                require_all_pages_beat_best_general_codec: false,
                ..LiveVramProofGate::default()
            },
        )
        .unwrap();
        assert!(mixed_proof.passed());
    }

    #[test]
    fn live_vram_residency_estimate_does_not_claim_whole_buffer_reclaim() {
        let first = compression_positive_live_vram_snapshot();
        let mut second = compression_positive_live_vram_snapshot();
        second.descriptor.kind = KvPageKind::Value;
        second.descriptor.token_start = 128;
        second.descriptor.token_end = 256;
        let snapshots = vec![first, second];
        let total_raw_bytes: usize = snapshots
            .iter()
            .map(|snapshot| snapshot.descriptor.raw_len)
            .sum();
        let report = build_live_vram_evidence_report(
            &snapshots,
            LiveVramSchedulerState {
                current_token: 0,
                queued_pages: 0,
                cpu_stored_bytes: 0,
            },
            LiveVramSchedulerPolicy {
                hot_window_tokens: 128,
                prefetch_window_tokens: 32,
                max_queued_pages: 16,
                max_cpu_stored_bytes: 8 * 1024 * 1024,
                require_qatq_beats_best_general_codec: true,
            },
            LiveVramLimits::default(),
        )
        .unwrap();

        let whole_context = estimate_live_vram_residency_after_offload(
            &report,
            total_raw_bytes,
            LiveVramGpuAllocationGranularity::WholeContext,
        );
        assert_eq!(whole_context.logical_offloaded_raw_bytes, total_raw_bytes);
        assert_eq!(whole_context.reclaimable_gpu_bytes, 0);
        assert_eq!(whole_context.gpu_context_bytes_after, total_raw_bytes);
        assert_eq!(whole_context.gpu_saved_ratio(), Some(0.0));
        let whole_context_json = report.to_json_with_residency_estimate(&whole_context);
        assert!(whole_context_json.contains("\"allocation_granularity\": \"whole-context\""));
        assert!(whole_context_json.contains("\"reclaimable_gpu_bytes\": 0"));

        let per_page = estimate_live_vram_residency_after_offload(
            &report,
            total_raw_bytes,
            LiveVramGpuAllocationGranularity::PerPage,
        );
        assert_eq!(per_page.logical_offloaded_raw_bytes, total_raw_bytes);
        assert_eq!(per_page.reclaimable_gpu_bytes, total_raw_bytes);
        assert_eq!(per_page.gpu_context_bytes_after, 0);
        assert_eq!(per_page.gpu_saved_ratio(), Some(1.0));

        let whole_tensor = estimate_live_vram_residency_from_runtime_allocation(
            &report,
            total_raw_bytes * 2,
            total_raw_bytes,
            LiveVramGpuAllocationGranularity::WholeTensor,
        )
        .unwrap();
        assert_eq!(whole_tensor.logical_offloaded_raw_bytes, total_raw_bytes);
        assert_eq!(whole_tensor.reclaimable_gpu_bytes, total_raw_bytes);
        assert_eq!(whole_tensor.gpu_context_bytes_after, total_raw_bytes);
        assert_eq!(whole_tensor.gpu_saved_ratio(), Some(0.5));
        assert_eq!(
            estimate_live_vram_residency_from_runtime_allocation(
                &report,
                total_raw_bytes,
                total_raw_bytes * 2,
                LiveVramGpuAllocationGranularity::WholeTensor,
            ),
            Err(QatqError::InvalidHeader)
        );
    }

    #[test]
    fn live_vram_prefetch_deadline_report_flags_restore_misses() {
        let mut snapshot = compression_positive_live_vram_snapshot();
        snapshot.descriptor.next_required_token = Some(2);
        let snapshots = vec![snapshot];
        let evidence = build_live_vram_evidence_report(
            &snapshots,
            LiveVramSchedulerState {
                current_token: 0,
                queued_pages: 0,
                cpu_stored_bytes: 0,
            },
            LiveVramSchedulerPolicy {
                hot_window_tokens: 0,
                prefetch_window_tokens: 0,
                max_queued_pages: 16,
                max_cpu_stored_bytes: 8 * 1024 * 1024,
                require_qatq_beats_best_general_codec: true,
            },
            LiveVramLimits::default(),
        )
        .unwrap();

        let tight = evaluate_live_vram_prefetch_deadlines(
            &evidence,
            0,
            LiveVramPrefetchBudget {
                restore_bytes_per_token: 1,
            },
        )
        .unwrap();
        assert_eq!(tight.evaluated_pages, 1);
        assert_eq!(tight.prefetch_misses, 1);
        assert!(tight.worst_deficit_bytes > 0);
        let json = evidence.to_json_with_restore_deadline_report(&tight);
        assert!(json.contains("\"restore_deadline_report\": {"));
        assert!(json.contains("\"prefetch_misses\": 1"));
        assert!(json.contains("\"worst_deficit_bytes\""));

        let roomy = evaluate_live_vram_prefetch_deadlines(
            &evidence,
            0,
            LiveVramPrefetchBudget {
                restore_bytes_per_token: 1 << 20,
            },
        )
        .unwrap();
        assert_eq!(roomy.evaluated_pages, 1);
        assert_eq!(roomy.prefetch_misses, 0);
    }

    #[test]
    fn live_vram_proof_gate_requires_real_reclaim_and_restore_deadlines() {
        let mut state = 0x9e37_79b9_u32;
        let mut bytes = Vec::new();
        for index in 0..4096 {
            state = lcg_next_for_test(state ^ index as u32);
            let mantissa = state & 0x007f_ffff;
            let exponent = 124 + (state % 6);
            let sign = (state >> 31) << 31;
            let value = f32::from_bits(sign | (exponent << 23) | mantissa);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let raw_len = bytes.len();
        let snapshots = vec![live_vram_snapshot(
            bytes,
            TensorDType::F32,
            vec![4096],
            Some(2),
        )];
        let evidence = build_live_vram_evidence_report(
            &snapshots,
            LiveVramSchedulerState {
                current_token: 0,
                queued_pages: 0,
                cpu_stored_bytes: 0,
            },
            LiveVramSchedulerPolicy {
                hot_window_tokens: 0,
                prefetch_window_tokens: 0,
                max_queued_pages: 16,
                max_cpu_stored_bytes: 8 * 1024 * 1024,
                require_qatq_beats_best_general_codec: true,
            },
            LiveVramLimits::default(),
        )
        .unwrap();
        let roomy_deadlines = evaluate_live_vram_prefetch_deadlines(
            &evidence,
            0,
            LiveVramPrefetchBudget {
                restore_bytes_per_token: 1 << 20,
            },
        )
        .unwrap();
        let per_page = estimate_live_vram_residency_after_offload(
            &evidence,
            raw_len * 2,
            LiveVramGpuAllocationGranularity::PerPage,
        );
        let passed = evaluate_live_vram_proof_gate(
            &evidence,
            Some(&per_page),
            Some(&roomy_deadlines),
            LiveVramProofGate {
                min_gpu_saved_ratio: 0.25,
                ..LiveVramProofGate::default()
            },
        )
        .unwrap();
        assert!(passed.passed());

        let key = KvPageKey::from_descriptor(&snapshots[0].descriptor);
        let checksum = snapshots[0].descriptor.checksum;
        let valid_trace = vec![
            live_vram_event(0, &key, LiveVramPageEventKind::Snapshot, Some(checksum)),
            live_vram_event(
                1,
                &key,
                LiveVramPageEventKind::OffloadCommitted,
                Some(checksum),
            ),
            live_vram_event(
                2,
                &key,
                LiveVramPageEventKind::RestoreCommitted,
                Some(checksum),
            ),
            live_vram_event(3, &key, LiveVramPageEventKind::AttentionUse, None),
        ];
        let live_paging_proof = evaluate_live_vram_live_paging_proof_gate(
            &evidence,
            Some(&per_page),
            Some(&roomy_deadlines),
            &valid_trace,
            LiveVramProofGate {
                min_gpu_saved_ratio: 0.25,
                ..LiveVramProofGate::default()
            },
            LiveVramEventTracePolicy::default(),
        )
        .unwrap();
        assert!(live_paging_proof.passed());

        let invalid_trace = vec![
            live_vram_event(0, &key, LiveVramPageEventKind::Snapshot, Some(checksum)),
            live_vram_event(
                1,
                &key,
                LiveVramPageEventKind::OffloadCommitted,
                Some(checksum),
            ),
            live_vram_event(2, &key, LiveVramPageEventKind::AttentionUse, None),
        ];
        let unsafe_live_paging_proof = evaluate_live_vram_live_paging_proof_gate(
            &evidence,
            Some(&per_page),
            Some(&roomy_deadlines),
            &invalid_trace,
            LiveVramProofGate {
                min_gpu_saved_ratio: 0.25,
                ..LiveVramProofGate::default()
            },
            LiveVramEventTracePolicy::default(),
        )
        .unwrap();
        assert!(unsafe_live_paging_proof.proof_gate.passed());
        assert!(!unsafe_live_paging_proof.passed());
        assert!(
            unsafe_live_paging_proof
                .event_trace
                .failures
                .iter()
                .any(|failure| matches!(
                    failure,
                    LiveVramEventTraceFailure::AttentionUseWhileOffloaded { .. }
                ))
        );

        let whole_context = estimate_live_vram_residency_after_offload(
            &evidence,
            raw_len * 2,
            LiveVramGpuAllocationGranularity::WholeContext,
        );
        let failed = evaluate_live_vram_proof_gate(
            &evidence,
            Some(&whole_context),
            Some(&roomy_deadlines),
            LiveVramProofGate::default(),
        )
        .unwrap();
        assert!(!failed.passed());
        assert!(failed.failures.iter().any(|failure| matches!(
            failure,
            LiveVramProofGateFailure::AllocationGranularityCannotReclaimPages { .. }
        )));
        assert!(
            failed
                .failures
                .contains(&LiveVramProofGateFailure::ReclaimableGpuBytesZero)
        );

        let whole_tensor = estimate_live_vram_residency_from_runtime_allocation(
            &evidence,
            raw_len * 4,
            raw_len * 2,
            LiveVramGpuAllocationGranularity::WholeTensor,
        )
        .unwrap();
        let coarse_runtime_gate = evaluate_live_vram_proof_gate(
            &evidence,
            Some(&whole_tensor),
            Some(&roomy_deadlines),
            LiveVramProofGate {
                min_gpu_saved_ratio: 0.25,
                require_page_granular_reclaim: false,
                ..LiveVramProofGate::default()
            },
        )
        .unwrap();
        assert!(coarse_runtime_gate.passed());

        let strict_page_gate = evaluate_live_vram_proof_gate(
            &evidence,
            Some(&whole_tensor),
            Some(&roomy_deadlines),
            LiveVramProofGate {
                min_gpu_saved_ratio: 0.25,
                ..LiveVramProofGate::default()
            },
        )
        .unwrap();
        assert!(!strict_page_gate.passed());
        assert!(strict_page_gate.failures.iter().any(|failure| matches!(
            failure,
            LiveVramProofGateFailure::AllocationGranularityCannotReclaimPages { .. }
        )));
    }

    #[test]
    fn live_paging_gate_rejects_trace_offload_for_resident_evidence_page() {
        let mut hot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4),
        );
        hot.descriptor.token_start = 0;
        hot.descriptor.token_end = 4;
        let mut cold = compression_positive_live_vram_snapshot();
        cold.descriptor.layer_id = 8;
        cold.descriptor.kind = KvPageKind::Value;
        cold.descriptor.token_start = 4;
        cold.descriptor.token_end = 260;
        cold.descriptor.next_required_token = Some(260);
        let hot_key = KvPageKey::from_descriptor(&hot.descriptor);
        let cold_key = KvPageKey::from_descriptor(&cold.descriptor);
        let hot_checksum = hot.descriptor.checksum;
        let cold_checksum = cold.descriptor.checksum;
        let evidence = build_live_vram_evidence_report(
            &[hot.clone(), cold.clone()],
            LiveVramSchedulerState {
                current_token: 0,
                queued_pages: 0,
                cpu_stored_bytes: 0,
            },
            LiveVramSchedulerPolicy {
                hot_window_tokens: 4,
                prefetch_window_tokens: 32,
                max_queued_pages: 16,
                max_cpu_stored_bytes: 8 * 1024 * 1024,
                require_qatq_beats_best_general_codec: true,
            },
            LiveVramLimits::default(),
        )
        .unwrap();
        assert_eq!(evidence.resident_pages, 1);
        assert_eq!(evidence.offloaded_pages, 1);

        let residency = estimate_live_vram_residency_after_offload(
            &evidence,
            hot.descriptor.raw_len + cold.descriptor.raw_len,
            LiveVramGpuAllocationGranularity::PerPage,
        );
        let deadlines = evaluate_live_vram_prefetch_deadlines(
            &evidence,
            0,
            LiveVramPrefetchBudget {
                restore_bytes_per_token: 1 << 20,
            },
        )
        .unwrap();
        let trace = vec![
            live_vram_event(
                0,
                &hot_key,
                LiveVramPageEventKind::Snapshot,
                Some(hot_checksum),
            ),
            live_vram_event(
                0,
                &cold_key,
                LiveVramPageEventKind::Snapshot,
                Some(cold_checksum),
            ),
            live_vram_event(
                1,
                &hot_key,
                LiveVramPageEventKind::OffloadCommitted,
                Some(hot_checksum),
            ),
            live_vram_event(
                1,
                &cold_key,
                LiveVramPageEventKind::OffloadCommitted,
                Some(cold_checksum),
            ),
            live_vram_event(
                2,
                &hot_key,
                LiveVramPageEventKind::RestoreCommitted,
                Some(hot_checksum),
            ),
            live_vram_event(
                2,
                &cold_key,
                LiveVramPageEventKind::RestoreCommitted,
                Some(cold_checksum),
            ),
            live_vram_event(3, &hot_key, LiveVramPageEventKind::AttentionUse, None),
            live_vram_event(3, &cold_key, LiveVramPageEventKind::AttentionUse, None),
        ];

        let proof = evaluate_live_vram_live_paging_proof_gate(
            &evidence,
            Some(&residency),
            Some(&deadlines),
            &trace,
            LiveVramProofGate {
                min_gpu_saved_ratio: 0.10,
                require_all_pages_beat_best_general_codec: false,
                ..LiveVramProofGate::default()
            },
            LiveVramEventTracePolicy::default(),
        )
        .unwrap();

        assert!(proof.proof_gate.passed());
        assert!(!proof.passed());
        assert!(proof.event_trace.failures.iter().any(|failure| matches!(
            failure,
            LiveVramEventTraceFailure::EvidenceResidentPageOffloadedInTrace { key }
                if **key == hot_key
        )));
    }

    #[test]
    fn live_vram_offload_store_commits_restores_and_removes_pages() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut store = LiveVramOffloadStore::new_with_shadow_validation(
            LiveVramLimits::default(),
            8,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
        );

        let key = store.commit_snapshot(&snapshot).unwrap();
        assert_eq!(store.len(), 1);
        assert!(store.contains_key(&key));
        assert_eq!(store.metrics().offloaded_pages, 1);
        assert_eq!(store.metrics().pending_pages, 1);
        assert!(store.metrics().cpu_stored_bytes < snapshot.bytes_le.len());

        let restored = store.restore(&key).unwrap();
        assert_eq!(restored, snapshot.bytes_le);

        let restored = store.restore_and_remove(&key).unwrap();
        assert_eq!(restored, snapshot.bytes_le);
        assert!(store.is_empty());
        assert_eq!(store.metrics().pending_pages, 0);
        assert_eq!(store.metrics().cpu_stored_bytes, 0);
    }

    #[test]
    fn live_vram_offload_store_rejects_duplicate_and_budget_exhaustion() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut store = LiveVramOffloadStore::new_with_shadow_validation(
            LiveVramLimits::default(),
            8,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
        );
        store.commit_snapshot(&snapshot).unwrap();
        assert_eq!(
            store.commit_snapshot(&snapshot),
            Err(QatqError::InvalidHeader)
        );
        assert_eq!(store.metrics().encode_failures, 1);

        let mut tight_store = LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 1);
        assert!(matches!(
            tight_store.commit_snapshot(&snapshot),
            Err(QatqError::ContainerLimitExceeded(
                "live-vram-offload-cpu-bytes"
            ))
        ));
        assert_eq!(tight_store.metrics().encode_failures, 1);
    }

    #[test]
    fn live_vram_offload_store_rejects_corrupt_payload_before_commit() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut encoded = try_encode_live_vram_page(&snapshot, LiveVramLimits::default()).unwrap();
        let last = encoded.bytes.last_mut().unwrap();
        *last ^= 0x01;

        let mut store = LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 8 * 1024 * 1024);
        assert!(store.commit_encoded(encoded).is_err());
        assert!(store.is_empty());
        assert_eq!(store.metrics().restore_failures, 1);
    }

    #[test]
    fn live_vram_offload_store_shadow_validation_accounts_and_cleans_up() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut store = LiveVramOffloadStore::new_with_shadow_validation(
            LiveVramLimits::default(),
            8,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
        );

        let key = store.commit_snapshot(&snapshot).unwrap();
        assert_eq!(store.metrics().shadow_cpu_bytes, snapshot.bytes_le.len());
        assert_eq!(
            store.entry(&key).unwrap().shadow_bytes.as_ref().unwrap(),
            &snapshot.bytes_le
        );

        let restored = store.restore_and_remove(&key).unwrap();
        assert_eq!(restored, snapshot.bytes_le);
        assert_eq!(store.metrics().shadow_cpu_bytes, 0);
        assert_eq!(store.metrics().cpu_stored_bytes, 0);
    }

    #[test]
    fn live_vram_offload_store_shadow_validation_rejects_budget_and_mismatch() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut tight_store = LiveVramOffloadStore::new_with_shadow_validation(
            LiveVramLimits::default(),
            8,
            8 * 1024 * 1024,
            1,
        );
        assert!(matches!(
            tight_store.commit_snapshot(&snapshot),
            Err(QatqError::ContainerLimitExceeded("live-vram-shadow-bytes"))
        ));

        let mut store = LiveVramOffloadStore::new_with_shadow_validation(
            LiveVramLimits::default(),
            8,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
        );
        let key = store.commit_snapshot(&snapshot).unwrap();
        let entry = store.entries.get_mut(&key).unwrap();
        entry.shadow_bytes.as_mut().unwrap()[0] ^= 0x01;

        assert!(matches!(
            store.restore(&key),
            Err(QatqError::ChecksumMismatch { .. })
        ));
        assert_eq!(store.metrics().checksum_failures, 1);
    }

    #[test]
    fn live_vram_controller_offloads_and_restores_through_adapter_in_order() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = TestLiveVramAdapter {
            snapshot: snapshot.clone(),
            committed: None,
            metrics: LiveVramAdapterMetrics {
                resident_pages: 1,
                current_gpu_bytes: snapshot.bytes_le.len(),
                peak_gpu_bytes: snapshot.bytes_le.len(),
                ..LiveVramAdapterMetrics::default()
            },
            fail_commit: false,
        };
        let mut store = LiveVramOffloadStore::new_with_shadow_validation(
            LiveVramLimits::default(),
            8,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
        );
        let scheduler = FixedWindowLiveVramScheduler {
            policy: LiveVramSchedulerPolicy {
                hot_window_tokens: 0,
                prefetch_window_tokens: 32,
                max_queued_pages: 8,
                max_cpu_stored_bytes: 8 * 1024 * 1024,
                require_qatq_beats_best_general_codec: true,
            },
        };

        let offload = try_offload_live_vram_page(
            &mut adapter,
            &mut store,
            &scheduler,
            &snapshot.descriptor,
            LiveVramSchedulerState {
                current_token: 0,
                queued_pages: 0,
                cpu_stored_bytes: 0,
            },
            LiveVramLimits::default(),
        )
        .unwrap();
        let LiveVramOffloadOutcome::Offloaded {
            key, stored_len, ..
        } = offload
        else {
            panic!("expected offload");
        };
        assert!(stored_len < snapshot.bytes_le.len());
        assert!(store.contains_key(&key));
        assert_eq!(store.metrics().shadow_cpu_bytes, snapshot.bytes_le.len());
        assert!(!adapter.is_page_resident(&snapshot.descriptor).unwrap());

        let restore = try_restore_live_vram_page_from_store(
            &mut adapter,
            &mut store,
            &key,
            LiveVramLimits::default(),
        )
        .unwrap();
        assert_eq!(
            restore,
            LiveVramRestoreOutcome::Restored {
                key,
                restored_len: snapshot.bytes_le.len()
            }
        );
        assert!(store.is_empty());
        assert_eq!(store.metrics().shadow_cpu_bytes, 0);
        assert!(adapter.is_page_resident(&snapshot.descriptor).unwrap());
    }

    #[test]
    fn live_vram_measured_offload_requires_gpu_byte_reclaim() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = TestLiveVramAdapter {
            snapshot: snapshot.clone(),
            committed: None,
            metrics: LiveVramAdapterMetrics {
                resident_pages: 1,
                current_gpu_bytes: snapshot.bytes_le.len(),
                peak_gpu_bytes: snapshot.bytes_le.len(),
                ..LiveVramAdapterMetrics::default()
            },
            fail_commit: false,
        };
        let mut store = LiveVramOffloadStore::new_with_shadow_validation(
            LiveVramLimits::default(),
            8,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
        );
        let scheduler = FixedWindowLiveVramScheduler {
            policy: LiveVramSchedulerPolicy {
                hot_window_tokens: 0,
                prefetch_window_tokens: 32,
                max_queued_pages: 8,
                max_cpu_stored_bytes: 8 * 1024 * 1024,
                require_qatq_beats_best_general_codec: true,
            },
        };

        let measured = try_offload_live_vram_page_with_reclaim_check(
            &mut adapter,
            &mut store,
            &scheduler,
            &snapshot.descriptor,
            LiveVramSchedulerState {
                current_token: 0,
                queued_pages: 0,
                cpu_stored_bytes: 0,
            },
            LiveVramLimits::default(),
            snapshot.bytes_le.len(),
        )
        .unwrap();
        let LiveVramMeasuredOffloadOutcome::Offloaded {
            key,
            gpu_bytes_before,
            gpu_bytes_after,
            reclaimed_gpu_bytes,
            ..
        } = measured
        else {
            panic!("expected measured offload");
        };
        assert_eq!(gpu_bytes_before, snapshot.bytes_le.len());
        assert_eq!(gpu_bytes_after, 0);
        assert_eq!(reclaimed_gpu_bytes, snapshot.bytes_le.len());
        assert!(store.contains_key(&key));
        assert!(!adapter.is_page_resident(&snapshot.descriptor).unwrap());

        try_restore_live_vram_page_from_store(
            &mut adapter,
            &mut store,
            &key,
            LiveVramLimits::default(),
        )
        .unwrap();
        assert!(store.is_empty());
        assert!(adapter.is_page_resident(&snapshot.descriptor).unwrap());
    }

    #[test]
    fn live_vram_measured_offload_rolls_back_when_reclaim_is_too_small() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = TestLiveVramAdapter {
            snapshot: snapshot.clone(),
            committed: None,
            metrics: LiveVramAdapterMetrics {
                resident_pages: 1,
                current_gpu_bytes: snapshot.bytes_le.len(),
                peak_gpu_bytes: snapshot.bytes_le.len(),
                ..LiveVramAdapterMetrics::default()
            },
            fail_commit: false,
        };
        let mut store = LiveVramOffloadStore::new_with_shadow_validation(
            LiveVramLimits::default(),
            8,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
        );
        let scheduler = FixedWindowLiveVramScheduler {
            policy: LiveVramSchedulerPolicy {
                hot_window_tokens: 0,
                prefetch_window_tokens: 32,
                max_queued_pages: 8,
                max_cpu_stored_bytes: 8 * 1024 * 1024,
                require_qatq_beats_best_general_codec: true,
            },
        };

        let result = try_offload_live_vram_page_with_reclaim_check(
            &mut adapter,
            &mut store,
            &scheduler,
            &snapshot.descriptor,
            LiveVramSchedulerState {
                current_token: 0,
                queued_pages: 0,
                cpu_stored_bytes: 0,
            },
            LiveVramLimits::default(),
            snapshot.bytes_le.len() + 1,
        );
        assert!(matches!(
            result,
            Err(LiveVramMeasuredOffloadError::InsufficientReclaim {
                reclaimed_gpu_bytes,
                ..
            }) if reclaimed_gpu_bytes == snapshot.bytes_le.len()
        ));
        assert!(store.is_empty());
        assert!(adapter.is_page_resident(&snapshot.descriptor).unwrap());
        assert_eq!(
            adapter.metrics().unwrap().current_gpu_bytes,
            snapshot.bytes_le.len()
        );
    }

    #[test]
    fn live_vram_measured_offload_rejects_zero_reclaim_floor() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = TestLiveVramAdapter {
            snapshot: snapshot.clone(),
            committed: None,
            metrics: LiveVramAdapterMetrics {
                resident_pages: 1,
                current_gpu_bytes: snapshot.bytes_le.len(),
                peak_gpu_bytes: snapshot.bytes_le.len(),
                ..LiveVramAdapterMetrics::default()
            },
            fail_commit: false,
        };
        let mut store = LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 8 * 1024 * 1024);
        let scheduler = FixedWindowLiveVramScheduler {
            policy: LiveVramSchedulerPolicy::default(),
        };

        assert_eq!(
            try_offload_live_vram_page_with_reclaim_check(
                &mut adapter,
                &mut store,
                &scheduler,
                &snapshot.descriptor,
                LiveVramSchedulerState {
                    current_token: 0,
                    queued_pages: 0,
                    cpu_stored_bytes: 0,
                },
                LiveVramLimits::default(),
                0,
            ),
            Err(LiveVramMeasuredOffloadError::InvalidReclaimPolicy)
        );
        assert!(store.is_empty());
        assert!(adapter.is_page_resident(&snapshot.descriptor).unwrap());
    }

    #[test]
    fn live_vram_controller_keeps_hot_pages_resident_without_snapshot() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(16),
        );
        let mut adapter = TestLiveVramAdapter {
            snapshot: snapshot.clone(),
            committed: None,
            metrics: LiveVramAdapterMetrics {
                resident_pages: 1,
                current_gpu_bytes: snapshot.bytes_le.len(),
                peak_gpu_bytes: snapshot.bytes_le.len(),
                ..LiveVramAdapterMetrics::default()
            },
            fail_commit: false,
        };
        let mut store = LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 8 * 1024 * 1024);
        let scheduler = FixedWindowLiveVramScheduler {
            policy: LiveVramSchedulerPolicy {
                hot_window_tokens: 128,
                prefetch_window_tokens: 32,
                max_queued_pages: 8,
                max_cpu_stored_bytes: 8 * 1024 * 1024,
                require_qatq_beats_best_general_codec: true,
            },
        };

        let outcome = try_offload_live_vram_page(
            &mut adapter,
            &mut store,
            &scheduler,
            &snapshot.descriptor,
            LiveVramSchedulerState {
                current_token: 0,
                queued_pages: 0,
                cpu_stored_bytes: 0,
            },
            LiveVramLimits::default(),
        )
        .unwrap();
        assert_eq!(
            outcome,
            LiveVramOffloadOutcome::KeptResident(LiveVramKeepReason::InsideHotWindow)
        );
        assert!(store.is_empty());
        assert!(adapter.is_page_resident(&snapshot.descriptor).unwrap());
    }

    #[test]
    fn live_vram_controller_drops_pending_store_entry_when_runtime_commit_fails() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = TestLiveVramAdapter {
            snapshot: snapshot.clone(),
            committed: None,
            metrics: LiveVramAdapterMetrics {
                resident_pages: 1,
                current_gpu_bytes: snapshot.bytes_le.len(),
                peak_gpu_bytes: snapshot.bytes_le.len(),
                ..LiveVramAdapterMetrics::default()
            },
            fail_commit: true,
        };
        let mut store = LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 8 * 1024 * 1024);
        let scheduler = FixedWindowLiveVramScheduler {
            policy: LiveVramSchedulerPolicy {
                hot_window_tokens: 0,
                prefetch_window_tokens: 32,
                max_queued_pages: 8,
                max_cpu_stored_bytes: 8 * 1024 * 1024,
                require_qatq_beats_best_general_codec: true,
            },
        };

        let result = try_offload_live_vram_page(
            &mut adapter,
            &mut store,
            &scheduler,
            &snapshot.descriptor,
            LiveVramSchedulerState {
                current_token: 0,
                queued_pages: 0,
                cpu_stored_bytes: 0,
            },
            LiveVramLimits::default(),
        );
        assert!(matches!(
            result,
            Err(LiveVramOffloadError::RuntimeCommit(
                LiveVramAdapterError::CommitFailed("forced commit failure")
            ))
        ));
        assert!(store.is_empty());
        let metrics = store.metrics();
        assert_eq!(metrics.pending_pages, 0);
        assert_eq!(metrics.cpu_stored_bytes, 0);
        assert_eq!(metrics.shadow_cpu_bytes, 0);
        assert!(adapter.is_page_resident(&snapshot.descriptor).unwrap());
    }

    #[test]
    fn live_vram_controller_records_restore_stalls_from_observed_latency() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = TestLiveVramAdapter {
            snapshot: snapshot.clone(),
            committed: None,
            metrics: LiveVramAdapterMetrics {
                resident_pages: 1,
                current_gpu_bytes: snapshot.bytes_le.len(),
                peak_gpu_bytes: snapshot.bytes_le.len(),
                ..LiveVramAdapterMetrics::default()
            },
            fail_commit: false,
        };
        let mut store = LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 8 * 1024 * 1024);
        let key = store.commit_snapshot(&snapshot).unwrap();
        let entry = store.entry(&key).unwrap().clone();
        adapter
            .commit_offload(&LiveVramPageEncodeResult {
                metadata: entry.metadata.clone(),
                bytes: entry.bytes.clone(),
            })
            .unwrap();

        let outcome = try_restore_live_vram_page_from_store_with_observed_latency(
            &mut adapter,
            &mut store,
            &key,
            LiveVramLimits::default(),
            2_000,
            LiveVramRestoreLatencyBudget {
                max_restore_ns_per_page: 1_000,
            },
        )
        .unwrap();

        assert_eq!(outcome.restored_len, snapshot.bytes_le.len());
        assert_eq!(outcome.observed_restore_ns, 2_000);
        assert!(outcome.stalled);
        assert_eq!(store.metrics().restore_stalls, 1);
        assert_eq!(store.metrics().restore_stall_ns_total, 2_000);
        assert!(store.is_empty());
    }

    #[test]
    fn live_vram_controller_does_not_record_restore_stall_inside_budget() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = TestLiveVramAdapter {
            snapshot: snapshot.clone(),
            committed: None,
            metrics: LiveVramAdapterMetrics {
                resident_pages: 1,
                current_gpu_bytes: snapshot.bytes_le.len(),
                peak_gpu_bytes: snapshot.bytes_le.len(),
                ..LiveVramAdapterMetrics::default()
            },
            fail_commit: false,
        };
        let mut store = LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 8 * 1024 * 1024);
        let key = store.commit_snapshot(&snapshot).unwrap();
        let entry = store.entry(&key).unwrap().clone();
        adapter
            .commit_offload(&LiveVramPageEncodeResult {
                metadata: entry.metadata.clone(),
                bytes: entry.bytes.clone(),
            })
            .unwrap();

        let outcome = try_restore_live_vram_page_from_store_with_observed_latency(
            &mut adapter,
            &mut store,
            &key,
            LiveVramLimits::default(),
            500,
            LiveVramRestoreLatencyBudget {
                max_restore_ns_per_page: 1_000,
            },
        )
        .unwrap();

        assert!(!outcome.stalled);
        assert_eq!(store.metrics().restore_stalls, 0);
        assert_eq!(store.metrics().restore_stall_ns_total, 0);
    }

    #[test]
    fn live_vram_controller_rejects_zero_restore_latency_budget() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = TestLiveVramAdapter {
            snapshot: snapshot.clone(),
            committed: None,
            metrics: LiveVramAdapterMetrics {
                resident_pages: 1,
                current_gpu_bytes: snapshot.bytes_le.len(),
                peak_gpu_bytes: snapshot.bytes_le.len(),
                ..LiveVramAdapterMetrics::default()
            },
            fail_commit: false,
        };
        let mut store = LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 8 * 1024 * 1024);
        let key = store.commit_snapshot(&snapshot).unwrap();

        assert_eq!(
            try_restore_live_vram_page_from_store_with_observed_latency(
                &mut adapter,
                &mut store,
                &key,
                LiveVramLimits::default(),
                1,
                LiveVramRestoreLatencyBudget {
                    max_restore_ns_per_page: 0,
                },
            ),
            Err(LiveVramRestoreError::Codec(QatqError::InvalidHeader))
        );
        assert!(store.contains_key(&key));
    }

    #[test]
    fn live_vram_cancellation_before_runtime_commit_drops_store_only() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = TestLiveVramAdapter {
            snapshot: snapshot.clone(),
            committed: None,
            metrics: LiveVramAdapterMetrics {
                resident_pages: 1,
                current_gpu_bytes: snapshot.bytes_le.len(),
                peak_gpu_bytes: snapshot.bytes_le.len(),
                ..LiveVramAdapterMetrics::default()
            },
            fail_commit: false,
        };
        let mut store = LiveVramOffloadStore::new_with_shadow_validation(
            LiveVramLimits::default(),
            8,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
        );
        let key = store.commit_snapshot(&snapshot).unwrap();

        let outcome = cancel_live_vram_offload(
            &mut adapter,
            &mut store,
            &key,
            LiveVramCancellationStage::BeforeRuntimeCommit,
            LiveVramLimits::default(),
        )
        .unwrap();

        assert_eq!(
            outcome,
            LiveVramCancellationOutcome::DroppedUncommitted { key }
        );
        assert!(store.is_empty());
        assert!(adapter.is_page_resident(&snapshot.descriptor).unwrap());
    }

    #[test]
    fn live_vram_cancellation_after_runtime_commit_restores_before_cleanup() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = TestLiveVramAdapter {
            snapshot: snapshot.clone(),
            committed: None,
            metrics: LiveVramAdapterMetrics {
                resident_pages: 1,
                current_gpu_bytes: snapshot.bytes_le.len(),
                peak_gpu_bytes: snapshot.bytes_le.len(),
                ..LiveVramAdapterMetrics::default()
            },
            fail_commit: false,
        };
        let mut store = LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 8 * 1024 * 1024);
        let key = store.commit_snapshot(&snapshot).unwrap();
        let entry = store.entry(&key).unwrap().clone();
        adapter
            .commit_offload(&LiveVramPageEncodeResult {
                metadata: entry.metadata.clone(),
                bytes: entry.bytes.clone(),
            })
            .unwrap();
        assert!(!adapter.is_page_resident(&snapshot.descriptor).unwrap());

        let outcome = cancel_live_vram_offload(
            &mut adapter,
            &mut store,
            &key,
            LiveVramCancellationStage::AfterRuntimeCommit,
            LiveVramLimits::default(),
        )
        .unwrap();

        assert_eq!(
            outcome,
            LiveVramCancellationOutcome::RestoredCommitted {
                key,
                restored_len: snapshot.bytes_le.len()
            }
        );
        assert!(store.is_empty());
        assert!(adapter.is_page_resident(&snapshot.descriptor).unwrap());
    }

    #[test]
    fn live_vram_restore_keeps_store_entry_when_runtime_does_not_report_resident() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = DishonestRestoreAdapter {
            snapshot: snapshot.clone(),
            resident: true,
            mode: DishonestRestoreMode::ReturnsRestoredButNotResident,
        };
        let mut store = LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 8 * 1024 * 1024);
        let key = store.commit_snapshot(&snapshot).unwrap();
        let entry = store.entry(&key).unwrap().clone();
        adapter
            .commit_offload(&LiveVramPageEncodeResult {
                metadata: entry.metadata.clone(),
                bytes: entry.bytes.clone(),
            })
            .unwrap();
        assert!(!adapter.is_page_resident(&snapshot.descriptor).unwrap());

        let result = try_restore_live_vram_page_from_store(
            &mut adapter,
            &mut store,
            &key,
            LiveVramLimits::default(),
        );

        assert_eq!(result, Err(LiveVramRestoreError::RuntimePageNotResident));
        assert!(store.contains_key(&key));
        assert_eq!(store.metrics().pending_pages, 1);
        assert_eq!(store.metrics().restore_failures, 1);
        assert!(!adapter.is_page_resident(&snapshot.descriptor).unwrap());
    }

    #[test]
    fn live_vram_restore_uses_sealed_adapter_boundary_when_store_is_sealed() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = SealedOnlyRestoreAdapter {
            snapshot: snapshot.clone(),
            resident: true,
            raw_restore_calls: 0,
            sealed_restore_calls: 0,
        };
        let mut store = LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 8 * 1024 * 1024)
            .with_page_seal_policy(live_vram_test_seal_policy());
        let key = store.commit_snapshot(&snapshot).unwrap();
        let entry = store.entry(&key).unwrap().clone();
        adapter
            .commit_offload(&LiveVramPageEncodeResult {
                metadata: entry.metadata.clone(),
                bytes: entry.bytes.clone(),
            })
            .unwrap();

        let result = try_restore_live_vram_page_from_store(
            &mut adapter,
            &mut store,
            &key,
            LiveVramLimits::default(),
        );

        assert_eq!(
            result,
            Ok(LiveVramRestoreOutcome::Restored {
                key: key.clone(),
                restored_len: snapshot.bytes_le.len()
            })
        );
        assert_eq!(adapter.raw_restore_calls, 0);
        assert_eq!(adapter.sealed_restore_calls, 1);
        assert!(adapter.is_page_resident(&snapshot.descriptor).unwrap());
        assert!(!store.contains_key(&key));
    }

    #[test]
    fn live_vram_restore_without_store_seals_uses_legacy_raw_boundary() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = SealedOnlyRestoreAdapter {
            snapshot: snapshot.clone(),
            resident: true,
            raw_restore_calls: 0,
            sealed_restore_calls: 0,
        };
        let mut store = LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 8 * 1024 * 1024);
        let key = store.commit_snapshot(&snapshot).unwrap();
        let entry = store.entry(&key).unwrap().clone();
        adapter
            .commit_offload(&LiveVramPageEncodeResult {
                metadata: entry.metadata.clone(),
                bytes: entry.bytes.clone(),
            })
            .unwrap();

        let result = try_restore_live_vram_page_from_store(
            &mut adapter,
            &mut store,
            &key,
            LiveVramLimits::default(),
        );

        assert_eq!(
            result,
            Err(LiveVramRestoreError::Runtime(
                LiveVramAdapterError::RestoreFailed("raw restore boundary forbidden")
            ))
        );
        assert_eq!(adapter.raw_restore_calls, 1);
        assert_eq!(adapter.sealed_restore_calls, 0);
        assert!(store.contains_key(&key));
        assert!(!adapter.is_page_resident(&snapshot.descriptor).unwrap());
    }

    #[test]
    fn live_vram_sealed_store_rejects_missing_seal_before_runtime_restore() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = TestLiveVramAdapter {
            snapshot: snapshot.clone(),
            committed: None,
            metrics: LiveVramAdapterMetrics {
                resident_pages: 1,
                current_gpu_bytes: snapshot.bytes_le.len(),
                peak_gpu_bytes: snapshot.bytes_le.len(),
                ..LiveVramAdapterMetrics::default()
            },
            fail_commit: false,
        };
        let mut store = LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 8 * 1024 * 1024)
            .with_page_seal_policy(live_vram_test_seal_policy());
        let key = store.commit_snapshot(&snapshot).unwrap();
        let entry = store.entry(&key).unwrap().clone();
        assert!(entry.metadata_seal.is_some());
        adapter
            .commit_offload(&LiveVramPageEncodeResult {
                metadata: entry.metadata.clone(),
                bytes: entry.bytes.clone(),
            })
            .unwrap();
        store
            .entries
            .get_mut(&key)
            .expect("sealed entry")
            .metadata_seal = None;

        let result = try_restore_live_vram_page_from_store(
            &mut adapter,
            &mut store,
            &key,
            LiveVramLimits::default(),
        );

        assert_eq!(
            result,
            Err(LiveVramRestoreError::Codec(QatqError::MetadataSealMismatch))
        );
        assert!(store.contains_key(&key));
        assert_eq!(store.metrics().restore_failures, 1);
        assert!(!adapter.is_page_resident(&snapshot.descriptor).unwrap());
    }

    #[test]
    fn live_vram_sealed_store_rejects_payload_tampering_before_runtime_restore() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = TestLiveVramAdapter {
            snapshot: snapshot.clone(),
            committed: None,
            metrics: LiveVramAdapterMetrics {
                resident_pages: 1,
                current_gpu_bytes: snapshot.bytes_le.len(),
                peak_gpu_bytes: snapshot.bytes_le.len(),
                ..LiveVramAdapterMetrics::default()
            },
            fail_commit: false,
        };
        let mut store = LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 8 * 1024 * 1024)
            .with_page_seal_policy(live_vram_test_seal_policy());
        let key = store.commit_snapshot(&snapshot).unwrap();
        let entry = store.entry(&key).unwrap().clone();
        adapter
            .commit_offload(&LiveVramPageEncodeResult {
                metadata: entry.metadata.clone(),
                bytes: entry.bytes.clone(),
            })
            .unwrap();
        store.entries.get_mut(&key).expect("sealed entry").bytes[0] ^= 0x01;

        let result = try_restore_live_vram_page_from_store(
            &mut adapter,
            &mut store,
            &key,
            LiveVramLimits::default(),
        );

        assert_eq!(
            result,
            Err(LiveVramRestoreError::Codec(QatqError::MetadataSealMismatch))
        );
        assert!(store.contains_key(&key));
        assert_eq!(store.metrics().restore_failures, 1);
        assert!(!adapter.is_page_resident(&snapshot.descriptor).unwrap());
    }

    #[test]
    fn live_vram_sealed_store_rejects_wrong_policy_before_runtime_restore() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = TestLiveVramAdapter {
            snapshot: snapshot.clone(),
            committed: None,
            metrics: LiveVramAdapterMetrics {
                resident_pages: 1,
                current_gpu_bytes: snapshot.bytes_le.len(),
                peak_gpu_bytes: snapshot.bytes_le.len(),
                ..LiveVramAdapterMetrics::default()
            },
            fail_commit: false,
        };
        let mut store = LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 8 * 1024 * 1024)
            .with_page_seal_policy(live_vram_test_seal_policy());
        let key = store.commit_snapshot(&snapshot).unwrap();
        let entry = store.entry(&key).unwrap().clone();
        adapter
            .commit_offload(&LiveVramPageEncodeResult {
                metadata: entry.metadata.clone(),
                bytes: entry.bytes.clone(),
            })
            .unwrap();
        store.page_seal_policy =
            Some(LiveVramPageSealPolicy::new([9_u8; 32], b"wrong-context".to_vec()).unwrap());

        let result = try_restore_live_vram_page_from_store(
            &mut adapter,
            &mut store,
            &key,
            LiveVramLimits::default(),
        );

        assert_eq!(
            result,
            Err(LiveVramRestoreError::Codec(QatqError::MetadataSealMismatch))
        );
        assert!(store.contains_key(&key));
        assert_eq!(store.metrics().restore_failures, 1);
        assert!(!adapter.is_page_resident(&snapshot.descriptor).unwrap());
    }

    #[test]
    fn live_vram_restore_keeps_store_entry_when_runtime_restore_fails() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = DishonestRestoreAdapter {
            snapshot: snapshot.clone(),
            resident: true,
            mode: DishonestRestoreMode::RuntimeRestoreFails,
        };
        let mut store = LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 8 * 1024 * 1024);
        let key = store.commit_snapshot(&snapshot).unwrap();
        let entry = store.entry(&key).unwrap().clone();
        adapter
            .commit_offload(&LiveVramPageEncodeResult {
                metadata: entry.metadata.clone(),
                bytes: entry.bytes.clone(),
            })
            .unwrap();

        let result = try_restore_live_vram_page_from_store(
            &mut adapter,
            &mut store,
            &key,
            LiveVramLimits::default(),
        );

        assert_eq!(
            result,
            Err(LiveVramRestoreError::Runtime(
                LiveVramAdapterError::RestoreFailed("forced restore allocation failure")
            ))
        );
        assert!(store.contains_key(&key));
        assert_eq!(store.metrics().pending_pages, 1);
        assert_eq!(store.metrics().restore_failures, 1);
        assert!(!adapter.is_page_resident(&snapshot.descriptor).unwrap());
    }

    #[test]
    fn live_vram_restore_keeps_store_entry_when_runtime_rejects_resource_limit() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = DishonestRestoreAdapter {
            snapshot: snapshot.clone(),
            resident: true,
            mode: DishonestRestoreMode::ReturnsResourceLimitRejected,
        };
        let mut store = LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 8 * 1024 * 1024);
        let key = store.commit_snapshot(&snapshot).unwrap();
        let entry = store.entry(&key).unwrap().clone();
        adapter
            .commit_offload(&LiveVramPageEncodeResult {
                metadata: entry.metadata.clone(),
                bytes: entry.bytes.clone(),
            })
            .unwrap();

        let result = try_restore_live_vram_page_from_store(
            &mut adapter,
            &mut store,
            &key,
            LiveVramLimits::default(),
        );

        assert_eq!(
            result,
            Err(LiveVramRestoreError::RuntimeStatus(
                LiveVramRestoreStatus::ResourceLimitRejected
            ))
        );
        assert!(store.contains_key(&key));
        assert_eq!(store.metrics().pending_pages, 1);
        assert_eq!(store.metrics().restore_failures, 1);
        assert!(!adapter.is_page_resident(&snapshot.descriptor).unwrap());
    }

    #[test]
    fn live_vram_restore_keeps_store_entry_when_residency_query_fails() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = DishonestRestoreAdapter {
            snapshot: snapshot.clone(),
            resident: true,
            mode: DishonestRestoreMode::ResidencyQueryFailsAfterRestore,
        };
        let mut store = LiveVramOffloadStore::new(LiveVramLimits::default(), 8, 8 * 1024 * 1024);
        let key = store.commit_snapshot(&snapshot).unwrap();
        let entry = store.entry(&key).unwrap().clone();
        adapter
            .commit_offload(&LiveVramPageEncodeResult {
                metadata: entry.metadata.clone(),
                bytes: entry.bytes.clone(),
            })
            .unwrap();

        let result = try_restore_live_vram_page_from_store(
            &mut adapter,
            &mut store,
            &key,
            LiveVramLimits::default(),
        );

        assert!(matches!(
            result,
            Err(LiveVramRestoreError::RuntimeResidency(
                LiveVramAdapterError::ResidencyQueryFailed("forced residency query failure")
            ))
        ));
        assert!(store.contains_key(&key));
        assert_eq!(store.metrics().pending_pages, 1);
        assert_eq!(store.metrics().restore_failures, 1);
    }

    #[test]
    fn live_vram_lifecycle_sequence_fails_closed_across_duplicate_cancel_restore_paths() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut adapter = TestLiveVramAdapter {
            snapshot: snapshot.clone(),
            committed: None,
            metrics: LiveVramAdapterMetrics {
                resident_pages: 1,
                current_gpu_bytes: snapshot.bytes_le.len(),
                peak_gpu_bytes: snapshot.bytes_le.len(),
                ..LiveVramAdapterMetrics::default()
            },
            fail_commit: false,
        };
        let mut store = LiveVramOffloadStore::new_with_shadow_validation(
            LiveVramLimits::default(),
            8,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
        );
        let scheduler = FixedWindowLiveVramScheduler {
            policy: LiveVramSchedulerPolicy {
                hot_window_tokens: 0,
                prefetch_window_tokens: 32,
                max_queued_pages: 8,
                max_cpu_stored_bytes: 8 * 1024 * 1024,
                require_qatq_beats_best_general_codec: true,
            },
        };

        let prepared_key = store.commit_snapshot(&snapshot).unwrap();
        let cancelled = cancel_live_vram_offload(
            &mut adapter,
            &mut store,
            &prepared_key,
            LiveVramCancellationStage::BeforeRuntimeCommit,
            LiveVramLimits::default(),
        )
        .unwrap();
        assert_eq!(
            cancelled,
            LiveVramCancellationOutcome::DroppedUncommitted {
                key: prepared_key.clone()
            }
        );
        assert!(store.is_empty());
        assert!(adapter.is_page_resident(&snapshot.descriptor).unwrap());

        let offloaded = try_offload_live_vram_page(
            &mut adapter,
            &mut store,
            &scheduler,
            &snapshot.descriptor,
            LiveVramSchedulerState {
                current_token: 0,
                queued_pages: 0,
                cpu_stored_bytes: 0,
            },
            LiveVramLimits::default(),
        )
        .unwrap();
        let LiveVramOffloadOutcome::Offloaded { key, .. } = offloaded else {
            panic!("expected offload");
        };
        assert!(!adapter.is_page_resident(&snapshot.descriptor).unwrap());
        assert!(store.contains_key(&key));

        let duplicate_state = LiveVramSchedulerState {
            current_token: 0,
            queued_pages: store.len(),
            cpu_stored_bytes: store.metrics().cpu_stored_bytes,
        };
        let duplicate = try_offload_live_vram_page(
            &mut adapter,
            &mut store,
            &scheduler,
            &snapshot.descriptor,
            duplicate_state,
            LiveVramLimits::default(),
        );
        assert!(matches!(
            duplicate,
            Err(LiveVramOffloadError::Codec(QatqError::InvalidHeader))
        ));
        assert!(store.contains_key(&key));
        assert!(!adapter.is_page_resident(&snapshot.descriptor).unwrap());

        let restored = try_restore_live_vram_page_from_store_with_observed_latency(
            &mut adapter,
            &mut store,
            &key,
            LiveVramLimits::default(),
            2_000,
            LiveVramRestoreLatencyBudget {
                max_restore_ns_per_page: 1_000,
            },
        )
        .unwrap();
        assert_eq!(restored.key, key);
        assert!(restored.stalled);
        assert!(store.is_empty());
        assert_eq!(store.metrics().restore_stalls, 1);
        assert!(adapter.is_page_resident(&snapshot.descriptor).unwrap());

        let mut corrupt = try_encode_live_vram_page(&snapshot, LiveVramLimits::default()).unwrap();
        corrupt.bytes[0] ^= 0xff;
        let before_len = store.len();
        assert!(store.commit_encoded(corrupt).is_err());
        assert_eq!(store.len(), before_len);
        assert!(adapter.is_page_resident(&snapshot.descriptor).unwrap());
    }

    #[test]
    fn live_vram_streaming_attention_matches_materialized_reference_with_page_bound() {
        let query = [0.25_f32, -0.5, 0.75, 0.125];
        let key_page_a = [0.5_f32, 0.0, -0.25, 1.0, -0.5, 0.25, 0.75, -1.0];
        let key_page_b = [
            1.0_f32, 0.5, -0.5, 0.25, -0.25, -0.75, 0.125, 0.5, 0.0, 0.25, 0.5, -0.125,
        ];
        let value_page_a = [1.0_f32, 0.0, -1.0, 0.5, 0.25, 0.75];
        let value_page_b = [0.0_f32, 1.0, 0.5, -0.25, 1.25, -0.75, 0.5, -0.5, 1.5];

        let report = compare_live_vram_streaming_attention_reference(
            &query,
            &[&key_page_a, &key_page_b],
            &[&value_page_a, &value_page_b],
            3,
            1.0e-6,
        )
        .unwrap();

        assert!(report.passed);
        assert!(report.max_abs_error <= 1.0e-6);
        assert_eq!(report.streaming.pages, 2);
        assert_eq!(report.streaming.tokens, 5);
        assert_eq!(report.streaming.head_dim, 4);
        assert_eq!(report.streaming.value_dim, 3);
        assert_eq!(report.streaming.peak_page_kv_values, 21);
        assert_eq!(report.streaming.materialized_kv_values, 35);
        assert!(report.streaming.peak_kv_value_ratio().unwrap() < 1.0);
    }

    #[test]
    fn live_vram_streaming_attention_rejects_bad_pages_and_non_finite_values() {
        assert_eq!(
            live_vram_streaming_attention_reference(&[1.0], &[&[1.0, 2.0]], &[&[1.0]], 1),
            Err(QatqError::InvalidHeader)
        );
        assert_eq!(
            live_vram_streaming_attention_reference(&[f32::NAN], &[&[1.0]], &[&[1.0]], 1),
            Err(QatqError::InvalidHeader)
        );
        assert_eq!(
            live_vram_streaming_attention_reference(&[1.0], &[&[1.0]], &[&[f32::INFINITY]], 1),
            Err(QatqError::InvalidHeader)
        );
    }

    #[test]
    fn live_vram_streaming_attention_is_split_invariant() {
        let query = [0.5_f32, -0.25, 0.75, 0.125];
        let keys_all = [
            0.5_f32, 0.0, -0.25, 1.0, -0.5, 0.25, 0.75, -1.0, 1.0, 0.5, -0.5, 0.25, -0.25, -0.75,
            0.125, 0.5,
        ];
        let values_all = [
            1.0_f32, 0.0, -1.0, 0.5, 0.25, 0.75, 0.0, 1.0, 0.5, -0.25, 1.25, -0.75,
        ];

        let one_page = compare_live_vram_streaming_attention_reference(
            &query,
            &[&keys_all],
            &[&values_all],
            3,
            1.0e-6,
        )
        .unwrap();
        let split = compare_live_vram_streaming_attention_reference(
            &query,
            &[&keys_all[..8], &keys_all[8..]],
            &[&values_all[..6], &values_all[6..]],
            3,
            1.0e-6,
        )
        .unwrap();

        assert!(one_page.passed);
        assert!(split.passed);
        assert_eq!(one_page.streaming.output, split.streaming.output);
        assert!(split.streaming.peak_page_kv_values < one_page.streaming.peak_page_kv_values);
        assert_eq!(
            split.streaming.materialized_kv_values,
            one_page.streaming.materialized_kv_values
        );

        let summary = compare_live_vram_segment_summary_attention_reference(
            &query,
            &[&keys_all[..8], &keys_all[8..]],
            &[&values_all[..6], &values_all[6..]],
            3,
            1.0e-6,
        )
        .unwrap();
        assert!(summary.passed);
        assert_eq!(summary.streaming.output, split.streaming.output);
        assert_eq!(
            summary.streaming.peak_page_kv_values,
            split.streaming.peak_page_kv_values
        );
    }

    #[test]
    fn live_vram_streaming_attention_handles_large_score_separation() {
        let query = [64.0_f32, -64.0];
        let key_page_a = [64.0_f32, -64.0];
        let key_page_b = [-64.0_f32, 64.0];
        let value_page_a = [3.0_f32, -2.0];
        let value_page_b = [-1000.0_f32, 1000.0];

        let report = compare_live_vram_streaming_attention_reference(
            &query,
            &[&key_page_a, &key_page_b],
            &[&value_page_a, &value_page_b],
            2,
            1.0e-6,
        )
        .unwrap();

        assert!(report.passed);
        assert_eq!(report.streaming.output, vec![3.0, -2.0]);
        assert_eq!(report.max_abs_error, 0.0);

        let summary = compare_live_vram_segment_summary_attention_reference(
            &query,
            &[&key_page_a, &key_page_b],
            &[&value_page_a, &value_page_b],
            2,
            0.0,
        )
        .unwrap();
        assert!(summary.passed);
        assert_eq!(summary.streaming.output, vec![3.0, -2.0]);
        assert_eq!(summary.max_abs_error, 0.0);
    }

    #[test]
    fn live_vram_typed_streaming_attention_accepts_f16_pages() {
        let query = [0.5_f32, -0.25];
        let key_page_a = f16_page(&[0.5, 0.0, -0.25, 1.0]);
        let key_page_b = f16_page(&[1.0, 0.5]);
        let value_page_a = f16_page(&[1.0, 0.0, 0.5, 0.25]);
        let value_page_b = f16_page(&[-0.5, 1.5]);

        let report = compare_live_vram_typed_streaming_attention_reference(
            &query,
            &[&key_page_a, &key_page_b],
            &[&value_page_a, &value_page_b],
            TensorDType::F16,
            2,
            2,
            1.0e-6,
        )
        .unwrap();
        let f32_report = compare_live_vram_streaming_attention_reference(
            &query,
            &[&[0.5, 0.0, -0.25, 1.0], &[1.0, 0.5]],
            &[&[1.0, 0.0, 0.5, 0.25], &[-0.5, 1.5]],
            2,
            1.0e-6,
        )
        .unwrap();

        assert!(report.passed);
        assert_eq!(report.streaming.output, f32_report.streaming.output);
        assert_eq!(report.streaming.peak_page_kv_values, 8);
        assert_eq!(report.streaming.materialized_kv_values, 12);
    }

    #[test]
    fn live_vram_typed_streaming_attention_accepts_bf16_pages() {
        let query = [0.5_f32, -0.25];
        let key_page = bf16_page(&[0.5, 0.0, -0.25, 1.0]);
        let value_page = bf16_page(&[1.0, 0.0, 0.5, 0.25]);

        let report = compare_live_vram_typed_streaming_attention_reference(
            &query,
            &[&key_page],
            &[&value_page],
            TensorDType::BF16,
            2,
            2,
            1.0e-6,
        )
        .unwrap();

        assert!(report.passed);
        assert_eq!(report.streaming.tokens, 2);
        assert_eq!(report.streaming.peak_page_kv_values, 8);
    }

    #[test]
    fn live_vram_typed_streaming_attention_rejects_malformed_or_non_finite_pages() {
        let malformed = vec![0_u8; 3];
        assert_eq!(
            compare_live_vram_typed_streaming_attention_reference(
                &[1.0],
                &[&malformed],
                &[&f16_page(&[1.0])],
                TensorDType::F16,
                1,
                1,
                1.0e-6,
            ),
            Err(QatqError::InvalidHeader)
        );

        assert_eq!(
            compare_live_vram_typed_streaming_attention_reference(
                &[1.0],
                &[&f16_page(&[f32::INFINITY])],
                &[&f16_page(&[1.0])],
                TensorDType::F16,
                1,
                1,
                1.0e-6,
            ),
            Err(QatqError::InvalidHeader)
        );
    }

    #[test]
    fn live_vram_event_trace_rejects_attention_before_restore() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let key = KvPageKey::from_descriptor(&snapshot.descriptor);
        let checksum = snapshot.descriptor.checksum;
        let trace = vec![
            live_vram_event(0, &key, LiveVramPageEventKind::Snapshot, Some(checksum)),
            live_vram_event(
                64,
                &key,
                LiveVramPageEventKind::OffloadCommitted,
                Some(checksum),
            ),
            live_vram_event(128, &key, LiveVramPageEventKind::AttentionUse, None),
            live_vram_event(
                129,
                &key,
                LiveVramPageEventKind::RestoreCommitted,
                Some(checksum),
            ),
        ];

        let report = evaluate_live_vram_event_trace(&trace, LiveVramEventTracePolicy::default());

        assert!(!report.passed());
        assert_eq!(report.events, 4);
        assert_eq!(report.peak_offloaded_pages, 1);
        assert!(report.failures.iter().any(|failure| matches!(
            failure,
            LiveVramEventTraceFailure::AttentionUseWhileOffloaded { token: 128, .. }
        )));
    }

    #[test]
    fn live_vram_event_trace_accepts_restore_before_attention_and_rejects_tampering() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let key = KvPageKey::from_descriptor(&snapshot.descriptor);
        let checksum = snapshot.descriptor.checksum;
        let valid_trace = vec![
            live_vram_event(0, &key, LiveVramPageEventKind::Snapshot, Some(checksum)),
            live_vram_event(
                64,
                &key,
                LiveVramPageEventKind::OffloadCommitted,
                Some(checksum),
            ),
            live_vram_event(
                120,
                &key,
                LiveVramPageEventKind::RestoreCommitted,
                Some(checksum),
            ),
            live_vram_event(128, &key, LiveVramPageEventKind::AttentionUse, None),
        ];

        let valid =
            evaluate_live_vram_event_trace(&valid_trace, LiveVramEventTracePolicy::default());
        assert!(valid.passed());
        assert_eq!(valid.offloads, 1);
        assert_eq!(valid.restores, 1);
        assert_eq!(valid.attention_uses, 1);
        assert_eq!(valid.offloaded_pages_at_end, 0);

        let mut tampered_trace = valid_trace;
        tampered_trace[2].checksum = Some(checksum ^ 1);
        let tampered =
            evaluate_live_vram_event_trace(&tampered_trace, LiveVramEventTracePolicy::default());

        assert!(!tampered.passed());
        assert!(tampered.failures.iter().any(|failure| matches!(
            failure,
            LiveVramEventTraceFailure::RestoreChecksumMismatch { .. }
        )));
    }

    #[test]
    fn live_vram_event_trace_handles_explicit_cancellation_stages() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let key = KvPageKey::from_descriptor(&snapshot.descriptor);
        let checksum = snapshot.descriptor.checksum;

        let before_commit_cancel = vec![
            live_vram_event(0, &key, LiveVramPageEventKind::Snapshot, Some(checksum)),
            live_vram_event(
                64,
                &key,
                LiveVramPageEventKind::CancelledBeforeRuntimeCommit,
                None,
            ),
            live_vram_event(128, &key, LiveVramPageEventKind::AttentionUse, None),
        ];
        let before_report = evaluate_live_vram_event_trace(
            &before_commit_cancel,
            LiveVramEventTracePolicy::default(),
        );
        assert!(before_report.passed());
        assert_eq!(before_report.cancellations, 1);
        assert_eq!(before_report.offloaded_pages_at_end, 0);

        let after_commit_cancel = vec![
            live_vram_event(0, &key, LiveVramPageEventKind::Snapshot, Some(checksum)),
            live_vram_event(
                64,
                &key,
                LiveVramPageEventKind::OffloadCommitted,
                Some(checksum),
            ),
            live_vram_event(
                120,
                &key,
                LiveVramPageEventKind::CancelledAfterRuntimeCommit,
                Some(checksum),
            ),
            live_vram_event(128, &key, LiveVramPageEventKind::AttentionUse, None),
        ];
        let after_report = evaluate_live_vram_event_trace(
            &after_commit_cancel,
            LiveVramEventTracePolicy::default(),
        );
        assert!(after_report.passed());
        assert_eq!(after_report.cancellations, 1);
        assert_eq!(after_report.offloaded_pages_at_end, 0);

        let mut tampered_after_commit_cancel = after_commit_cancel;
        tampered_after_commit_cancel[2].checksum = Some(checksum ^ 1);
        let tampered_report = evaluate_live_vram_event_trace(
            &tampered_after_commit_cancel,
            LiveVramEventTracePolicy::default(),
        );
        assert!(!tampered_report.passed());
        assert!(tampered_report.failures.iter().any(|failure| matches!(
            failure,
            LiveVramEventTraceFailure::CancellationChecksumMismatch { .. }
        )));
    }

    #[test]
    fn live_vram_event_trace_rejects_cancellation_stage_mismatches() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let key = KvPageKey::from_descriptor(&snapshot.descriptor);
        let checksum = snapshot.descriptor.checksum;

        let after_without_offload = vec![
            live_vram_event(0, &key, LiveVramPageEventKind::Snapshot, Some(checksum)),
            live_vram_event(
                64,
                &key,
                LiveVramPageEventKind::CancelledAfterRuntimeCommit,
                Some(checksum),
            ),
            live_vram_event(128, &key, LiveVramPageEventKind::AttentionUse, None),
        ];
        let after_report = evaluate_live_vram_event_trace(
            &after_without_offload,
            LiveVramEventTracePolicy::default(),
        );
        assert!(!after_report.passed());
        assert!(after_report.failures.iter().any(|failure| matches!(
            failure,
            LiveVramEventTraceFailure::CancellationAfterRuntimeCommitWithoutOffload { .. }
        )));

        let before_after_offload = vec![
            live_vram_event(0, &key, LiveVramPageEventKind::Snapshot, Some(checksum)),
            live_vram_event(
                64,
                &key,
                LiveVramPageEventKind::OffloadCommitted,
                Some(checksum),
            ),
            live_vram_event(
                120,
                &key,
                LiveVramPageEventKind::CancelledBeforeRuntimeCommit,
                None,
            ),
            live_vram_event(128, &key, LiveVramPageEventKind::AttentionUse, None),
        ];
        let before_report = evaluate_live_vram_event_trace(
            &before_after_offload,
            LiveVramEventTracePolicy::default(),
        );
        assert!(!before_report.passed());
        assert!(before_report.failures.iter().any(|failure| matches!(
            failure,
            LiveVramEventTraceFailure::CancellationBeforeRuntimeCommitAfterOffload { .. }
        )));
        assert!(before_report.failures.iter().any(|failure| matches!(
            failure,
            LiveVramEventTraceFailure::AttentionUseWhileOffloaded { .. }
        )));
    }

    #[test]
    fn live_vram_event_trace_rejects_unfinished_offloads_and_non_monotonic_tokens() {
        let snapshot = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let key = KvPageKey::from_descriptor(&snapshot.descriptor);
        let checksum = snapshot.descriptor.checksum;
        let trace = vec![
            live_vram_event(16, &key, LiveVramPageEventKind::Snapshot, Some(checksum)),
            live_vram_event(
                32,
                &key,
                LiveVramPageEventKind::OffloadCommitted,
                Some(checksum),
            ),
            live_vram_event(31, &key, LiveVramPageEventKind::AttentionUse, None),
        ];

        let report = evaluate_live_vram_event_trace(&trace, LiveVramEventTracePolicy::default());

        assert!(!report.passed());
        assert!(report.failures.iter().any(|failure| matches!(
            failure,
            LiveVramEventTraceFailure::NonMonotonicToken { token: 31, .. }
        )));
        assert!(report.failures.iter().any(|failure| matches!(
            failure,
            LiveVramEventTraceFailure::OffloadedPagesRemaining { count: 1 }
        )));
    }

    #[test]
    fn live_vram_metrics_export_operator_counters_without_tensor_contents() {
        let compressed = live_vram_snapshot(
            repeated_u16_bytes(0x3f80, 512),
            TensorDType::BF16,
            vec![512],
            Some(4096),
        );
        let mut passthrough = live_vram_snapshot(
            deterministic_bytes(1024),
            TensorDType::F16,
            vec![512],
            Some(4096),
        );
        passthrough.descriptor.layer_id = 8;
        passthrough.descriptor.kind = KvPageKind::Value;
        let mut store = LiveVramOffloadStore::new_with_shadow_validation(
            LiveVramLimits::default(),
            8,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
        );
        store.commit_snapshot(&compressed).unwrap();
        store.commit_snapshot(&passthrough).unwrap();

        let metrics = LiveVramOperatorMetrics::from_store(&store, 3).with_offload_skipped_total(2);
        assert_eq!(metrics.pages_resident_gpu, 3);
        assert_eq!(metrics.pages_offloaded_qatq, 1);
        assert_eq!(metrics.pages_offloaded_cpu_raw, 1);
        assert_eq!(metrics.pass_through_total, 1);
        assert_eq!(
            metrics.offload_bytes_raw,
            compressed.bytes_le.len() + passthrough.bytes_le.len()
        );

        let text = metrics.to_prometheus_text();
        assert!(text.contains("qatq_live_pages_resident_gpu 3\n"));
        assert!(text.contains("qatq_live_pages_offloaded_qatq 1\n"));
        assert!(text.contains("qatq_live_pages_offloaded_cpu_raw 1\n"));
        assert!(text.contains("qatq_live_restore_stall_nanoseconds_total 0\n"));
        assert!(text.contains("qatq_live_offload_skipped_total 2\n"));
        assert!(!text.contains("Qwen/Qwen2.5-0.5B-Instruct"));
        assert!(!text.contains("seq-0001"));
    }

    #[test]
    fn live_vram_storage_labels_reject_unknown_manifest_values() {
        assert_eq!(
            LiveVramStorage::from_label("qatq-live").unwrap(),
            LiveVramStorage::Qatq
        );
        assert_eq!(
            LiveVramStorage::from_label("raw-typed-pass-through").unwrap(),
            LiveVramStorage::RawTypedPassThrough
        );
        assert_eq!(
            LiveVramStorage::from_label("unknown-storage"),
            Err(QatqError::InvalidHeader)
        );
    }

    #[test]
    fn llama_cpp_manifest_replay_rejects_missing_required_metadata() {
        let manifest = r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 16,
  "streams": 1,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0.f16le","active_cells":4,"embedding":8,"row_bytes":16}
  ]
}
"#;

        assert_eq!(
            parse_llama_cpp_kv_manifest(manifest),
            Err(QatqError::InvalidHeader)
        );
    }

    #[test]
    fn llama_cpp_manifest_replay_builds_live_vram_snapshots() {
        let dir = unique_test_dir("llama-cpp-live-vram");
        std::fs::create_dir_all(&dir).unwrap();
        let k_bytes = repeated_u16_bytes(0x3c00, 32);
        let v_bytes = repeated_u16_bytes(0x4000, 32);
        std::fs::write(dir.join("cache_k_l0_s0.f16le"), &k_bytes).unwrap();
        std::fs::write(dir.join("cache_v_l0_s0.f16le"), &v_bytes).unwrap();
        std::fs::write(
            dir.join("manifest.json"),
            r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 16,
  "streams": 1,
  "live_page_residency_granularity": "per-page",
  "gpu_allocation_granularity": "whole-context",
  "gpu_context_bytes": 128,
  "total_context_bytes": 256,
  "gpu_resident_tensors": 1,
  "total_tensors": 2,
  "gpu_page_staging_bytes": 64,
  "gpu_page_staging_tensors": 1,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0.f16le","dtype":"f16le","token_start":8,"token_end":12,"active_cells":4,"embedding":8,"row_bytes":16},
    {"name":"cache_v_l0","kind":"v","stream":0,"file":"cache_v_l0_s0.f16le","dtype":"f16le","token_start":8,"token_end":12,"active_cells":4,"embedding":8,"row_bytes":16,"transposed":true}
  ]
}
"#,
        )
        .unwrap();
        let parsed_manifest = parse_llama_cpp_kv_manifest(
            &std::fs::read_to_string(dir.join("manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            parsed_manifest.live_page_residency_granularity,
            Some(LiveVramPageResidencyGranularity::PerPage)
        );
        assert_eq!(
            parsed_manifest.gpu_allocation_granularity,
            Some(LiveVramGpuAllocationGranularity::WholeContext)
        );
        assert_eq!(parsed_manifest.gpu_context_bytes, Some(128));
        assert_eq!(parsed_manifest.total_context_bytes, Some(256));
        assert_eq!(parsed_manifest.gpu_resident_tensors, Some(1));
        assert_eq!(parsed_manifest.total_tensors, Some(2));
        assert_eq!(parsed_manifest.gpu_page_staging_bytes, Some(64));
        assert_eq!(parsed_manifest.gpu_page_staging_tensors, Some(1));

        let config = LlamaCppKvExportReplayConfig {
            runtime_commit: "7992aa7c8".to_string(),
            adapter_version: "qatq-kv-export-7992aa7c8".to_string(),
            model_id: "test-model.gguf:sha256:abc123".to_string(),
            max_tensors: 16,
            next_required_token: Some(2048),
        };
        let snapshots =
            live_vram_snapshots_from_llama_cpp_export_dir(&dir, &config, LiveVramLimits::default())
                .unwrap();

        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].descriptor.kind, KvPageKind::Key);
        assert_eq!(snapshots[0].descriptor.shape, vec![4, 8]);
        assert_eq!(snapshots[0].descriptor.layout, KvPageLayout::Contiguous);
        assert_eq!(snapshots[0].descriptor.token_start, 8);
        assert_eq!(snapshots[0].descriptor.token_end, 12);
        assert_eq!(snapshots[1].descriptor.kind, KvPageKind::Value);
        assert_eq!(snapshots[1].descriptor.shape, vec![8, 4]);
        assert_eq!(snapshots[1].descriptor.layout, KvPageLayout::Transposed);
        assert_eq!(snapshots[1].descriptor.token_start, 8);
        assert_eq!(snapshots[1].descriptor.token_end, 12);

        let report = build_live_vram_evidence_report(
            &snapshots,
            LiveVramSchedulerState {
                current_token: 0,
                queued_pages: 0,
                cpu_stored_bytes: 0,
            },
            LiveVramSchedulerPolicy::default(),
            LiveVramLimits::default(),
        )
        .unwrap();
        assert_eq!(report.total_pages, 2);
        assert_eq!(report.verified_restores, 2);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn llama_cpp_manifest_replay_rejects_unsafe_manifest_file_paths() {
        let manifest = r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 16,
  "streams": 1,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"../cache_k_l0_s0.f16le","dtype":"f16le","active_cells":4,"embedding":8,"row_bytes":16}
  ]
}
"#;
        let dir = unique_test_dir("llama-cpp-live-vram-unsafe");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("manifest.json"), manifest).unwrap();
        let config = LlamaCppKvExportReplayConfig {
            runtime_commit: "7992aa7c8".to_string(),
            adapter_version: "qatq-kv-export-7992aa7c8".to_string(),
            model_id: "test-model.gguf:sha256:abc123".to_string(),
            max_tensors: 16,
            next_required_token: Some(2048),
        };

        assert_eq!(
            parse_llama_cpp_kv_manifest(manifest),
            Err(QatqError::InvalidHeader)
        );
        assert_eq!(
            live_vram_snapshots_from_llama_cpp_export_dir(&dir, &config, LiveVramLimits::default()),
            Err(QatqError::InvalidHeader)
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn llama_cpp_manifest_parser_rejects_duplicate_and_mismatched_tensor_metadata() {
        let duplicate_page = r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 16,
  "streams": 1,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0.f16le","dtype":"f16le","active_cells":4,"embedding":8,"row_bytes":16},
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0_dup.f16le","dtype":"f16le","active_cells":4,"embedding":8,"row_bytes":16}
  ]
}
"#;
        assert_eq!(
            parse_llama_cpp_kv_manifest(duplicate_page),
            Err(QatqError::InvalidHeader)
        );

        let split_pages = r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 16,
  "streams": 1,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0_t0_4.f16le","dtype":"f16le","token_start":0,"token_end":4,"active_cells":4,"embedding":8,"row_bytes":16},
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0_t4_8.f16le","dtype":"f16le","token_start":4,"token_end":8,"active_cells":4,"embedding":8,"row_bytes":16}
  ]
}
"#;
        assert_eq!(
            parse_llama_cpp_kv_manifest(split_pages)
                .unwrap()
                .tensors
                .len(),
            2
        );

        let reordered_split_pages = r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 16,
  "streams": 1,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0_t4_8.f16le","dtype":"f16le","token_start":4,"token_end":8,"active_cells":4,"embedding":8,"row_bytes":16},
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0_t0_4.f16le","dtype":"f16le","token_start":0,"token_end":4,"active_cells":4,"embedding":8,"row_bytes":16}
  ]
}
"#;
        assert_eq!(
            parse_llama_cpp_kv_manifest(reordered_split_pages)
                .unwrap()
                .tensors
                .len(),
            2
        );

        let overlapping_pages = r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 16,
  "streams": 1,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0_t0_4.f16le","dtype":"f16le","token_start":0,"token_end":4,"active_cells":4,"embedding":8,"row_bytes":16},
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0_t2_6.f16le","dtype":"f16le","token_start":2,"token_end":6,"active_cells":4,"embedding":8,"row_bytes":16}
  ]
}
"#;
        assert_eq!(
            parse_llama_cpp_kv_manifest(overlapping_pages),
            Err(QatqError::InvalidHeader)
        );

        let stream_out_of_bounds = r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 16,
  "streams": 1,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":1,"file":"cache_k_l0_s1_t0_4.f16le","dtype":"f16le","token_start":0,"token_end":4,"active_cells":4,"embedding":8,"row_bytes":16}
  ]
}
"#;
        assert_eq!(
            parse_llama_cpp_kv_manifest(stream_out_of_bounds),
            Err(QatqError::InvalidHeader)
        );

        let cross_layer_file = r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 16,
  "streams": 2,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l1_s0_t0_4.f16le","dtype":"f16le","token_start":0,"token_end":4,"active_cells":4,"embedding":8,"row_bytes":16}
  ]
}
"#;
        assert_eq!(
            parse_llama_cpp_kv_manifest(cross_layer_file),
            Err(QatqError::InvalidHeader)
        );

        let cross_kind_file = r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 16,
  "streams": 2,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_v_l0_s0_t0_4.f16le","dtype":"f16le","token_start":0,"token_end":4,"active_cells":4,"embedding":8,"row_bytes":16}
  ]
}
"#;
        assert_eq!(
            parse_llama_cpp_kv_manifest(cross_kind_file),
            Err(QatqError::InvalidHeader)
        );

        let cross_stream_file = r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 16,
  "streams": 2,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s1_t0_4.f16le","dtype":"f16le","token_start":0,"token_end":4,"active_cells":4,"embedding":8,"row_bytes":16}
  ]
}
"#;
        assert_eq!(
            parse_llama_cpp_kv_manifest(cross_stream_file),
            Err(QatqError::InvalidHeader)
        );

        let cross_range_file = r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 16,
  "streams": 1,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0_t4_8.f16le","dtype":"f16le","token_start":0,"token_end":4,"active_cells":4,"embedding":8,"row_bytes":16}
  ]
}
"#;
        assert_eq!(
            parse_llama_cpp_kv_manifest(cross_range_file),
            Err(QatqError::InvalidHeader)
        );

        let token_range_mismatch = r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 16,
  "streams": 1,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0_t0_5.f16le","dtype":"f16le","token_start":0,"token_end":5,"active_cells":4,"embedding":8,"row_bytes":16}
  ]
}
"#;
        assert_eq!(
            parse_llama_cpp_kv_manifest(token_range_mismatch),
            Err(QatqError::InvalidHeader)
        );

        let token_range_out_of_bounds = r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 16,
  "streams": 1,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0_t14_18.f16le","dtype":"f16le","token_start":14,"token_end":18,"active_cells":4,"embedding":8,"row_bytes":16}
  ]
}
"#;
        assert_eq!(
            parse_llama_cpp_kv_manifest(token_range_out_of_bounds),
            Err(QatqError::InvalidHeader)
        );

        let duplicate_file = r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 16,
  "streams": 1,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_l0_s0.f16le","dtype":"f16le","active_cells":4,"embedding":8,"row_bytes":16},
    {"name":"cache_v_l0","kind":"v","stream":0,"file":"cache_l0_s0.f16le","dtype":"f16le","active_cells":4,"embedding":8,"row_bytes":16}
  ]
}
"#;
        assert_eq!(
            parse_llama_cpp_kv_manifest(duplicate_file),
            Err(QatqError::InvalidHeader)
        );

        let count_mismatch = r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 16,
  "streams": 1,
  "total_tensors": 1,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0.f16le","dtype":"f16le","active_cells":4,"embedding":8,"row_bytes":16},
    {"name":"cache_v_l0","kind":"v","stream":0,"file":"cache_v_l0_s0.f16le","dtype":"f16le","active_cells":4,"embedding":8,"row_bytes":16}
  ]
}
"#;
        assert_eq!(
            parse_llama_cpp_kv_manifest(count_mismatch),
            Err(QatqError::InvalidHeader)
        );
    }

    #[test]
    fn llama_cpp_manifest_parser_rejects_zero_sized_context_or_empty_tensor_list() {
        let zero_kv = r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 0,
  "streams": 1,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0.f16le","dtype":"f16le","active_cells":4,"embedding":8,"row_bytes":16}
  ]
}
"#;
        assert_eq!(
            parse_llama_cpp_kv_manifest(zero_kv),
            Err(QatqError::InvalidHeader)
        );

        let empty_tensors = r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 16,
  "streams": 1,
  "tensors": []
}
"#;
        assert_eq!(
            parse_llama_cpp_kv_manifest(empty_tensors),
            Err(QatqError::InvalidHeader)
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

    fn live_vram_snapshot(
        bytes_le: Vec<u8>,
        dtype: TensorDType,
        shape: Vec<usize>,
        next_required_token: Option<u64>,
    ) -> KvPageSnapshot {
        KvPageSnapshot {
            descriptor: KvPageDescriptor {
                runtime_id: "llama.cpp".to_string(),
                runtime_commit: "test-commit".to_string(),
                adapter_version: "qatq-test-adapter/0.1.0".to_string(),
                model_id: "Qwen/Qwen2.5-0.5B-Instruct".to_string(),
                seq_id: "seq-0001".to_string(),
                layer_id: 7,
                kind: KvPageKind::Key,
                dtype,
                shape,
                layout: KvPageLayout::Paged,
                token_start: 0,
                token_end: 128,
                next_required_token,
                raw_len: bytes_le.len(),
                checksum: live_vram_page_checksum(&bytes_le),
            },
            bytes_le,
        }
    }

    fn live_vram_test_seal_policy() -> LiveVramPageSealPolicy {
        LiveVramPageSealPolicy::new([0x42_u8; 32], b"qatq-test-live-vram-store".to_vec()).unwrap()
    }

    fn repeated_u16_bytes(value: u16, count: usize) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(count * 2);
        for _ in 0..count {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    fn compression_positive_live_vram_snapshot() -> KvPageSnapshot {
        for seed in 0_u16..64 {
            let mut bytes = Vec::with_capacity(16_384);
            let mut state = 0x9e37_79b9_u32 ^ seed as u32;
            for index in 0..8192_u16 {
                state = lcg_next_for_test(state ^ index as u32);
                let low = (state >> 16) as u16;
                let high = 0x3c00_u16.wrapping_add((index % 4) << 8);
                bytes.extend_from_slice(&(high | (low & 0x00ff)).to_le_bytes());
            }
            let snapshot = live_vram_snapshot(bytes, TensorDType::F16, vec![8192], Some(4096));
            let encoded = try_encode_live_vram_page(&snapshot, LiveVramLimits::default()).unwrap();
            let zstd_bytes = zstd::bulk::compress(&snapshot.bytes_le, 3).unwrap().len();
            let lz4_bytes = lz4_flex::compress_prepend_size(&snapshot.bytes_le).len();
            if encoded.should_compress() && encoded.bytes.len() < zstd_bytes.min(lz4_bytes) {
                return snapshot;
            }
        }
        panic!("failed to find a compression-positive live VRAM fixture");
    }

    fn qatq_smaller_than_raw_but_not_best_general_snapshot() -> KvPageSnapshot {
        for count in [
            16_usize, 24, 32, 48, 64, 96, 128, 192, 256, 384, 512, 768, 1024,
        ] {
            for value in [0x0000_u16, 0x3c00, 0x4000, 0x4100, 0x7bff] {
                let snapshot = live_vram_snapshot(
                    repeated_u16_bytes(value, count),
                    TensorDType::F16,
                    vec![count],
                    Some(4096),
                );
                let encoded =
                    try_encode_live_vram_page(&snapshot, LiveVramLimits::default()).unwrap();
                let zstd_bytes = zstd::bulk::compress(&snapshot.bytes_le, 3).unwrap().len();
                let lz4_bytes = lz4_flex::compress_prepend_size(&snapshot.bytes_le).len();
                if encoded.should_compress()
                    && encoded.bytes.len() < snapshot.bytes_le.len()
                    && encoded.bytes.len() >= zstd_bytes.min(lz4_bytes)
                {
                    return snapshot;
                }
            }
            for seed in 0_u16..512 {
                let mut bytes = Vec::with_capacity(count * 2);
                let mut state = 0x517c_c1b7_u32 ^ seed as u32;
                for index in 0..count as u16 {
                    state = lcg_next_for_test(state ^ ((index as u32) << 5));
                    let high = 0x3c00_u16.wrapping_add((index % 16) << 4);
                    let low = (state >> 16) as u16 & 0x00ff;
                    bytes.extend_from_slice(&(high | low).to_le_bytes());
                }
                let snapshot = live_vram_snapshot(bytes, TensorDType::F16, vec![count], Some(4096));
                let encoded =
                    try_encode_live_vram_page(&snapshot, LiveVramLimits::default()).unwrap();
                let zstd_bytes = zstd::bulk::compress(&snapshot.bytes_le, 3).unwrap().len();
                let lz4_bytes = lz4_flex::compress_prepend_size(&snapshot.bytes_le).len();
                if encoded.should_compress()
                    && encoded.bytes.len() < snapshot.bytes_le.len()
                    && encoded.bytes.len() >= zstd_bytes.min(lz4_bytes)
                {
                    return snapshot;
                }
            }
        }
        panic!("failed to find a QATQ-smaller-than-raw but not best-general fixture");
    }

    fn f16_page(values: &[f32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(values.len() * 2);
        for value in values {
            let bits = match *value {
                0.0 => 0x0000,
                0.25 => 0x3400,
                0.5 => 0x3800,
                1.0 => 0x3c00,
                1.5 => 0x3e00,
                -0.25 => 0xb400,
                -0.5 => 0xb800,
                f32::INFINITY => 0x7c00,
                _ => panic!("test value is not in f16_page lookup: {value}"),
            };
            bytes.extend_from_slice(&u16::to_le_bytes(bits));
        }
        bytes
    }

    fn bf16_page(values: &[f32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(values.len() * 2);
        for value in values {
            bytes.extend_from_slice(&((value.to_bits() >> 16) as u16).to_le_bytes());
        }
        bytes
    }

    fn deterministic_bytes(len: usize) -> Vec<u8> {
        let mut state = 0x9e37_79b9_u32;
        let mut bytes = Vec::with_capacity(len);
        for index in 0..len {
            state = lcg_next_for_test(state ^ index as u32);
            bytes.push((state >> 16) as u8);
        }
        bytes
    }

    fn unique_test_dir(prefix: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("{prefix}-{}-{nanos}", std::process::id()));
        path
    }

    fn live_vram_event(
        token: u64,
        key: &KvPageKey,
        kind: LiveVramPageEventKind,
        checksum: Option<u64>,
    ) -> LiveVramPageEvent {
        LiveVramPageEvent {
            token,
            key: key.clone(),
            kind,
            checksum,
        }
    }

    struct TestLiveVramAdapter {
        snapshot: KvPageSnapshot,
        committed: Option<LiveVramPageEncodeResult>,
        metrics: LiveVramAdapterMetrics,
        fail_commit: bool,
    }

    impl LiveVramRuntimeAdapter for TestLiveVramAdapter {
        fn identity(&self) -> LiveVramAdapterIdentity {
            LiveVramAdapterIdentity {
                runtime_id: "llama.cpp".to_string(),
                runtime_commit: "test-commit".to_string(),
                adapter_version: "qatq-test-adapter/0.1.0".to_string(),
                adapter_contract_version: LIVE_VRAM_ADAPTER_CONTRACT_VERSION.to_string(),
            }
        }

        fn snapshot_page(
            &mut self,
            descriptor: &KvPageDescriptor,
            limits: LiveVramLimits,
        ) -> Result<KvPageSnapshot, LiveVramAdapterError> {
            descriptor
                .validate(limits)
                .map_err(|_| LiveVramAdapterError::SnapshotFailed("invalid descriptor"))?;
            if descriptor != &self.snapshot.descriptor {
                return Err(LiveVramAdapterError::SnapshotFailed("unknown page"));
            }
            Ok(self.snapshot.clone())
        }

        fn commit_offload(
            &mut self,
            encoded: &LiveVramPageEncodeResult,
        ) -> Result<(), LiveVramAdapterError> {
            if self.fail_commit {
                return Err(LiveVramAdapterError::CommitFailed("forced commit failure"));
            }
            self.metrics.resident_pages = self.metrics.resident_pages.saturating_sub(1);
            self.metrics.offloaded_pages += 1;
            self.metrics.cpu_stored_bytes += encoded.bytes.len();
            self.metrics.current_gpu_bytes = self
                .metrics
                .current_gpu_bytes
                .saturating_sub(encoded.metadata.descriptor.raw_len);
            self.committed = Some(encoded.clone());
            Ok(())
        }

        fn restore_committed_page(
            &mut self,
            metadata: &LiveVramPageMetadata,
            bytes: &[u8],
            limits: LiveVramLimits,
        ) -> Result<LiveVramRestoreStatus, LiveVramAdapterError> {
            restore_live_vram_page(metadata, bytes, limits)
                .map_err(|_| LiveVramAdapterError::RestoreFailed("restore rejected"))?;
            self.metrics.resident_pages += 1;
            self.metrics.offloaded_pages = self.metrics.offloaded_pages.saturating_sub(1);
            self.metrics.current_gpu_bytes += metadata.descriptor.raw_len;
            Ok(LiveVramRestoreStatus::Restored)
        }

        fn is_page_resident(
            &self,
            descriptor: &KvPageDescriptor,
        ) -> Result<bool, LiveVramAdapterError> {
            if descriptor != &self.snapshot.descriptor {
                return Err(LiveVramAdapterError::ResidencyQueryFailed("unknown page"));
            }
            Ok(self.metrics.resident_pages > 0)
        }

        fn metrics(&self) -> Result<LiveVramAdapterMetrics, LiveVramAdapterError> {
            Ok(self.metrics.clone())
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum DishonestRestoreMode {
        ReturnsRestoredButNotResident,
        RuntimeRestoreFails,
        ReturnsResourceLimitRejected,
        ResidencyQueryFailsAfterRestore,
    }

    struct DishonestRestoreAdapter {
        snapshot: KvPageSnapshot,
        resident: bool,
        mode: DishonestRestoreMode,
    }

    impl LiveVramRuntimeAdapter for DishonestRestoreAdapter {
        fn identity(&self) -> LiveVramAdapterIdentity {
            LiveVramAdapterIdentity {
                runtime_id: "llama.cpp".to_string(),
                runtime_commit: "test-commit".to_string(),
                adapter_version: "qatq-dishonest-adapter/0.1.0".to_string(),
                adapter_contract_version: LIVE_VRAM_ADAPTER_CONTRACT_VERSION.to_string(),
            }
        }

        fn snapshot_page(
            &mut self,
            descriptor: &KvPageDescriptor,
            limits: LiveVramLimits,
        ) -> Result<KvPageSnapshot, LiveVramAdapterError> {
            descriptor
                .validate(limits)
                .map_err(|_| LiveVramAdapterError::SnapshotFailed("invalid descriptor"))?;
            if descriptor != &self.snapshot.descriptor || !self.resident {
                return Err(LiveVramAdapterError::SnapshotFailed("page unavailable"));
            }
            Ok(self.snapshot.clone())
        }

        fn commit_offload(
            &mut self,
            encoded: &LiveVramPageEncodeResult,
        ) -> Result<(), LiveVramAdapterError> {
            if encoded.metadata.descriptor != self.snapshot.descriptor {
                return Err(LiveVramAdapterError::CommitFailed("unknown page"));
            }
            self.resident = false;
            Ok(())
        }

        fn restore_committed_page(
            &mut self,
            metadata: &LiveVramPageMetadata,
            bytes: &[u8],
            limits: LiveVramLimits,
        ) -> Result<LiveVramRestoreStatus, LiveVramAdapterError> {
            if self.mode == DishonestRestoreMode::RuntimeRestoreFails {
                return Err(LiveVramAdapterError::RestoreFailed(
                    "forced restore allocation failure",
                ));
            }
            restore_live_vram_page(metadata, bytes, limits)
                .map_err(|_| LiveVramAdapterError::RestoreFailed("restore rejected"))?;
            if self.mode == DishonestRestoreMode::ReturnsResourceLimitRejected {
                return Ok(LiveVramRestoreStatus::ResourceLimitRejected);
            }
            if self.mode == DishonestRestoreMode::ResidencyQueryFailsAfterRestore {
                self.resident = true;
            }
            Ok(LiveVramRestoreStatus::Restored)
        }

        fn is_page_resident(
            &self,
            descriptor: &KvPageDescriptor,
        ) -> Result<bool, LiveVramAdapterError> {
            if descriptor != &self.snapshot.descriptor {
                return Err(LiveVramAdapterError::ResidencyQueryFailed("unknown page"));
            }
            if self.mode == DishonestRestoreMode::ResidencyQueryFailsAfterRestore {
                return Err(LiveVramAdapterError::ResidencyQueryFailed(
                    "forced residency query failure",
                ));
            }
            Ok(self.resident)
        }

        fn metrics(&self) -> Result<LiveVramAdapterMetrics, LiveVramAdapterError> {
            Ok(LiveVramAdapterMetrics {
                resident_pages: usize::from(self.resident),
                offloaded_pages: usize::from(!self.resident),
                current_gpu_bytes: if self.resident {
                    self.snapshot.bytes_le.len()
                } else {
                    0
                },
                peak_gpu_bytes: self.snapshot.bytes_le.len(),
                ..LiveVramAdapterMetrics::default()
            })
        }
    }

    struct SealedOnlyRestoreAdapter {
        snapshot: KvPageSnapshot,
        resident: bool,
        raw_restore_calls: usize,
        sealed_restore_calls: usize,
    }

    impl LiveVramRuntimeAdapter for SealedOnlyRestoreAdapter {
        fn identity(&self) -> LiveVramAdapterIdentity {
            LiveVramAdapterIdentity {
                runtime_id: "llama.cpp".to_string(),
                runtime_commit: "test-commit".to_string(),
                adapter_version: "qatq-sealed-only-test-adapter/0.1.0".to_string(),
                adapter_contract_version: LIVE_VRAM_ADAPTER_CONTRACT_VERSION.to_string(),
            }
        }

        fn snapshot_page(
            &mut self,
            descriptor: &KvPageDescriptor,
            limits: LiveVramLimits,
        ) -> Result<KvPageSnapshot, LiveVramAdapterError> {
            descriptor
                .validate(limits)
                .map_err(|_| LiveVramAdapterError::SnapshotFailed("invalid descriptor"))?;
            if descriptor != &self.snapshot.descriptor || !self.resident {
                return Err(LiveVramAdapterError::SnapshotFailed("page unavailable"));
            }
            Ok(self.snapshot.clone())
        }

        fn commit_offload(
            &mut self,
            encoded: &LiveVramPageEncodeResult,
        ) -> Result<(), LiveVramAdapterError> {
            if encoded.metadata.descriptor != self.snapshot.descriptor {
                return Err(LiveVramAdapterError::CommitFailed("unknown page"));
            }
            self.resident = false;
            Ok(())
        }

        fn restore_committed_page(
            &mut self,
            _metadata: &LiveVramPageMetadata,
            _bytes: &[u8],
            _limits: LiveVramLimits,
        ) -> Result<LiveVramRestoreStatus, LiveVramAdapterError> {
            self.raw_restore_calls += 1;
            Err(LiveVramAdapterError::RestoreFailed(
                "raw restore boundary forbidden",
            ))
        }

        fn restore_sealed_committed_page(
            &mut self,
            request: LiveVramSealedRestoreRequest<'_>,
            limits: LiveVramLimits,
        ) -> Result<LiveVramRestoreStatus, LiveVramAdapterError> {
            self.sealed_restore_calls += 1;
            let restored = request
                .restore_bytes(limits)
                .map_err(|_| LiveVramAdapterError::RestoreFailed("sealed restore rejected"))?;
            if request.metadata().descriptor != self.snapshot.descriptor
                || restored != self.snapshot.bytes_le
            {
                return Ok(LiveVramRestoreStatus::ChecksumFailure);
            }
            self.resident = true;
            Ok(LiveVramRestoreStatus::Restored)
        }

        fn is_page_resident(
            &self,
            descriptor: &KvPageDescriptor,
        ) -> Result<bool, LiveVramAdapterError> {
            if descriptor != &self.snapshot.descriptor {
                return Err(LiveVramAdapterError::ResidencyQueryFailed("unknown page"));
            }
            Ok(self.resident)
        }

        fn metrics(&self) -> Result<LiveVramAdapterMetrics, LiveVramAdapterError> {
            Ok(LiveVramAdapterMetrics {
                resident_pages: usize::from(self.resident),
                offloaded_pages: usize::from(!self.resident),
                current_gpu_bytes: if self.resident {
                    self.snapshot.bytes_le.len()
                } else {
                    0
                },
                peak_gpu_bytes: self.snapshot.bytes_le.len(),
                ..LiveVramAdapterMetrics::default()
            })
        }
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
