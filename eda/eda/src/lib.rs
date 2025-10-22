pub mod diptrace;
pub mod easyeda;
pub mod kicad;

pub mod criteria;
pub mod placement;
pub mod substitution;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum EdaTool {
    DipTrace,
    KiCad,
    EasyEda,
}
