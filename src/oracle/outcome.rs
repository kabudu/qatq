use serde::{Deserialize, Serialize};

use super::{ImpossibilityCertificate, ResourceConsumption};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OracleOutcome {
    Constructed(ConstructionCertificate),
    InfeasibleUnderModel(Box<InfeasibilityReport>),
    Unknown(UnknownReport),
    Refused(RefusalReport),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InfeasibilityReport {
    #[serde(flatten)]
    pub metadata: OutcomeMetadata,
    pub certificate: ImpossibilityCertificate,
}

impl OracleOutcome {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Constructed(_) => 0,
            Self::InfeasibleUnderModel(_) => 1,
            Self::Unknown(_) => 2,
            Self::Refused(_) => 3,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstructionCertificate {
    pub metadata: OutcomeMetadata,
    pub request_digest: String,
    pub input_artifact_sha256: String,
    pub encoded_artifact_sha256: String,
    pub replay_command: Vec<String>,
    pub constraints_passed: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeMetadata {
    pub normalized_request_digest: Option<String>,
    pub tool_version: String,
    pub schema_version: u32,
    pub resources: ResourceConsumption,
    pub assumptions: Vec<String>,
    pub supported_scope: Vec<String>,
    pub warnings: Vec<String>,
    pub outcome_code: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnknownReport {
    #[serde(flatten)]
    pub metadata: OutcomeMetadata,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefusalReport {
    #[serde(flatten)]
    pub metadata: OutcomeMetadata,
    pub reason: String,
}
