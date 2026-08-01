#![cfg(feature = "oracle")]

use num_bigint::BigUint;
use qatq::oracle::{
    BoundWitness, CertificateCheck, OracleOutcome, check_certificate, check_certificate_json,
    evaluate,
};

fn binary_request(required: &str) -> Vec<u8> {
    format!(
        r#"{{
          "schema_version": 1,
          "request_id": "binary-certificate",
          "representation": {{
            "kind": "binary",
            "dimension": 8,
            "required_states": "{required}",
            "separation": {{ "minimum_hamming_distance": 3 }}
          }},
          "bounds": {{ "engines": ["finite-binary-hamming"], "require_rigorous_certificate": true }}
        }}"#
    )
    .into_bytes()
}

fn spherical_request(dimension: u32, required: &str, separation: &str) -> Vec<u8> {
    format!(
        r#"{{
          "schema_version": 1,
          "request_id": "spherical-certificate",
          "representation": {{
            "kind": "spherical",
            "ambient_dimension": {dimension},
            "required_states": "{required}",
            "normalization": "unit_l2",
            "separation": {{ "maximum_inner_product": "{separation}" }}
          }},
          "bounds": {{ "engines": ["finite-spherical-rankin"], "require_rigorous_certificate": true }}
        }}"#
    )
    .into_bytes()
}

#[test]
fn binary_hamming_certificate_is_independently_valid() {
    let result = evaluate(&binary_request("29"));
    let certificate = result.certificate.expect("certificate");
    assert_eq!(certificate.claimed_upper_bound, BigUint::from(28_u8));
    assert_eq!(check_certificate(&certificate), CertificateCheck::Valid);
}

#[test]
fn equality_with_binary_upper_bound_is_unknown() {
    let result = evaluate(&binary_request("28"));
    assert!(matches!(result.outcome, OracleOutcome::Unknown(_)));
    assert!(result.certificate.is_none());
}

#[test]
fn spherical_orthoplex_and_negative_certificates_are_valid() {
    let orthoplex = evaluate(&spherical_request(64, "129", "0"))
        .certificate
        .expect("orthoplex certificate");
    assert_eq!(orthoplex.claimed_upper_bound, BigUint::from(128_u16));
    assert_eq!(check_certificate(&orthoplex), CertificateCheck::Valid);

    let negative = evaluate(&spherical_request(64, "6", "-0.25"))
        .certificate
        .expect("negative certificate");
    assert_eq!(negative.claimed_upper_bound, BigUint::from(5_u8));
    assert_eq!(check_certificate(&negative), CertificateCheck::Valid);
}

#[test]
fn positive_spherical_separation_remains_unknown() {
    let result = evaluate(&spherical_request(64, "18446744073709551616", "0.25"));
    assert!(matches!(result.outcome, OracleOutcome::Unknown(_)));
    assert!(result.certificate.is_none());
}

#[test]
fn checker_rejects_corrupted_bound_witness_and_digest() {
    let original = evaluate(&binary_request("29"))
        .certificate
        .expect("certificate");

    let mut bad_bound = original.clone();
    bad_bound.claimed_upper_bound += 1_u8;
    assert!(matches!(
        check_certificate(&bad_bound),
        CertificateCheck::Invalid(_)
    ));

    let mut bad_digest = original.clone();
    bad_digest.normalized_request.request_id.push('x');
    assert!(matches!(
        check_certificate(&bad_digest),
        CertificateCheck::Invalid(_)
    ));

    let mut bad_volume = original;
    if let BoundWitness::BinaryHamming(qatq::oracle::BinaryHammingWitness {
        hamming_ball_volume,
        ..
    }) = &mut bad_volume.witness
    {
        *hamming_ball_volume += 1_u8;
    }
    assert!(matches!(
        check_certificate(&bad_volume),
        CertificateCheck::Invalid(_)
    ));
}

#[test]
fn strict_certificate_schema_rejects_unknown_witness_fields() {
    let certificate = evaluate(&binary_request("29"))
        .certificate
        .expect("certificate");
    let mut value = serde_json::to_value(certificate).unwrap();
    value["witness"]["critical_surprise"] = serde_json::json!(true);
    let bytes = serde_json::to_vec(&value).unwrap();
    assert!(matches!(
        check_certificate_json(&bytes, 1024 * 1024),
        CertificateCheck::Invalid(_)
    ));
}

#[test]
fn checker_reports_unknown_theorem_as_unsupported() {
    let certificate = evaluate(&binary_request("29"))
        .certificate
        .expect("certificate");
    let mut value = serde_json::to_value(certificate).unwrap();
    value["theorem"] = serde_json::json!("future-unchecked-theorem-v9");
    let bytes = serde_json::to_vec(&value).unwrap();
    assert!(matches!(
        check_certificate_json(&bytes, 1024 * 1024),
        CertificateCheck::Unsupported(_)
    ));
}
