use pnp::package::ManufacturerPackageCode;

use crate::csv::sequential_fields_helper::{FlatRecord, SequentialFields};

/// Implementation for manufacturer codes (pair of fields)
pub struct ManufacturerCodeFieldHelper;

impl SequentialFields<ManufacturerPackageCode> for ManufacturerCodeFieldHelper {
    fn field_names(index: usize) -> Vec<String> {
        let num = index + 1; // 1-based indexing
        vec![format!("Mfr{}", num), format!("MfrCode{}", num)]
    }

    fn to_flat_record(items: &[ManufacturerPackageCode]) -> FlatRecord {
        let mut record = FlatRecord::new();

        for (i, item) in items.iter().enumerate() {
            let field_names = Self::field_names(i);
            record.set_field(field_names[0].clone(), item.manufacturer.clone());
            record.set_field(field_names[1].clone(), item.code.clone());
        }

        record
    }

    fn from_flat_record(record: &FlatRecord) -> Vec<ManufacturerPackageCode> {
        let mut result = Vec::new();

        // Try to read sequential pairs
        for i in 0.. {
            let field_names = Self::field_names(i);

            match (record.get_field(&field_names[0]), record.get_field(&field_names[1])) {
                (Some(mfr), Some(code)) if !mfr.is_empty() && !code.is_empty() => {
                    result.push(ManufacturerPackageCode {
                        manufacturer: mfr.clone(),
                        code: code.clone(),
                    });
                }
                _ => break, // Stop when we don't find a complete pair
            }
        }

        result
    }
}
