use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use soldisco_domain::Commitment;
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecoveryCheckpoint {
    pub program_id: String,
    pub last_slot: u64,
    pub last_signature: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TransactionRecord {
    pub slot: u64,
    pub signature: String,
    pub block_time_unix_seconds: Option<i64>,
    pub encoded_transaction: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountRecord {
    pub address: String,
    pub owner: String,
    pub slot: u64,
    pub encoded_data: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RpcHealth {
    Up,
    Degraded,
    Down,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadContext {
    pub commitment: Commitment,
    pub minimum_slot: Option<u64>,
}

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("RPC request timed out")]
    Timeout,
    #[error("RPC rate limit was reached")]
    RateLimited,
    #[error("RPC returned unavailable data: {0}")]
    Unavailable(String),
    #[error("RPC returned invalid data: {0}")]
    InvalidResponse(String),
}

/// Provider-neutral read boundary. Transaction construction, signing, and
/// submission intentionally do not appear here.
#[async_trait]
pub trait SolanaReader: Send + Sync {
    async fn health(&self) -> RpcHealth;

    async fn transaction(
        &self,
        signature: &str,
        context: ReadContext,
    ) -> Result<Option<TransactionRecord>, RpcError>;

    async fn account(
        &self,
        address: &str,
        context: ReadContext,
    ) -> Result<Option<AccountRecord>, RpcError>;

    async fn signatures_after(
        &self,
        program_id: &str,
        checkpoint: Option<&RecoveryCheckpoint>,
        limit: usize,
    ) -> Result<Vec<String>, RpcError>;
}

#[cfg(test)]
mod tests {
    use super::RecoveryCheckpoint;

    #[test]
    fn recovery_checkpoint_keeps_slot_and_signature_together() {
        let checkpoint = RecoveryCheckpoint {
            program_id: "program".to_owned(),
            last_slot: 900,
            last_signature: "signature".to_owned(),
        };

        assert_eq!(checkpoint.last_slot, 900);
        assert!(!checkpoint.last_signature.is_empty());
    }
}
