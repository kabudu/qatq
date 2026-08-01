#![cfg(feature = "oracle")]

use num_bigint::BigUint;
use qatq::oracle::evaluate;

fn certified_upper(dimension: u32, distance: u32) -> BigUint {
    let request = format!(
        r#"{{
          "schema_version": 1,
          "request_id": "binary-conformance-{dimension}-{distance}",
          "representation": {{
            "kind": "binary",
            "dimension": {dimension},
            "required_states": "100000000000000000000000000000000000000000000000000000000000000000000000000000000",
            "separation": {{ "minimum_hamming_distance": {distance} }}
          }},
          "bounds": {{ "engines": ["finite-binary-hamming"], "require_rigorous_certificate": true }}
        }}"#
    );
    evaluate(request.as_bytes())
        .certificate
        .expect("decisive certificate")
        .claimed_upper_bound
}

#[test]
fn reproduces_phase_zero_exact_hamming_rows() {
    for (dimension, distance, expected) in [
        (64, 8, "421688057462785"),
        (64, 16, "26184380591"),
        (64, 24, "19883522"),
        (128, 16, "3395184828163349608402497490"),
        (128, 32, "22396032652922403638"),
        (128, 48, "19391329499178"),
        (
            256,
            32,
            "162372326214067081141448504628655429554456941526568067",
        ),
        (256, 64, "12071934357881141822097984615890981597"),
        (256, 96, "13683528021137272041170604"),
    ] {
        assert_eq!(
            certified_upper(dimension, distance),
            BigUint::parse_bytes(expected.as_bytes(), 10).unwrap()
        );
    }
}

#[test]
fn hamming_upper_bound_is_nonincreasing_with_distance() {
    let values: Vec<_> = [8, 16, 24, 32]
        .into_iter()
        .map(|distance| certified_upper(64, distance))
        .collect();
    assert!(values.windows(2).all(|pair| pair[0] >= pair[1]));
}
