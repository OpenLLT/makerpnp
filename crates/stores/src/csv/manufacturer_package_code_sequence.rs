use pnp::package::ManufacturerPackageCode;

use crate::csv::sequential_fields_adaptor::SequentialFields;

pub struct ManufacturerCodeFields;

impl SequentialFields for ManufacturerCodeFields {
    type Item = ManufacturerPackageCode;

    fn field_prefixes() -> &'static [&'static str] {
        &["Mfr", "MfrCode"]
    }

    fn from_values(values: Vec<String>) -> Option<Self::Item> {
        if values.len() == 2 && !values[0].is_empty() && !values[1].is_empty() {
            Some(ManufacturerPackageCode {
                manufacturer: values[0].clone(),
                code: values[1].clone(),
            })
        } else {
            None
        }
    }

    fn to_values(item: &Self::Item) -> Vec<String> {
        vec![item.manufacturer.clone(), item.code.clone()]
    }
}
