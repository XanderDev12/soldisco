use soldisco_domain::SourceProgram;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectorSubscription {
    pub source_program: SourceProgram,
    pub program_id: String,
}
