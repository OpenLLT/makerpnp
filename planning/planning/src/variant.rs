use std::fmt::{Display, Formatter};
use std::ops::Deref;
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
pub struct VariantName(String);

impl FromStr for VariantName {
    type Err = VariantNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(VariantName(s.to_string()))
    }
}

impl Display for VariantName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl From<&str> for VariantName {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl Deref for VariantName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Error)]
#[error("Variant name error")]
pub struct VariantNameError;
