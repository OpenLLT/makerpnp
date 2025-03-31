use std::fmt::Display;

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
#[serde(rename_all = "lowercase")]
pub enum AddOrRemoveOperation {
    Add,
    Remove,
}

impl TryFrom<&String> for AddOrRemoveOperation {
    type Error = ();

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "add" => Ok(AddOrRemoveOperation::Add),
            "remove" => Ok(AddOrRemoveOperation::Remove),
            _ => Err(()),
        }
    }
}

impl Display for AddOrRemoveOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddOrRemoveOperation::Add => f.write_str("add"),
            AddOrRemoveOperation::Remove => f.write_str("remove"),
        }
    }
}

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
#[serde(rename_all = "lowercase")]
pub enum SetOrClearOperation {
    Set,
    Clear,
}

impl TryFrom<&String> for SetOrClearOperation {
    type Error = ();

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "add" => Ok(SetOrClearOperation::Set),
            "remove" => Ok(SetOrClearOperation::Clear),
            _ => Err(()),
        }
    }
}

impl Display for SetOrClearOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SetOrClearOperation::Set => f.write_str("add"),
            SetOrClearOperation::Clear => f.write_str("remove"),
        }
    }
}
