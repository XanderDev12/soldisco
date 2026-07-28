use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Score(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScoreOutOfRange {
    value: u8,
}

impl Score {
    pub fn new(value: u8) -> Result<Self, ScoreOutOfRange> {
        if value <= 100 {
            Ok(Self(value))
        } else {
            Err(ScoreOutOfRange { value })
        }
    }

    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl Serialize for Score {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(self.0)
    }
}

impl<'de> Deserialize<'de> for Score {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

impl fmt::Display for ScoreOutOfRange {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "score {} is outside the inclusive 0..=100 range",
            self.value
        )
    }
}

impl std::error::Error for ScoreOutOfRange {}

#[cfg(test)]
mod tests {
    use super::Score;

    #[test]
    fn rejects_values_above_one_hundred() {
        assert!(Score::new(100).is_ok());
        assert!(Score::new(101).is_err());
    }

    #[test]
    fn rejects_out_of_range_json_at_the_boundary() {
        let error = serde_json::from_str::<Score>("101")
            .expect_err("deserialization must enforce the same score range");

        assert!(error.to_string().contains("0..=100"));
    }
}
