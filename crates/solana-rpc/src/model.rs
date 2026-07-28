use serde::{Deserialize, Serialize};
use soldisco_domain::Commitment;
use thiserror::Error;

pub const MAX_SIGNATURE_PAGE_SIZE: usize = 1_000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecoveryCheckpoint {
    pub program_id: String,
    pub last_slot: u64,
    pub last_transaction_index: Option<u64>,
    pub last_signature: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TransactionRecord {
    pub slot: u64,
    /// Optional provider extension identifying this transaction's exact
    /// position inside its block. Standard Solana RPC nodes may omit it.
    pub transaction_index: Option<u64>,
    pub signature: String,
    pub block_time_unix_seconds: Option<i64>,
    pub instructions: Vec<TransactionInstructionRecord>,
    pub log_messages: Vec<String>,
    /// Compact upstream transaction error JSON. `None` means the transaction
    /// succeeded; callers must not decode failed transactions as market facts.
    pub transaction_error: Option<String>,
}

impl TransactionRecord {
    #[must_use]
    pub const fn succeeded(&self) -> bool {
        self.transaction_error.is_none()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TransactionInstructionRecord {
    /// Top-level message instruction which owns this execution context.
    pub outer_instruction_index: u16,
    /// `None` for the top-level instruction; otherwise the position inside the
    /// corresponding `meta.innerInstructions` group.
    pub inner_instruction_index: Option<u16>,
    pub stack_height: Option<u32>,
    pub program_id: String,
    /// Strict base58 decoding of the compiled instruction's data.
    pub data: Vec<u8>,
}

impl TransactionInstructionRecord {
    #[must_use]
    pub const fn is_inner(&self) -> bool {
        self.inner_instruction_index.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountRecord {
    pub address: String,
    pub owner: String,
    pub slot: u64,
    pub encoded_data: String,
    pub lamports: u64,
    pub executable: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RpcHealth {
    Up,
    Degraded,
    Down,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReadContext {
    pub commitment: Commitment,
    pub minimum_slot: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignaturePageRequest {
    pub program_id: String,
    /// Exclusive cursor: results are older than this signature.
    pub before: Option<String>,
    pub limit: usize,
    pub context: ReadContext,
}

impl SignaturePageRequest {
    pub fn new(
        program_id: impl Into<String>,
        before: Option<String>,
        limit: usize,
        context: ReadContext,
    ) -> Result<Self, RpcError> {
        let program_id = program_id.into();
        if program_id.is_empty() {
            return Err(RpcError::InvalidRequest(
                "program id cannot be empty".to_owned(),
            ));
        }
        if !(1..=MAX_SIGNATURE_PAGE_SIZE).contains(&limit) {
            return Err(RpcError::InvalidRequest(format!(
                "signature page limit must be between 1 and {MAX_SIGNATURE_PAGE_SIZE}"
            )));
        }
        if before.as_deref() == Some("") {
            return Err(RpcError::InvalidRequest(
                "before signature cannot be empty".to_owned(),
            ));
        }
        Ok(Self {
            program_id,
            before,
            limit,
            context,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignatureRecord {
    pub slot: u64,
    /// Optional provider extension identifying this transaction's exact
    /// position inside its block. Standard Solana RPC nodes may omit it.
    pub transaction_index: Option<u64>,
    pub signature: String,
    pub block_time_unix_seconds: Option<i64>,
    pub confirmation_status: Option<String>,
    pub transaction_error: Option<String>,
}

impl SignatureRecord {
    #[must_use]
    pub const fn succeeded(&self) -> bool {
        self.transaction_error.is_none()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignaturePage {
    pub request: SignaturePageRequest,
    /// Solana RPC ordering: newest to oldest.
    pub records_newest_first: Vec<SignatureRecord>,
    /// Cursor for the next older page. `None` when this page is exhausted.
    pub next_before: Option<String>,
    pub exhausted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProgramLogNotification {
    pub subscription_id: u64,
    pub slot: u64,
    pub signature: String,
    pub log_messages: Vec<String>,
    pub transaction_error: Option<String>,
}

impl ProgramLogNotification {
    #[must_use]
    pub const fn succeeded(&self) -> bool {
        self.transaction_error.is_none()
    }
}

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("RPC request timed out")]
    Timeout,
    #[error("RPC rate limit was reached")]
    RateLimited,
    #[error("RPC transport failed: {0}")]
    Transport(String),
    #[error("RPC returned HTTP status {status}: {body}")]
    HttpStatus { status: u16, body: String },
    #[error("RPC error {code}: {message}")]
    Rpc { code: i64, message: String },
    #[error("invalid RPC request: {0}")]
    InvalidRequest(String),
    #[error("RPC returned unavailable data: {0}")]
    Unavailable(String),
    #[error("RPC returned invalid data: {0}")]
    InvalidResponse(String),
    #[error(
        "recovery checkpoint {signature} at slot {slot} was not found in history for {program_id}"
    )]
    RecoveryCheckpointNotFound {
        program_id: String,
        slot: u64,
        signature: String,
    },
    #[error("recovery exceeded its {max_records}-record safety limit before checkpoint")]
    RecoveryLimitExceeded { max_records: usize },
    #[error("invalid recovery page sequence: {0}")]
    RecoveryProtocol(String),
    #[error("Solana PubSub connection closed: {0}")]
    SubscriptionClosed(String),
}
