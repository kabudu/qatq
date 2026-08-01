use serde::{Deserialize, Serialize};

pub const HARD_MAX_DIMENSION: u32 = 16_384;
pub const HARD_MAX_DEGREE: u32 = 4_096;
pub const HARD_MAX_CERTIFICATE_BYTES: u64 = 64 * 1024 * 1024;
pub const HARD_MAX_MEMORY_BYTES: u64 = 16 * 1024 * 1024 * 1024;
pub const HARD_MAX_SECONDS: u64 = 86_400;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OracleResourceLimits {
    pub maximum_dimension: u32,
    pub maximum_polynomial_degree: u32,
    pub maximum_memory_bytes: u64,
    pub maximum_certificate_bytes: u64,
    pub maximum_input_bytes: u64,
    pub maximum_captured_states: u64,
    pub maximum_pairwise_comparisons: u64,
    pub maximum_interval_subdivisions: u64,
    pub maximum_runtime_seconds: u64,
}

impl Default for OracleResourceLimits {
    fn default() -> Self {
        Self {
            maximum_dimension: 1_024,
            maximum_polynomial_degree: 128,
            maximum_memory_bytes: 1_073_741_824,
            maximum_certificate_bytes: 16_777_216,
            maximum_input_bytes: 1_073_741_824,
            maximum_captured_states: 65_536,
            maximum_pairwise_comparisons: 10_000_000,
            maximum_interval_subdivisions: 1_000_000,
            maximum_runtime_seconds: 60,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceConsumption {
    pub input_bytes: u64,
    pub elapsed_milliseconds: u64,
    pub peak_memory_bytes: u64,
}
