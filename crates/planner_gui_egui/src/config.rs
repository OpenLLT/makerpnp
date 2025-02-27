#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct Config {
    pub show_home_tab_on_startup: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            show_home_tab_on_startup: true,
        }
    }
}
