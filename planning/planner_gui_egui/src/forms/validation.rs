use std::str::FromStr;

use planner_app::{LoadOutSource, ProcessReference, Reference};
use validator::ValidationError;

pub struct CommonValidation {}
impl CommonValidation {
    pub fn validate_reference(reference: &String) -> Result<(), ValidationError> {
        Reference::from_str(&reference)
            .map_err(|_e| ValidationError::new("form-input-error-reference-invalid"))
            .map(|_| ())
    }

    pub fn validate_optional_process_reference(process_reference: &ProcessReference) -> Result<(), ValidationError> {
        match process_reference.is_valid() {
            true => Ok(()),
            false => Err(ValidationError::new("form-input-error-process-reference-invalid")),
        }
    }

    pub fn validate_optional_loadout_source(load_out_source: &String) -> Result<(), ValidationError> {
        LoadOutSource::from_str(&load_out_source)
            .map_err(|_e| ValidationError::new("form-input-error-loadout-source-invalid"))
            .map(|_loadout_source| ())
    }
}
