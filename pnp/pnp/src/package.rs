use std::cmp::Ordering;

use nalgebra::Vector3;
use rust_decimal::Decimal;
// FUTURE investigate removal of `Hash` derive and using `IndexSet` for the 'manufacturer_aliases'
#[derive(Debug, Clone)]
#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
#[derive(serde::Serialize, serde::Deserialize)]
/// Defines a component package (body style / case)
///
/// The package definition can be used to find footprints, datasheets, 3D models or more precise sizing information.
/// which are specifically NOT defined in this structure.
///
/// Since there are many ways to cross-reference various EDA and assembly we need fields to be explicit as possible.
pub struct Package {
    /// Library name (human-readable identifier, e.g. "QFN-48-1EP-7x7mm").
    pub name: String,

    //
    // Disambiguation parameters
    //
    pub lead_count: Option<u32>,
    /// The pitch between adjacent pins
    ///
    /// Note: not the body width.
    pub lead_pitch_mm: Option<Decimal>,

    /// Includes terminals (i.e. not just the body size)
    pub dimensions_mm: Option<PackageDimensions>,

    //
    // Standardized identifiers
    //
    /// e.g. 0603, SOT-223
    pub generic_shorthand: Option<String>,

    // Note: usually there is a 1:1 relationship between eia metric and imperial codes.
    // so at the time of entry, the corresponding values can be looked up for confirmation by the user.
    /// e.g. 0603
    pub eia_imperial_code: Option<String>,
    /// e.g. 1608
    pub eia_metric_code: Option<String>,
    pub jeita_code: Option<String>,

    pub ipc7351_code: Option<String>,
    pub jedec_mo_code: Option<String>,
    pub jedec_package_code: Option<String>,

    //
    // Non-standardized identifiers
    //
    /// e.g. 'AVX, F98 Series Case M'
    pub manufacturer_codes: Vec<ManufacturerPackageCode>,
}

#[derive(Debug, Clone)]
#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ManufacturerPackageCode {
    pub manufacturer: String,
    pub code: String,
}

impl Package {
    pub fn new(name: String) -> Self {
        Self {
            name,
            lead_count: None,
            lead_pitch_mm: None,
            dimensions_mm: None,
            generic_shorthand: None,
            eia_imperial_code: None,
            eia_metric_code: None,
            ipc7351_code: None,
            jedec_mo_code: None,
            jedec_package_code: None,
            jeita_code: None,
            manufacturer_codes: vec![],
        }
    }

    pub fn with_lead_count(mut self, lead_count: u32) -> Self {
        self.lead_count = Some(lead_count);
        self
    }

    pub fn with_dimensions(mut self, dimensions_mm: PackageDimensions) -> Self {
        self.dimensions_mm = Some(dimensions_mm);
        self
    }

    pub fn with_pitch(mut self, pitch_mm: Decimal) -> Self {
        self.lead_pitch_mm = Some(pitch_mm);
        self
    }

    pub fn with_generic_shorthand(mut self, generic_shorthand: String) -> Self {
        self.generic_shorthand = Some(generic_shorthand);
        self
    }

    pub fn with_eia_imperial_code(mut self, eia_imperial_code: String) -> Self {
        self.eia_imperial_code = Some(eia_imperial_code);
        self
    }
    pub fn with_eia_metric_code(mut self, eia_metric_code: String) -> Self {
        self.eia_metric_code = Some(eia_metric_code);
        self
    }

    pub fn with_ipc7351_code(mut self, ipc7351_code: String) -> Self {
        self.ipc7351_code = Some(ipc7351_code);
        self
    }

    pub fn with_jedec_mo_code(mut self, jedec_mo_code: String) -> Self {
        self.jedec_mo_code = Some(jedec_mo_code);
        self
    }

    pub fn with_jedec_package_code(mut self, jedec_package_code: String) -> Self {
        self.jedec_package_code = Some(jedec_package_code);
        self
    }
    pub fn with_jeita_code(mut self, jeita_code: String) -> Self {
        self.jeita_code = Some(jeita_code);
        self
    }

    pub fn with_manufacturer_code(mut self, manufacturer: String, alias: String) -> Self {
        self.manufacturer_codes
            .push(ManufacturerPackageCode {
                manufacturer,
                code: alias,
            });
        self
    }

    pub fn with_manufacturer_codees(mut self, manufacturer_codees: Vec<ManufacturerPackageCode>) -> Self {
        self.manufacturer_codes = manufacturer_codees;
        self
    }

    pub fn add_manufacturer_code(&mut self, manufacturer: String, code: String) -> Result<(), PackageError> {
        let package_code = ManufacturerPackageCode {
            manufacturer,
            code,
        };
        if self
            .manufacturer_codes
            .contains(&package_code)
        {
            return Err(PackageError::DuplicateManufacturerAlias);
        }

        self.manufacturer_codes
            .push(package_code);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("Duplicate manufacturer alias")]
    DuplicateManufacturerAlias,
}

#[cfg(feature = "testing")]
impl Default for Package {
    fn default() -> Self {
        Self {
            name: "Default name".to_string(),
            lead_count: None,
            dimensions_mm: None,
            lead_pitch_mm: None,
            generic_shorthand: None,
            eia_imperial_code: None,
            eia_metric_code: None,
            ipc7351_code: None,
            jedec_mo_code: None,
            jedec_package_code: None,
            jeita_code: None,
            manufacturer_codes: vec![],
        }
    }
}

#[derive(Debug, Clone)]
#[derive(Hash, PartialEq, Eq)]
#[derive(serde::Serialize, serde::Deserialize)]
/// Defines the minimal sizing requirements for disambiguation of parts and for determining placement ordering.
///
/// Do NOT add PnP vision system concerns into this structure
///
/// Volume is used for ordering (smallest first)
pub struct PackageDimensions(Vector3<Decimal>);

impl PackageDimensions {
    pub fn new(x: Decimal, y: Decimal, z: Decimal) -> Self {
        Self(Vector3::new(x, y, z))
    }

    pub fn area(&self) -> Decimal {
        self.0.x * self.0.y
    }

    pub fn volume(&self) -> Decimal {
        self.0.x * self.0.y * self.0.z
    }

    pub fn as_vector3(&self) -> &Vector3<Decimal> {
        &self.0
    }

    pub fn size_x(&self) -> Decimal {
        self.0.x
    }

    pub fn size_y(&self) -> Decimal {
        self.0.y
    }

    pub fn size_z(&self) -> Decimal {
        self.0.z
    }
}

impl PartialOrd for PackageDimensions {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.volume().cmp(&other.volume()))
    }
}

impl Ord for PackageDimensions {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}
