use num_bigint::BigUint;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use super::OracleResourceLimits;

pub const ORACLE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleRequest {
    pub schema_version: u32,
    pub request_id: String,
    pub representation: RepresentationModel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageRequest>,
    #[serde(default)]
    pub construction: ConstructionRequest,
    #[serde(default)]
    pub bounds: BoundRequest,
    #[serde(default)]
    pub resources: OracleResourceLimits,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RepresentationModel {
    Binary(BinaryRepresentation),
    Spherical(SphericalRepresentation),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BinaryRepresentation {
    pub dimension: u32,
    #[serde(with = "decimal_biguint")]
    pub required_states: BigUint,
    pub separation: BinarySeparation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SphericalRepresentation {
    pub ambient_dimension: u32,
    #[serde(with = "decimal_biguint")]
    pub required_states: BigUint,
    pub normalization: SphericalNormalization,
    pub separation: SphericalSeparation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BinaryCodeModel {
    pub dimension: u32,
    pub required_states: BigUint,
    pub minimum_hamming_distance: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SphericalCodeModel {
    pub ambient_dimension: u32,
    pub required_states: BigUint,
    pub maximum_inner_product: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BinarySeparation {
    pub minimum_hamming_distance: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SphericalSeparation {
    pub maximum_inner_product: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SphericalNormalization {
    UnitL2,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageRequest {
    pub maximum_bits_per_state: u64,
    pub dtype: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConstructionRequest {
    pub enabled: bool,
    pub input_artifact: Option<String>,
    pub codec_candidates: Vec<String>,
    pub maximum_search_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BoundRequest {
    pub engines: Vec<String>,
    pub maximum_degree: u32,
    pub maximum_seconds: u64,
    pub require_rigorous_certificate: bool,
}

impl Default for BoundRequest {
    fn default() -> Self {
        Self {
            engines: Vec::new(),
            maximum_degree: 0,
            maximum_seconds: 0,
            require_rigorous_certificate: true,
        }
    }
}

pub(crate) mod decimal_biguint {
    use super::*;

    pub fn serialize<S>(value: &BigUint, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_str_radix(10))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<BigUint, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value.is_empty()
            || !value.bytes().all(|byte| byte.is_ascii_digit())
            || (value.len() > 1 && value.starts_with('0'))
        {
            return Err(de::Error::custom(
                "required_states must be a canonical unsigned decimal string",
            ));
        }
        BigUint::parse_bytes(value.as_bytes(), 10)
            .ok_or_else(|| de::Error::custom("invalid required_states"))
    }
}
