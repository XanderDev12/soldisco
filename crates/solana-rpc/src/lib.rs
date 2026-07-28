//! Provider-neutral Solana reads plus concrete standard JSON-RPC adapters.
//!
//! HTTP is authoritative for transaction recovery and account reads. WebSocket
//! PubSub lowers discovery latency, but reconnect/backfill orchestration remains
//! the server supervisor's responsibility.

mod http;
mod model;
mod pubsub;
mod recovery;

pub use http::SolanaHttpClient;
pub use model::{
    AccountRecord, ProgramLogNotification, ReadContext, RecoveryCheckpoint, RpcError, RpcHealth,
    SignaturePage, SignaturePageRequest, SignatureRecord, TransactionInstructionRecord,
    TransactionRecord,
};
pub use pubsub::{ProgramLogSubscription, SolanaPubsubClient};
pub use recovery::{RecoveryBatch, RecoveryPager, RecoveryProgress};

use async_trait::async_trait;

/// Read-only Solana boundary. Transaction construction, signing, and
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

    /// Fetch exactly one Solana `getSignaturesForAddress` page. Records are
    /// newest-first, matching the upstream RPC. Use [`RecoveryPager`] before
    /// releasing recovered work in chronological order.
    async fn signature_page(
        &self,
        request: SignaturePageRequest,
    ) -> Result<SignaturePage, RpcError>;
}
