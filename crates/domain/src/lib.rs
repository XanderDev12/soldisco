//! Provider-neutral facts and deterministic decisions shared by the backend.

mod assessment;
mod market;
mod observation;
mod score;

pub use assessment::{AssessmentDecision, RuleResult};
pub use market::{MarketIdentity, Network, Venue};
pub use observation::{
    ChainCoordinate, Commitment, NormalizedObservation, ObservationKey, ObservationPayload,
    SourceProgram, TradeSide,
};
pub use score::{Score, ScoreOutOfRange};
