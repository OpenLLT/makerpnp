use std::collections::BTreeMap;
use std::fmt::Debug;
use std::path::PathBuf;

use gerber::{detect_purpose, GerberFile, GerberFileFunction};
use indexmap::IndexSet;
use itertools::Itertools;
use pnp::panel::PanelSizing;
use pnp::pcb::{PcbUnitIndex, PcbUnitNumber};
use serde_with::serde_as;
use thiserror::Error;
use tracing::{info, trace};

use crate::design::{DesignIndex, DesignName};
use crate::project::PcbOperationError;

/// Defines a PCB
///
/// A PCB can have its own gerber files and gerber files for each design, or not at all.
#[serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Pcb {
    /// A name for this PCB.  e.g. the reference number provided by the PCB fabricator which is often found on the
    /// PCB silk-screen.
    pub name: String,

    /// The count of individual units in the pcb (regardless of the number of designs or design variants)
    ///
    /// This is used to populate the unit_assignments and to define the range used for 'skips' during assembly.
    ///
    /// A value of 0 is invalid
    // TODO validate this after deserializing
    pub units: u16,

    #[serde(skip_serializing_if = "IndexSet::is_empty")]
    #[serde(default)]
    pub design_names: IndexSet<DesignName>,

    /// A hash map of pcb unit number to design index
    /// It's possible that units are not assigned to designs
    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub unit_map: BTreeMap<PcbUnitIndex, DesignIndex>,

    /// A set of gerbers that define the panel boundary, usually rectangular, and all the designs within.
    ///
    /// This occurs when you take multiple designs and place them on a single panel or when you design a panel from
    /// scratch.
    ///
    /// This also frequently occurs when you place a single design in the center of a rectangular panel, especially
    /// when the design is not rectangular and/or will not fit in the machines used for assembly.
    ///
    /// panel gerbers are often provided by a 3rd party when you have a 3rd party do the panelization; You give them
    /// the design gerbers, and they give you the panel gerbers.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub pcb_gerbers: Vec<GerberFile>,

    /// A set of gerbers for each design used on this PCB
    ///
    /// If the PCB only has one design, with no fiducials, then [`pcb_gerbers`] could be used.
    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub design_gerbers: BTreeMap<DesignIndex, Vec<GerberFile>>,

    #[serde(default)]
    pub panel_sizing: PanelSizing,
}

#[derive(Error, Debug)]
pub enum PcbError {
    #[error("Unknown design. name: {0:?}")]
    UnknownDesign(DesignName),

    #[error("Unit index {index} is out of range [{min}..{max}] (inclusive)")]
    UnitIndexOutOfRange {
        index: PcbUnitIndex,
        min: PcbUnitIndex,
        max: PcbUnitIndex,
    },
    #[error("Design index {index} is out of range [{min}..{max}] (inclusive)")]
    DesignIndexOutOfRange {
        index: DesignIndex,
        min: DesignIndex,
        max: DesignIndex,
    },
}

impl PartialEq for Pcb {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.units == other.units
            && self.design_names == other.design_names
            && self.unit_map == other.unit_map
    }
}

impl Pcb {
    pub fn new(
        name: String,
        units: u16,
        design_names: IndexSet<DesignName>,
        unit_map: BTreeMap<PcbUnitIndex, DesignIndex>,
    ) -> Self {
        Self {
            name,
            units,
            design_names,
            unit_map,
            pcb_gerbers: vec![],
            design_gerbers: Default::default(),
            panel_sizing: Default::default(),
        }
    }

    /// returns true if the design exists
    pub fn has_design(&mut self, design_name: &DesignName) -> bool {
        self.design_names
            .iter()
            .any(|candidate| candidate.eq(design_name))
    }

    /// returns the design index if the design exists, otherwise None
    pub fn design_index(&mut self, design_name: &DesignName) -> Option<DesignIndex> {
        self.design_names
            .iter()
            .position(|candidate| candidate.eq(design_name))
    }

    pub fn unique_designs_iter(&self) -> impl Iterator<Item = &DesignName> {
        self.design_names.iter()
    }

    /// If `design` is None, then the changes are applied to the PCB, otherwise they are applied to the design.
    /// returns a [`Result`] containing the modified state of the PCB, or an error.
    pub fn update_gerbers(
        &mut self,
        design: Option<DesignName>,
        files: Vec<(PathBuf, Option<GerberFileFunction>)>,
    ) -> Result<bool, PcbError> {
        let gerbers = self.gerbers_for_pcb_or_design(design)?;
        let mut modified = false;

        for (file, optional_function) in files {
            if let Some(existing_gerber) = gerbers
                .iter_mut()
                .find(|candidate| candidate.file.eq(&file))
            {
                if let Some(function) = optional_function {
                    // change it
                    existing_gerber.function = Some(function);
                    modified |= true;
                }
            } else {
                // add it

                let function = optional_function.or_else(|| {
                    // try and detect the purpose, otherwise leave it as None
                    detect_purpose(file.clone()).ok()
                });

                gerbers.push(GerberFile {
                    file,
                    function,
                });
                modified |= true;
            }
        }

        Ok(modified)
    }

    /// If `design` is None, then the changers are applied to the PCB, otherwise they are applied to the design.
    /// returns a [`Result`] containing the modified state of the PCB, or an error.
    // FUTURE currently this silently ignore paths that were not in the list, but perhaps we should return a result to
    //        allow the user to be informed which files could not be removed.
    pub fn remove_gerbers(
        &mut self,
        design: Option<DesignName>,
        files: Vec<PathBuf>,
    ) -> Result<(bool, Vec<PathBuf>), PcbError> {
        let gerbers = self.gerbers_for_pcb_or_design(design)?;
        let mut modified = false;
        let mut unremoved_files = files;

        unremoved_files.retain(|file| {
            let mut should_remove = false;

            gerbers.retain(|candidate| {
                should_remove = candidate.file.eq(file);
                if should_remove {
                    trace!("Removing gerber file. file: {:?}", file);
                }
                modified |= should_remove;

                !should_remove
            });

            !should_remove
        });

        Ok((modified, unremoved_files))
    }

    fn gerbers_for_pcb_or_design(&mut self, design: Option<DesignName>) -> Result<&mut Vec<GerberFile>, PcbError> {
        let gerbers = match design {
            Some(design_name) => {
                let design_index = self
                    .design_index(&design_name)
                    .ok_or(PcbError::UnknownDesign(design_name))?;

                self.design_gerbers
                    .entry(design_index)
                    .or_default()
            }
            None => &mut self.pcb_gerbers,
        };
        Ok(gerbers)
    }
}

pub fn create_pcb(
    name: String,
    units: u16,
    unit_to_design_name_map: BTreeMap<PcbUnitNumber, DesignName>,
) -> Result<Pcb, PcbOperationError> {
    info!("Creating PCB. name: '{}'", name);

    let (design_names, unit_to_design_index_mapping) = build_unit_to_design_index_mappping(unit_to_design_name_map);

    let design_count = design_names.len();
    let mut pcb = Pcb::new(name, units, design_names, unit_to_design_index_mapping);
    pcb.panel_sizing
        .ensure_unit_positionings(units);
    pcb.panel_sizing
        .ensure_design_sizings(design_count);

    Ok(pcb)
}

pub fn build_unit_to_design_index_mappping(
    unit_to_design_name_map: BTreeMap<PcbUnitNumber, DesignName>,
) -> (IndexSet<DesignName>, BTreeMap<PcbUnitIndex, DesignIndex>) {
    trace!("unit_to_design_name_map: {:?}", unit_to_design_name_map);

    // 'Intern' the DesignNames
    let mut unit_to_design_index_mapping: BTreeMap<PcbUnitIndex, DesignIndex> = BTreeMap::new();
    let mut unique_strings: Vec<DesignName> = Vec::new();
    let mut design_names: IndexSet<DesignName> = IndexSet::new();

    for (pcb_unit_number, design) in unit_to_design_name_map {
        // Insert into unique list if not seen
        let design_index = if let Some(position) = unique_strings
            .iter()
            .position(|s| s == &design)
        {
            position
        } else {
            unique_strings.push(design.clone());
            unique_strings.len() - 1
        };

        design_names.insert(design.clone());
        let pcb_unit_index = pcb_unit_number - 1;
        unit_to_design_index_mapping.insert(pcb_unit_index, design_index);
    }

    info!("Added designs to PCB. design: [{}]", unique_strings.iter().join(", "));
    trace!("unit_to_design_index_mapping: {:?}", unit_to_design_index_mapping);

    (design_names, unit_to_design_index_mapping)
}
