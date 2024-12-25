pub mod diptrace;
pub mod kicad;
pub mod easyeda;

pub mod placement;
pub mod substitution;
pub mod criteria;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum EdaTool {
    DipTrace,
    KiCad,
    EasyEda,
}
