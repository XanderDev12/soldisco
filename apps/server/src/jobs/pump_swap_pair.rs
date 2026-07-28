use soldisco_discovery_engine::{CANONICAL_USDC_MINT, NATIVE_SOL_MINT, WRAPPED_SOL_MINT};
use soldisco_domain::Network;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PumpSwapSourceOrientation {
    BaseIsToken,
    QuoteIsToken,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PumpSwapPair {
    pub token_mint: String,
    pub quote_mint: String,
    pub source_orientation: PumpSwapSourceOrientation,
}

/// Converts a source-order PumpSwap pair into Soldisco's token/quote order.
///
/// A pair is eligible only when exactly one source mint is an explicitly
/// supported quote asset. This prevents quote/quote and token/token pools from
/// creating ambiguous discovery windows.
#[must_use]
pub fn normalize_pump_swap_pair(
    network: Network,
    source_base_mint: &str,
    source_quote_mint: &str,
) -> Option<PumpSwapPair> {
    let base_is_quote = is_supported_quote_mint(network, source_base_mint);
    let quote_is_quote = is_supported_quote_mint(network, source_quote_mint);

    match (base_is_quote, quote_is_quote) {
        (false, true) => Some(PumpSwapPair {
            token_mint: source_base_mint.to_owned(),
            quote_mint: source_quote_mint.to_owned(),
            source_orientation: PumpSwapSourceOrientation::BaseIsToken,
        }),
        (true, false) => Some(PumpSwapPair {
            token_mint: source_quote_mint.to_owned(),
            quote_mint: source_base_mint.to_owned(),
            source_orientation: PumpSwapSourceOrientation::QuoteIsToken,
        }),
        (false, false) | (true, true) => None,
    }
}

#[must_use]
pub fn is_supported_quote_mint(network: Network, mint: &str) -> bool {
    matches!(mint, NATIVE_SOL_MINT | WRAPPED_SOL_MINT)
        || (network == Network::SolanaMainnet && mint == CANONICAL_USDC_MINT)
}

#[cfg(test)]
mod tests {
    use soldisco_discovery_engine::{CANONICAL_USDC_MINT, WRAPPED_SOL_MINT};
    use soldisco_domain::Network;

    use super::{PumpSwapSourceOrientation, is_supported_quote_mint, normalize_pump_swap_pair};

    #[test]
    fn normalizes_both_source_orientations_around_the_supported_quote() {
        let standard = normalize_pump_swap_pair(Network::SolanaMainnet, "token", WRAPPED_SOL_MINT)
            .expect("wSOL quote should be eligible");
        assert_eq!(standard.token_mint, "token");
        assert_eq!(standard.quote_mint, WRAPPED_SOL_MINT);
        assert_eq!(
            standard.source_orientation,
            PumpSwapSourceOrientation::BaseIsToken
        );

        let reversed = normalize_pump_swap_pair(Network::SolanaMainnet, WRAPPED_SOL_MINT, "token")
            .expect("reversed wSOL pair should be eligible");
        assert_eq!(reversed.token_mint, "token");
        assert_eq!(reversed.quote_mint, WRAPPED_SOL_MINT);
        assert_eq!(
            reversed.source_orientation,
            PumpSwapSourceOrientation::QuoteIsToken
        );
    }

    #[test]
    fn rejects_pairs_without_exactly_one_supported_quote() {
        assert!(normalize_pump_swap_pair(Network::SolanaMainnet, "token-a", "token-b").is_none());
        assert!(
            normalize_pump_swap_pair(
                Network::SolanaMainnet,
                WRAPPED_SOL_MINT,
                CANONICAL_USDC_MINT,
            )
            .is_none()
        );
        assert!(
            normalize_pump_swap_pair(Network::SolanaMainnet, WRAPPED_SOL_MINT, WRAPPED_SOL_MINT,)
                .is_none()
        );
    }

    #[test]
    fn canonical_usdc_is_supported_only_on_mainnet() {
        assert!(is_supported_quote_mint(
            Network::SolanaMainnet,
            CANONICAL_USDC_MINT
        ));
        assert!(!is_supported_quote_mint(
            Network::SolanaDevnet,
            CANONICAL_USDC_MINT
        ));
    }
}
