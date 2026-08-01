//! Bounded, descriptive geometry profiling for exported KV tensors.
//!
//! This module reports observations only. It does not derive application
//! capacity requirements or emit Capacity Oracle verdicts.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use half::{bf16, f16};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CAPTURE_SCHEMA_VERSION: u32 = 1;
pub const RESULT_SCHEMA_VERSION: u32 = 1;
const HARD_MAX_METADATA_BYTES: usize = 16 * 1024 * 1024;
const HARD_MAX_CAPTURE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const HARD_MAX_SCALAR_VALUES: u64 = 64_000_000;
const HARD_MAX_VECTORS: u64 = 4_000_000;
const HARD_MAX_DIMENSION: u32 = 16_384;
const HARD_MAX_PAIRS: u64 = 10_000_000;
const HARD_MAX_BLOCK_VECTORS: u32 = 65_536;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DType {
    F16,
    Bf16,
    F32,
}

impl DType {
    fn width(self) -> usize {
        match self {
            Self::F16 | Self::Bf16 => 2,
            Self::F32 => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KvKind {
    Key,
    Value,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RopeStage {
    PreRope,
    PostRope,
    NotApplicable,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TensorLayout {
    TokenHeadDimension,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TensorDescriptor {
    pub id: String,
    pub offset_bytes: u64,
    pub byte_length: u64,
    pub layer: u32,
    pub kind: KvKind,
    pub rope_stage: RopeStage,
    pub token_start: u64,
    pub token_count: u64,
    pub heads: u32,
    pub dimension: u32,
    pub layout: TensorLayout,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureMetadata {
    pub schema_version: u32,
    pub capture_id: String,
    pub model: String,
    pub model_family: String,
    pub runtime: String,
    pub runtime_version: String,
    pub prompt_class: String,
    pub prompt_sha256: String,
    pub context_length: u64,
    pub dtype: DType,
    pub tensors: Vec<TensorDescriptor>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PartitionMode {
    LayerHeadToken,
    LayerToken,
    LayerHeadChunk,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Normalization {
    UnitL2,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResultStatus {
    Exact,
    DeterministicSample,
    Approximate,
    Refused,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilePolicy {
    pub partition: PartitionMode,
    pub normalization: Normalization,
    pub seed: u64,
    pub max_pairs: u64,
    pub exact_vector_threshold: u64,
    pub max_capture_bytes: u64,
    pub max_vectors: u64,
    pub max_scalar_values: u64,
    pub max_dimension: u32,
    pub max_spectral_dimension: u32,
    pub chunk_tokens: u32,
    pub block_vectors: u32,
    pub near_duplicate_cosine: f64,
    pub thresholds: Vec<f64>,
}

impl Default for ProfilePolicy {
    fn default() -> Self {
        Self {
            partition: PartitionMode::LayerHeadToken,
            normalization: Normalization::UnitL2,
            seed: 42,
            max_pairs: 1_000_000,
            exact_vector_threshold: 2_048,
            max_capture_bytes: 8 * 1024 * 1024 * 1024,
            max_vectors: 4_000_000,
            max_scalar_values: 32_000_000,
            max_dimension: 16_384,
            max_spectral_dimension: 1_024,
            chunk_tokens: 256,
            block_vectors: 256,
            near_duplicate_cosine: 0.9999,
            thresholds: vec![0.0],
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Distribution {
    pub count: u64,
    pub minimum: Option<f64>,
    pub p01: Option<f64>,
    pub p05: Option<f64>,
    pub p25: Option<f64>,
    pub p50: Option<f64>,
    pub p75: Option<f64>,
    pub p95: Option<f64>,
    pub p99: Option<f64>,
    pub maximum: Option<f64>,
    pub mean: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PairPlan {
    pub status: ResultStatus,
    pub population_pairs: u64,
    pub evaluated_pairs: u64,
    pub coverage_fraction: f64,
    pub seed: u64,
    pub mean_standard_error: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PairMetrics {
    pub plan: PairPlan,
    pub cosine_similarity: Distribution,
    pub angular_distance_radians: Distribution,
    pub nearest_neighbor_cosine: Distribution,
    pub nearest_neighbor_angular_distance_radians: Distribution,
    pub nearest_neighbor_scope: String,
    pub near_duplicate_pairs: u64,
    pub near_duplicate_rate: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SpectralMetrics {
    pub status: ResultStatus,
    pub centered_covariance_trace: Option<f64>,
    pub effective_rank_participation_ratio: Option<f64>,
    pub top_eigenvalue_fraction_estimate: Option<f64>,
    pub power_iterations: u32,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BinaryMetrics {
    pub rule: String,
    pub threshold: Option<f64>,
    pub status: ResultStatus,
    pub hamming_distance: Distribution,
    pub collision_classes: u64,
    pub colliding_vectors: u64,
    pub collision_rate: Option<f64>,
    pub sensitivity_note: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct TokenPositionMetrics {
    pub segment: String,
    pub token_start: u64,
    pub token_end_exclusive: u64,
    pub l2_norm: Distribution,
}

#[derive(Clone, Debug, Serialize)]
pub struct GroupMetrics {
    pub group_id: String,
    pub tensor_id: String,
    pub layer: u32,
    pub head: Option<u32>,
    pub token_start: u64,
    pub token_end_exclusive: u64,
    pub kind: KvKind,
    pub rope_stage: RopeStage,
    pub dimension: u32,
    pub vector_count: u64,
    pub finite_vector_count: u64,
    pub zero_vectors: u64,
    pub non_finite_vectors: u64,
    pub normalization_failures: u64,
    pub exact_duplicate_classes: u64,
    pub exact_duplicate_vectors: u64,
    pub l2_norm: Distribution,
    pub token_position_l2_norm: Vec<TokenPositionMetrics>,
    pub pairwise: PairMetrics,
    pub spectral: SpectralMetrics,
    pub binary: Vec<BinaryMetrics>,
    pub status: ResultStatus,
}

#[derive(Clone, Debug, Serialize)]
pub struct Refusal {
    pub status: ResultStatus,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct GeometryReport {
    pub schema_version: u32,
    pub tool: String,
    pub capture_id: String,
    pub capture_sha256: String,
    pub metadata_sha256: String,
    pub policy: ProfilePolicy,
    pub status: ResultStatus,
    pub refusal: Option<Refusal>,
    pub groups: Vec<GroupMetrics>,
    pub claim_boundary: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CaptureManifest {
    pub schema_version: u32,
    pub capture_path: String,
    pub capture_sha256: String,
    pub capture_bytes: u64,
    pub metadata_path: String,
    pub metadata_sha256: String,
    pub metadata: CaptureMetadata,
}

#[derive(Clone, Debug, Serialize)]
pub struct SamplingPlanReport {
    pub schema_version: u32,
    pub seed: u64,
    pub max_pairs: u64,
    pub exact_vector_threshold: u64,
    pub block_vectors: u32,
    pub groups: Vec<PairPlanEntry>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PairPlanEntry {
    pub group_id: String,
    pub plan: PairPlan,
}

#[derive(Clone, Debug, Serialize)]
pub struct AggregateMetrics {
    pub schema_version: u32,
    pub status: ResultStatus,
    pub group_count: u64,
    pub vector_count: u64,
    pub finite_vector_count: u64,
    pub evaluated_pairs: u64,
    pub exact_groups: u64,
    pub sampled_groups: u64,
    pub approximate_groups: u64,
    pub refused_groups: u64,
}

#[derive(Debug)]
pub enum GeometryError {
    Io(String),
    Invalid(String),
    Policy(String),
}

impl std::fmt::Display for GeometryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(message) | Self::Invalid(message) | Self::Policy(message) => {
                f.write_str(message)
            }
        }
    }
}

impl std::error::Error for GeometryError {}

#[derive(Clone)]
struct VectorGroup {
    group_id: String,
    tensor_id: String,
    layer: u32,
    head: Option<u32>,
    token_start: u64,
    token_end_exclusive: u64,
    kind: KvKind,
    rope_stage: RopeStage,
    dimension: usize,
    vectors: Vec<Vec<f64>>,
}

pub fn profile_capture(
    capture_path: &Path,
    metadata_path: &Path,
    policy: ProfilePolicy,
) -> Result<
    (
        CaptureManifest,
        GeometryReport,
        SamplingPlanReport,
        AggregateMetrics,
    ),
    GeometryError,
> {
    validate_policy(&policy)?;
    let capture_bytes = fs::read(capture_path).map_err(|error| {
        GeometryError::Io(format!(
            "failed to read {}: {error}",
            capture_path.display()
        ))
    })?;
    let metadata_bytes = fs::read(metadata_path).map_err(|error| {
        GeometryError::Io(format!(
            "failed to read {}: {error}",
            metadata_path.display()
        ))
    })?;
    if metadata_bytes.len() > HARD_MAX_METADATA_BYTES {
        return Err(GeometryError::Policy(format!(
            "metadata bytes {} exceed hard maximum {HARD_MAX_METADATA_BYTES}",
            metadata_bytes.len()
        )));
    }
    let metadata: CaptureMetadata = serde_json::from_slice(&metadata_bytes)
        .map_err(|error| GeometryError::Invalid(format!("invalid capture metadata: {error}")))?;
    validate_metadata(&metadata, capture_bytes.len() as u64)?;
    let capture_sha256 = hex_digest(&capture_bytes);
    let metadata_sha256 = hex_digest(&metadata_bytes);
    let manifest = CaptureManifest {
        schema_version: CAPTURE_SCHEMA_VERSION,
        capture_path: capture_path.display().to_string(),
        capture_sha256: capture_sha256.clone(),
        capture_bytes: capture_bytes.len() as u64,
        metadata_path: metadata_path.display().to_string(),
        metadata_sha256: metadata_sha256.clone(),
        metadata: metadata.clone(),
    };
    if capture_bytes.len() as u64 > policy.max_capture_bytes {
        let reason = format!(
            "capture bytes {} exceed policy maximum {}",
            capture_bytes.len(),
            policy.max_capture_bytes
        );
        let report = refused_report(&metadata, &capture_sha256, &metadata_sha256, policy, reason);
        let sampling = sampling_report(&report);
        let aggregate = aggregate_report(&report);
        return Ok((manifest, report, sampling, aggregate));
    }
    let scalar_values = metadata.tensors.iter().try_fold(0_u64, |total, tensor| {
        tensor
            .token_count
            .checked_mul(tensor.heads as u64)
            .and_then(|value| value.checked_mul(tensor.dimension as u64))
            .and_then(|value| total.checked_add(value))
    });
    if scalar_values.is_none_or(|value| value > policy.max_scalar_values) {
        let reason = format!(
            "capture scalar count exceeds policy maximum {}",
            policy.max_scalar_values
        );
        let report = refused_report(&metadata, &capture_sha256, &metadata_sha256, policy, reason);
        let sampling = sampling_report(&report);
        let aggregate = aggregate_report(&report);
        return Ok((manifest, report, sampling, aggregate));
    }
    if let Some(tensor) = metadata.tensors.iter().find(|tensor| {
        let dimension = tensor.dimension as u64;
        dimension > policy.max_dimension as u64
            || (policy.partition == PartitionMode::LayerToken
                && dimension.saturating_mul(tensor.heads as u64) > policy.max_dimension as u64)
    }) {
        let reason = format!(
            "tensor {} partition dimension exceeds policy maximum {}",
            tensor.id, policy.max_dimension
        );
        let report = refused_report(&metadata, &capture_sha256, &metadata_sha256, policy, reason);
        let sampling = sampling_report(&report);
        let aggregate = aggregate_report(&report);
        return Ok((manifest, report, sampling, aggregate));
    }
    let groups = build_groups(&capture_bytes, &metadata, &policy)?;
    let vector_count: u64 = groups.iter().map(|group| group.vectors.len() as u64).sum();
    if vector_count > policy.max_vectors {
        let reason = format!(
            "partitioned vector count {vector_count} exceeds policy maximum {}",
            policy.max_vectors
        );
        let report = refused_report(&metadata, &capture_sha256, &metadata_sha256, policy, reason);
        let sampling = sampling_report(&report);
        let aggregate = aggregate_report(&report);
        return Ok((manifest, report, sampling, aggregate));
    }
    let mut metrics = Vec::with_capacity(groups.len());
    for (index, group) in groups.iter().enumerate() {
        metrics.push(profile_group(group, &policy, index as u64)?);
    }
    let status = if metrics
        .iter()
        .any(|group| group.status == ResultStatus::Refused)
    {
        ResultStatus::Refused
    } else if metrics
        .iter()
        .any(|group| group.status == ResultStatus::Approximate)
    {
        ResultStatus::Approximate
    } else if metrics
        .iter()
        .any(|group| group.status == ResultStatus::DeterministicSample)
    {
        ResultStatus::DeterministicSample
    } else {
        ResultStatus::Exact
    };
    let report = GeometryReport {
        schema_version: RESULT_SCHEMA_VERSION,
        tool: "qatq-kv-geometry".into(),
        capture_id: metadata.capture_id.clone(),
        capture_sha256,
        metadata_sha256,
        policy,
        status,
        refusal: None,
        groups: metrics,
        claim_boundary: claim_boundary(),
    };
    let sampling = sampling_report(&report);
    let aggregate = aggregate_report(&report);
    Ok((manifest, report, sampling, aggregate))
}

fn validate_policy(policy: &ProfilePolicy) -> Result<(), GeometryError> {
    if policy.max_pairs == 0
        || policy.exact_vector_threshold == 0
        || policy.max_capture_bytes == 0
        || policy.max_vectors == 0
        || policy.max_scalar_values == 0
        || policy.max_dimension == 0
        || policy.max_spectral_dimension == 0
        || policy.block_vectors == 0
        || policy.chunk_tokens == 0
    {
        return Err(GeometryError::Invalid(
            "all resource bounds must be greater than zero".into(),
        ));
    }
    if policy.max_capture_bytes > HARD_MAX_CAPTURE_BYTES
        || policy.max_scalar_values > HARD_MAX_SCALAR_VALUES
        || policy.max_vectors > HARD_MAX_VECTORS
        || policy.max_dimension > HARD_MAX_DIMENSION
        || policy.max_spectral_dimension > HARD_MAX_DIMENSION
        || policy.max_pairs > HARD_MAX_PAIRS
        || policy.block_vectors > HARD_MAX_BLOCK_VECTORS
    {
        return Err(GeometryError::Policy(
            "requested resource ceiling exceeds the compiled hard limit".into(),
        ));
    }
    if !policy.near_duplicate_cosine.is_finite()
        || !(-1.0..=1.0).contains(&policy.near_duplicate_cosine)
    {
        return Err(GeometryError::Invalid(
            "near duplicate cosine must be finite and within [-1,1]".into(),
        ));
    }
    if policy.thresholds.len() > 32 || policy.thresholds.iter().any(|value| !value.is_finite()) {
        return Err(GeometryError::Invalid(
            "at most 32 finite thresholds are supported".into(),
        ));
    }
    Ok(())
}

fn validate_metadata(metadata: &CaptureMetadata, capture_len: u64) -> Result<(), GeometryError> {
    if metadata.schema_version != CAPTURE_SCHEMA_VERSION {
        return Err(GeometryError::Invalid(format!(
            "unsupported capture schema version {}",
            metadata.schema_version
        )));
    }
    if metadata.capture_id.is_empty()
        || metadata.model.is_empty()
        || metadata.model_family.is_empty()
        || metadata.runtime.is_empty()
        || metadata.prompt_class.is_empty()
        || metadata.tensors.is_empty()
    {
        return Err(GeometryError::Invalid(
            "capture identity, provenance, and tensors must be non-empty".into(),
        ));
    }
    if metadata.prompt_sha256.len() != 64
        || !metadata
            .prompt_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(GeometryError::Invalid(
            "prompt_sha256 must contain 64 hexadecimal characters".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let width = metadata.dtype.width() as u64;
    for tensor in &metadata.tensors {
        if !ids.insert(&tensor.id) {
            return Err(GeometryError::Invalid(format!(
                "duplicate tensor id {}",
                tensor.id
            )));
        }
        if tensor.token_count == 0 || tensor.heads == 0 || tensor.dimension == 0 {
            return Err(GeometryError::Invalid(format!(
                "tensor {} has a zero shape component",
                tensor.id
            )));
        }
        let values = tensor
            .token_count
            .checked_mul(tensor.heads as u64)
            .and_then(|value| value.checked_mul(tensor.dimension as u64))
            .ok_or_else(|| {
                GeometryError::Invalid(format!("tensor {} shape overflows", tensor.id))
            })?;
        let expected = values.checked_mul(width).ok_or_else(|| {
            GeometryError::Invalid(format!("tensor {} bytes overflow", tensor.id))
        })?;
        if expected != tensor.byte_length {
            return Err(GeometryError::Invalid(format!(
                "tensor {} byte length is {}, expected {expected}",
                tensor.id, tensor.byte_length
            )));
        }
        let end = tensor
            .offset_bytes
            .checked_add(tensor.byte_length)
            .ok_or_else(|| {
                GeometryError::Invalid(format!("tensor {} range overflows", tensor.id))
            })?;
        if end > capture_len {
            return Err(GeometryError::Invalid(format!(
                "tensor {} extends beyond capture bytes",
                tensor.id
            )));
        }
    }
    Ok(())
}

fn build_groups(
    bytes: &[u8],
    metadata: &CaptureMetadata,
    policy: &ProfilePolicy,
) -> Result<Vec<VectorGroup>, GeometryError> {
    let mut groups = Vec::new();
    for tensor in &metadata.tensors {
        let dimension = tensor.dimension as usize;
        let heads = tensor.heads as usize;
        if dimension > policy.max_dimension as usize
            || (policy.partition == PartitionMode::LayerToken
                && dimension.saturating_mul(heads) > policy.max_dimension as usize)
        {
            return Err(GeometryError::Policy(format!(
                "tensor {} dimension exceeds policy maximum {}",
                tensor.id, policy.max_dimension
            )));
        }
        let start = tensor.offset_bytes as usize;
        let end = start + tensor.byte_length as usize;
        let values = decode_values(&bytes[start..end], metadata.dtype)?;
        match policy.partition {
            PartitionMode::LayerHeadToken => {
                for head in 0..heads {
                    let vectors = tensor_head_vectors(&values, tensor, head);
                    groups.push(VectorGroup {
                        group_id: format!("{}:head:{head}", tensor.id),
                        tensor_id: tensor.id.clone(),
                        layer: tensor.layer,
                        head: Some(head as u32),
                        token_start: tensor.token_start,
                        token_end_exclusive: tensor.token_start + tensor.token_count,
                        kind: tensor.kind,
                        rope_stage: tensor.rope_stage,
                        dimension,
                        vectors,
                    });
                }
            }
            PartitionMode::LayerToken => {
                let mut vectors = Vec::with_capacity(tensor.token_count as usize);
                for token in 0..tensor.token_count as usize {
                    let base = token * heads * dimension;
                    vectors.push(values[base..base + heads * dimension].to_vec());
                }
                groups.push(VectorGroup {
                    group_id: format!("{}:all-heads", tensor.id),
                    tensor_id: tensor.id.clone(),
                    layer: tensor.layer,
                    head: None,
                    token_start: tensor.token_start,
                    token_end_exclusive: tensor.token_start + tensor.token_count,
                    kind: tensor.kind,
                    rope_stage: tensor.rope_stage,
                    dimension: dimension * heads,
                    vectors,
                });
            }
            PartitionMode::LayerHeadChunk => {
                let chunk = policy.chunk_tokens as usize;
                for head in 0..heads {
                    let all = tensor_head_vectors(&values, tensor, head);
                    for (chunk_index, vectors) in all.chunks(chunk).enumerate() {
                        let local_start = chunk_index * chunk;
                        groups.push(VectorGroup {
                            group_id: format!("{}:head:{head}:chunk:{chunk_index}", tensor.id),
                            tensor_id: tensor.id.clone(),
                            layer: tensor.layer,
                            head: Some(head as u32),
                            token_start: tensor.token_start + local_start as u64,
                            token_end_exclusive: tensor.token_start
                                + (local_start + vectors.len()) as u64,
                            kind: tensor.kind,
                            rope_stage: tensor.rope_stage,
                            dimension,
                            vectors: vectors.to_vec(),
                        });
                    }
                }
            }
        }
    }
    Ok(groups)
}

fn tensor_head_vectors(values: &[f64], tensor: &TensorDescriptor, head: usize) -> Vec<Vec<f64>> {
    let heads = tensor.heads as usize;
    let dimension = tensor.dimension as usize;
    let mut vectors = Vec::with_capacity(tensor.token_count as usize);
    for token in 0..tensor.token_count as usize {
        let base = (token * heads + head) * dimension;
        vectors.push(values[base..base + dimension].to_vec());
    }
    vectors
}

fn decode_values(bytes: &[u8], dtype: DType) -> Result<Vec<f64>, GeometryError> {
    if !bytes.len().is_multiple_of(dtype.width()) {
        return Err(GeometryError::Invalid(
            "tensor byte length is not aligned to dtype".into(),
        ));
    }
    let values = match dtype {
        DType::F16 => bytes
            .chunks_exact(2)
            .map(|chunk| f16::from_bits(u16::from_le_bytes([chunk[0], chunk[1]])).to_f64())
            .collect(),
        DType::Bf16 => bytes
            .chunks_exact(2)
            .map(|chunk| bf16::from_bits(u16::from_le_bytes([chunk[0], chunk[1]])).to_f64())
            .collect(),
        DType::F32 => bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as f64)
            .collect(),
    };
    Ok(values)
}

fn profile_group(
    group: &VectorGroup,
    policy: &ProfilePolicy,
    group_index: u64,
) -> Result<GroupMetrics, GeometryError> {
    let mut norms = Vec::new();
    let mut finite = Vec::new();
    let mut zero_vectors = 0_u64;
    let mut non_finite_vectors = 0_u64;
    for vector in &group.vectors {
        if vector.iter().any(|value| !value.is_finite()) {
            non_finite_vectors += 1;
            continue;
        }
        let norm = vector.iter().map(|value| value * value).sum::<f64>().sqrt();
        norms.push(norm);
        if norm == 0.0 {
            zero_vectors += 1;
            continue;
        }
        finite.push(vector.iter().map(|value| value / norm).collect::<Vec<_>>());
    }
    let normalization_failures = zero_vectors + non_finite_vectors;
    let (exact_duplicate_classes, exact_duplicate_vectors) = duplicate_counts(&finite);
    let population_pairs = pair_count(finite.len() as u64)?;
    let exact = finite.len() as u64 <= policy.exact_vector_threshold
        && population_pairs <= policy.max_pairs;
    let pair_indices = if exact {
        all_pairs(finite.len())
    } else {
        sampled_pairs(
            finite.len(),
            policy.max_pairs.min(population_pairs) as usize,
            policy.seed ^ group_index.wrapping_mul(0x9e37_79b9_7f4a_7c15),
        )
    };
    let pair_status = if exact {
        ResultStatus::Exact
    } else {
        ResultStatus::DeterministicSample
    };
    let pairwise = pair_metrics(
        &finite,
        &pair_indices,
        population_pairs,
        pair_status,
        policy.seed,
        policy.near_duplicate_cosine,
        policy.block_vectors as usize,
    );
    let spectral = spectral_metrics(&finite, policy.max_spectral_dimension as usize);
    let mut binary = Vec::new();
    binary.push(binary_metrics(
        &finite,
        &pair_indices,
        pair_status,
        "sign_bit".into(),
        None,
        policy.block_vectors as usize,
    ));
    for threshold in &policy.thresholds {
        binary.push(binary_metrics(
            &finite,
            &pair_indices,
            pair_status,
            "threshold".into(),
            Some(*threshold),
            policy.block_vectors as usize,
        ));
    }
    let status = if spectral.status == ResultStatus::Refused {
        pair_status
    } else if spectral.status == ResultStatus::Approximate {
        ResultStatus::Approximate
    } else {
        pair_status
    };
    Ok(GroupMetrics {
        group_id: group.group_id.clone(),
        tensor_id: group.tensor_id.clone(),
        layer: group.layer,
        head: group.head,
        token_start: group.token_start,
        token_end_exclusive: group.token_end_exclusive,
        kind: group.kind,
        rope_stage: group.rope_stage,
        dimension: group.dimension as u32,
        vector_count: group.vectors.len() as u64,
        finite_vector_count: finite.len() as u64,
        zero_vectors,
        non_finite_vectors,
        normalization_failures,
        exact_duplicate_classes,
        exact_duplicate_vectors,
        l2_norm: distribution(norms),
        token_position_l2_norm: token_position_metrics(group),
        pairwise,
        spectral,
        binary,
        status,
    })
}

fn pair_count(vectors: u64) -> Result<u64, GeometryError> {
    vectors
        .checked_mul(vectors.saturating_sub(1))
        .map(|value| value / 2)
        .ok_or_else(|| GeometryError::Policy("pair population overflows u64".into()))
}

fn all_pairs(count: usize) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for left in 0..count {
        for right in left + 1..count {
            pairs.push((left, right));
        }
    }
    pairs
}

fn sampled_pairs(count: usize, target: usize, seed: u64) -> Vec<(usize, usize)> {
    if target == 0 || count < 2 {
        return Vec::new();
    }
    let population = count as u64 * (count as u64 - 1) / 2;
    if target as u64 >= population {
        return all_pairs(count);
    }
    let mut selected = BTreeSet::new();
    let mut state = seed;
    let start = population - target as u64;
    for current in start..population {
        let candidate = splitmix64(&mut state) % (current + 1);
        if !selected.insert(candidate) {
            selected.insert(current);
        }
    }
    selected
        .into_iter()
        .map(|index| pair_from_index(count, index))
        .collect()
}

fn pair_from_index(count: usize, mut index: u64) -> (usize, usize) {
    let mut low = 0_usize;
    let mut high = count - 1;
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if pairs_before(count, middle) <= index {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    let left = low;
    index -= pairs_before(count, left);
    (left, left + 1 + index as usize)
}

fn pairs_before(count: usize, left: usize) -> u64 {
    let left = left as u128;
    let count = count as u128;
    (left * (2 * count - left - 1) / 2) as u64
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut value = *state;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn pair_metrics(
    vectors: &[Vec<f64>],
    pairs: &[(usize, usize)],
    population_pairs: u64,
    status: ResultStatus,
    seed: u64,
    near_duplicate_cosine: f64,
    block_pairs: usize,
) -> PairMetrics {
    let mut cosine = Vec::with_capacity(pairs.len());
    let mut angular = Vec::with_capacity(pairs.len());
    let mut nearest = vec![f64::NEG_INFINITY; vectors.len()];
    let mut near_duplicates = 0_u64;
    for block in pairs.chunks(block_pairs) {
        for &(left, right) in block {
            let similarity = dot(&vectors[left], &vectors[right]).clamp(-1.0, 1.0);
            cosine.push(similarity);
            angular.push(similarity.acos());
            nearest[left] = nearest[left].max(similarity);
            nearest[right] = nearest[right].max(similarity);
            if similarity >= near_duplicate_cosine {
                near_duplicates += 1;
            }
        }
    }
    let nearest: Vec<f64> = nearest
        .into_iter()
        .filter(|value| value.is_finite())
        .collect();
    let nearest_angular = nearest.iter().map(|value| value.acos()).collect();
    let evaluated = pairs.len() as u64;
    PairMetrics {
        plan: PairPlan {
            status,
            population_pairs,
            evaluated_pairs: evaluated,
            coverage_fraction: if population_pairs == 0 {
                1.0
            } else {
                evaluated as f64 / population_pairs as f64
            },
            seed,
            mean_standard_error: standard_error(&cosine, population_pairs),
        },
        cosine_similarity: distribution(cosine),
        angular_distance_radians: distribution(angular),
        nearest_neighbor_cosine: distribution(nearest),
        nearest_neighbor_angular_distance_radians: distribution(nearest_angular),
        nearest_neighbor_scope: if status == ResultStatus::Exact {
            "full_population".into()
        } else {
            "sampled_pair_candidates".into()
        },
        near_duplicate_pairs: near_duplicates,
        near_duplicate_rate: if evaluated == 0 {
            None
        } else {
            Some(near_duplicates as f64 / evaluated as f64)
        },
    }
}

fn spectral_metrics(vectors: &[Vec<f64>], maximum_dimension: usize) -> SpectralMetrics {
    if vectors.len() < 2 {
        return SpectralMetrics {
            status: ResultStatus::Refused,
            centered_covariance_trace: None,
            effective_rank_participation_ratio: None,
            top_eigenvalue_fraction_estimate: None,
            power_iterations: 0,
            reason: Some("at least two finite nonzero vectors are required".into()),
        };
    }
    let dimension = vectors[0].len();
    if dimension > maximum_dimension {
        return SpectralMetrics {
            status: ResultStatus::Refused,
            centered_covariance_trace: None,
            effective_rank_participation_ratio: None,
            top_eigenvalue_fraction_estimate: None,
            power_iterations: 0,
            reason: Some(format!(
                "dimension {dimension} exceeds spectral policy maximum {maximum_dimension}"
            )),
        };
    }
    let mut mean = vec![0.0; dimension];
    for vector in vectors {
        for (target, value) in mean.iter_mut().zip(vector) {
            *target += *value;
        }
    }
    for value in &mut mean {
        *value /= vectors.len() as f64;
    }
    let mut covariance = vec![0.0; dimension * dimension];
    for vector in vectors {
        for row in 0..dimension {
            let left = vector[row] - mean[row];
            for column in 0..dimension {
                covariance[row * dimension + column] += left * (vector[column] - mean[column]);
            }
        }
    }
    let scale = 1.0 / vectors.len() as f64;
    for value in &mut covariance {
        *value *= scale;
    }
    let trace: f64 = (0..dimension)
        .map(|index| covariance[index * dimension + index])
        .sum();
    let frobenius_sq: f64 = covariance.iter().map(|value| value * value).sum();
    let participation = if frobenius_sq > 0.0 {
        Some(trace * trace / frobenius_sq)
    } else {
        None
    };
    let iterations = 64;
    let top = power_eigenvalue(&covariance, dimension, iterations);
    SpectralMetrics {
        status: ResultStatus::Approximate,
        centered_covariance_trace: Some(trace),
        effective_rank_participation_ratio: participation,
        top_eigenvalue_fraction_estimate: if trace > 0.0 { Some(top / trace) } else { None },
        power_iterations: iterations,
        reason: Some("top eigenvalue uses deterministic power iteration".into()),
    }
}

fn power_eigenvalue(matrix: &[f64], dimension: usize, iterations: u32) -> f64 {
    let mut vector = vec![1.0 / (dimension as f64).sqrt(); dimension];
    for _ in 0..iterations {
        let mut next = vec![0.0; dimension];
        for row in 0..dimension {
            next[row] = (0..dimension)
                .map(|column| matrix[row * dimension + column] * vector[column])
                .sum();
        }
        let norm = next.iter().map(|value| value * value).sum::<f64>().sqrt();
        if norm == 0.0 {
            return 0.0;
        }
        for value in &mut next {
            *value /= norm;
        }
        vector = next;
    }
    let transformed: Vec<f64> = (0..dimension)
        .map(|row| {
            (0..dimension)
                .map(|column| matrix[row * dimension + column] * vector[column])
                .sum()
        })
        .collect();
    dot(&vector, &transformed)
}

fn binary_metrics(
    vectors: &[Vec<f64>],
    pairs: &[(usize, usize)],
    status: ResultStatus,
    rule: String,
    threshold: Option<f64>,
    block_pairs: usize,
) -> BinaryMetrics {
    let encoded: Vec<Vec<u64>> = vectors
        .iter()
        .map(|vector| encode_binary(vector, threshold))
        .collect();
    let mut distances = Vec::with_capacity(pairs.len());
    for block in pairs.chunks(block_pairs) {
        for &(left, right) in block {
            distances.push(hamming(&encoded[left], &encoded[right]) as f64);
        }
    }
    let mut counts: BTreeMap<Vec<u64>, u64> = BTreeMap::new();
    for bits in encoded {
        *counts.entry(bits).or_default() += 1;
    }
    let collision_classes = counts.values().filter(|&&count| count > 1).count() as u64;
    let colliding_vectors: u64 = counts.values().filter(|&&count| count > 1).sum();
    BinaryMetrics {
        rule,
        threshold,
        status,
        hamming_distance: distribution(distances),
        collision_classes,
        colliding_vectors,
        collision_rate: if vectors.is_empty() {
            None
        } else {
            Some(colliding_vectors as f64 / vectors.len() as f64)
        },
        sensitivity_note: "Descriptive only. This mapping does not establish application-level distinguishability.".into(),
    }
}

fn token_position_metrics(group: &VectorGroup) -> Vec<TokenPositionMetrics> {
    let count = group.vectors.len();
    if count == 0 {
        return Vec::new();
    }
    let names = ["early", "middle", "late"];
    names
        .into_iter()
        .enumerate()
        .filter_map(|(index, name)| {
            let start = index * count / 3;
            let end = (index + 1) * count / 3;
            if start == end {
                return None;
            }
            let norms = group.vectors[start..end]
                .iter()
                .filter(|vector| vector.iter().all(|value| value.is_finite()))
                .map(|vector| vector.iter().map(|value| value * value).sum::<f64>().sqrt())
                .collect();
            Some(TokenPositionMetrics {
                segment: name.into(),
                token_start: group.token_start + start as u64,
                token_end_exclusive: group.token_start + end as u64,
                l2_norm: distribution(norms),
            })
        })
        .collect()
}

fn encode_binary(vector: &[f64], threshold: Option<f64>) -> Vec<u64> {
    let mut words = vec![0_u64; vector.len().div_ceil(64)];
    for (index, value) in vector.iter().enumerate() {
        let bit = match threshold {
            Some(threshold) => *value >= threshold,
            None => value.is_sign_negative(),
        };
        if bit {
            words[index / 64] |= 1_u64 << (index % 64);
        }
    }
    words
}

fn hamming(left: &[u64], right: &[u64]) -> u32 {
    left.iter()
        .zip(right)
        .map(|(left, right)| (left ^ right).count_ones())
        .sum()
}

fn duplicate_counts(vectors: &[Vec<f64>]) -> (u64, u64) {
    let mut counts: BTreeMap<Vec<u64>, u64> = BTreeMap::new();
    for vector in vectors {
        let key = vector.iter().map(|value| value.to_bits()).collect();
        *counts.entry(key).or_default() += 1;
    }
    (
        counts.values().filter(|&&count| count > 1).count() as u64,
        counts.values().filter(|&&count| count > 1).sum(),
    )
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

fn distribution(mut values: Vec<f64>) -> Distribution {
    values.retain(|value| value.is_finite());
    values.sort_by(f64::total_cmp);
    let count = values.len();
    let mean = if count == 0 {
        None
    } else {
        Some(values.iter().sum::<f64>() / count as f64)
    };
    Distribution {
        count: count as u64,
        minimum: percentile(&values, 0.0),
        p01: percentile(&values, 0.01),
        p05: percentile(&values, 0.05),
        p25: percentile(&values, 0.25),
        p50: percentile(&values, 0.50),
        p75: percentile(&values, 0.75),
        p95: percentile(&values, 0.95),
        p99: percentile(&values, 0.99),
        maximum: percentile(&values, 1.0),
        mean,
    }
}

fn percentile(values: &[f64], percentile: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let position = percentile * (values.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        Some(values[lower])
    } else {
        let fraction = position - lower as f64;
        Some(values[lower] * (1.0 - fraction) + values[upper] * fraction)
    }
}

fn standard_error(values: &[f64], population: u64) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values
        .iter()
        .map(|value| (value - mean) * (value - mean))
        .sum::<f64>()
        / (values.len() - 1) as f64;
    let correction = if population > 1 {
        ((population - values.len() as u64) as f64 / (population - 1) as f64)
            .max(0.0)
            .sqrt()
    } else {
        0.0
    };
    Some(variance.sqrt() / (values.len() as f64).sqrt() * correction)
}

fn refused_report(
    metadata: &CaptureMetadata,
    capture_sha256: &str,
    metadata_sha256: &str,
    policy: ProfilePolicy,
    reason: String,
) -> GeometryReport {
    GeometryReport {
        schema_version: RESULT_SCHEMA_VERSION,
        tool: "qatq-kv-geometry".into(),
        capture_id: metadata.capture_id.clone(),
        capture_sha256: capture_sha256.into(),
        metadata_sha256: metadata_sha256.into(),
        policy,
        status: ResultStatus::Refused,
        refusal: Some(Refusal {
            status: ResultStatus::Refused,
            reason,
        }),
        groups: Vec::new(),
        claim_boundary: claim_boundary(),
    }
}

fn claim_boundary() -> Vec<String> {
    vec![
        "Observed capture geometry is not an application-required geometry.".into(),
        "Observed vector count is not a required capacity.".into(),
        "Binary mappings are descriptive and do not prove semantic distinguishability.".into(),
        "This profiler does not emit Capacity Oracle verdicts.".into(),
    ]
}

fn sampling_report(report: &GeometryReport) -> SamplingPlanReport {
    SamplingPlanReport {
        schema_version: RESULT_SCHEMA_VERSION,
        seed: report.policy.seed,
        max_pairs: report.policy.max_pairs,
        exact_vector_threshold: report.policy.exact_vector_threshold,
        block_vectors: report.policy.block_vectors,
        groups: report
            .groups
            .iter()
            .map(|group| PairPlanEntry {
                group_id: group.group_id.clone(),
                plan: group.pairwise.plan.clone(),
            })
            .collect(),
    }
}

fn aggregate_report(report: &GeometryReport) -> AggregateMetrics {
    AggregateMetrics {
        schema_version: RESULT_SCHEMA_VERSION,
        status: report.status,
        group_count: report.groups.len() as u64,
        vector_count: report.groups.iter().map(|group| group.vector_count).sum(),
        finite_vector_count: report
            .groups
            .iter()
            .map(|group| group.finite_vector_count)
            .sum(),
        evaluated_pairs: report
            .groups
            .iter()
            .map(|group| group.pairwise.plan.evaluated_pairs)
            .sum(),
        exact_groups: report
            .groups
            .iter()
            .filter(|group| group.status == ResultStatus::Exact)
            .count() as u64,
        sampled_groups: report
            .groups
            .iter()
            .filter(|group| group.status == ResultStatus::DeterministicSample)
            .count() as u64,
        approximate_groups: report
            .groups
            .iter()
            .filter(|group| group.status == ResultStatus::Approximate)
            .count() as u64,
        refused_groups: report
            .groups
            .iter()
            .filter(|group| group.status == ResultStatus::Refused)
            .count() as u64,
    }
}

pub fn render_summary(report: &GeometryReport) -> String {
    let mut output = String::from("# QATQ KV Geometry Profile\n\n");
    output.push_str(&format!("Status: `{:?}`\n\n", report.status).to_uppercase());
    if let Some(refusal) = &report.refusal {
        output.push_str(&format!("Refusal: {}\n\n", refusal.reason));
    }
    output.push_str("| group | kind | layer | head | vectors | dimension | pair mode | max cosine | effective rank |\n");
    output.push_str("| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |\n");
    for group in &report.groups {
        output.push_str(&format!(
            "| {} | {:?} | {} | {} | {} | {} | {:?} | {} | {} |\n",
            group.group_id,
            group.kind,
            group.layer,
            group
                .head
                .map_or_else(|| "all".into(), |value| value.to_string()),
            group.vector_count,
            group.dimension,
            group.pairwise.plan.status,
            optional(group.pairwise.cosine_similarity.maximum),
            optional(group.spectral.effective_rank_participation_ratio),
        ));
    }
    output.push_str("\n## Claim boundary\n\n");
    for statement in &report.claim_boundary {
        output.push_str(&format!("- {statement}\n"));
    }
    output
}

fn optional(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".into(), |value| format!("{value:.6}"))
}

fn hex_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Serialize)]
struct EvidenceEntry {
    path: String,
    bytes: u64,
    sha256: String,
}

#[derive(Serialize)]
struct EvidenceManifest {
    schema_version: u32,
    files: Vec<EvidenceEntry>,
}

pub fn write_profile_bundle(
    output: &Path,
    capture: &CaptureManifest,
    geometry: &GeometryReport,
    sampling: &SamplingPlanReport,
    metrics: &AggregateMetrics,
) -> Result<(), GeometryError> {
    if output.exists() {
        return Err(GeometryError::Io(format!(
            "refusing to overwrite {}",
            output.display()
        )));
    }
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| {
        GeometryError::Io(format!("failed to create {}: {error}", parent.display()))
    })?;
    let name = output
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| GeometryError::Io("output name must be valid UTF-8".into()))?;
    let temporary = parent.join(format!(".{name}.qatq-kv-geometry-{}", std::process::id()));
    if temporary.exists() {
        return Err(GeometryError::Io(format!(
            "temporary output already exists: {}",
            temporary.display()
        )));
    }
    fs::create_dir(&temporary).map_err(|error| {
        GeometryError::Io(format!("failed to create {}: {error}", temporary.display()))
    })?;
    let result =
        write_bundle_files(&temporary, capture, geometry, sampling, metrics).and_then(|()| {
            fs::rename(&temporary, output).map_err(|error| {
                GeometryError::Io(format!(
                    "failed to publish {} atomically: {error}",
                    output.display()
                ))
            })
        });
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&temporary);
        return Err(error);
    }
    Ok(())
}

fn write_bundle_files(
    directory: &Path,
    capture: &CaptureManifest,
    geometry: &GeometryReport,
    sampling: &SamplingPlanReport,
    metrics: &AggregateMetrics,
) -> Result<(), GeometryError> {
    let files = [
        ("capture-manifest.json", pretty(capture)?),
        ("geometry.json", pretty(geometry)?),
        ("summary.md", render_summary(geometry).into_bytes()),
        ("sampling-plan.json", pretty(sampling)?),
        ("metrics.json", pretty(metrics)?),
    ];
    let mut entries = Vec::new();
    for (name, bytes) in files {
        fs::write(directory.join(name), &bytes)
            .map_err(|error| GeometryError::Io(format!("failed to write {name}: {error}")))?;
        entries.push(EvidenceEntry {
            path: name.into(),
            bytes: bytes.len() as u64,
            sha256: hex_digest(&bytes),
        });
    }
    let manifest = pretty(&EvidenceManifest {
        schema_version: RESULT_SCHEMA_VERSION,
        files: entries,
    })?;
    fs::write(directory.join("manifest.json"), manifest)
        .map_err(|error| GeometryError::Io(format!("failed to write manifest.json: {error}")))
}

fn pretty<T: Serialize>(value: &T) -> Result<Vec<u8>, GeometryError> {
    serde_json::to_vec_pretty(value)
        .map_err(|error| GeometryError::Invalid(format!("serialization failed: {error}")))
}

pub fn parse_partition(value: &str) -> Result<PartitionMode, GeometryError> {
    match value {
        "layer-head-token" => Ok(PartitionMode::LayerHeadToken),
        "layer-token" => Ok(PartitionMode::LayerToken),
        "layer-head-chunk" => Ok(PartitionMode::LayerHeadChunk),
        _ => Err(GeometryError::Invalid(format!(
            "unsupported partition {value}"
        ))),
    }
}

pub fn verify_bundle(path: &Path) -> Result<(), GeometryError> {
    let manifest_path = path.join("manifest.json");
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).map_err(|error| {
            GeometryError::Io(format!(
                "failed to read {}: {error}",
                manifest_path.display()
            ))
        })?)
        .map_err(|error| GeometryError::Invalid(format!("invalid manifest: {error}")))?;
    let files = manifest["files"]
        .as_array()
        .ok_or_else(|| GeometryError::Invalid("manifest files must be an array".into()))?;
    for entry in files {
        let name = entry["path"]
            .as_str()
            .ok_or_else(|| GeometryError::Invalid("manifest path must be a string".into()))?;
        let expected_bytes = entry["bytes"]
            .as_u64()
            .ok_or_else(|| GeometryError::Invalid("manifest bytes must be an integer".into()))?;
        let expected_hash = entry["sha256"]
            .as_str()
            .ok_or_else(|| GeometryError::Invalid("manifest sha256 must be a string".into()))?;
        let file = safe_child(path, name)?;
        let bytes = fs::read(&file).map_err(|error| {
            GeometryError::Io(format!("failed to read {}: {error}", file.display()))
        })?;
        if bytes.len() as u64 != expected_bytes || hex_digest(&bytes) != expected_hash {
            return Err(GeometryError::Invalid(format!(
                "manifest mismatch for {name}"
            )));
        }
    }
    Ok(())
}

fn safe_child(parent: &Path, name: &str) -> Result<PathBuf, GeometryError> {
    let child = Path::new(name);
    if child.components().count() != 1 || child.is_absolute() {
        return Err(GeometryError::Invalid(format!(
            "unsafe manifest path {name}"
        )));
    }
    Ok(parent.join(child))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata() -> CaptureMetadata {
        CaptureMetadata {
            schema_version: 1,
            capture_id: "fixture".into(),
            model: "fixture-model".into(),
            model_family: "fixture-family".into(),
            runtime: "fixture-runtime".into(),
            runtime_version: "1".into(),
            prompt_class: "factual".into(),
            prompt_sha256: "0".repeat(64),
            context_length: 4,
            dtype: DType::F32,
            tensors: vec![TensorDescriptor {
                id: "k-l0".into(),
                offset_bytes: 0,
                byte_length: 32,
                layer: 0,
                kind: KvKind::Key,
                rope_stage: RopeStage::Unknown,
                token_start: 0,
                token_count: 4,
                heads: 1,
                dimension: 2,
                layout: TensorLayout::TokenHeadDimension,
            }],
        }
    }

    fn capture() -> Vec<u8> {
        [[1.0_f32, 0.0], [0.0, 1.0], [-1.0, 0.0], [1.0, 0.0]]
            .into_iter()
            .flatten()
            .flat_map(f32::to_le_bytes)
            .collect()
    }

    #[test]
    fn exact_fixture_reports_geometry_and_binary_collisions() {
        let metadata = metadata();
        let groups = build_groups(&capture(), &metadata, &ProfilePolicy::default()).unwrap();
        let result = profile_group(&groups[0], &ProfilePolicy::default(), 0).unwrap();
        assert_eq!(result.vector_count, 4);
        assert_eq!(result.exact_duplicate_vectors, 2);
        assert_eq!(result.pairwise.plan.status, ResultStatus::Exact);
        assert_eq!(result.pairwise.cosine_similarity.maximum, Some(1.0));
        assert_eq!(result.binary.len(), 2);
    }

    #[test]
    fn sample_is_deterministic_and_unique() {
        let first = sampled_pairs(10_000, 10_000, 42);
        let second = sampled_pairs(10_000, 10_000, 42);
        assert_eq!(first, second);
        assert_eq!(first.len(), 10_000);
        assert_eq!(first.iter().copied().collect::<BTreeSet<_>>().len(), 10_000);
    }

    #[test]
    fn logarithmic_pair_index_matches_lexicographic_pairs() {
        for count in 2..100 {
            for (index, expected) in all_pairs(count).into_iter().enumerate() {
                assert_eq!(pair_from_index(count, index as u64), expected);
            }
        }
    }

    #[test]
    fn metadata_rejects_shape_byte_mismatch() {
        let mut metadata = metadata();
        metadata.tensors[0].byte_length = 31;
        assert!(validate_metadata(&metadata, 32).is_err());
    }

    #[test]
    fn refusal_status_is_not_an_oracle_verdict() {
        let report = refused_report(
            &metadata(),
            &"1".repeat(64),
            &"2".repeat(64),
            ProfilePolicy::default(),
            "policy".into(),
        );
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("REFUSED"));
        let forbidden = ["INFEASIBLE_UNDER_MODEL", "CONSTRUCTED", "UNKNOWN"];
        assert!(forbidden.iter().all(|value| !json.contains(value)));
    }

    #[test]
    fn manifest_paths_are_single_components() {
        assert!(safe_child(Path::new("bundle"), "geometry.json").is_ok());
        assert!(safe_child(Path::new("bundle"), "../capture").is_err());
    }
}
