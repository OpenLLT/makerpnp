use std::collections::HashMap;

use anyhow::Error;
use pnp::package::{ManufacturerPackageCode, Package, PackageDimensions};
use rust_decimal::Decimal;

// Define constants for package CSV field names
pub const FIELD_NAME: &str = "Name";
pub const FIELD_LEAD_COUNT: &str = "LeadCount";
pub const FIELD_LEAD_PITCH_MM: &str = "LeadPitchMm";
pub const FIELD_SIZE_X: &str = "SizeX";
pub const FIELD_SIZE_Y: &str = "SizeY";
pub const FIELD_SIZE_Z: &str = "SizeZ";
pub const FIELD_GENERIC_SHORTHAND: &str = "GenericShorthand";
pub const FIELD_EIA_IMPERIAL_CODE: &str = "EiaImperialCode";
pub const FIELD_EIA_METRIC_CODE: &str = "EiaMetricCode";
pub const FIELD_JEITA_CODE: &str = "JeitaCode";
pub const FIELD_IPC7351_CODE: &str = "Ipc7351Code";
pub const FIELD_JEDEC_MO_CODE: &str = "JedecMoCode";
pub const FIELD_JEDEC_PACKAGE_CODE: &str = "JedecPackageCode";

// Function to get the base header fields (excluding manufacturer codes)
pub fn get_base_package_headers() -> Vec<String> {
    vec![
        FIELD_NAME.into(),
        FIELD_LEAD_COUNT.into(),
        FIELD_LEAD_PITCH_MM.into(),
        FIELD_SIZE_X.into(),
        FIELD_SIZE_Y.into(),
        FIELD_SIZE_Z.into(),
        FIELD_GENERIC_SHORTHAND.into(),
        FIELD_EIA_IMPERIAL_CODE.into(),
        FIELD_EIA_METRIC_CODE.into(),
        FIELD_JEITA_CODE.into(),
        FIELD_IPC7351_CODE.into(),
        FIELD_JEDEC_MO_CODE.into(),
        FIELD_JEDEC_PACKAGE_CODE.into(),
    ]
}

pub fn build_package_from_field_map(fields: &HashMap<String, String>) -> Result<Package, Error> {
    // Get basic fields
    let name = fields
        .get(FIELD_NAME)
        .cloned()
        .unwrap_or_default();

    let mut package = Package::new(name);

    // Process optional fields
    if let Some(lead_count) = fields
        .get(FIELD_LEAD_COUNT)
        .and_then(|v| v.parse::<u32>().ok())
    {
        package = package.with_lead_count(lead_count);
    }

    if let Some(lead_pitch) = fields
        .get(FIELD_LEAD_PITCH_MM)
        .and_then(|v| v.parse::<Decimal>().ok())
    {
        package = package.with_pitch(lead_pitch);
    }

    // Handle other basic fields
    if let Some(shorthand) = fields
        .get(FIELD_GENERIC_SHORTHAND)
        .filter(|s| !s.is_empty())
    {
        package = package.with_generic_shorthand(shorthand.clone());
    }

    if let Some(imperial) = fields
        .get(FIELD_EIA_IMPERIAL_CODE)
        .filter(|s| !s.is_empty())
    {
        package = package.with_eia_imperial_code(imperial.clone());
    }

    if let Some(metric) = fields
        .get(FIELD_EIA_METRIC_CODE)
        .filter(|s| !s.is_empty())
    {
        package = package.with_eia_metric_code(metric.clone());
    }

    if let Some(jeita) = fields
        .get(FIELD_JEITA_CODE)
        .filter(|s| !s.is_empty())
    {
        package = package.with_jeita_code(jeita.clone());
    }

    if let Some(ipc) = fields
        .get(FIELD_IPC7351_CODE)
        .filter(|s| !s.is_empty())
    {
        package = package.with_ipc7351_code(ipc.clone());
    }

    if let Some(jedec_mo) = fields
        .get(FIELD_JEDEC_MO_CODE)
        .filter(|s| !s.is_empty())
    {
        package = package.with_jedec_mo_code(jedec_mo.clone());
    }

    if let Some(jedec_pkg) = fields
        .get(FIELD_JEDEC_PACKAGE_CODE)
        .filter(|s| !s.is_empty())
    {
        package = package.with_jedec_package_code(jedec_pkg.clone());
    }

    // Handle dimensions if present
    let size_x = fields
        .get(FIELD_SIZE_X)
        .and_then(|v| v.parse::<Decimal>().ok());
    let size_y = fields
        .get(FIELD_SIZE_Y)
        .and_then(|v| v.parse::<Decimal>().ok());
    let size_z = fields
        .get(FIELD_SIZE_Z)
        .and_then(|v| v.parse::<Decimal>().ok());

    if let (Some(x), Some(y), Some(z)) = (size_x, size_y, size_z) {
        package = package.with_dimensions(PackageDimensions::new(x, y, z));
    }

    // Extract manufacturer codes
    let mut manufacturer_codes = Vec::new();
    let mut i = 1;

    loop {
        let mfr_key = format!("Mfr{}", i);
        let code_key = format!("MfrCode{}", i);

        match (fields.get(&mfr_key), fields.get(&code_key)) {
            (Some(mfr), Some(code)) if !mfr.is_empty() && !code.is_empty() => {
                manufacturer_codes.push(ManufacturerPackageCode {
                    manufacturer: mfr.clone(),
                    code: code.clone(),
                });
                i += 1;
            }
            _ => break, // No more valid pairs
        }
    }

    package = package.with_manufacturer_codees(manufacturer_codes);

    Ok(package)
}
