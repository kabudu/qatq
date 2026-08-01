use num_bigint::BigUint;
use serde::{Deserialize, Serialize};

use super::NormalizedRequest;

pub const CERTIFICATE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImpossibilityCertificate {
    pub schema_version: u32,
    pub request_digest: String,
    pub normalized_request: NormalizedRequest,
    pub theorem: TheoremIdentity,
    #[serde(with = "super::model::decimal_biguint")]
    pub claimed_upper_bound: BigUint,
    #[serde(with = "super::model::decimal_biguint")]
    pub required_states: BigUint,
    pub witness: BoundWitness,
    pub arithmetic: ArithmeticProfile,
    pub checker_requirements: CheckerRequirements,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TheoremIdentity {
    BinaryHammingBoundV1,
    SphericalRankinNegativeV1,
    SphericalRankinOrthoplexV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BoundWitness {
    BinaryHamming(BinaryHammingWitness),
    SphericalRankinNegative(SphericalRankinNegativeWitness),
    SphericalRankinOrthoplex,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BinaryHammingWitness {
    pub correction_radius: u32,
    #[serde(with = "super::model::decimal_biguint")]
    pub hamming_ball_volume: BigUint,
    #[serde(with = "super::model::decimal_biguint")]
    pub ambient_space_size: BigUint,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SphericalRankinNegativeWitness {
    #[serde(with = "super::model::decimal_biguint")]
    pub separation_magnitude_numerator: BigUint,
    #[serde(with = "super::model::decimal_biguint")]
    pub separation_denominator: BigUint,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArithmeticProfile {
    pub integer_arithmetic: String,
    pub rounding: String,
    pub floating_point_used: bool,
}

impl Default for ArithmeticProfile {
    fn default() -> Self {
        Self {
            integer_arithmetic: "arbitrary_precision_unsigned".into(),
            rounding: "floor_upper_bound".into(),
            floating_point_used: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckerRequirements {
    pub checker: String,
    pub minimum_checker_schema_version: u32,
    pub complete_domain_check: bool,
}

impl Default for CheckerRequirements {
    fn default() -> Self {
        Self {
            checker: "qatq-oracle".into(),
            minimum_checker_schema_version: 1,
            complete_domain_check: true,
        }
    }
}
