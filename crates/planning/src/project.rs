use std::cmp::Ordering;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::Error;
use csv::QuoteStyle;
use heck::ToShoutySnakeCase;
use indexmap::IndexSet;
use itertools::Itertools;
use pnp;
use pnp::load_out::LoadOutItem;
use pnp::object_path::ObjectPath;
use pnp::part::Part;
use pnp::pcb::{PcbSide, PcbUnitIndex, PcbUnitNumber};
use pnp::placement::Placement;
use pnp::reference::{Reference, ReferenceError};
use regex::Regex;
use rust_decimal::Decimal;
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use thiserror::Error;
use time::OffsetDateTime;
use tracing::{debug, error, info, trace, warn};
use util::sorting::SortOrder;

use crate::actions::{AddOrRemoveAction, SetOrClearAction};
use crate::design::{DesignIndex, DesignName, DesignVariant};
use crate::file::FileReference;
use crate::gerber::{GerberFile, GerberPurpose};
use crate::operation_history::{
    AutomatedSolderingOperationTaskHistoryKind, LoadPcbsOperationTaskHistoryKind,
    ManualSolderingOperationTaskHistoryKind, OperationHistoryItem, OperationHistoryKind,
    PlaceComponentsOperationTaskHistoryKind, PlacementOperationHistoryKind,
};
use crate::part::PartState;
use crate::pcb::Pcb;
use crate::phase::{Phase, PhaseError, PhaseOrderings, PhaseReference, PhaseState};
use crate::placement::{
    PlacementOperation, PlacementSortingItem, PlacementSortingMode, PlacementState, PlacementStatus,
    ProjectPlacementStatus,
};
use crate::process::{
    OperationDefinition, OperationReference, OperationStatus, ProcessDefinition, ProcessError, ProcessReference,
    ProcessRuleReference, SerializableTaskState, TaskAction, TaskReference, TaskStatus,
};
#[cfg(feature = "markdown")]
use crate::report::project_report_json_to_markdown;
use crate::report::{IssueKind, IssueSeverity, ProjectReportIssue};
use crate::variant::VariantName;
use crate::{file, operation_history, placement, report};

#[serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Project {
    pub name: String,

    /// The *definition* of the processes used by this project.
    pub processes: Vec<ProcessDefinition>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub pcbs: Vec<ProjectPcb>,

    #[serde_as(as = "Vec<(DisplayFromStr, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub design_gerbers: BTreeMap<DesignName, Vec<GerberFile>>,

    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub part_states: BTreeMap<Part, PartState>,

    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub phases: BTreeMap<Reference, Phase>,

    #[serde(skip_serializing_if = "IndexSet::is_empty")]
    #[serde(default)]
    pub phase_orderings: IndexSet<Reference>,

    /// The state of the phases, and the process operations
    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub phase_states: BTreeMap<Reference, PhaseState>,

    #[serde_as(as = "Vec<(DisplayFromStr, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub placements: BTreeMap<ObjectPath, PlacementState>,
}

impl Project {
    pub fn new(name: String) -> Self {
        Self {
            name,
            ..Self::default()
        }
    }

    /// Safety: Silently ignores errors when building unit assignments fails. e.g. pcb not loaded.
    ///
    /// FUTURE improve this so it returns a `Result` with an `Err` if one of the Pcbs has not been loaded.
    pub fn all_unit_assignments(&self) -> Vec<(ObjectPath, DesignVariant)> {
        self.pcbs
            .iter()
            .enumerate()
            .flat_map(|(pcb_index, project_pcb)| {
                project_pcb
                    .unit_assignments()
                    .unwrap_or_default()
                    .into_iter()
                    .enumerate()
                    .filter_map(move |(unit_index, unit_assignment)| {
                        unit_assignment.map(|design_variant| {
                            let mut object_path = ObjectPath::default();
                            object_path.set_pcb_instance(pcb_index as u16 + 1);
                            object_path.set_pcb_unit(unit_index as u16 + 1);

                            (object_path, design_variant)
                        })
                    })
            })
            .collect::<Vec<_>>()
    }

    pub fn ensure_process(&mut self, process: &ProcessDefinition) -> anyhow::Result<()> {
        if !self.processes.contains(process) {
            info!("Adding process to project.  process: '{}'", process.reference);
            self.processes.push(process.clone())
        }
        Ok(())
    }

    /// makes the assignment if possible.
    ///
    /// returns if the assignment was modified (added or changed) or an error.
    pub fn update_assignment(&mut self, object_path: ObjectPath, variant_name: VariantName) -> anyhow::Result<bool> {
        // reminder: instance and pcb_unit are 1-based in the object path

        let Ok((pcb_instance, pcb_unit)) = object_path.pcb_instance_and_unit() else {
            return Err(anyhow::anyhow!(
                "Unable to determine PCB instance and unit from object path: {:?}",
                object_path
            ));
        };

        let pcb_instance_index: u16 = pcb_instance - 1;
        let pcb_unit_index: u16 = pcb_unit - 1;

        let Some(project_pcb) = self
            .pcbs
            .get_mut(pcb_instance_index as usize)
        else {
            return Err(anyhow::anyhow!("Unable to find PCB. instance: {}", pcb_instance));
        };

        let modified = match project_pcb.assign_unit(pcb_unit_index, variant_name.clone()) {
            Ok(None) => {
                info!(
                    "Unit assignment added. unit: '{}', variant_name: {}",
                    object_path, variant_name
                );
                true
            }
            Ok(Some(old_design_variant)) => {
                info!(
                    "Unit assignment updated. unit: '{}', old: {}, new: {}",
                    object_path, old_design_variant, variant_name
                );
                true
            }
            Err(ProjectPcbError::UnitAlreadyAssigned {
                ..
            }) => {
                info!("Unit assignment unchanged.");
                false
            }
            Err(cause) => return Err(anyhow::anyhow!("Unable to assign unit to PCB. cause: {:?}", cause)),
        };

        Ok(modified)
    }

    pub fn update_phase(
        &mut self,
        reference: PhaseReference,
        process_name: ProcessReference,
        load_out_source: String,
        pcb_side: PcbSide,
    ) -> anyhow::Result<()> {
        match self.phases.entry(reference.clone()) {
            Entry::Vacant(entry) => {
                let phase = Phase {
                    reference: reference.clone(),
                    process: process_name.clone(),
                    load_out_source: load_out_source.clone(),
                    pcb_side: pcb_side.clone(),
                    placement_orderings: vec![],
                };
                entry.insert(phase);
                info!(
                    "Created phase. reference: '{}', process: {}, load_out: {:?}",
                    reference, process_name, load_out_source
                );
                self.phase_orderings
                    .insert(reference.clone());
                info!("Phase ordering: {}", PhaseOrderings(&self.phase_orderings));

                let process = self.find_process(&process_name)?;

                self.phase_states
                    .insert(reference, PhaseState::from_process(process));
            }
            Entry::Occupied(mut entry) => {
                let existing_phase = entry.get_mut();
                let old_phase = existing_phase.clone();

                existing_phase.process = process_name;
                existing_phase.load_out_source = load_out_source;

                info!("Updated phase. old: {:?}, new: {:?}", old_phase, existing_phase);
            }
        }

        Ok(())
    }

    pub fn find_process(&self, process_reference: &ProcessReference) -> Result<&ProcessDefinition, ProcessError> {
        self.processes
            .iter()
            .find(|&process| process.reference.eq(&process_reference))
            .ok_or(ProcessError::UndefinedProcessError {
                processes: self.processes.clone(),
                process: process_reference.to_string(),
            })
    }

    /// Warning: Silently ignores errors when building unit assignments fails. e.g. pcb not loaded.
    ///
    /// FUTURE improve this so it returns a `Result` with an `Err` if one of the Pcbs has not been loaded.
    pub fn unique_design_variants(&self) -> HashSet<DesignVariant> {
        self.pcbs
            .iter()
            .filter_map(|pcb| pcb.unit_assignments().ok())
            .flat_map(|unit_assignments| unit_assignments)
            .flatten()
            .collect()
    }

    #[must_use]
    pub fn remove_unused_placements(&mut self, phase_reference: Option<Reference>) -> bool {
        let mut modified = false;

        self.placements
            .retain(|object_path, state| match state.project_status {
                ProjectPlacementStatus::Unused => {
                    let should_remove = match (&phase_reference, &state.phase) {
                        (None, _) => true,
                        (Some(phase), Some(candidate)) if phase.eq(candidate) => true,
                        _ => false,
                    };

                    if should_remove {
                        info!("Removing unknown placement, object_path: {:?}", object_path);
                        modified |= true;
                    }
                    !should_remove
                }
                _ => true,
            });

        modified
    }

    pub fn add_gerbers(&mut self, design: DesignName, files: Vec<(PathBuf, Option<PcbSide>, GerberPurpose)>) -> bool {
        let mut modified = false;
        let gerbers = self
            .design_gerbers
            .entry(design)
            .or_insert(vec![]);

        for (file, pcb_side, purpose) in files {
            if let Some(existing_gerber) = gerbers
                .iter_mut()
                .find(|candidate| candidate.file.eq(&file))
            {
                // change it
                existing_gerber.purpose = purpose;
                existing_gerber.pcb_side = pcb_side;
                modified |= true;
            } else {
                // add it
                gerbers.push(GerberFile {
                    file,
                    purpose,
                    pcb_side,
                });
                modified |= true;
            }
        }

        modified
    }

    // FUTURE currently this silently ignore paths that were not in the list, but perhaps we should return a result to
    //        allow the user to be informed which files could not be removed.
    pub fn remove_gerbers(&mut self, design: DesignName, files: Vec<PathBuf>) -> bool {
        let mut modified = false;
        let gerbers = self
            .design_gerbers
            .entry(design)
            .or_insert(vec![]);

        for file in files {
            gerbers.retain(|candidate| {
                let should_remove = candidate.file.eq(&file);
                modified |= should_remove;

                !should_remove
            });
        }

        modified
    }
}

#[serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ProjectPcb {
    pub pcb_file: FileReference,

    /// Loaded from the path specified by `pcb_file`
    #[serde(skip)]
    pub pcb: Option<Pcb>,

    /// Individual units can have a design variant assigned.
    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    // TODO remove `DesignIndex` from this tuple, calling code should use `pcb.design_to_unit_mapping` to avoid data mismatch
    pub unit_assignments: BTreeMap<PcbUnitIndex, (DesignIndex, VariantName)>,
}

impl ProjectPcb {
    pub fn new(pcb_file: FileReference, pcb: Pcb) -> Self {
        Self {
            pcb_file,
            pcb: Some(pcb),
            unit_assignments: BTreeMap::default(),
        }
    }

    pub fn load(&mut self, project_directory: &Path) -> Result<(), std::io::Error> {
        let pcb = file::load(
            &self
                .pcb_file
                .build_path(&project_directory),
        )?;
        self.pcb = Some(pcb);
        Ok(())
    }

    pub fn save(&mut self, project_path: &PathBuf) -> Result<(), std::io::Error> {
        if let Some(pcb) = &self.pcb {
            file::save(pcb, &self.pcb_file.build_path(project_path))?;
        }
        Ok(())
    }

    pub fn pcb(&self) -> Option<&Pcb> {
        self.pcb.as_ref()
    }

    pub fn unit_assignments(&self) -> Result<Vec<Option<DesignVariant>>, ProjectPcbError> {
        let Some(pcb) = &self.pcb else {
            return Err(ProjectPcbError::NotLoaded);
        };

        let mut unit_assignments = vec![None; pcb.units as usize];

        for (unit_index, (design_index, variant_name)) in self.unit_assignments.iter() {
            unit_assignments[*unit_index as usize] = Some(DesignVariant {
                design_name: pcb
                    .design_names
                    .iter()
                    .nth(*design_index as usize)
                    .unwrap()
                    .clone(),
                variant_name: variant_name.clone(),
            });
        }

        Ok(unit_assignments)
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
    pub fn assign_unit(
        &mut self,
        unit: u16,
        variant_name: VariantName,
    ) -> Result<Option<VariantName>, ProjectPcbError> {
        let Some(pcb) = &self.pcb else {
            return Err(ProjectPcbError::NotLoaded);
        };

        if unit >= pcb.units {
            return Err(ProjectPcbError::UnitOutOfRange {
                unit,
                min: 0,
                max: pcb.units - 1,
            });
        }

        let design_index: DesignIndex = *pcb.unit_map.get(&unit).unwrap();

        match self.unit_assignments.entry(unit) {
            Entry::Vacant(entry) => {
                entry.insert((design_index, variant_name));
                Ok(None)
            }
            Entry::Occupied(mut entry) => {
                let (other_design_index, other_variant_name) = entry.get();
                if other_design_index.eq(&design_index) && other_variant_name.eq(&variant_name) {
                    return Err(ProjectPcbError::UnitAlreadyAssigned {
                        unit,
                    });
                }

                let old_assigment = entry.insert((design_index, variant_name));

                Ok(Some(old_assigment.1))
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum ProjectPcbError {
    #[error("Unit {unit} is out of range [{min}..{max}] (inclusive)")]
    UnitOutOfRange { unit: u16, min: u16, max: u16 },
    #[error("Unit {unit} has already been assigned")]
    UnitAlreadyAssigned { unit: u16 },

    #[error("Unknown design. design: {0}.  Assign the design to the PCB.")]
    UnknownDesign(DesignName),

    #[error("Project PCB not loaded")]
    NotLoaded,
}

#[derive(Error, Debug)]
pub enum ProcessFactoryError {
    #[error("Unknown error, reason: {reason:?}")]
    ErrorCreatingProcessReference { reason: ReferenceError },
    #[error("unknown process.  process: {}", process)]
    UnknownProcessName { process: String },
}

pub struct ProcessFactory {}

impl ProcessFactory {
    pub fn by_name(name: &str) -> Result<ProcessDefinition, ProcessFactoryError> {
        let process_name =
            ProcessReference::from_str(name).map_err(|e| ProcessFactoryError::ErrorCreatingProcessReference {
                reason: e,
            })?;

        // FUTURE add support for more named processes

        match name {
            "pnp" => Ok(ProcessDefinition {
                reference: process_name,
                operations: vec![
                    OperationDefinition {
                        reference: Reference::from_raw_str("load_pcbs"),
                        tasks: vec![TaskReference::from_raw_str("core::load_pcbs")],
                    },
                    OperationDefinition {
                        reference: Reference::from_raw_str("automated_pnp"),
                        tasks: vec![TaskReference::from_raw_str("core::place_components")],
                    },
                    OperationDefinition {
                        reference: Reference::from_raw_str("reflow_oven_soldering"),
                        tasks: vec![TaskReference::from_raw_str("core::automated_soldering")],
                    },
                ],
                rules: vec![ProcessRuleReference::from_raw_str("core::unique_feeder_references")],
            }),
            "manual" => Ok(ProcessDefinition {
                reference: process_name,
                operations: vec![
                    OperationDefinition {
                        reference: Reference::from_raw_str("load_pcbs"),
                        tasks: vec![TaskReference::from_raw_str("core::load_pcbs")],
                    },
                    OperationDefinition {
                        reference: Reference::from_raw_str("manually_solder_components"),
                        tasks: vec![
                            TaskReference::from_raw_str("core::place_components"),
                            TaskReference::from_raw_str("core::manual_soldering"),
                        ],
                    },
                ],
                rules: vec![],
            }),
            _ => Err(ProcessFactoryError::UnknownProcessName {
                process: process_name.to_string(),
            }),
        }
    }
}

impl Default for Project {
    fn default() -> Self {
        Self {
            name: "Unnamed".to_string(),
            processes: vec![
                ProcessFactory::by_name("pnp").unwrap(),
                ProcessFactory::by_name("manual").unwrap(),
            ],
            pcbs: vec![],
            design_gerbers: Default::default(),
            part_states: Default::default(),
            phases: Default::default(),
            placements: Default::default(),
            phase_orderings: Default::default(),
            phase_states: Default::default(),
        }
    }
}

#[derive(Error, Debug)]
pub enum PcbOperationError {
    #[error("Unknown error")]
    Unknown,
    #[error("PCB not loaded")]
    PcbNotLoaded,
}

pub fn add_pcb(
    path: &PathBuf,
    project: &mut Project,
    name: String,
    units: u16,
    unit_to_design_name_map: BTreeMap<PcbUnitNumber, DesignName>,
) -> Result<(), PcbOperationError> {
    info!("Added PCB. name: '{}'", name);
    trace!("unit_to_design_name_map: {:?}", unit_to_design_name_map);

    // 'Intern' the DesignNames
    let mut unit_to_design_index_mapping: BTreeMap<PcbUnitIndex, DesignIndex> = BTreeMap::new();
    let mut unique_strings: Vec<DesignName> = Vec::new();
    let mut design_names: BTreeSet<DesignName> = BTreeSet::new();

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

    let pcb_file_name = format!("{}_.pcb.json", name);

    let pcb = Pcb::new(name, units, design_names, unit_to_design_index_mapping);

    let mut pcb_path = path.clone();
    pcb_path.push(pcb_file_name.clone());

    let pcb_file = FileReference::Relative(PathBuf::from(pcb_file_name));

    project
        .pcbs
        .push(ProjectPcb::new(pcb_file, pcb));
    Ok(())
}

#[derive(Error, Debug)]
pub enum ArtifactGenerationError {
    #[error("Unable to generate phase placements. cause: {0:}")]
    PhasePlacementsGenerationError(Error),

    #[error("Unable to load items. source: {load_out_source}, error: {reason}")]
    UnableToLoadItems { load_out_source: String, reason: Error },

    #[error("Unable to generate report. error: {reason}")]
    ReportGenerationError { reason: Error },

    #[error("Unable to save report. cause: {reason:}")]
    UnableToSaveReport { reason: Error },
}

pub fn generate_artifacts(
    project: &Project,
    directory: &Path,
    phase_load_out_items_map: BTreeMap<Reference, Vec<LoadOutItem>>,
) -> Result<(), ArtifactGenerationError> {
    let mut issues: BTreeSet<ProjectReportIssue> = BTreeSet::new();

    for reference in project.phase_orderings.iter() {
        let phase = project.phases.get(reference).unwrap();

        let load_out_items = phase_load_out_items_map
            .get(reference)
            .unwrap();

        generate_phase_artifacts(project, phase, load_out_items.as_slice(), directory, &mut issues)?;
    }

    let report = report::project_generate_report(project, &phase_load_out_items_map, &mut issues);

    let report_file_path = report::build_report_file_path(&project.name, directory);

    report::project_report_save_as_json(&report, &report_file_path).map_err(|err| {
        ArtifactGenerationError::UnableToSaveReport {
            reason: err,
        }
    })?;

    #[cfg(feature = "markdown")]
    project_report_json_to_markdown(&report_file_path).map_err(|err| ArtifactGenerationError::UnableToSaveReport {
        reason: err.into(),
    })?;

    info!("Generated artifacts.");

    Ok(())
}

fn generate_phase_artifacts(
    project: &Project,
    phase: &Phase,
    load_out_items: &[LoadOutItem],
    directory: &Path,
    issues: &mut BTreeSet<ProjectReportIssue>,
) -> Result<(), ArtifactGenerationError> {
    let mut placement_states: Vec<(&ObjectPath, &PlacementState)> = project
        .placements
        .iter()
        .filter_map(|(object_path, state)| match &state.phase {
            Some(placement_phase) if placement_phase.eq(&phase.reference) => Some((object_path, state)),
            _ => None,
        })
        .collect();

    sort_placements(&mut placement_states, &phase.placement_orderings, load_out_items);

    for (_object_path, placement_state) in placement_states.iter() {
        let feeder_reference =
            match pnp::load_out::find_load_out_item_by_part(load_out_items, &placement_state.placement.part) {
                Some(load_out_item) => load_out_item.reference.clone(),
                _ => None,
            };

        if feeder_reference.is_none() {
            let issue = ProjectReportIssue {
                message: "A part has not been assigned to a feeder".to_string(),
                severity: IssueSeverity::Warning,
                kind: IssueKind::UnassignedPartFeeder {
                    part: placement_state.placement.part.clone(),
                },
            };
            issues.insert(issue);
        };
    }

    let mut phase_placements_path = PathBuf::from(directory);
    phase_placements_path.push(format!("{}_placements.csv", phase.reference));

    store_phase_placements_as_csv(&phase_placements_path, &placement_states, load_out_items)
        .map_err(|e| ArtifactGenerationError::PhasePlacementsGenerationError(e))?;

    info!(
        "Generated phase placements. phase: '{}', path: {:?}",
        phase.reference, phase_placements_path
    );

    Ok(())
}

pub fn sort_placements(
    placement_states: &mut Vec<(&ObjectPath, &PlacementState)>,
    placement_orderings: &[PlacementSortingItem],
    load_out_items: &[LoadOutItem],
) {
    placement_states.sort_by(
        |(object_path_a, placement_state_a), (object_path_b, placement_state_b)| {
            placement_orderings
                .iter()
                .fold(Ordering::Equal, |mut acc, sort_ordering| {
                    if !matches!(acc, Ordering::Equal) {
                        return acc;
                    }
                    acc = match sort_ordering.mode {
                        PlacementSortingMode::FeederReference => {
                            let feeder_reference_a = match pnp::load_out::find_load_out_item_by_part(
                                load_out_items,
                                &placement_state_a.placement.part,
                            ) {
                                Some(load_out_item) => load_out_item.reference.clone(),
                                _ => None,
                            };
                            let feeder_reference_b = match pnp::load_out::find_load_out_item_by_part(
                                load_out_items,
                                &placement_state_b.placement.part,
                            ) {
                                Some(load_out_item) => load_out_item.reference.clone(),
                                _ => None,
                            };

                            trace!(
                                "Comparing feeder references. feeder_reference_a: '{:?}' feeder_reference_a: '{:?}'",
                                feeder_reference_a,
                                feeder_reference_b
                            );
                            feeder_reference_a.cmp(&feeder_reference_b)
                        }
                        PlacementSortingMode::PcbUnit => {
                            let pcb_unit_a = object_path_a.pcb_unit_path();
                            let pcb_unit_b = object_path_b.pcb_unit_path();

                            trace!(
                                "Comparing pcb units, pcb_unit_a: '{:?}', pcb_unit_b: '{:?}'",
                                pcb_unit_a,
                                pcb_unit_b
                            );
                            pcb_unit_a.cmp(&pcb_unit_b)
                        }
                        PlacementSortingMode::RefDes => {
                            trace!(
                                "Comparing ref-des, ref_des_a: '{:?}', ref_des_b: '{:?}'",
                                placement_state_a.placement.ref_des,
                                placement_state_b.placement.ref_des,
                            );

                            placement_state_a
                                .placement
                                .ref_des
                                .cmp(&placement_state_b.placement.ref_des)
                        }
                    };

                    match sort_ordering.sort_order {
                        SortOrder::Asc => acc,
                        SortOrder::Desc => acc.reverse(),
                    }
                })
        },
    );
}

#[serde_as]
#[derive(Debug, serde::Serialize)]
#[serde(rename_all(serialize = "PascalCase"))]
pub struct PhasePlacementRecord {
    #[serde_as(as = "DisplayFromStr")]
    pub object_path: ObjectPath,

    pub feeder_reference: Option<Reference>,
    pub manufacturer: String,
    pub mpn: String,
    pub x: Decimal,
    pub y: Decimal,
    pub rotation: Decimal,
}

pub fn store_phase_placements_as_csv(
    output_path: &PathBuf,
    placement_states: &[(&ObjectPath, &PlacementState)],
    load_out_items: &[LoadOutItem],
) -> Result<(), Error> {
    trace!("Writing phase placements. output_path: {:?}", output_path);

    let mut writer = csv::WriterBuilder::new()
        .quote_style(QuoteStyle::Always)
        .from_path(output_path)?;

    for (object_path, placement_state) in placement_states.iter() {
        let feeder_reference =
            match pnp::load_out::find_load_out_item_by_part(&load_out_items, &placement_state.placement.part) {
                Some(load_out_item) => load_out_item.reference.clone(),
                _ => None,
            };

        writer.serialize(PhasePlacementRecord {
            object_path: (*object_path).clone(),
            feeder_reference,
            manufacturer: placement_state
                .placement
                .part
                .manufacturer
                .to_string(),
            mpn: placement_state
                .placement
                .part
                .mpn
                .to_string(),
            x: placement_state.placement.x,
            y: placement_state.placement.y,
            rotation: placement_state.placement.rotation,
        })?;
    }

    writer.flush()?;

    Ok(())
}

pub fn assign_placements_to_phase(
    project: &mut Project,
    phase: &Phase,
    action: SetOrClearAction,
    placements_pattern: Regex,
) -> BTreeSet<Part> {
    let mut required_load_out_parts = BTreeSet::new();

    debug!(
        "Assigning phase placements to {:?}, action: {:?}, pattern: {:?}",
        phase, action, placements_pattern
    );
    let matched_placements: Vec<(&ObjectPath, &mut PlacementState)> = project
        .placements
        .iter_mut()
        .filter(|(path, state)| {
            let path_str = format!("{}", path);

            placements_pattern.is_match(&path_str)
                && state
                    .placement
                    .pcb_side
                    .eq(&phase.pcb_side)
        })
        .collect();

    trace!("matched_placements: {:?}", matched_placements);

    for (placement_path, state) in matched_placements {
        // FUTURE consider refactoring this into the filter above, and then working on the remaining results...
        match action {
            SetOrClearAction::Set => {
                let should_assign = match &state.phase {
                    // different
                    Some(assigned_phase) if !assigned_phase.eq(&phase.reference) => true,
                    // none
                    None => true,
                    // same (ignore)
                    _ => false,
                };

                if should_assign {
                    info!(
                        "Assigning placement to phase. phase: {}, placement_path: {}",
                        phase.reference, placement_path
                    );
                    state.phase = Some(phase.reference.clone());
                }
            }
            SetOrClearAction::Clear => {
                let should_remove = match &state.phase {
                    // different
                    Some(assigned_phase) if !assigned_phase.eq(&phase.reference) => false,
                    // none (ignore)
                    None => false,
                    // same
                    _ => true,
                };

                if should_remove {
                    info!(
                        "Removing placement from phase. phase: {}, placement_path: {}",
                        phase.reference, placement_path
                    );
                    state.phase.take();
                }
            }
        }

        let _inserted = required_load_out_parts.insert(state.placement.part.clone());
    }

    required_load_out_parts
}

pub struct ProjectRefreshResult {
    pub modified: bool,
    pub unique_parts: Vec<Part>,
}

pub fn refresh_from_design_variants(
    project: &mut Project,
    design_variant_placement_map: BTreeMap<DesignVariant, Vec<Placement>>,
) -> ProjectRefreshResult {
    let unique_parts = placement::build_unique_parts(&design_variant_placement_map);

    let mut modified = refresh_parts(project, unique_parts.as_slice());

    modified |= refresh_placements(project, &design_variant_placement_map);

    ProjectRefreshResult {
        modified,
        unique_parts,
    }
}

/// Returns 'true' if project is modified
fn refresh_placements(
    project: &mut Project,
    design_variant_placement_map: &BTreeMap<DesignVariant, Vec<Placement>>,
) -> bool {
    let changes: Vec<(Change, ObjectPath, Placement)> = find_placement_changes(project, design_variant_placement_map);

    let mut modified = false;

    for (change, unit_path, placement) in changes.iter() {
        let mut path: ObjectPath = unit_path.clone();
        path.set_ref_des(placement.ref_des.clone());

        let placement_state_entry = project.placements.entry(path);

        match (change, placement) {
            (Change::New, placement) => {
                info!("New placement. placement: {:?}", placement);
                modified |= true;

                let placement_state = PlacementState {
                    unit_path: unit_path.clone(),
                    placement: placement.clone(),
                    operation_status: PlacementStatus::Pending,
                    project_status: ProjectPlacementStatus::Used,
                    phase: None,
                };

                placement_state_entry.or_insert(placement_state);
            }
            (Change::Existing, _) => {
                placement_state_entry.and_modify(|ps| {
                    if !ps.placement.eq(placement) {
                        info!("Updating placement. old: {:?}, new: {:?}", ps.placement, placement);
                        modified |= true;
                        ps.placement = placement.clone();
                    }
                });
            }
            (Change::Unused, placement) => {
                info!("Marking placement as unused. placement: {:?}", placement);
                modified |= true;

                placement_state_entry.and_modify(|ps| {
                    ps.project_status = ProjectPlacementStatus::Unused;
                });
            }
        }
    }

    modified
}

fn find_placement_changes(
    project: &mut Project,
    design_variant_placement_map: &BTreeMap<DesignVariant, Vec<Placement>>,
) -> Vec<(Change, ObjectPath, Placement)> {
    let mut changes: Vec<(Change, ObjectPath, Placement)> = vec![];

    let unit_assignments = project
        .all_unit_assignments()
        .into_iter()
        .map(|(path, unit_assignment)| (path, unit_assignment.clone()))
        .collect::<Vec<_>>();

    // find new or existing placements that are in the updated design_variant_placement_map

    for (design_variant, placements) in design_variant_placement_map.iter() {
        for (unit_path, assignment_design_variant) in unit_assignments.iter() {
            if !design_variant.eq(assignment_design_variant) {
                continue;
            }

            for placement in placements {
                let mut path: ObjectPath = unit_path.clone();
                path.set_ref_des(placement.ref_des.clone());

                // look for a placement state for the placement for this object path

                match project.placements.contains_key(&path) {
                    true => changes.push((Change::Existing, unit_path.clone(), placement.clone())),
                    false => changes.push((Change::New, unit_path.clone(), placement.clone())),
                }
            }
        }
    }

    // find the placements that we knew about previously, but that are no-longer in the design_variant_placement_map

    for (path, state) in project.placements.iter_mut() {
        for (unit_path, design_variant) in unit_assignments.iter() {
            let path_str = path.to_string();
            let unit_path_str = unit_path.to_string();
            let is_matched_unit = path_str.starts_with(&unit_path_str);
            trace!(
                "path_str: {}, unit_path_str: {}, is_matched_unit: {}",
                path_str,
                unit_path_str,
                is_matched_unit
            );

            if is_matched_unit {
                if let Some(placements) = design_variant_placement_map.get(design_variant) {
                    match placements.iter().find(|placement| {
                        placement
                            .ref_des
                            .eq(&state.placement.ref_des)
                    }) {
                        Some(_) => {
                            trace!("known placement");
                        }
                        None => {
                            trace!("unknown placement");
                            match state.project_status {
                                ProjectPlacementStatus::Unused => (),
                                ProjectPlacementStatus::Used => {
                                    changes.push((Change::Unused, unit_path.clone(), state.placement.clone()))
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    debug!("placement changes:\n{:?}", changes);

    changes
}

#[derive(Debug)]
enum Change {
    New,
    Existing,
    Unused,
}

/// Returns 'true' if any changes were made.
fn refresh_parts(project: &mut Project, all_parts: &[Part]) -> bool {
    let changes = find_part_changes(project, all_parts);

    let mut modified = false;

    for change_item in changes.iter() {
        match change_item {
            (Change::New, part) => {
                info!("New part. part: {:?}", part);
                modified = true;
                let _ = project
                    .part_states
                    .entry(part.clone())
                    .or_default();
            }
            (Change::Existing, _part) => {}
            (Change::Unused, part) => {
                info!("Removing unused part. part: {:?}", part);
                modified = true;
                let _ = project.part_states.remove(&part);
            }
        }
    }

    modified
}

fn find_part_changes(project: &mut Project, all_parts: &[Part]) -> Vec<(Change, Part)> {
    let mut changes: Vec<(Change, Part)> = vec![];

    for part in all_parts.iter() {
        match project.part_states.contains_key(part) {
            true => changes.push((Change::Existing, part.clone())),
            false => changes.push((Change::New, part.clone())),
        }
    }

    for (part, _process) in project.part_states.iter() {
        if !all_parts.contains(part) {
            changes.push((Change::Unused, part.clone()))
        }
    }

    debug!("part changes:\n{:?}", changes);

    changes
}

#[must_use]
pub fn update_applicable_processes(
    project: &mut Project,
    all_parts: &[Part],
    process: ProcessDefinition,
    action: AddOrRemoveAction,
    manufacturer_pattern: Regex,
    mpn_pattern: Regex,
) -> bool {
    let mut modified = false;
    let changes = find_part_changes(project, all_parts);

    for change in changes.iter() {
        match change {
            (Change::Existing, part) => {
                if manufacturer_pattern.is_match(part.manufacturer.as_str()) && mpn_pattern.is_match(part.mpn.as_str())
                {
                    project
                        .part_states
                        .entry(part.clone())
                        .and_modify(|part_state| {
                            modified |= match action {
                                AddOrRemoveAction::Add => {
                                    add_process_to_part(part_state, part, process.reference.clone())
                                }
                                AddOrRemoveAction::Remove => {
                                    remove_process_from_part(part_state, part, process.reference.clone())
                                }
                            }
                        });
                }
            }
            _ => {
                panic!("unexpected change. change: {:?}", change);
            }
        }
    }

    modified
}

#[must_use]
pub fn add_process_to_part(part_state: &mut PartState, part: &Part, process: ProcessReference) -> bool {
    let inserted = part_state
        .applicable_processes
        .insert(process);

    if inserted {
        info!(
            "Added process. part: {:?}, applicable_processes: {:?}",
            part,
            part_state
                .applicable_processes
                .iter()
                .map(|it| it.to_string())
                .collect::<Vec<String>>()
        );
    }

    inserted
}

#[must_use]
pub fn remove_process_from_part(part_state: &mut PartState, part: &Part, process: ProcessReference) -> bool {
    let removed = part_state
        .applicable_processes
        .remove(&process);

    if removed {
        info!(
            "Removed process. part: {:?}, applicable_processes: {:?}",
            part,
            part_state
                .applicable_processes
                .iter()
                .map(|it| it.to_string())
                .collect::<Vec<String>>()
        );
    }

    removed
}

pub fn update_placements_operation(
    project: &mut Project,
    directory: &Path,
    object_path_patterns: Vec<Regex>,
    placement_operation: PlacementOperation,
) -> anyhow::Result<bool> {
    let mut modified = false;

    // first, find the only tasks for each phase that allow placement changes.

    let phase_operation_task_map = project
        .phase_states
        .iter()
        .filter_map(|(phase_reference, phase_state)| {
            let operation_and_task_references = phase_state
                .operation_states
                .iter()
                .find_map(|operation_state| {
                    operation_state
                        .task_states
                        .iter()
                        .find_map(|(task_reference, task_state)| {
                            match task_state.requires_placements() && task_state.status() != TaskStatus::Abandoned {
                                true => Some((operation_state.reference.clone(), task_reference.clone())),
                                false => None,
                            }
                        })
                });
            operation_and_task_references.map(|(operation_reference, task_reference)| {
                (
                    phase_reference.clone(),
                    (operation_reference.clone(), task_reference.clone()),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();

    let mut history_item_map: HashMap<Reference, Vec<Box<dyn OperationHistoryKind>>> = HashMap::new();

    for object_path_pattern in object_path_patterns.iter() {
        let placements: Vec<_> = project
            .placements
            .iter_mut()
            .filter(|(object_path, _placement_state)| object_path_pattern.is_match(&object_path.to_string()))
            .collect();

        if placements.is_empty() {
            warn!(
                "Unmatched object path pattern. object_path_pattern: {}",
                object_path_pattern
            );
        }

        for (object_path, placement_state) in placements {
            if placement_state.phase.is_none() {
                // we cannot modify placement state when no phase is assigned.
                continue;
            }

            let placement_phase_reference = placement_state.phase.as_ref().unwrap();

            let phase_map_entry = phase_operation_task_map.get(placement_phase_reference);
            if phase_map_entry.is_none() {
                // if a phase doesn't have a map entry then we cannot update any placement with that phase reference
                continue;
            }

            let should_log = match placement_operation {
                PlacementOperation::Place => match placement_state.operation_status {
                    PlacementStatus::Placed => {
                        warn!("Placement already marked as placed. object_path: {}", object_path);
                        false
                    }
                    PlacementStatus::Skipped => {
                        warn!("Placement was previously skipped. object_path: {}", object_path);
                        placement_state.operation_status = PlacementStatus::Placed;
                        modified = true;
                        true
                    }
                    PlacementStatus::Pending => {
                        info!("Placement marked as placed. object_path: {}", object_path);
                        placement_state.operation_status = PlacementStatus::Placed;
                        modified = true;
                        true
                    }
                },
                PlacementOperation::Reset => match placement_state.operation_status {
                    PlacementStatus::Placed | PlacementStatus::Skipped => {
                        info!("Resetting placed flag. object_path: {}", object_path);
                        placement_state.operation_status = PlacementStatus::Pending;
                        modified = true;
                        true
                    }
                    PlacementStatus::Pending => {
                        warn!("Placed flag already pending. object_path: {}", object_path);
                        false
                    }
                },
                PlacementOperation::Skip => match placement_state.operation_status {
                    PlacementStatus::Placed => {
                        warn!("Placement was previously placed. object_path: {}", object_path);
                        placement_state.operation_status = PlacementStatus::Skipped;
                        modified = true;
                        true
                    }
                    PlacementStatus::Skipped => {
                        warn!("Placement already marked as skipped. object_path: {}", object_path);
                        false
                    }
                    PlacementStatus::Pending => {
                        info!("Placement marked as skipped. object_path: {}", object_path);
                        placement_state.operation_status = PlacementStatus::Skipped;
                        modified = true;
                        true
                    }
                },
            };

            if should_log {
                let task_history = Box::new(PlacementOperationHistoryKind {
                    object_path: object_path.clone(),
                    operation: placement_operation.clone(),
                }) as Box<dyn OperationHistoryKind>;

                let history_items = history_item_map
                    .entry(placement_phase_reference.clone())
                    .or_default();

                history_items.push(task_history);

                modified = true;
            }
        }
    }

    if modified {
        let states_modified = refresh_phase_operation_states(project);
        // redundant, but consistent.
        modified |= states_modified;

        for (phase_reference, task_histories) in history_item_map {
            // Safety: code above should prevent this unwrap from failing
            let (operation_reference, _task_reference) = phase_operation_task_map
                .get(&phase_reference)
                .unwrap();

            let now = OffsetDateTime::now_utc();

            let history_items = task_histories
                .into_iter()
                .map(|task_history| OperationHistoryItem {
                    date_time: now,
                    phase: phase_reference.clone(),
                    extra: Default::default(),
                    operation_reference: operation_reference.clone(),
                    task_reference: TaskReference::from_raw_str("core::place_components"),
                    task_history,
                })
                .collect::<Vec<_>>();

            let mut phase_log_path = PathBuf::from(directory);
            phase_log_path.push(format!("{}_log.json", phase_reference));

            let mut operation_history: Vec<OperationHistoryItem> = operation_history::read_or_default(&phase_log_path)?;

            operation_history.extend(history_items);

            operation_history::write(phase_log_path, &operation_history)?;
        }
    }

    Ok(modified)
}

/// Sometimes it's necessary to refresh the phase operation states
///
/// e.g.
/// 1) adding a placements may make a complete phase incomplete and removing
/// 2) and removing placements my make a phase complete
pub fn refresh_phase_operation_states(project: &mut Project) -> bool {
    let mut modified = false;

    for (phase_reference, phase_state) in project.phase_states.iter_mut() {
        trace!("reference: {:?}, phase_state: {:?}", phase_reference, phase_state);

        for operation_state in phase_state.operation_states.iter_mut() {
            trace!(
                "operation: {:?}, operation_state: {:?}",
                operation_state.reference,
                operation_state
            );

            // FUTURE optimize this if there's ever more then one task that needs placements

            for (task_reference, task_state) in operation_state.task_states.iter_mut() {
                trace!("task: {:?}, task_state: {:?}", task_reference, task_state);

                if task_state.requires_placements() {
                    let original_task_state = dyn_clone::clone(task_state);

                    {
                        let placement_api = task_state
                            .placements_state_mut()
                            .unwrap();
                        placement_api.reset();

                        let phase_placements = project.placements
                            .iter()
                            .filter(|(_object_path, placement_state)|
                                matches!(&placement_state.phase, Some(candidate_phase_reference) if candidate_phase_reference.eq(phase_reference))
                            ).collect::<Vec<_>>();

                        let total = phase_placements.len();
                        placement_api.set_total_placements(total);

                        for (_object_path, placement) in phase_placements {
                            placement_api
                                .on_placement_status_change(&PlacementStatus::Pending, &placement.operation_status);
                        }

                        debug!("summary: {:?}", placement_api.summary());
                    }

                    trace!(
                        "Refreshing placement task state complete.  before: {:?}, after: {:?}",
                        original_task_state,
                        task_state
                    );

                    let state_updated = task_state != &original_task_state;

                    info!(
                        "Refreshed placement task state. phase: {}, operation: {}, task: {}, status: {}, updated: {}",
                        phase_reference,
                        operation_state.reference,
                        task_reference,
                        task_state.status(),
                        state_updated
                    );

                    modified |= state_updated;
                }
            }
        }
    }
    modified
}

#[derive(Error, Debug)]
pub enum PartStateError {
    #[error("No part state found. manufacturer: {}, mpn: {}", part.manufacturer, part.mpn)]
    NoPartStateFound { part: Part },
}

#[cfg(test)]
mod apply_phase_operation_task_action_tests {
    use indexmap::IndexMap;
    use pnp::reference::Reference;
    use rstest::rstest;

    use crate::phase;
    use crate::phase::PhaseState;
    use crate::process::TaskAction;
    use crate::process::{OperationState, SerializableTaskState, TaskReference, TaskStatus};
    use crate::project::{Project, TaskActionError};

    #[rstest]
    #[case(
        TaskAction::Start,
        ("p1", "o1", "core::load_pcbs"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Pending),
                ])
            ])
        ],
        Ok(true)
    )]
    #[case(
        TaskAction::Start,
        ("p1", "o1", "core::place_components"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Complete),
                    ("core::place_components", TaskStatus::Pending),
                ])
            ]),
        ],
        Ok(true)
    )]
    #[case(
        TaskAction::Start,
        ("p2", "o2", "core::manual_soldering"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Complete),
                ]),
                ("o2", vec![
                    // Cannot create place_components task with completed state, requires placements to be placed.
                    //("core::place_components", TaskStatus::Complete),
                    // Instead use a test-only task.
                    ("core::test_task", TaskStatus::Complete),
                ]),
                ("o3", vec![
                    ("core::automated_soldering", TaskStatus::Complete),
                ]),
            ]),
            ("p2", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Complete),
                ]),
                ("o2", vec![
                    // Cannot create place_components task with completed state, requires placements to be placed.
                    //("core::place_components", TaskStatus::Complete),
                    // Instead use a test-only task.
                    ("core::test_task", TaskStatus::Complete),
                    ("core::manual_soldering", TaskStatus::Pending),
                ])
            ])
        ],
        Ok(true)
    )]
    #[case(
        TaskAction::Complete,
        ("p1", "o1", "core::place_components"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Complete),
                    ("core::place_components", TaskStatus::Started),
                ])
            ])
        ],
        Ok(true)
    )]
    #[case(
        TaskAction::Abandon,
        ("p1", "o1", "core::place_components"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Complete),
                    ("core::place_components", TaskStatus::Started),
                ])
            ])
        ],
        Ok(true)
    )]
    #[case(
        TaskAction::Start,
        ("p1", "o2", "core::place_components"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Started)
                ]),
                ("o2", vec![
                    ("core::place_components", TaskStatus::Pending)
                ]),
            ])
        ],
        Err(TaskActionError::PrecedingOperationNotComplete)
    )]
    #[case(
        TaskAction::Start,
        ("p1", "o1", "core::place_components"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Pending),
                    ("core::place_components", TaskStatus::Pending),
                ])
            ])
        ],
        Err(TaskActionError::PrecedingTaskNotComplete)
    )]
    #[case(
        TaskAction::Start,
        ("p1", "o1", "core::load_pcbs"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Started)
                ])
            ])
        ],
        Err(TaskActionError::TaskAlreadyStarted)
    )]
    #[case(
        TaskAction::Complete,
        ("p1", "o1", "core::load_pcbs"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Complete)
                ])
            ])
        ],
        Err(TaskActionError::TaskAlreadyComplete)
    )]
    #[case(
        TaskAction::Abandon,
        ("p1", "o1", "core::load_pcbs"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Abandoned)
                ])
            ])
        ],
        Err(TaskActionError::TaskAlreadyAbandoned)
    )]
    #[case(
        TaskAction::Complete,
        ("p1", "o1", "core::load_pcbs"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Pending)
                ])
            ])
        ],
        Err(TaskActionError::TaskNotStarted)
    )]
    pub fn can_apply_action(
        #[case] action: TaskAction,
        #[case] references: (&str, &str, &str),
        #[case] phase_operation_task_status_map: Vec<(&str, Vec<(&str, Vec<(&str, TaskStatus)>)>)>,
        #[case] expected_result: Result<bool, TaskActionError>,
    ) {
        let (phase_reference, operation_reference, task_reference) = {
            (
                Reference::from_raw_str(references.0),
                Reference::from_raw_str(references.1),
                TaskReference::from_raw_str(references.2),
            )
        };

        let mut project = Project::default();
        for (phase_reference, operation_task_status_map) in phase_operation_task_status_map.iter() {
            let operation_states = operation_task_status_map
                .iter()
                .map(|(operation_reference, task_status_map)| {
                    let task_states = task_status_map
                        .iter()
                        .map(|(task_reference, task_status)| {
                            let mut task_state = phase::make_task_state(&TaskReference::from_raw_str(task_reference));
                            match task_status {
                                // Default state is pending
                                TaskStatus::Pending => {}

                                TaskStatus::Started => task_state.set_started(),
                                TaskStatus::Complete => task_state.set_completed(),
                                TaskStatus::Abandoned => task_state.set_abandoned(),
                            }

                            (TaskReference::from_raw_str(task_reference), task_state)
                        })
                        .collect::<Vec<(TaskReference, Box<dyn SerializableTaskState>)>>();
                    let operation_state = OperationState {
                        reference: Reference::from_raw_str(operation_reference),
                        task_states: IndexMap::from_iter(task_states),
                    };
                    operation_state
                })
                .collect::<Vec<OperationState>>();

            let phase_state = PhaseState {
                operation_states,
            };
            let _ = project
                .phase_states
                .insert(Reference::from_raw_str(phase_reference), phase_state);
        }

        // when
        let result = super::can_apply_action(
            &mut project,
            &phase_reference,
            &operation_reference,
            &task_reference,
            &action,
        );

        // then
        let result = result.map(|_task_state| true);

        assert_eq!(result, expected_result);
    }
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum TaskActionError {
    #[error("Preceding operation not complete.")]
    PrecedingOperationNotComplete,
    #[error("Preceding task not complete.")]
    PrecedingTaskNotComplete,
    #[error("Task already started.")]
    TaskAlreadyStarted,
    #[error("Task already complete.")]
    TaskAlreadyComplete,
    #[error("Task already abandoned.")]
    TaskAlreadyAbandoned,
    #[error("Task not started.")]
    TaskNotStarted,
}

/// Safety: assumes all references are valid.
fn can_apply_action<'p>(
    project: &'p mut Project,
    phase_reference: &Reference,
    operation_reference: &OperationReference,
    task_reference: &TaskReference,
    task_action: &TaskAction,
) -> Result<&'p mut Box<dyn SerializableTaskState>, TaskActionError> {
    let phase_state = project
        .phase_states
        .get_mut(phase_reference)
        .unwrap();

    let operation_state = phase_state
        .operation_states
        .iter_mut()
        .try_fold(None, |mut acc, operation_state| {
            if acc.is_some() {
                // already found the operation state
                return Ok(acc);
            }

            if operation_state
                .reference
                .eq(operation_reference)
            {
                acc = Some(operation_state);
                return Ok(acc);
            }

            //
            // check overall-state of preceding operation
            //
            let preceding_operation_status = operation_state.status();
            if preceding_operation_status != OperationStatus::Complete {
                return Err(TaskActionError::PrecedingOperationNotComplete);
            }

            Ok(acc)
        })?
        .unwrap();

    let task_state = operation_state
        .task_states
        .iter_mut()
        .try_fold(None, |mut acc, (candidate_task_reference, task_state)| {
            if acc.is_some() {
                return Ok(acc);
            }

            if task_reference.eq(candidate_task_reference) {
                //
                // check the state of this task
                //
                match (task_action, task_state.status()) {
                    (TaskAction::Start, TaskStatus::Started) => return Err(TaskActionError::TaskAlreadyStarted),
                    (TaskAction::Complete, TaskStatus::Complete) => return Err(TaskActionError::TaskAlreadyComplete),
                    (TaskAction::Abandon, TaskStatus::Abandoned) => return Err(TaskActionError::TaskAlreadyAbandoned),

                    // 'start' with wrong state
                    (TaskAction::Start, TaskStatus::Abandoned) => return Err(TaskActionError::TaskAlreadyStarted),
                    (TaskAction::Start, TaskStatus::Complete) => return Err(TaskActionError::TaskAlreadyComplete),

                    // 'complete' with wrong state
                    (TaskAction::Complete, TaskStatus::Abandoned) => return Err(TaskActionError::TaskAlreadyAbandoned),
                    (TaskAction::Complete, TaskStatus::Pending) => return Err(TaskActionError::TaskNotStarted),

                    // 'abandon' with wrong state
                    (TaskAction::Abandon, TaskStatus::Pending) => return Err(TaskActionError::TaskNotStarted),
                    _ => {}
                }
                acc = Some(task_state);
            } else {
                //
                // check the state of the preceding task
                //
                let preceding_task_status = task_state.status();
                if preceding_task_status != TaskStatus::Complete {
                    return Err(TaskActionError::PrecedingTaskNotComplete);
                }
            }

            Ok(acc)
        })?
        .unwrap();

    Ok(task_state)
}

pub fn apply_phase_operation_task_action(
    project: &mut Project,
    directory: &Path,
    phase_reference: &Reference,
    operation_reference: OperationReference,
    task_reference: TaskReference,
    action: TaskAction,
) -> anyhow::Result<bool> {
    let mut modified = false;

    let phase_state = project
        .phase_states
        .get_mut(phase_reference)
        .ok_or(PhaseError::UnknownPhase(phase_reference.clone()))?;

    // in case of an error, we need to help the user by giving them a list of the possible operation references
    let possible_operation_references = phase_state
        .operation_states
        .iter()
        .map(|state| state.reference.clone())
        .collect::<Vec<_>>();

    let operation_state = phase_state
        .operation_states
        .iter_mut()
        .find(|state| state.reference.eq(&operation_reference));

    // If we didn't find the operation we were looking, bail.
    let operation_state = operation_state.ok_or(PhaseError::InvalidOperationForPhase(
        phase_reference.clone(),
        operation_reference.clone(),
        possible_operation_references,
    ))?;

    // in case of an error, we need to help the user by giving them a list of the possible task references
    let possible_task_references = operation_state
        .task_states
        .iter()
        .map(|(reference, _state)| reference.clone())
        .collect::<Vec<_>>();

    let task_ref_and_state = operation_state
        .task_states
        .iter_mut()
        .find(|(&ref reference, _state)| reference.eq(&task_reference));

    // If we didn't find the task we were looking, bail.
    let _task_ref_and_state = task_ref_and_state.ok_or(PhaseError::InvalidTaskForOperation(
        phase_reference.clone(),
        operation_reference.clone(),
        task_reference.clone(),
        possible_task_references,
    ))?;

    // make sure the operation's CAN be changed.
    // reasons why it might not be possible include:
    // 1) trying to change a task where preceding tasks or operations are not in the correct state
    // 2) trying to complete AutomatedPnp/ManuallySolderComponents when not all components have been placed (or skipped)
    // 3) some other task-defined reason.

    let task_state = can_apply_action(project, phase_reference, &operation_reference, &task_reference, &action)?;

    match action {
        TaskAction::Start => {
            info!(
                "Marking task as started. phase: {}, operation: {}, task: {}",
                phase_reference, operation_reference, task_reference
            );
            task_state.set_started()
        }
        TaskAction::Complete => {
            info!(
                "Marking task as completed. phase: {}, operation: {}, task: {}",
                phase_reference, operation_reference, task_reference
            );
            task_state.set_completed()
        }
        TaskAction::Abandon => {
            info!(
                "Marking task as abandoned. phase: {}, operation: {}, task: {}",
                phase_reference, operation_reference, task_reference
            );
            task_state.set_abandoned()
        }
    }

    let mut task_history_items: Vec<(&TaskReference, Box<dyn OperationHistoryKind>)> = Vec::new();

    if let Some(task_history_item) = build_operation_task_history_item(&task_reference, task_state.status()) {
        task_history_items.push(task_history_item);
    }

    fn build_operation_task_history_item(
        reference: &TaskReference,
        new_status: TaskStatus,
    ) -> Option<(&TaskReference, Box<dyn OperationHistoryKind>)> {
        if reference.eq(&TaskReference::from_raw_str("core::load_pcbs")) {
            Some((
                reference,
                Box::new(LoadPcbsOperationTaskHistoryKind {
                    status: new_status,
                }) as Box<dyn OperationHistoryKind>,
            ))
        } else if reference.eq(&TaskReference::from_raw_str("core::place_components")) {
            Some((
                reference,
                Box::new(PlaceComponentsOperationTaskHistoryKind {
                    status: new_status,
                }) as Box<dyn OperationHistoryKind>,
            ))
        } else if reference.eq(&TaskReference::from_raw_str("core::manual_soldering")) {
            Some((
                reference,
                Box::new(ManualSolderingOperationTaskHistoryKind {
                    status: new_status,
                }) as Box<dyn OperationHistoryKind>,
            ))
        } else if reference.eq(&TaskReference::from_raw_str("core::automated_soldering")) {
            Some((
                reference,
                Box::new(AutomatedSolderingOperationTaskHistoryKind {
                    status: new_status,
                }) as Box<dyn OperationHistoryKind>,
            ))
        } else {
            warn!("Unable to build history. task_reference: {:?}", reference);
            None
        }
    }

    if !task_history_items.is_empty() {
        modified = true;

        for (task_reference, task_history) in task_history_items.into_iter() {
            let now = OffsetDateTime::now_utc();

            let history_item = OperationHistoryItem {
                date_time: now,
                phase: phase_reference.clone(),
                operation_reference: operation_reference.clone(),
                task_reference: task_reference.clone(),
                task_history,
                extra: Default::default(),
            };

            let mut phase_log_path = PathBuf::from(directory);
            phase_log_path.push(format!("{}_log.json", phase_reference));

            let mut operation_history: Vec<OperationHistoryItem> = operation_history::read_or_default(&phase_log_path)?;

            operation_history.push(history_item);

            operation_history::write(phase_log_path, &operation_history)?;
        }
    }

    Ok(modified)
}

pub fn update_placement_orderings(
    project: &mut Project,
    reference: &Reference,
    placement_orderings: &Vec<PlacementSortingItem>,
) -> anyhow::Result<bool> {
    let phase = project
        .phases
        .get_mut(reference)
        .ok_or(PhaseError::UnknownPhase(reference.clone()))?;

    let modified = if phase
        .placement_orderings
        .eq(placement_orderings)
    {
        false
    } else {
        phase
            .placement_orderings
            .clone_from(placement_orderings);

        info!(
            "Phase placement orderings set. phase: '{}', orderings: [{}]",
            reference,
            placement_orderings
                .iter()
                .map(|item| {
                    format!(
                        "{}:{}",
                        item.mode
                            .to_string()
                            .to_shouty_snake_case(),
                        item.sort_order
                            .to_string()
                            .to_shouty_snake_case()
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        );
        true
    };

    Ok(modified)
}

pub fn reset_operations(project: &mut Project) -> anyhow::Result<()> {
    reset_placement_operations(project);
    reset_phase_operations(project);

    refresh_phase_operation_states(project);

    Ok(())
}

fn reset_placement_operations(project: &mut Project) {
    for (_object_path, placement_state) in project.placements.iter_mut() {
        placement_state.operation_status = PlacementStatus::Pending;
    }

    info!("Placement operations reset.");
}

fn reset_phase_operations(project: &mut Project) {
    for (reference, phase_state) in project.phase_states.iter_mut() {
        phase_state.reset();
        info!("Phase operations reset. phase: {}", reference);
    }
}

pub fn find_phase_parts(
    project: &Project,
    phase_reference: &Reference,
    manufacturer_pattern: Regex,
    mpn_pattern: Regex,
) -> BTreeSet<Part> {
    project
        .placements
        .iter()
        .filter_map(|(_object_path, placement_state)| match &placement_state.phase {
            Some(candidate_phase) if candidate_phase.eq(phase_reference) => {
                if manufacturer_pattern.is_match(
                    &placement_state
                        .placement
                        .part
                        .manufacturer,
                ) && mpn_pattern.is_match(&placement_state.placement.part.mpn)
                {
                    Some(placement_state.placement.part.clone())
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect()
}
