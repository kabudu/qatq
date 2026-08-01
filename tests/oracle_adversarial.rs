#![cfg(feature = "oracle")]

use qatq::oracle::{
    CertificateCheck, NormalizedModel, check_certificate, check_certificate_json, evaluate,
    recompute_request_digest,
};

fn certificate() -> qatq::oracle::ImpossibilityCertificate {
    let request = br#"{
      "schema_version": 1,
      "request_id": "adversarial",
      "representation": {
        "kind": "binary",
        "dimension": 8,
        "required_states": "29",
        "separation": { "minimum_hamming_distance": 3 }
      },
      "bounds": { "engines": ["finite-binary-hamming"], "require_rigorous_certificate": true }
    }"#;
    evaluate(request).certificate.expect("certificate")
}

#[test]
fn truncated_and_oversized_certificates_fail_closed() {
    let bytes = serde_json::to_vec(&certificate()).unwrap();
    assert!(matches!(
        check_certificate_json(&bytes[..bytes.len() / 2], 1024 * 1024),
        CertificateCheck::Invalid(_)
    ));
    assert!(matches!(
        check_certificate_json(&bytes, bytes.len() - 1),
        CertificateCheck::Invalid(_)
    ));
}

#[test]
fn unsupported_schema_fails_closed() {
    let mut value = serde_json::to_value(certificate()).unwrap();
    value["schema_version"] = serde_json::json!(99);
    assert!(matches!(
        check_certificate_json(&serde_json::to_vec(&value).unwrap(), 1024 * 1024),
        CertificateCheck::Unsupported(_)
    ));
}

#[test]
fn false_decisive_inequality_is_invalid_even_with_rebound_digest() {
    let mut certificate = certificate();
    certificate.required_states = certificate.claimed_upper_bound.clone();
    match &mut certificate.normalized_request.model {
        NormalizedModel::Binary(model) => {
            model.required_states = certificate.claimed_upper_bound.clone();
        }
        NormalizedModel::Spherical(_) => unreachable!(),
    }
    certificate.normalized_request.digest =
        recompute_request_digest(&certificate.normalized_request).unwrap();
    certificate.request_digest = certificate.normalized_request.digest.clone();
    assert!(matches!(
        check_certificate(&certificate),
        CertificateCheck::Invalid(_)
    ));
}

#[test]
fn altered_arithmetic_profile_and_enormous_coefficient_are_invalid() {
    let mut profile = certificate();
    profile.arithmetic.rounding = "nearest".into();
    assert!(matches!(
        check_certificate(&profile),
        CertificateCheck::Invalid(_)
    ));

    let mut value = serde_json::to_value(certificate()).unwrap();
    value["claimed_upper_bound"] = serde_json::Value::String("9".repeat(100_000));
    let bytes = serde_json::to_vec(&value).unwrap();
    assert!(matches!(
        check_certificate_json(&bytes, 1024 * 1024),
        CertificateCheck::Invalid(_)
    ));
}
