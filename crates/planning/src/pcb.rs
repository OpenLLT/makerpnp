use std::collections::{BTreeMap, BTreeSet};

use pnp::pcb::PcbUnitIndex;
use serde_with::serde_as;

use crate::design::{DesignIndex, DesignName};
use crate::gerber::GerberFile;

/// Defines a PCB
///
/// A PCB can have its own gerber files and gerber files for each design, or not at all.
#[serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Pcb {
    // Reminder: do not store anything in here that is related to the project, e.g. unit assignments were specifically
    //           moved out of this struct.
    //           The intention is that this struct can be independently serialized and deserialized and re-used in
    //           multiple projects.
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

    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    #[serde(default)]
    pub design_names: BTreeSet<DesignName>,

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
    // TODO consider adding fiducials here?  Creates a dependency on the gerber types and requires the gerber units (mil, mm) too.
}

impl Pcb {
    pub fn new(
        name: String,
        units: u16,
        design_names: BTreeSet<DesignName>,
        unit_map: BTreeMap<PcbUnitIndex, DesignIndex>,
    ) -> Self {
        Self {
            name,
            units,
            design_names,
            unit_map,
            pcb_gerbers: vec![],
            design_gerbers: Default::default(),
        }
    }
}
