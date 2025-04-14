use std::cmp::Ordering;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::Error;
use csv::QuoteStyle;
use heck::ToShoutySnakeCase;
use indexmap::IndexSet;
use pnp;
use pnp::load_out::LoadOutItem;
use pnp::object_path::ObjectPath;
use pnp::part::Part;
use pnp::pcb::{Pcb, PcbKind, PcbSide};
use pnp::placement::Placement;
use regex::Regex;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use thiserror::Error;
use time::OffsetDateTime;
use tracing::{debug, error, info, trace, warn};
use util::dynamic::dynamic_eq::DynamicEq;
use util::sorting::SortOrder;

use crate::design::DesignVariant;
use crate::operation_history::{LoadPcbsOperationTaskHistoryKind, OperationHistoryItem, OperationHistoryKind, PlacementOperationHistoryKind};
use crate::actions::{AddOrRemoveAction, SetOrClearAction};
use crate::part::PartState;
use crate::phase::{Phase, PhaseError, PhaseOrderings, PhaseState};
use crate::placement::{PlacementStatus, PlacementSortingItem, PlacementSortingMode, PlacementState, ProjectPlacementStatus, PlacementOperation};
use crate::process::{ProcessDefinition, ProcessError, ProcessReference, TaskStatus, OperationDefinition, ProcessRuleReference, TaskReference, OperationReference, OperationAction};
use crate::reference::{Reference, ReferenceError};
#[cfg(feature = "markdown")]
use crate::report::project_report_json_to_markdown;
use crate::report::{IssueKind, IssueSeverity, ProjectReportIssue};
use crate::{operation_history, placement, report};

#[serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Project {
    pub name: String,

    /// The *definition* of the processes used by this project.
    pub processes: Vec<ProcessDefinition>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub pcbs: Vec<Pcb>,

    #[serde_as(as = "Vec<(DisplayFromStr, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub unit_assignments: BTreeMap<ObjectPath, DesignVariant>,

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

    pub fn ensure_process(&mut self, process: &ProcessDefinition) -> anyhow::Result<()> {
        if !self.processes.contains(process) {
            info!("Adding process to project.  process: '{}'", process.reference);
            self.processes.push(process.clone())
        }
        Ok(())
    }

    pub fn update_assignment(&mut self, object_path: ObjectPath, design_variant: DesignVariant) -> anyhow::Result<()> {
        match self
            .unit_assignments
            .entry(object_path.clone())
        {
            Entry::Vacant(entry) => {
                entry.insert(design_variant.clone());
                info!(
                    "Unit assignment added. unit: '{}', design_variant: {}",
                    object_path, design_variant
                )
            }
            Entry::Occupied(mut entry) => {
                if entry.get().eq(&design_variant) {
                    info!("Unit assignment unchanged.")
                } else {
                    let old_value = entry.insert(design_variant.clone());
                    info!(
                        "Unit assignment updated. unit: '{}', old: {}, new: {}",
                        object_path, old_value, design_variant
                    )
                }
            }
        }

        Ok(())
    }

    pub fn update_phase(
        &mut self,
        reference: Reference,
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

    pub fn unique_design_variants(&self) -> Vec<DesignVariant> {
        let unique_design_variants: Vec<DesignVariant> =
            self.unit_assignments
                .iter()
                .fold(vec![], |mut acc, (_path, design_variant)| {
                    if !acc.contains(design_variant) {
                        acc.push(design_variant.clone())
                    }

                    acc
                });

        unique_design_variants
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
        let process_name = ProcessReference::from_str(name).map_err(|e| ProcessFactoryError::ErrorCreatingProcessReference {
            reason: e,
        })?;

        // FUTURE add support for more named processes

        match name {
            "pnp" => Ok(ProcessDefinition {
                reference: process_name,
                operations: vec![
                    OperationDefinition {
                        reference: Reference::from_raw_str("load_pcbs"),
                        tasks: vec![
                            TaskReference::from_raw_str("core::load_pcbs"),
                        ],
                    },
                    OperationDefinition {
                        reference: Reference::from_raw_str("automated_pnp"),
                        tasks: vec![
                            TaskReference::from_raw_str("core::place_components"),
                        ],
                    },
                    OperationDefinition {
                        reference: Reference::from_raw_str("reflow_oven_soldering"),
                        tasks: vec![
                            TaskReference::from_raw_str("core::automated_soldering"),
                        ],
                    },
                ],
                rules: vec![
                    ProcessRuleReference::from_raw_str("core::unique_feeder_references")
                ],
            }),
            "manual" => Ok(ProcessDefinition {
                reference: process_name,
                operations: vec![
                    OperationDefinition {
                        reference: Reference::from_raw_str("load_pcbs"),
                        tasks: vec![
                            TaskReference::from_raw_str("core::load_pcbs"),
                        ],
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
            unit_assignments: Default::default(),
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
}

pub fn add_pcb(project: &mut Project, kind: PcbKind, name: String) -> Result<(), PcbOperationError> {
    project.pcbs.push(Pcb {
        kind: kind.clone(),
        name: name.clone(),
    });

    match kind {
        PcbKind::Single => info!("Added single PCB. name: '{}'", name),
        PcbKind::Panel => info!("Added panel PCB. name: '{}'", name),
    }
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
                _ => "".to_string(),
            };

        if feeder_reference.is_empty() {
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
                                _ => "".to_string(),
                            };
                            let feeder_reference_b = match pnp::load_out::find_load_out_item_by_part(
                                load_out_items,
                                &placement_state_b.placement.part,
                            ) {
                                Some(load_out_item) => load_out_item.reference.clone(),
                                _ => "".to_string(),
                            };

                            trace!(
                                "Comparing feeder references. feeder_reference_a: '{}' feeder_reference_a: '{}'",
                                feeder_reference_a,
                                feeder_reference_b
                            );
                            feeder_reference_a.cmp(&feeder_reference_b)
                        }
                        PlacementSortingMode::PcbUnit => {
                            let pcb_unit_a = object_path_a.pcb_unit();
                            let pcb_unit_b = object_path_b.pcb_unit();

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

    pub feeder_reference: String,
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
                _ => "".to_string(),
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

    // find new or existing placements that are in the updated design_variant_placement_map

    for (design_variant, placements) in design_variant_placement_map.iter() {
        for (unit_path, assignment_design_variant) in project.unit_assignments.iter() {
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
        for (unit_path, design_variant) in project.unit_assignments.iter() {
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

pub fn load(project_file_path: &PathBuf) -> Result<Project, std::io::Error> {
    let project_file = File::open(project_file_path.clone())?;
    let mut de = serde_json::Deserializer::from_reader(project_file);
    let project = Project::deserialize(&mut de)?;
    Ok(project)
}

pub fn save(project: &Project, project_file_path: &PathBuf) -> Result<(), std::io::Error> {
    let project_file = File::create(project_file_path)?;
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(project_file, formatter);
    project.serialize(&mut ser)?;

    let mut project_file = ser.into_inner();
    let _written = project_file.write(b"\n")?;

    Ok(())
}

pub fn update_placements_operation(
    project: &mut Project,
    directory: &Path,
    object_path_patterns: Vec<Regex>,
    placement_operation: PlacementOperation,
) -> anyhow::Result<bool> {
    let mut modified = false;
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
            let should_log = match placement_operation {
                PlacementOperation::Place => {
                    match placement_state.operation_status {
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
                    }
                }
                PlacementOperation::Reset => {
                    match placement_state.operation_status {
                        PlacementStatus::Placed |
                        PlacementStatus::Skipped => {
                            info!("Resetting placed flag. object_path: {}", object_path);
                            placement_state.operation_status = PlacementStatus::Pending;
                            modified = true;
                            true
                        }
                        PlacementStatus::Pending => {
                            warn!("Placed flag already pending. object_path: {}", object_path);
                            false
                        }
                    }
                }
                PlacementOperation::Skip => {
                    match placement_state.operation_status {
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
                    }

                }

            };

            if should_log {
                let phase = placement_state.phase.as_ref().unwrap();

                let task_history = Box::new(PlacementOperationHistoryKind {
                        object_path: object_path.clone(),
                        operation: placement_operation.clone(),
                    }) as Box<dyn OperationHistoryKind>;

                let history_items = history_item_map
                    .entry(phase.clone())
                    .or_default();

                history_items.push(task_history);

                modified = true;
            }
        }
    }

    if modified {
        refresh_phase_operation_states(project);

        for (phase_reference, task_histories) in history_item_map {

            let phase = project.phase_states.get_mut(&phase_reference).unwrap();
            let operation_reference = phase.operation_states
                .iter()
                .find_map(|operation_state|{
                    if let Some((task_reference, state)) = operation_state.task_states.iter().find(|(task_reference, task_state)|task_state.requires_placements()) {
                        Some(operation_state.reference.clone())
                    } else {
                        None
                    }
                });

            // FIXME Probably we should check for a phase with a placements tasks BEFORE changing placements state
            assert!(operation_reference.is_some());

            let operation_reference = operation_reference.unwrap();

            let now = OffsetDateTime::now_utc();

            let history_items = task_histories.into_iter().map(|task_history|{
                OperationHistoryItem {
                    date_time: now,
                    phase: phase_reference.clone(),
                    extra: Default::default(),
                    operation_reference: operation_reference.clone(),
                    task_reference: TaskReference::from_raw_str("core::place_components"),
                    task_history,
                }
            }).collect::<Vec<_>>();


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
            trace!("operation: {:?}, operation_state: {:?}", operation_state.reference, operation_state);

            // FUTURE optimize this if there's ever more then one task that needs placements

            for (task_reference, task_state) in operation_state.task_states.iter_mut() {
                trace!("task: {:?}, task_state: {:?}", task_reference, task_state);

                if task_state.requires_placements() {

                    let original_task_state = dyn_clone::clone(task_state);

                    {
                        let placement_api = task_state.placements_state_mut().unwrap();
                        placement_api.reset();

                        let phase_placements = project.placements
                            .iter()
                            .filter(|(_object_path, placement_state)|
                                matches!(&placement_state.phase, Some(candidate_phase_reference) if candidate_phase_reference.eq(phase_reference))
                            ).collect::<Vec<_>>();

                        let total = phase_placements.len();
                        placement_api.set_total_placements(total);

                        for (_object_path, placement) in phase_placements {
                            placement_api.on_placement_status_change(&PlacementStatus::Pending, &placement.operation_status);
                        }
                    }

                    trace!("Refreshing placement task state complete.  before: {:?}, after: {:?}", original_task_state, task_state);

                    let state_updated = task_state != &original_task_state;

                    info!("Refreshed placement task state. phase: {}, operation: {}, task: {}, status: {}, updated: {}", phase_reference, operation_state.reference, task_reference, task_state.status(), state_updated);

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

pub fn apply_phase_operation_action(
    project: &mut Project,
    directory: &Path,
    phase_reference: &Reference,
    operation: OperationReference,
    action: OperationAction,
) -> anyhow::Result<bool> {
    let phase_state = project
        .phase_states
        .get_mut(phase_reference)
        .ok_or(PhaseError::UnknownPhase(phase_reference.clone()))?;

    // in case of an error, we need to help the user by giving them a list of the possible operation references
    let possible_operation_references = phase_state.operation_states.iter().map(|state|state.reference.clone()).collect::<Vec<_>>();

    let mut modified = false;

    // We can only complete an operation if all preceding operation have been completed
    let (is_complete, _preceding_operation, state) = phase_state
        .operation_states
        .iter_mut()
        .try_fold(
            (true, None, None),
            |(preceding_phase_complete, preceding_operation_reference, found_state), state| {
                let is_this_state_complete = state.is_complete();
                let candidate_operation_reference = state.reference.clone();

                match (preceding_phase_complete, preceding_operation_reference, found_state) {
                    (true, _, Some(state)) => Ok((true, Some(candidate_operation_reference), Some(state))),
                    result @ (false, _, Some(_)) => {
                        // we found what we were looking for on a previous iteration
                        // FUTURE find someway to shortcut this try_fold, no need to look at remaining operations
                        Ok(result)
                    }
                    (_, preceding_operation, None) => {
                        if candidate_operation_reference.eq(&operation) {
                            if preceding_phase_complete {
                                Ok((is_this_state_complete, Some(candidate_operation_reference), Some(state)))
                            } else {
                                // Safety: the `unwrap` here is safe, as the first iteration will prevent this branch from being executed, and all other branches set the value to `Some`
                                Err(PhaseError::PrecedingOperationIncomplete(
                                    phase_reference.clone(),
                                    preceding_operation.unwrap().clone(),
                                ))
                            }
                        } else {
                            Ok((is_this_state_complete, Some(candidate_operation_reference), None))
                        }
                    }
                }
            },
        )?;

    // If we didn't find the operation we were looking, bail.
    let state = state.ok_or(
        PhaseError::InvalidOperationForPhase(
            phase_reference.clone(),
            operation.clone(),
            possible_operation_references,
        )
    )?;

    let mut task_history_items: Vec<(&TaskReference, Box<dyn OperationHistoryKind>)> = Vec::new();

    match action {
        OperationAction::Completed => {
            if !is_complete {
                // make sure the operation CAN be completed.
                // reasons why it might not be possible include:
                // 1) trying to complete AutomatedPnp/ManuallySolderComponents when not all components have been placed (or skipped)
                // 2) some other task-defined reason.

                let uncompletable_tasks = state.task_states.iter().filter(|(_reference, state)| {
                    !state.is_complete() && !state.can_complete()
                }).collect::<Vec<_>>();


                if uncompletable_tasks.is_empty() {
                    info!("Marking phase operation as complete");

                    let stuff = state.task_states
                        .iter_mut()
                        .filter(|(_reference, state)| state.can_complete())
                        .map(|(reference, state)| {

                            state.set_completed();

                            if reference.eq(&TaskReference::from_raw_str("core::load_pcbs")) {
                                (
                                    reference,
                                    Box::new(LoadPcbsOperationTaskHistoryKind {
                                        status: TaskStatus::Complete,
                                    }) as Box<dyn OperationHistoryKind>
                                )
                            } else if reference.eq(&TaskReference::from_raw_str("core::place_components")) {
                                // core::place_components should not be completable.
                                unreachable!()
                            } else {
                                todo!()
                            }

                        }).collect::<Vec<_>>();

                    task_history_items.extend(stuff);

                } else {

                    let task_references: Vec<&TaskReference> = uncompletable_tasks.iter().map(|(reference, _state)|*reference).collect::<Vec<_>>();
                    error!("Incomplete tasks.  references:  {:?}", task_references);
                }
            } else {
                error!("Phase operation is already complete");
            }
        }
        OperationAction::Started => {
            todo!()
        }
        OperationAction::Abandoned => {
            todo!()
        }
    }

    if !task_history_items.is_empty() {
        modified = true;

        for (task_reference, task_history) in task_history_items.into_iter() {
            let now = OffsetDateTime::now_utc();

            let history_item = OperationHistoryItem {
                date_time: now,
                phase: phase_reference.clone(),
                operation_reference: operation.clone(),
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
