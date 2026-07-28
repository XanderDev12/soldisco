#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectionChange {
    Discovery,
    RejectionReasons,
    ComponentHealth,
}
