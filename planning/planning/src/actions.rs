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
pub enum AddOrRemoveAction {
    Add,
    Remove,
}

impl TryFrom<&String> for AddOrRemoveAction {
    type Error = ();

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "add" => Ok(AddOrRemoveAction::Add),
            "remove" => Ok(AddOrRemoveAction::Remove),
            _ => Err(()),
        }
    }
}

impl Display for AddOrRemoveAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddOrRemoveAction::Add => f.write_str("add"),
            AddOrRemoveAction::Remove => f.write_str("remove"),
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
pub enum SetOrClearAction {
    Set,
    Clear,
}

impl TryFrom<&String> for SetOrClearAction {
    type Error = ();

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "add" => Ok(SetOrClearAction::Set),
            "remove" => Ok(SetOrClearAction::Clear),
            _ => Err(()),
        }
    }
}

impl Display for SetOrClearAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SetOrClearAction::Set => f.write_str("add"),
            SetOrClearAction::Clear => f.write_str("remove"),
        }
    }
}
