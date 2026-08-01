#![cfg(feature = "oracle")]

use qatq::oracle::{OracleOutcome, evaluate_json, normalize_request, parse_request};

const VALID_BINARY: &str = r#"{
  "schema_version": 1,
  "request_id": "binary-test",
  "representation": {
    "kind": "binary",
    "dimension": 128,
    "required_states": "281474976710656",
    "separation": { "minimum_hamming_distance": 48 }
  },
  "construction": { "enabled": false },
  "bounds": { "engines": ["finite-binary-hamming"], "maximum_degree": 0, "maximum_seconds": 0, "require_rigorous_certificate": true },
  "resources": { "maximum_dimension": 1024, "maximum_polynomial_degree": 128, "maximum_memory_bytes": 1073741824, "maximum_certificate_bytes": 16777216, "maximum_input_bytes": 1073741824, "maximum_captured_states": 65536, "maximum_pairwise_comparisons": 10000000, "maximum_interval_subdivisions": 1000000, "maximum_runtime_seconds": 60 }
}"#;

#[test]
fn valid_request_normalizes_deterministically() {
    let first = normalize_request(parse_request(VALID_BINARY.as_bytes()).unwrap()).unwrap();
    let second = normalize_request(parse_request(VALID_BINARY.as_bytes()).unwrap()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.digest.len(), 64);
}

#[test]
fn finite_binary_request_is_certified_infeasible() {
    let outcome = evaluate_json(VALID_BINARY.as_bytes());
    assert!(matches!(outcome, OracleOutcome::InfeasibleUnderModel(_)));
    assert_eq!(outcome.exit_code(), 1);
}

#[test]
fn unknown_field_is_refused() {
    let changed = VALID_BINARY.replace(
        "\"request_id\": \"binary-test\"",
        "\"request_id\": \"binary-test\", \"critical_surprise\": true",
    );
    assert!(matches!(
        evaluate_json(changed.as_bytes()),
        OracleOutcome::Refused(_)
    ));
}

#[test]
fn json_number_state_count_is_refused() {
    let changed = VALID_BINARY.replace("\"281474976710656\"", "281474976710656");
    assert!(matches!(
        evaluate_json(changed.as_bytes()),
        OracleOutcome::Refused(_)
    ));
}

#[test]
fn leading_zero_state_count_is_refused() {
    let changed = VALID_BINARY.replace("\"281474976710656\"", "\"0281474976710656\"");
    assert!(matches!(
        evaluate_json(changed.as_bytes()),
        OracleOutcome::Refused(_)
    ));
}

#[test]
fn invalid_distance_and_resource_excess_are_refused() {
    let bad_distance = VALID_BINARY.replace(
        "\"minimum_hamming_distance\": 48",
        "\"minimum_hamming_distance\": 129",
    );
    assert!(matches!(
        evaluate_json(bad_distance.as_bytes()),
        OracleOutcome::Refused(_)
    ));

    let bad_limit = VALID_BINARY.replace(
        "\"maximum_dimension\": 1024",
        "\"maximum_dimension\": 999999",
    );
    assert!(matches!(
        evaluate_json(bad_limit.as_bytes()),
        OracleOutcome::Refused(_)
    ));
}

#[test]
fn spherical_decimal_is_canonicalized_before_hashing() {
    let base = include_str!("../examples/oracle/spherical-128-s0-16bit.json");
    let first = normalize_request(parse_request(base.as_bytes()).unwrap()).unwrap();
    let alternate = base.replace(
        "\"maximum_inner_product\": \"0\"",
        "\"maximum_inner_product\": \"00.000\"",
    );
    let second = normalize_request(parse_request(alternate.as_bytes()).unwrap()).unwrap();
    assert_eq!(first.digest, second.digest);
}

#[test]
fn example_request_has_golden_normalized_digest() {
    let source = include_str!("../examples/oracle/binary-128-d48-48bit.json");
    let normalized = normalize_request(parse_request(source.as_bytes()).unwrap()).unwrap();
    assert_eq!(
        normalized.digest,
        "bf07d33d87446d29c461e6bcbbad64a9bbb94675268c7f142cf00f7fc04546f9"
    );
}

#[test]
fn duplicate_known_field_is_refused() {
    let changed = VALID_BINARY.replace(
        "\"schema_version\": 1",
        "\"schema_version\": 1, \"schema_version\": 1",
    );
    assert!(matches!(
        evaluate_json(changed.as_bytes()),
        OracleOutcome::Refused(_)
    ));
}
