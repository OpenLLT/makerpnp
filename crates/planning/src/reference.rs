use std::fmt::{Display, Formatter};
use std::str::FromStr;

use thiserror::Error;

/// Reference should be a string with no-whitespace characters at all
/// 
/// This is mostly so they can be used on the command line without parsing issues.
/// 
/// Some references use namespacing, in the form `<value>[::<...>]`  e.g. `this::that::the_other`
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

impl Reference {
    fn is_valid(value: &String) -> bool {
        if value.is_empty() {
            return false;
        }
        
        if value.chars().all(|c| !(c.is_whitespace() || c.is_ascii_control() || c.is_control())) {
            return false;
        }
        
        true
    }

    pub fn from_raw_str(value: &str) -> Self {
        let value = value.to_string();
        assert!(Self::is_valid(&value));

        Self(value)
    }
    
    pub fn from_raw(value: String) -> Self {
        assert!(Self::is_valid(&value));
        
        Self(value.to_string())
    }
}

impl FromStr for Reference {
    type Err = ReferenceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s.to_string())
    }
}

impl Display for Reference {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl TryFrom<String> for Reference {
    type Error = ReferenceError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if Self::is_valid(&value) {
            Ok(Self(value))
        } else {
            Err(ReferenceError::InvalidReference(value))
        }
    }
}

#[derive(Debug, Error)]
pub enum ReferenceError {
    #[error("Invalid reference: '{0}'")]
    InvalidReference(String),
}

#[cfg(test)]
mod reference_tests {
    use rstest::rstest;
    use crate::reference::Reference;

    #[rstest]
    #[case("", false)]
    #[case("\n", false)]
    #[case("\r", false)]
    #[case("\t", false)]
    #[case(" leading_whitespace", false)]
    #[case("trailing_whitespace ", false)]
    #[case("control\ncharacters\rpresent", false)]
    #[case("tab\tcharacters\tpresent", false)]
    #[case("a", true)]
    #[case("example_reference", true)]
    #[case("namespaced::example_reference", true)]
    #[case("longer::namespaced::example_reference", true)]
    fn is_valid(#[case] value: &str, #[case] expected_result: bool) {
        assert_eq!(Reference::is_valid(&value.to_string()), expected_result);
    }
}