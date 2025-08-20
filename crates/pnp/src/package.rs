use rust_decimal::Decimal;
// FUTURE investigate removal of `Hash` derive and using `IndexSet` for the 'manufacturer_aliases'
#[derive(Debug, Clone)]
#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
#[derive(serde::Serialize, serde::Deserialize)]
/// Defines a component package (body style / case)
///
/// The package definition can be used to find footprints and 3D models or more precise sizing information.
///
/// Since there are many ways to cross-reference various EDA and assembly we need fields to be explicit as possible.
pub struct Package {
    /// Library name (human-readable identifier, e.g. "QFN-48-1EP-7x7mm").
    pub name: String,

    // Disambiguation parameters
    pub lead_count: Option<u32>,
    pub body_length_mm: Option<Decimal>,
    pub body_width_mm: Option<Decimal>,
    pub height_mm: Option<Decimal>,
    pub pitch_mm: Option<Decimal>,

    // Standardized identifiers
    /// e.g. 0603, SOT-223
    pub generic_shorthand: Option<String>,

    // Note: usually there is a 1:1 relationship between eia metric and imperial codes.
    // so at the time of entry, the corresponding values can be looked up for confirmation by the user.
    /// e.g. 0603
    pub eia_imperial_code: Option<String>,
    /// e.g. 1608
    pub eia_metric_code: Option<String>,

    pub ipc7351_code: Option<String>,
    pub jedec_mo_code: Option<String>,
    pub jedec_package_code: Option<String>,
    pub jeita_code: Option<String>,

    /// e.g. 'AVX, F98 Case M'
    pub manufacturer_aliases: Vec<ManufacturerPackageAlias>,
}

#[derive(Debug, Clone)]
#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ManufacturerPackageAlias {
    pub manufacturer: String,
    pub alias: String,
}

impl Package {
    pub fn new(name: String) -> Self {
        Self {
            name,
            lead_count: None,
            body_length_mm: None,
            body_width_mm: None,
            height_mm: None,
            pitch_mm: None,
            generic_shorthand: None,
            eia_imperial_code: None,
            eia_metric_code: None,
            ipc7351_code: None,
            jedec_mo_code: None,
            jedec_package_code: None,
            jeita_code: None,
            manufacturer_aliases: vec![],
        }
    }

    pub fn with_lead_count(mut self, lead_count: u32) -> Self {
        self.lead_count = Some(lead_count);
        self
    }

    pub fn with_body_length(mut self, body_length_mm: Decimal) -> Self {
        self.body_length_mm = Some(body_length_mm);
        self
    }

    pub fn with_body_width(mut self, body_width_mm: Decimal) -> Self {
        self.body_width_mm = Some(body_width_mm);
        self
    }

    pub fn with_height(mut self, height_mm: Decimal) -> Self {
        self.height_mm = Some(height_mm);
        self
    }

    pub fn with_pitch(mut self, pitch_mm: Decimal) -> Self {
        self.pitch_mm = Some(pitch_mm);
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

    pub fn with_manufacturer_alias(mut self, manufacturer: String, alias: String) -> Self {
        self.manufacturer_aliases
            .push(ManufacturerPackageAlias {
                manufacturer,
                alias,
            });
        self
    }

    pub fn with_manufacturer_aliases(mut self, manufacturer_aliases: Vec<ManufacturerPackageAlias>) -> Self {
        self.manufacturer_aliases = manufacturer_aliases;
        self
    }

    pub fn add_manufacturer_alias(&mut self, manufacturer: String, alias: String) -> Result<(), PackageError> {
        let package_alias = ManufacturerPackageAlias {
            manufacturer,
            alias,
        };
        if self
            .manufacturer_aliases
            .contains(&package_alias)
        {
            return Err(PackageError::DuplicateManufacturerAlias);
        }

        self.manufacturer_aliases
            .push(package_alias);
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
            body_length_mm: None,
            body_width_mm: None,
            height_mm: None,
            pitch_mm: None,
            generic_shorthand: None,
            eia_imperial_code: None,
            eia_metric_code: None,
            ipc7351_code: None,
            jedec_mo_code: None,
            jedec_package_code: None,
            jeita_code: None,
            manufacturer_aliases: vec![],
        }
    }
}
