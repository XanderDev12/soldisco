#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionPolicy {
    pub raw_observation_days: u16,
    pub warning_bytes: u64,
}
