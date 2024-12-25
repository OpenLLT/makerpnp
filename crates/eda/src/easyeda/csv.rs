use std::ops::{Add, Sub};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use thiserror::Error;
use pnp::pcb::PcbSide;
use crate::placement::{EdaPlacement, EdaPlacementField};

#[derive(Error, Debug)]
pub enum EasyEdaPlacementRecordError {
    #[error("Unknown")]
    Unknown
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct EasyEdaPlacementRecord {
    #[serde(rename(deserialize = "Designator"))]
    ref_des: String,
    device: String,
    value: String,
    #[serde(rename(deserialize = "Layer"))]
    side: EasyEdaPcbSide,
    #[serde(rename(deserialize = "Mid X"))]
    x: Decimal,
    #[serde(rename(deserialize = "Mid Y"))]
    y: Decimal,
    /// Positive values indicate anti-clockwise rotation.
    /// Range is >=0 to 359.999
    /// Rounding occurs on the 4th decimal, e.g. 359.9994 rounds to 359.999 and 359.995 rounds to 360, then gets converted to 0.
    rotation: Decimal,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all(deserialize = "lowercase"))]
enum EasyEdaPcbSide {
    #[serde(rename(deserialize = "T"))]
    Top,
    #[serde(rename(deserialize = "B"))]
    Bottom,
}

impl From<&EasyEdaPcbSide> for PcbSide {
    fn from(value: &EasyEdaPcbSide) -> Self {
        match value {
            EasyEdaPcbSide::Top => PcbSide::Top,
            EasyEdaPcbSide::Bottom => PcbSide::Bottom,
        }
    }
}

impl EasyEdaPlacementRecord {
    pub fn build_eda_placement(&self) -> Result<EdaPlacement, EasyEdaPlacementRecordError> {
        Ok(EdaPlacement {
            ref_des: self.ref_des.to_string(),
            place: true,
            fields: vec![
                EdaPlacementField { name: "device".to_string(), value: self.device.to_string() },
                EdaPlacementField { name: "value".to_string(), value: self.value.to_string() },
            ],
            pcb_side: PcbSide::from(&self.side),
            x: self.x,
            y: self.y,
            rotation: EasyEdaRotationConverter::convert(self.rotation),
        })

        // _ => Err(EasyEdaPlacementRecordError::Unknown)
    }
}


struct EasyEdaRotationConverter {}
impl EasyEdaRotationConverter {
    pub fn convert(mut input: Decimal) -> Decimal {
        while input >= dec!(360) {
            input = input.sub(dec!(360));
        }
        while input < dec!(0) {
            input = input.add( dec!(360));
        }
        if input > dec!(180) {
            input = input.sub(dec!(360));
        }
        input
    }
}

#[cfg(test)]
mod rotation_conversion_tests {

    use rstest::rstest;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use crate::easyeda::csv::EasyEdaRotationConverter;

    #[rstest]
    #[case(dec!(0), dec!(0))]
    #[case(dec!(180), dec!(180))]
    #[case(dec!(-180), dec!(180))]
    #[case(dec!(360), dec!(0))]
    #[case(dec!(185), dec!(-175))]
    #[case(dec!(-185), dec!(175))]
    #[case(dec!(0.001), dec!(0.001))]
    #[case(dec!(359.999), dec!(-0.001))]
    fn easyeda_to_eda_placement(#[case] value: Decimal, #[case] expected_value: Decimal) {
        assert_eq!(EasyEdaRotationConverter::convert(value), expected_value);
    }
}
