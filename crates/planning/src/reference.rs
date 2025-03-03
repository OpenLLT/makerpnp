use std::fmt::{Display, Formatter};
use std::str::FromStr;

use thiserror::Error;

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash
)]
pub struct Reference(pub String);

impl FromStr for Reference {
    type Err = ReferenceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Reference(s.to_string()))
    }
}

impl Display for Reference {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl From<String> for Reference {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Debug, Error)]
#[error("Reference error")]
pub struct ReferenceError;
