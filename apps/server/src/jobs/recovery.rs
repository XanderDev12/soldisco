use soldisco_domain::{Network, SourceProgram};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryRequest {
    pub network: Network,
    pub source_program: SourceProgram,
    pub after_slot: Option<u64>,
}
