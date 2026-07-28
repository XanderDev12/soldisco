use soldisco_domain::MarketIdentity;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RaydiumEnrichmentRequest {
    pub pump_market: MarketIdentity,
}
