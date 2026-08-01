use std::{fmt, time::Instant};

use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    ArithmeticProfile, BinaryCodeModel, BinaryHammingWitness, BoundWitness, CertificateCheck,
    CheckerRequirements, ImpossibilityCertificate, InfeasibilityReport, OracleOutcome,
    OracleRequest, OutcomeMetadata, RefusalReport, RepresentationModel, ResourceConsumption,
    SphericalCodeModel, SphericalRankinNegativeWitness, TheoremIdentity, UnknownReport,
    certificate::CERTIFICATE_SCHEMA_VERSION,
    checker::{check_certificate, hamming_ball_volume_exact, negative_decimal_ratio},
    limits::{
        HARD_MAX_CERTIFICATE_BYTES, HARD_MAX_DEGREE, HARD_MAX_DIMENSION, HARD_MAX_MEMORY_BYTES,
        HARD_MAX_SECONDS,
    },
    model::ORACLE_SCHEMA_VERSION,
};

const MAX_REQUEST_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NormalizedModel {
    Binary(NormalizedBinaryModel),
    Spherical(NormalizedSphericalModel),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedBinaryModel {
    pub dimension: u32,
    #[serde(with = "super::model::decimal_biguint")]
    pub required_states: BigUint,
    pub minimum_hamming_distance: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedSphericalModel {
    pub ambient_dimension: u32,
    #[serde(with = "super::model::decimal_biguint")]
    pub required_states: BigUint,
    pub maximum_inner_product: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedRequest {
    pub schema_version: u32,
    pub request_id: String,
    pub model: NormalizedModel,
    pub storage: Option<super::model::StorageRequest>,
    pub construction: super::ConstructionRequest,
    pub bounds: super::BoundRequest,
    pub resource_limits: super::OracleResourceLimits,
    pub digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OracleError {
    Malformed(String),
    Unsupported(String),
    ResourceLimit(String),
}

impl fmt::Display for OracleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(reason) => write!(f, "malformed request: {reason}"),
            Self::Unsupported(reason) => write!(f, "unsupported request: {reason}"),
            Self::ResourceLimit(reason) => write!(f, "resource limit: {reason}"),
        }
    }
}

impl std::error::Error for OracleError {}

pub fn parse_request(bytes: &[u8]) -> Result<OracleRequest, OracleError> {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(OracleError::ResourceLimit(format!(
            "request is {} bytes; maximum is {MAX_REQUEST_BYTES}",
            bytes.len()
        )));
    }
    serde_json::from_slice(bytes).map_err(|error| OracleError::Malformed(error.to_string()))
}

pub fn normalize_request(request: OracleRequest) -> Result<NormalizedRequest, OracleError> {
    if request.schema_version != ORACLE_SCHEMA_VERSION {
        return Err(OracleError::Unsupported(format!(
            "schema version {}; supported version is {ORACLE_SCHEMA_VERSION}",
            request.schema_version
        )));
    }
    validate_request_id(&request.request_id)?;
    validate_limits(&request)?;

    let model = match request.representation {
        RepresentationModel::Binary(super::model::BinaryRepresentation {
            dimension,
            required_states,
            separation,
        }) => {
            if dimension == 0 || dimension > request.resources.maximum_dimension {
                return Err(OracleError::ResourceLimit(format!(
                    "binary dimension {dimension} is outside 1..={}",
                    request.resources.maximum_dimension
                )));
            }
            if separation.minimum_hamming_distance == 0
                || separation.minimum_hamming_distance > dimension
            {
                return Err(OracleError::Malformed(
                    "minimum_hamming_distance must be in 1..=dimension".into(),
                ));
            }
            require_positive(&required_states)?;
            let model = BinaryCodeModel {
                dimension,
                required_states,
                minimum_hamming_distance: separation.minimum_hamming_distance,
            };
            NormalizedModel::Binary(NormalizedBinaryModel {
                dimension: model.dimension,
                required_states: model.required_states,
                minimum_hamming_distance: model.minimum_hamming_distance,
            })
        }
        RepresentationModel::Spherical(super::model::SphericalRepresentation {
            ambient_dimension,
            required_states,
            normalization: _,
            separation,
        }) => {
            if ambient_dimension < 2 || ambient_dimension > request.resources.maximum_dimension {
                return Err(OracleError::ResourceLimit(format!(
                    "spherical dimension {ambient_dimension} is outside 2..={}",
                    request.resources.maximum_dimension
                )));
            }
            require_positive(&required_states)?;
            let maximum_inner_product = canonical_bounded_real(&separation.maximum_inner_product)?;
            let model = SphericalCodeModel {
                ambient_dimension,
                required_states,
                maximum_inner_product,
            };
            NormalizedModel::Spherical(NormalizedSphericalModel {
                ambient_dimension: model.ambient_dimension,
                required_states: model.required_states,
                maximum_inner_product: model.maximum_inner_product,
            })
        }
    };

    let mut bounds = request.bounds;
    bounds.engines.sort();
    if bounds.engines.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(OracleError::Malformed("duplicate bound engine".into()));
    }
    if let Some(storage) = &request.storage
        && (!matches!(storage.dtype.as_str(), "f32" | "f16" | "bf16")
            || storage.maximum_bits_per_state == 0)
    {
        return Err(OracleError::Malformed(
            "storage dtype must be f32, f16, or bf16 and its bit limit must be positive".into(),
        ));
    }
    let mut normalized = NormalizedRequest {
        schema_version: request.schema_version,
        request_id: request.request_id,
        model,
        storage: request.storage,
        construction: request.construction,
        bounds,
        resource_limits: request.resources,
        digest: String::new(),
    };
    let canonical = serde_json::to_vec(&normalized)
        .map_err(|error| OracleError::Malformed(error.to_string()))?;
    normalized.digest = format!("{:x}", Sha256::digest(canonical));
    Ok(normalized)
}

#[derive(Clone, Debug)]
pub struct EvaluationResult {
    pub normalized_request: Option<NormalizedRequest>,
    pub outcome: OracleOutcome,
    pub certificate: Option<ImpossibilityCertificate>,
}

pub fn evaluate_json(bytes: &[u8]) -> OracleOutcome {
    evaluate(bytes).outcome
}

pub fn evaluate(bytes: &[u8]) -> EvaluationResult {
    let started = Instant::now();
    match parse_request(bytes).and_then(normalize_request) {
        Ok(normalized) => evaluate_normalized(normalized, bytes.len(), started),
        Err(error) => EvaluationResult {
            normalized_request: None,
            outcome: OracleOutcome::Refused(RefusalReport {
                metadata: metadata(
                    None,
                    bytes.len(),
                    started,
                    "ORACLE_REQUEST_REFUSED",
                    Vec::new(),
                ),
                reason: error.to_string(),
            }),
            certificate: None,
        },
    }
}

fn evaluate_normalized(
    normalized: NormalizedRequest,
    input_bytes: usize,
    started: Instant,
) -> EvaluationResult {
    let bound = produce_bound(&normalized);
    match bound {
        Ok(Some(certificate)) if certificate.required_states > certificate.claimed_upper_bound => {
            let certificate_bytes = match serde_json::to_vec(&certificate) {
                Ok(value) => value.len() as u64,
                Err(error) => {
                    return refused_result(
                        normalized,
                        input_bytes,
                        started,
                        OracleError::Malformed(error.to_string()),
                    );
                }
            };
            if certificate_bytes > normalized.resource_limits.maximum_certificate_bytes {
                let maximum_certificate_bytes =
                    normalized.resource_limits.maximum_certificate_bytes;
                return refused_result(
                    normalized,
                    input_bytes,
                    started,
                    OracleError::ResourceLimit(format!(
                        "certificate is {certificate_bytes} bytes; configured maximum is {}",
                        maximum_certificate_bytes
                    )),
                );
            }
            if let check @ (CertificateCheck::Invalid(_) | CertificateCheck::Unsupported(_)) =
                check_certificate(&certificate)
            {
                return refused_result(
                    normalized,
                    input_bytes,
                    started,
                    OracleError::Unsupported(format!(
                        "generated certificate failed independent checking: {check:?}"
                    )),
                );
            }
            let outcome = OracleOutcome::InfeasibleUnderModel(Box::new(InfeasibilityReport {
                metadata: metadata(
                    Some(normalized.digest.clone()),
                    input_bytes,
                    started,
                    "ORACLE_FINITE_BOUND_EXCEEDED",
                    Vec::new(),
                ),
                certificate: certificate.clone(),
            }));
            EvaluationResult {
                normalized_request: Some(normalized),
                outcome,
                certificate: Some(certificate),
            }
        }
        Ok(_) if normalized.construction.enabled => refused_result(
            normalized,
            input_bytes,
            started,
            OracleError::Unsupported("construction search in this release".into()),
        ),
        Ok(_) => EvaluationResult {
            normalized_request: Some(normalized.clone()),
            outcome: OracleOutcome::Unknown(UnknownReport {
                metadata: metadata(
                    Some(normalized.digest),
                    input_bytes,
                    started,
                    "ORACLE_FINITE_BOUNDS_INCONCLUSIVE",
                    Vec::new(),
                ),
                reason: "applicable finite bounds do not rule out the required state count".into(),
            }),
            certificate: None,
        },
        Err(error) => refused_result(normalized, input_bytes, started, error),
    }
}

fn refused_result(
    normalized: NormalizedRequest,
    input_bytes: usize,
    started: Instant,
    error: OracleError,
) -> EvaluationResult {
    EvaluationResult {
        normalized_request: Some(normalized.clone()),
        outcome: OracleOutcome::Refused(RefusalReport {
            metadata: metadata(
                Some(normalized.digest),
                input_bytes,
                started,
                "ORACLE_ENGINE_REFUSED",
                Vec::new(),
            ),
            reason: error.to_string(),
        }),
        certificate: None,
    }
}

fn produce_bound(
    normalized: &NormalizedRequest,
) -> Result<Option<ImpossibilityCertificate>, OracleError> {
    match &normalized.model {
        NormalizedModel::Binary(NormalizedBinaryModel {
            dimension,
            required_states,
            minimum_hamming_distance,
        }) => {
            if !normalized
                .bounds
                .engines
                .iter()
                .any(|value| value == "finite-binary-hamming")
            {
                return Ok(None);
            }
            reject_unknown_engines(
                normalized,
                &["finite-binary-hamming", "asymptotic-planning"],
            )?;
            let radius = (minimum_hamming_distance - 1) / 2;
            let volume = hamming_ball_volume_exact(*dimension, radius);
            let ambient = BigUint::from(1_u8) << *dimension;
            let upper = &ambient / &volume;
            Ok(Some(certificate(
                normalized,
                required_states.clone(),
                upper,
                TheoremIdentity::BinaryHammingBoundV1,
                BoundWitness::BinaryHamming(BinaryHammingWitness {
                    correction_radius: radius,
                    hamming_ball_volume: volume,
                    ambient_space_size: ambient,
                }),
            )))
        }
        NormalizedModel::Spherical(NormalizedSphericalModel {
            ambient_dimension,
            required_states,
            maximum_inner_product,
        }) => {
            if !normalized
                .bounds
                .engines
                .iter()
                .any(|value| value == "finite-spherical-rankin")
            {
                return Ok(None);
            }
            reject_unknown_engines(
                normalized,
                &["finite-spherical-rankin", "asymptotic-planning"],
            )?;
            if maximum_inner_product == "0" {
                return Ok(Some(certificate(
                    normalized,
                    required_states.clone(),
                    BigUint::from(*ambient_dimension) * 2_u8,
                    TheoremIdentity::SphericalRankinOrthoplexV1,
                    BoundWitness::SphericalRankinOrthoplex,
                )));
            }
            let Some((numerator, denominator)) = negative_decimal_ratio(maximum_inner_product)
            else {
                return Ok(None);
            };
            let upper = BigUint::from(1_u8) + (&denominator / &numerator);
            Ok(Some(certificate(
                normalized,
                required_states.clone(),
                upper,
                TheoremIdentity::SphericalRankinNegativeV1,
                BoundWitness::SphericalRankinNegative(SphericalRankinNegativeWitness {
                    separation_magnitude_numerator: numerator,
                    separation_denominator: denominator,
                }),
            )))
        }
    }
}

fn certificate(
    normalized: &NormalizedRequest,
    required_states: BigUint,
    claimed_upper_bound: BigUint,
    theorem: TheoremIdentity,
    witness: BoundWitness,
) -> ImpossibilityCertificate {
    ImpossibilityCertificate {
        schema_version: CERTIFICATE_SCHEMA_VERSION,
        request_digest: normalized.digest.clone(),
        normalized_request: normalized.clone(),
        theorem,
        claimed_upper_bound,
        required_states,
        witness,
        arithmetic: ArithmeticProfile::default(),
        checker_requirements: CheckerRequirements::default(),
    }
}

fn reject_unknown_engines(
    normalized: &NormalizedRequest,
    supported: &[&str],
) -> Result<(), OracleError> {
    if let Some(engine) = normalized
        .bounds
        .engines
        .iter()
        .find(|engine| !supported.contains(&engine.as_str()))
    {
        return Err(OracleError::Unsupported(format!("bound engine {engine}")));
    }
    Ok(())
}

pub fn recompute_request_digest(normalized: &NormalizedRequest) -> Result<String, OracleError> {
    let mut unsigned = normalized.clone();
    unsigned.digest.clear();
    let canonical =
        serde_json::to_vec(&unsigned).map_err(|error| OracleError::Malformed(error.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(canonical)))
}

fn metadata(
    digest: Option<String>,
    input_bytes: usize,
    started: Instant,
    code: &str,
    warnings: Vec<String>,
) -> OutcomeMetadata {
    OutcomeMetadata {
        normalized_request_digest: digest,
        tool_version: env!("CARGO_PKG_VERSION").into(),
        schema_version: ORACLE_SCHEMA_VERSION,
        resources: ResourceConsumption {
            input_bytes: input_bytes as u64,
            elapsed_milliseconds: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
            peak_memory_bytes: 0,
        },
        assumptions: Vec::new(),
        supported_scope: vec!["binary_code_model".into(), "spherical_code_model".into()],
        warnings,
        outcome_code: code.into(),
    }
}

fn validate_request_id(value: &str) -> Result<(), OracleError> {
    if value.is_empty() || value.len() > 128 {
        return Err(OracleError::Malformed(
            "request_id must contain 1..=128 bytes".into(),
        ));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(OracleError::Malformed(
            "request_id may contain only ASCII letters, digits, '-', '_', and '.'".into(),
        ));
    }
    Ok(())
}

fn validate_limits(request: &OracleRequest) -> Result<(), OracleError> {
    let limits = &request.resources;
    if limits.maximum_dimension == 0 || limits.maximum_dimension > HARD_MAX_DIMENSION {
        return Err(OracleError::ResourceLimit(format!(
            "maximum_dimension must be in 1..={HARD_MAX_DIMENSION}"
        )));
    }
    if limits.maximum_polynomial_degree > HARD_MAX_DEGREE {
        return Err(OracleError::ResourceLimit(format!(
            "maximum_polynomial_degree exceeds {HARD_MAX_DEGREE}"
        )));
    }
    if request.bounds.maximum_degree > limits.maximum_polynomial_degree {
        return Err(OracleError::ResourceLimit(
            "bounds.maximum_degree exceeds resources.maximum_polynomial_degree".into(),
        ));
    }
    if limits.maximum_memory_bytes == 0 || limits.maximum_memory_bytes > HARD_MAX_MEMORY_BYTES {
        return Err(OracleError::ResourceLimit(
            "maximum_memory_bytes is outside policy".into(),
        ));
    }
    if limits.maximum_certificate_bytes == 0
        || limits.maximum_certificate_bytes > HARD_MAX_CERTIFICATE_BYTES
    {
        return Err(OracleError::ResourceLimit(
            "maximum_certificate_bytes is outside policy".into(),
        ));
    }
    if limits.maximum_runtime_seconds > HARD_MAX_SECONDS
        || request.bounds.maximum_seconds > limits.maximum_runtime_seconds
        || request.construction.maximum_search_seconds > limits.maximum_runtime_seconds
    {
        return Err(OracleError::ResourceLimit(
            "runtime seconds exceed policy".into(),
        ));
    }
    if limits.maximum_input_bytes == 0
        || limits.maximum_captured_states == 0
        || limits.maximum_pairwise_comparisons == 0
        || limits.maximum_interval_subdivisions == 0
    {
        return Err(OracleError::ResourceLimit(
            "input, capture, comparison, and subdivision limits must be positive".into(),
        ));
    }
    Ok(())
}

fn require_positive(value: &BigUint) -> Result<(), OracleError> {
    if value == &BigUint::from(0_u8) {
        return Err(OracleError::Malformed(
            "required_states must be positive".into(),
        ));
    }
    Ok(())
}

pub(crate) fn canonical_bounded_real(value: &str) -> Result<String, OracleError> {
    if value.is_empty() || value.len() > 64 || value.contains(['e', 'E', '+']) {
        return Err(OracleError::Malformed(
            "maximum_inner_product must be a short decimal string without exponent notation".into(),
        ));
    }
    let negative = value.starts_with('-');
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(OracleError::Malformed(
            "invalid maximum_inner_product".into(),
        ));
    }
    let whole = whole.trim_start_matches('0');
    let whole = if whole.is_empty() { "0" } else { whole };
    let fraction = fraction.trim_end_matches('0');
    let mut canonical = if fraction.is_empty() {
        whole.to_string()
    } else {
        format!("{whole}.{fraction}")
    };
    if negative && canonical != "0" {
        canonical.insert(0, '-');
    }
    let magnitude = canonical.strip_prefix('-').unwrap_or(&canonical);
    let outside_range = if canonical.starts_with('-') {
        magnitude != "0" && magnitude != "1" && !magnitude.starts_with("0.")
    } else {
        magnitude != "0" && !magnitude.starts_with("0.")
    };
    if outside_range {
        return Err(OracleError::Malformed(
            "maximum_inner_product must be in [-1,1)".into(),
        ));
    }
    Ok(canonical)
}
