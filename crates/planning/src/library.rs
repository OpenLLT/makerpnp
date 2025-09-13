use serde_with::serde_as;
use util::source::Source;

#[serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize, Default, PartialEq, Clone, Eq)]
#[serde(rename_all = "snake_case")]
pub struct LibraryConfig {
    pub package_source: Option<Source>,
    pub package_mappings_source: Option<Source>,
}
