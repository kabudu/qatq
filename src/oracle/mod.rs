//! Proof-carrying capacity analysis contracts.
//!
//! Only finite, independently checkable witnesses can produce
//! [`OracleOutcome::InfeasibleUnderModel`]. Asymptotic evidence remains
//! planning-only.

mod certificate;
mod checker;
mod limits;
mod model;
mod normalize;
mod outcome;

pub use certificate::{
    ArithmeticProfile, BinaryHammingWitness, BoundWitness, CheckerRequirements,
    ImpossibilityCertificate, SphericalRankinNegativeWitness, TheoremIdentity,
};
pub use checker::{CertificateCheck, check_certificate, check_certificate_json};
pub use limits::{OracleResourceLimits, ResourceConsumption};
pub use model::{
    BinaryCodeModel, BoundRequest, ConstructionRequest, OracleRequest, RepresentationModel,
    SphericalCodeModel,
};
pub use normalize::{
    EvaluationResult, NormalizedBinaryModel, NormalizedModel, NormalizedRequest,
    NormalizedSphericalModel, OracleError, evaluate, evaluate_json, normalize_request,
    parse_request, recompute_request_digest,
};
pub use outcome::{
    ConstructionCertificate, InfeasibilityReport, OracleOutcome, OutcomeMetadata, RefusalReport,
    UnknownReport,
};
