#[derive(Debug, Clone)]
#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Part {
    pub manufacturer: String,
    pub mpn: String,
}

impl Part {
    pub fn new(manufacturer: String, mpn: String) -> Self {
        Self {
            manufacturer,
            mpn,
        }
    }
}

#[cfg(feature = "testing")]
impl Default for Part {
    fn default() -> Self {
        Self {
            manufacturer: "Default Manufacturer".to_string(),
            mpn: "Default MPN".to_string(),
        }
    }
}
