use egui_data_table::draw::Translator;
use egui_i18n::tr;

#[derive(Default)]
pub struct FluentTranslator {}

impl Translator for FluentTranslator {
    fn translate(&self, key: &str) -> String {
        tr!(key)
    }
}
