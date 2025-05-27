#[derive(serde::Deserialize, serde::Serialize, Debug)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct Config {
    pub show_home_tab_on_startup: bool,
    pub language_identifier: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            show_home_tab_on_startup: true,
            language_identifier: egui_i18n::get_language(),
        }
    }
}
