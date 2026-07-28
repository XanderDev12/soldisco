use soldisco_domain::ObservationKey;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScreeningRequest {
    pub observation: ObservationKey,
    pub ruleset_version: String,
}
