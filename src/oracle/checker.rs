use num_bigint::BigUint;
use serde::{Deserialize, Serialize};

use super::limits::{
    HARD_MAX_CERTIFICATE_BYTES, HARD_MAX_DEGREE, HARD_MAX_DIMENSION, HARD_MAX_MEMORY_BYTES,
    HARD_MAX_SECONDS,
};
use super::model::ORACLE_SCHEMA_VERSION;
use super::{
    BoundWitness, ImpossibilityCertificate, NormalizedModel, TheoremIdentity,
    certificate::CERTIFICATE_SCHEMA_VERSION, normalize::canonical_bounded_real,
    recompute_request_digest,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    content = "reason",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum CertificateCheck {
    Valid,
    Invalid(String),
    Unsupported(String),
}

pub fn check_certificate_json(bytes: &[u8], maximum_bytes: usize) -> CertificateCheck {
    if bytes.len() > maximum_bytes {
        return CertificateCheck::Invalid(format!(
            "certificate is {} bytes; configured maximum is {maximum_bytes}",
            bytes.len()
        ));
    }
    let envelope: serde_json::Value = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(error) => {
            return CertificateCheck::Invalid(format!("invalid certificate JSON: {error}"));
        }
    };
    match envelope
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
    {
        Some(value) if value == u64::from(CERTIFICATE_SCHEMA_VERSION) => {}
        Some(value) => {
            return CertificateCheck::Unsupported(format!("certificate schema version {value}"));
        }
        None => return CertificateCheck::Invalid("missing certificate schema version".into()),
    }
    match envelope.get("theorem").and_then(serde_json::Value::as_str) {
        Some(
            "binary-hamming-bound-v1"
            | "spherical-rankin-negative-v1"
            | "spherical-rankin-orthoplex-v1",
        ) => {}
        Some(value) => {
            return CertificateCheck::Unsupported(format!("theorem identifier {value}"));
        }
        None => return CertificateCheck::Invalid("missing theorem identifier".into()),
    }
    let certificate: ImpossibilityCertificate = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(error) => {
            return CertificateCheck::Invalid(format!("invalid certificate JSON: {error}"));
        }
    };
    if bytes.len() as u64
        > certificate
            .normalized_request
            .resource_limits
            .maximum_certificate_bytes
    {
        return CertificateCheck::Invalid(
            "certificate exceeds the normalized request's byte limit".into(),
        );
    }
    check_certificate(&certificate)
}

pub fn check_certificate(certificate: &ImpossibilityCertificate) -> CertificateCheck {
    if certificate.schema_version != CERTIFICATE_SCHEMA_VERSION {
        return CertificateCheck::Unsupported(format!(
            "certificate schema version {}",
            certificate.schema_version
        ));
    }
    if certificate
        .checker_requirements
        .minimum_checker_schema_version
        > CERTIFICATE_SCHEMA_VERSION
    {
        return CertificateCheck::Unsupported("checker schema requirement is too new".into());
    }
    if !certificate.checker_requirements.complete_domain_check
        || certificate.arithmetic.floating_point_used
        || certificate.arithmetic.integer_arithmetic != "arbitrary_precision_unsigned"
        || certificate.arithmetic.rounding != "floor_upper_bound"
        || certificate.checker_requirements.checker != "qatq-oracle"
    {
        return CertificateCheck::Invalid(
            "certificate does not require exact complete-domain checking".into(),
        );
    }
    let digest = match recompute_request_digest(&certificate.normalized_request) {
        Ok(value) => value,
        Err(error) => return CertificateCheck::Invalid(error.to_string()),
    };
    if digest != certificate.request_digest || digest != certificate.normalized_request.digest {
        return CertificateCheck::Invalid("normalized request digest mismatch".into());
    }
    if certificate.normalized_request.schema_version != ORACLE_SCHEMA_VERSION
        || certificate.normalized_request.request_id.is_empty()
        || certificate.normalized_request.request_id.len() > 128
        || !certificate
            .normalized_request
            .request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return CertificateCheck::Invalid("invalid normalized request identity".into());
    }
    let engines = &certificate.normalized_request.bounds.engines;
    if !engines.windows(2).all(|pair| pair[0] < pair[1]) {
        return CertificateCheck::Invalid("bound engines are not canonical and unique".into());
    }
    let limits = &certificate.normalized_request.resource_limits;
    if limits.maximum_dimension == 0
        || limits.maximum_dimension > HARD_MAX_DIMENSION
        || limits.maximum_polynomial_degree > HARD_MAX_DEGREE
        || limits.maximum_memory_bytes == 0
        || limits.maximum_memory_bytes > HARD_MAX_MEMORY_BYTES
        || limits.maximum_certificate_bytes == 0
        || limits.maximum_certificate_bytes > HARD_MAX_CERTIFICATE_BYTES
        || limits.maximum_runtime_seconds > HARD_MAX_SECONDS
        || certificate.normalized_request.bounds.maximum_degree > limits.maximum_polynomial_degree
        || certificate.normalized_request.bounds.maximum_seconds > limits.maximum_runtime_seconds
        || certificate
            .normalized_request
            .construction
            .maximum_search_seconds
            > limits.maximum_runtime_seconds
        || limits.maximum_input_bytes == 0
        || limits.maximum_captured_states == 0
        || limits.maximum_pairwise_comparisons == 0
        || limits.maximum_interval_subdivisions == 0
    {
        return CertificateCheck::Invalid(
            "normalized resource policy is outside hard limits".into(),
        );
    }

    let (required_states, computed_upper) = match (
        certificate.theorem,
        &certificate.normalized_request.model,
        &certificate.witness,
    ) {
        (
            TheoremIdentity::BinaryHammingBoundV1,
            NormalizedModel::Binary(super::NormalizedBinaryModel {
                dimension,
                required_states,
                minimum_hamming_distance,
            }),
            BoundWitness::BinaryHamming(super::BinaryHammingWitness {
                correction_radius,
                hamming_ball_volume,
                ambient_space_size,
            }),
        ) => {
            if *dimension == 0
                || *dimension > HARD_MAX_DIMENSION
                || dimension > &limits.maximum_dimension
                || *minimum_hamming_distance == 0
                || minimum_hamming_distance > dimension
            {
                return CertificateCheck::Invalid("binary model is outside checker policy".into());
            }
            if !certificate
                .normalized_request
                .bounds
                .engines
                .iter()
                .any(|engine| engine == "finite-binary-hamming")
            {
                return CertificateCheck::Invalid("binary theorem was not requested".into());
            }
            if engines.iter().any(|engine| {
                !matches!(
                    engine.as_str(),
                    "finite-binary-hamming" | "asymptotic-planning"
                )
            }) {
                return CertificateCheck::Invalid("unsupported binary engine in request".into());
            }
            let expected_radius = (minimum_hamming_distance - 1) / 2;
            if *correction_radius != expected_radius {
                return CertificateCheck::Invalid("incorrect Hamming correction radius".into());
            }
            let expected_volume = hamming_ball_volume_exact(*dimension, expected_radius);
            let expected_ambient = BigUint::from(1_u8) << *dimension;
            if *hamming_ball_volume != expected_volume || *ambient_space_size != expected_ambient {
                return CertificateCheck::Invalid("incorrect Hamming witness arithmetic".into());
            }
            (required_states, expected_ambient / expected_volume)
        }
        (
            TheoremIdentity::SphericalRankinNegativeV1,
            NormalizedModel::Spherical(super::NormalizedSphericalModel {
                ambient_dimension,
                required_states,
                maximum_inner_product,
            }),
            BoundWitness::SphericalRankinNegative(super::SphericalRankinNegativeWitness {
                separation_magnitude_numerator,
                separation_denominator,
            }),
        ) => {
            if *ambient_dimension < 2
                || *ambient_dimension > HARD_MAX_DIMENSION
                || ambient_dimension > &limits.maximum_dimension
            {
                return CertificateCheck::Invalid(
                    "spherical model is outside checker policy".into(),
                );
            }
            if !certificate
                .normalized_request
                .bounds
                .engines
                .iter()
                .any(|engine| engine == "finite-spherical-rankin")
            {
                return CertificateCheck::Invalid("spherical theorem was not requested".into());
            }
            if engines.iter().any(|engine| {
                !matches!(
                    engine.as_str(),
                    "finite-spherical-rankin" | "asymptotic-planning"
                )
            }) {
                return CertificateCheck::Invalid("unsupported spherical engine in request".into());
            }
            if canonical_bounded_real(maximum_inner_product).as_ref() != Ok(maximum_inner_product) {
                return CertificateCheck::Invalid("noncanonical spherical separation".into());
            }
            let Some((numerator, denominator)) = negative_decimal_ratio(maximum_inner_product)
            else {
                return CertificateCheck::Invalid("Rankin-negative theorem requires s < 0".into());
            };
            if *separation_magnitude_numerator != numerator
                || *separation_denominator != denominator
            {
                return CertificateCheck::Invalid("incorrect exact separation ratio".into());
            }
            let upper = BigUint::from(1_u8) + (&denominator / &numerator);
            (required_states, upper)
        }
        (
            TheoremIdentity::SphericalRankinOrthoplexV1,
            NormalizedModel::Spherical(super::NormalizedSphericalModel {
                ambient_dimension,
                required_states,
                maximum_inner_product,
            }),
            BoundWitness::SphericalRankinOrthoplex,
        ) => {
            if *ambient_dimension < 2
                || *ambient_dimension > HARD_MAX_DIMENSION
                || ambient_dimension > &limits.maximum_dimension
            {
                return CertificateCheck::Invalid(
                    "spherical model is outside checker policy".into(),
                );
            }
            if !certificate
                .normalized_request
                .bounds
                .engines
                .iter()
                .any(|engine| engine == "finite-spherical-rankin")
            {
                return CertificateCheck::Invalid("spherical theorem was not requested".into());
            }
            if engines.iter().any(|engine| {
                !matches!(
                    engine.as_str(),
                    "finite-spherical-rankin" | "asymptotic-planning"
                )
            }) {
                return CertificateCheck::Invalid("unsupported spherical engine in request".into());
            }
            if maximum_inner_product != "0" {
                return CertificateCheck::Invalid("orthoplex theorem requires s = 0".into());
            }
            (required_states, BigUint::from(*ambient_dimension) * 2_u8)
        }
        _ => return CertificateCheck::Invalid("theorem, model, and witness do not match".into()),
    };

    if computed_upper != certificate.claimed_upper_bound {
        return CertificateCheck::Invalid("claimed upper bound is false".into());
    }
    if required_states != &certificate.required_states {
        return CertificateCheck::Invalid("required state count mismatch".into());
    }
    if required_states <= &computed_upper {
        return CertificateCheck::Invalid("decisive inequality is false".into());
    }
    CertificateCheck::Valid
}

pub(crate) fn hamming_ball_volume_exact(dimension: u32, radius: u32) -> BigUint {
    let mut coefficient = BigUint::from(1_u8);
    let mut volume = coefficient.clone();
    for index in 1..=radius {
        coefficient *= dimension - index + 1;
        coefficient /= index;
        volume += &coefficient;
    }
    volume
}

pub(crate) fn negative_decimal_ratio(value: &str) -> Option<(BigUint, BigUint)> {
    let unsigned = value.strip_prefix('-')?;
    if unsigned == "0" {
        return None;
    }
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let digits = format!("{whole}{fraction}");
    let numerator = BigUint::parse_bytes(digits.as_bytes(), 10)?;
    let denominator = BigUint::from(10_u8).pow(fraction.len().try_into().ok()?);
    Some((numerator, denominator))
}
