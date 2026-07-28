use soldisco_api_contracts::PrefilterDefaultsValidationError;
use soldisco_domain::Network;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("database migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("JSON serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("observation key network does not match market network")]
    NetworkMismatch,
    #[error("database is bound to {bound:?}; refusing startup for requested network {requested:?}")]
    DatabaseNetworkMismatch { bound: Network, requested: Network },
    #[error("{field} cannot be represented by PostgreSQL BIGINT")]
    ValueOutOfRange { field: &'static str },
    #[error("{field} must not be empty")]
    EmptyField { field: &'static str },
    #[error("{field} must be greater than zero")]
    MustBePositive { field: &'static str },
    #[error("{field} exceeds the supported maximum of {maximum}")]
    LimitTooLarge { field: &'static str, maximum: u32 },
    #[error("invalid prefilter defaults: {0}")]
    InvalidPrefilterDefaults(#[from] PrefilterDefaultsValidationError),
    #[error("prefilter defaults revision conflict: expected {expected}, actual {actual:?}")]
    PrefilterDefaultsRevisionConflict { expected: u64, actual: Option<u64> },
    #[error("{field} is not valid standard base64")]
    InvalidBase64 { field: &'static str },
    #[error("{field} is longer than the supported maximum of {maximum} bytes")]
    FieldTooLong { field: &'static str, maximum: usize },
    #[error("work item {work_id} is not held by worker {worker_id} under a live lease")]
    WorkLeaseLost { work_id: i64, worker_id: String },
    #[error("discovery work item {work_id} requires an observed, approved, or rejected outcome")]
    DiscoveryOutcomeRequired { work_id: i64 },
    #[error("work item {work_id} has kind {actual}; expected {expected}")]
    UnexpectedWorkKind {
        work_id: i64,
        expected: &'static str,
        actual: String,
    },
    #[error("stored {field} has invalid value {value}")]
    InvalidStoredValue { field: &'static str, value: String },
    #[error("discovery token does not match its durable source observation: {field}")]
    DiscoverySourceMismatch { field: &'static str },
    #[error("discovery observation must have stage OBSERVED")]
    ObservationMustBeUnapproved,
    #[error("discovery approval must have stage APPROVED")]
    ApprovalMustBeExplicit,
    #[error("token {mint} must be observed before it can be approved")]
    CandidateNotObserved { mint: String },
    #[error("discovery activity totals are internally inconsistent")]
    InvalidActivityTotals,
    #[error("{field} must contain only unsigned decimal digits")]
    InvalidUnsignedDecimal { field: &'static str },
    #[error("screening run aggregate decision does not match its rule results")]
    ScreeningDecisionMismatch,
    #[error("a deterministic PASS requires at least one explicit passing rule")]
    EmptyPassingRules,
    #[error("work item {work_id} already has a different immutable screening run")]
    ScreeningRunConflict { work_id: i64 },
    #[error("PumpSwap base and quote mints must be different")]
    IdenticalPoolMints,
    #[error("PumpSwap pool {pool_address} already maps to a different base or quote mint")]
    PumpSwapPoolIdentityConflict { pool_address: String },
    #[error("collection gap resolution cannot precede its prior checkpoint")]
    InvalidGapResolution,
}
