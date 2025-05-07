use std::collections::BTreeMap;

use serde_with::serde_as;
use serde_with::DisplayFromStr;
use thiserror::Error;

use crate::design::{DesignName, DesignVariant};
use crate::gerber::GerberFile;

/// Defines a PCB
///
/// A PCB can have gerber files as follows
///
/// Panel - A set of gerbers that define the panel boundary, usually rectangular, and the designs within.
///         There may be multiple designs on a single panel.
///         This occurs when you take multiple designs and place them on a single panel.
///
/// Single - A set of gerbers that define the panel boundary, usually rectangular, and the designs within.
///         There is only a single designs on a single panel.
///         This frequently occurs when you place a single design in the center of a regtangular panel, especially
///         when the design is not rectangular and/or will not fit in the machines used for assembly.
///
/// A PCB may or may not have fiducials, the designs may also have their own fiducials which are sometimes used for the
/// PCB fiducials. e.g. in the case with the PCB does not have edge-rails.
/// Fiducials are not specified here but may be added or specified separately elsewhere.  e.g. `HashMap<Pcb,Vec<GerberPosition>>`
#[serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Pcb {
    pub name: String,

    /// The count of individual units in the pcb (regardless of the number of designs or design variants)
    ///
    /// This is used to populate the unit_assignments and to define the range used for 'skips' during assembly.
    ///
    /// A value of 0 is invalid
    // TODO validate this after deserializing
    pub units: u16,

    /// Individual units can have a design assigned.
    ///
    /// It is invalid for the length of this vector to be more or less than the value of [`units`].
    // TODO find a way to ensure the length of this vector is the same as `units` after deserialization.
    pub(crate) unit_assignments: Vec<Option<DesignVariant>>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub pcb_gerbers: Vec<GerberFile>,

    /// It is invalid to use add gerbers for a design that has not been assigned to a unit.
    #[serde_as(as = "Vec<(DisplayFromStr, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    // TODO validate that the design names used appear in the `unit_assignments` after deserialization.
    pub design_gerbers: BTreeMap<DesignName, Vec<GerberFile>>,
}

impl Pcb {
    pub fn new(name: String, units: u16) -> Self {
        let unit_assignments = vec![None; units as usize];

        Self {
            name,
            units,
            unit_assignments,
            pcb_gerbers: vec![],
            design_gerbers: Default::default(),
        }
    }

    pub fn unique_designs(&self) -> Vec<&DesignName> {
        let mut design_names = self
            .unit_assignments
            .iter()
            .filter_map(|design_variant| {
                design_variant
                    .as_ref()
                    .map(|design_variant| &design_variant.design_name)
            })
            .collect::<Vec<_>>();
        design_names.dedup();
        design_names
    }

    pub fn has_design(&self, other: &DesignName) -> bool {
        self.unique_designs().contains(&other)
    }

    pub fn unit_assignments(&self) -> &[Option<DesignVariant>] {
        &self.unit_assignments
    }

    /// Makes an assignment
    ///
    /// unit is 0-based
    ///
    /// Returns the previous assignment, which may be `None`
    ///
    /// Returns an error
    /// * if the assignment has already been made
    /// * if the unit is out of range
    pub fn assign_unit(&mut self, unit: u16, design_variant: DesignVariant) -> Result<Option<DesignVariant>, PcbError> {
        if unit >= self.units {
            return Err(PcbError::UnitOutOfRange {
                unit,
                min: 0,
                max: self.units - 1,
            });
        }

        let unit_index = unit as usize;

        if matches!(&self.unit_assignments[unit_index], Some(current_assignment) if current_assignment.eq(&design_variant))
        {
            return Err(PcbError::UnitAlreadyAssigned {
                unit,
            });
        }

        let old_assignment = self.unit_assignments[unit_index].replace(design_variant);

        Ok(old_assignment)
    }
}

#[derive(Debug, Error)]
pub enum PcbError {
    #[error("Unit {unit} is out of range [{min}..{max}] (inclusive)")]
    UnitOutOfRange { unit: u16, min: u16, max: u16 },
    #[error("Unit {unit} has already been assigned")]
    UnitAlreadyAssigned { unit: u16 },
}
