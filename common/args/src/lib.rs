#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone, Eq)]
pub enum Arg {
    Boolean(bool),
    String(String),
    Integer(i64),
    // Add other types, like 'Number' here as required.
}
