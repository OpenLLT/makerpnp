use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use dyn_clone::DynClone;
#[cfg(feature = "markdown")]
use json2markdown::MarkdownRenderer;
use pnp::load_out::LoadOutItem;
use pnp::object_path::ObjectPath;
use pnp::part::Part;
use pnp::pcb::{Pcb, PcbKind};
use serde::Serialize;
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use tracing::{error, info, trace};
use util::sorting::SortOrder;

use crate::design::{DesignName, DesignVariant};
use crate::placement::{PlacementState, ProjectPlacementStatus};
use crate::process::{OperationReference, OperationStatus, TaskReference};
use crate::project::Project;
use crate::reference::Reference;
use crate::variant::VariantName;

// FUTURE add a test to ensure that duplicate issues are not added to the report.
//        currently a BTreeSet is used to prevent duplicate issues.

pub fn project_generate_report(
    project: &Project,
    phase_load_out_items_map: &BTreeMap<Reference, Vec<LoadOutItem>>,
    issue_set: &mut BTreeSet<ProjectReportIssue>,
) -> ProjectReport {
    let mut report = ProjectReport::default();

    report.name.clone_from(&project.name);
    if project.pcbs.is_empty() {
        issue_set.insert(ProjectReportIssue {
            message: "No PCBs have been assigned to the project.".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPcbsAssigned,
        });
    }

    let mut all_phases_complete = true;

    if !project.phases.is_empty() {
        report.phase_overviews.extend(
            project
                .phase_orderings
                .iter()
                .map(|reference| {
                    let phase = project.phases.get(reference).unwrap();
                    let phase_state = project
                        .phase_states
                        .get(reference)
                        .unwrap();
                    trace!("phase: {:?}, phase_state: {:?}", phase, phase_state);

                    let mut operations_overview = vec![];

                    let phase_status = phase_state
                        .operation_states
                        .iter()
                        .fold(PhaseStatus::Complete, |mut phase_status, operation_state| {
                            let task_overviews = operation_state
                                .task_states
                                .iter()
                                .filter_map(|(task_reference, task_state)| {
                                    let report = if task_reference.eq(&TaskReference::from_raw_str("core::load_pcbs")) {
                                        Some(Box::new(LoadPcbsTaskOverview {}) as Box<dyn TaskOverview>)
                                    } else if task_reference.eq(&TaskReference::from_raw_str("core::place_components"))
                                    {
                                        task_state
                                            .placements_state()
                                            .map(|state| {
                                                let summary = state.summary();
                                                Box::new(PlaceComponentsTaskOverview {
                                                    placed: summary.placed,
                                                    skipped: summary.skipped,
                                                    total: summary.total,
                                                })
                                                    as Box<dyn TaskOverview>
                                            })
                                    } else if task_reference
                                        .eq(&TaskReference::from_raw_str("core::automated_soldering"))
                                    {
                                        Some(Box::new(AutomatedSolderingTaskOverview {}) as Box<dyn TaskOverview>)
                                    } else if task_reference.eq(&TaskReference::from_raw_str("core::manual_soldering"))
                                    {
                                        Some(Box::new(ManualSolderingTaskOverview {}) as Box<dyn TaskOverview>)
                                    } else {
                                        None
                                    };
                                    Some((task_reference.clone(), report))
                                })
                                .collect::<Vec<_>>();

                            let operation_status = operation_state.status();
                            phase_status = match (phase_status, &operation_status) {
                                (PhaseStatus::Complete, OperationStatus::Complete) => PhaseStatus::Complete,

                                (PhaseStatus::Abandoned, _) => PhaseStatus::Abandoned,
                                (_, OperationStatus::Abandoned) => PhaseStatus::Abandoned,

                                (PhaseStatus::Incomplete, _) => PhaseStatus::Incomplete,
                                (_, OperationStatus::Pending) => PhaseStatus::Incomplete,
                                (_, OperationStatus::Started) => PhaseStatus::Incomplete,
                            };

                            let overview = PhaseOperationOverview {
                                operation: operation_state.reference.clone(),
                                status: operation_status,
                                tasks: task_overviews,
                            };

                            operations_overview.push(overview);

                            phase_status
                        });

                    if phase_status == PhaseStatus::Incomplete {
                        all_phases_complete = false
                    }

                    PhaseOverview {
                        phase_name: phase.reference.to_string(),
                        status: phase_status,
                        process: phase.process.to_string(),
                        operations_overview,
                    }
                }),
        );
    } else {
        issue_set.insert(ProjectReportIssue {
            message: "No phases have been created.".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPhasesCreated,
        });
    }

    report.status = match all_phases_complete {
        true => ProjectStatus::Complete,
        false => ProjectStatus::Incomplete,
    };

    let invalid_unit_assignment_issues = generate_issues_for_invalid_unit_assignments(project);
    issue_set.extend(invalid_unit_assignment_issues);

    let phase_specifications: Vec<PhaseSpecification> = project
        .phase_orderings
        .iter()
        .map(|reference| build_phase_specification(project, phase_load_out_items_map, reference))
        .collect();

    report
        .phase_specifications
        .extend(phase_specifications);

    project_report_add_placement_issues(project, issue_set);
    let mut issues: Vec<ProjectReportIssue> = issue_set.iter().cloned().collect();

    project_report_sort_issues(&mut issues);

    for issue in issues.iter() {
        info!(
            "Issue detected. severity: {:?}, message: '{}', kind: {:?}",
            issue.severity, issue.message, issue.kind
        );
    }

    report.issues = issues;

    report
}

fn generate_issues_for_invalid_unit_assignments(project: &Project) -> BTreeSet<ProjectReportIssue> {
    let mut issues: BTreeSet<ProjectReportIssue> = BTreeSet::new();

    for (object_path, _design_variant) in project.unit_assignments.iter() {
        let pcb_kind_counts = count_pcb_kinds(&project.pcbs);

        if let Some((pcb_kind, index)) = object_path.pcb_kind_and_instance() {
            let issue = match pcb_kind_counts.get(&pcb_kind) {
                Some(count) => {
                    if index > *count {
                        Some(ProjectReportIssue {
                            message: "Invalid unit assignment, index out of range.".to_string(),
                            severity: IssueSeverity::Severe,
                            kind: IssueKind::InvalidUnitAssignment {
                                object_path: object_path.clone(),
                            },
                        })
                    } else {
                        None
                    }
                }
                None => Some(ProjectReportIssue {
                    message: "Invalid unit assignment, no pcbs match the assignment.".to_string(),
                    severity: IssueSeverity::Severe,
                    kind: IssueKind::InvalidUnitAssignment {
                        object_path: object_path.clone(),
                    },
                }),
            };

            if let Some(issue) = issue {
                issues.insert(issue);
            }
        }
    }

    issues
}

fn count_pcb_kinds(pcbs: &[Pcb]) -> HashMap<PcbKind, usize> {
    let mut pcb_kind_counts: HashMap<PcbKind, usize> = Default::default();
    for pcb in pcbs.iter() {
        pcb_kind_counts
            .entry(pcb.kind.clone())
            .and_modify(|e| *e += 1)
            .or_insert(1);
    }
    pcb_kind_counts
}

fn build_phase_specification(
    project: &Project,
    phase_load_out_items_map: &BTreeMap<Reference, Vec<LoadOutItem>>,
    reference: &Reference,
) -> PhaseSpecification {
    let phase = project.phases.get(reference).unwrap();
    let phase_state = project
        .phase_states
        .get(reference)
        .unwrap();

    let load_out_items = phase_load_out_items_map
        .get(reference)
        .unwrap();

    let load_out_assignments = load_out_items.iter().map(|load_out_item| {
        let quantity = project.placements.iter()
            .filter(|(_object_path, placement_state)| {
                matches!(&placement_state.phase, Some(other_phase_reference) if phase.reference.eq(other_phase_reference))
                    && placement_state.placement.place
                    && load_out_item.manufacturer.eq(&placement_state.placement.part.manufacturer)
                    && load_out_item.mpn.eq(&placement_state.placement.part.mpn)
            })
            .fold(0_u32, |quantity, _placement_state| {
                quantity + 1
            });

        PhaseLoadOutAssignmentItem {
            feeder_reference: load_out_item.reference.clone(),
            manufacturer: load_out_item.manufacturer.clone(),
            mpn: load_out_item.mpn.clone(),
            quantity,
        }
    }).collect();

    let operations = phase_state
        .operation_states
        .iter()
        .map(|operation_state| {
            let task_specifications = operation_state
                .task_states
                .iter()
                .filter_map(|(task_reference, task_state)| {
                    let report = if task_reference.eq(&TaskReference::from_raw_str("core::load_pcbs")) {
                        let pcbs = build_operation_load_pcbs(project);
                        Some(Box::new(LoadPcbsTaskSpecification {
                            pcbs,
                        }) as Box<dyn TaskSpecification>)
                    } else if task_reference.eq(&TaskReference::from_raw_str("core::place_components")) {
                        Some(Box::new(PlaceComponentsTaskSpecification {}) as Box<dyn TaskSpecification>)
                    } else if task_reference.eq(&TaskReference::from_raw_str("core::automated_soldering")) {
                        Some(Box::new(AutomatedSolderingTaskSpecification {}) as Box<dyn TaskSpecification>)
                    } else if task_reference.eq(&TaskReference::from_raw_str("core::manual_soldering")) {
                        Some(Box::new(ManualSolderingTaskSpecification {}) as Box<dyn TaskSpecification>)
                    } else {
                        None
                    };
                    Some((task_reference.clone(), report))
                })
                .collect::<Vec<_>>();

            let operation_item = OperationItem {
                operation: operation_state.reference.clone(),
                task_specifications,
            };

            operation_item
        })
        .collect();

    PhaseSpecification {
        phase_name: phase.reference.to_string(),
        operations,
        load_out_assignments,
    }
}

fn build_operation_load_pcbs(project: &Project) -> Vec<PcbReportItem> {
    let unit_paths_with_placements = build_unit_paths_with_placements(&project.placements);

    let pcbs: Vec<PcbReportItem> = unit_paths_with_placements
        .iter()
        .find_map(|unit_path| {
            if let Some((kind, mut index)) = unit_path.pcb_kind_and_instance() {
                // TODO consider if unit paths should use zero-based index
                index -= 1;

                // Note: the user may not have made any unit assignments yet.
                let mut unit_assignments = find_unit_assignments(project, unit_path);

                match kind {
                    PcbKind::Panel => {
                        let pcb = project.pcbs.get(index).unwrap();

                        Some(PcbReportItem::Panel {
                            name: pcb.name.clone(),
                            unit_assignments,
                        })
                    }
                    PcbKind::Single => {
                        let pcb = project.pcbs.get(index).unwrap();

                        assert!(unit_assignments.len() <= 1);

                        Some(PcbReportItem::Single {
                            name: pcb.name.clone(),
                            unit_assignment: unit_assignments.pop(),
                        })
                    }
                }
            } else {
                None
            }
        })
        .into_iter()
        .collect();

    pcbs
}

fn build_unit_paths_with_placements(placement_states: &BTreeMap<ObjectPath, PlacementState>) -> BTreeSet<ObjectPath> {
    placement_states.iter().fold(
        BTreeSet::<ObjectPath>::new(),
        |mut acc, (object_path, placement_state)| {
            if placement_state.placement.place {
                if let Ok(pcb_unit) = object_path.pcb_unit() {
                    if acc.insert(pcb_unit) {
                        trace!("Phase pcb unit found.  object_path: {}", object_path);
                    }
                } else {
                    error!("pcb unit not specified.  object_path: {}", object_path);
                }
            }
            acc
        },
    )
}

fn project_report_add_placement_issues(project: &Project, issues: &mut BTreeSet<ProjectReportIssue>) {
    for (object_path, _placement_state) in project
        .placements
        .iter()
        .filter(|(_object_path, placement_state)| {
            placement_state.phase.is_none() && placement_state.project_status == ProjectPlacementStatus::Used
        })
    {
        issues.insert(ProjectReportIssue {
            message: "A placement has not been assigned to a phase".to_string(),
            severity: IssueSeverity::Warning,
            kind: IssueKind::UnassignedPlacement {
                object_path: object_path.clone(),
            },
        });
    }
}

fn project_report_sort_issues(issues: &mut [ProjectReportIssue]) {
    issues.sort_by(|a, b| {
        let sort_orderings = &[
            ("severity", SortOrder::Desc),
            ("kind", SortOrder::Asc),
            ("message", SortOrder::Asc),
        ];

        sort_orderings
            .iter()
            .fold(Ordering::Equal, |mut acc, (&ref mode, sort_order)| {
                if !matches!(acc, Ordering::Equal) {
                    return acc;
                }

                fn kind_ordinal(kind: &IssueKind) -> usize {
                    match kind {
                        IssueKind::NoPcbsAssigned => 0,
                        IssueKind::NoPhasesCreated => 1,
                        IssueKind::InvalidUnitAssignment {
                            ..
                        } => 2,
                        IssueKind::UnassignedPlacement {
                            ..
                        } => 3,
                        IssueKind::UnassignedPartFeeder {
                            ..
                        } => 4,
                    }
                }
                fn severity_ordinal(severity: &IssueSeverity) -> usize {
                    match severity {
                        IssueSeverity::Warning => 0,
                        IssueSeverity::Severe => 1,
                    }
                }

                acc = match mode {
                    "kind" => {
                        let a_ordinal = kind_ordinal(&a.kind);
                        let b_ordinal = kind_ordinal(&b.kind);
                        let ordinal_ordering = a_ordinal.cmp(&b_ordinal);

                        match ordinal_ordering {
                            Ordering::Less => ordinal_ordering,
                            Ordering::Equal => match (&a.kind, &b.kind) {
                                (
                                    IssueKind::InvalidUnitAssignment {
                                        object_path: object_path_a,
                                    },
                                    IssueKind::InvalidUnitAssignment {
                                        object_path: object_path_b,
                                    },
                                ) => object_path_a.cmp(object_path_b),
                                (
                                    IssueKind::UnassignedPlacement {
                                        object_path: object_path_a,
                                    },
                                    IssueKind::UnassignedPlacement {
                                        object_path: object_path_b,
                                    },
                                ) => object_path_a.cmp(object_path_b),
                                (
                                    IssueKind::UnassignedPartFeeder {
                                        part: part_a,
                                    },
                                    IssueKind::UnassignedPartFeeder {
                                        part: part_b,
                                    },
                                ) => part_a.cmp(part_b),
                                _ => ordinal_ordering,
                            },
                            Ordering::Greater => ordinal_ordering,
                        }
                    }
                    "message" => a.message.cmp(&b.message),
                    "severity" => {
                        let a_ordinal = severity_ordinal(&a.severity);
                        let b_ordinal = severity_ordinal(&b.severity);
                        let ordinal_ordering = a_ordinal.cmp(&b_ordinal);
                        ordinal_ordering
                    }
                    _ => unreachable!(),
                };

                match sort_order {
                    SortOrder::Asc => acc,
                    SortOrder::Desc => acc.reverse(),
                }
            })
    });
}

#[cfg(test)]
mod report_issue_sorting {
    use std::str::FromStr;

    use pnp::object_path::ObjectPath;
    use pnp::part::Part;

    use crate::report::{project_report_sort_issues, IssueKind, IssueSeverity, ProjectReportIssue};

    #[test]
    pub fn sort_by_severity_with_equal_message_and_kind() {
        // given
        let issue1 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPcbsAssigned,
        };
        let issue2 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Warning,
            kind: IssueKind::NoPcbsAssigned,
        };

        let mut issues: Vec<ProjectReportIssue> = vec![issue2.clone(), issue1.clone()];
        let expected_issues: Vec<ProjectReportIssue> = vec![issue1.clone(), issue2.clone()];

        // when
        project_report_sort_issues(&mut issues);

        // then
        assert_eq!(&issues, &expected_issues);
    }

    #[test]
    pub fn sort_by_message_with_severity_and_kind() {
        // given
        let issue1 = ProjectReportIssue {
            message: "MESSAGE_1".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPcbsAssigned,
        };
        let issue2 = ProjectReportIssue {
            message: "MESSAGE_2".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPcbsAssigned,
        };

        let mut issues: Vec<ProjectReportIssue> = vec![issue2.clone(), issue1.clone()];
        let expected_issues: Vec<ProjectReportIssue> = vec![issue1.clone(), issue2.clone()];

        // when
        project_report_sort_issues(&mut issues);

        // then
        assert_eq!(&issues, &expected_issues);
    }

    #[test]
    pub fn sort_by_kind_with_equal_message_and_severity() {
        // given
        let issue1 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPcbsAssigned,
        };
        let issue2 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPhasesCreated,
        };
        let issue3 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::InvalidUnitAssignment {
                object_path: ObjectPath::from_str("pcb=panel::instance=1").expect("always ok"),
            },
        };
        let issue4 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::InvalidUnitAssignment {
                object_path: ObjectPath::from_str("pcb=panel::instance=2").expect("always ok"),
            },
        };
        let issue5 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::UnassignedPlacement {
                object_path: ObjectPath::from_str("pcb=panel::instance=1::unit=1::ref_des=R1").expect("always ok"),
            },
        };
        let issue6 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::UnassignedPlacement {
                object_path: ObjectPath::from_str("pcb=panel::instance=1::unit=1::ref_des=R2").expect("always ok"),
            },
        };
        let issue7 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::UnassignedPartFeeder {
                part: Part {
                    manufacturer: "MFR1".to_string(),
                    mpn: "MPN1".to_string(),
                },
            },
        };
        let issue8 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::UnassignedPartFeeder {
                part: Part {
                    manufacturer: "MFR1".to_string(),
                    mpn: "MPN2".to_string(),
                },
            },
        };
        let issue9 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::UnassignedPartFeeder {
                part: Part {
                    manufacturer: "MFR2".to_string(),
                    mpn: "MPN1".to_string(),
                },
            },
        };

        let mut issues: Vec<ProjectReportIssue> = vec![
            issue9.clone(),
            issue8.clone(),
            issue7.clone(),
            issue6.clone(),
            issue5.clone(),
            issue4.clone(),
            issue3.clone(),
            issue2.clone(),
            issue1.clone(),
        ];
        let expected_issues: Vec<ProjectReportIssue> = vec![
            issue1.clone(),
            issue2.clone(),
            issue3.clone(),
            issue4.clone(),
            issue5.clone(),
            issue6.clone(),
            issue7.clone(),
            issue8.clone(),
            issue9.clone(),
        ];

        // when
        project_report_sort_issues(&mut issues);

        // then
        assert_eq!(&issues, &expected_issues);
    }

    #[test]
    pub fn sort_by_severity_kind_and_message() {
        // given
        let issue1 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPcbsAssigned,
        };
        let issue2 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Warning,
            kind: IssueKind::NoPcbsAssigned,
        };
        let issue3 = ProjectReportIssue {
            message: "DIFFERENT".to_string(),
            severity: IssueSeverity::Warning,
            kind: IssueKind::NoPcbsAssigned,
        };

        let issue4 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPhasesCreated,
        };
        let issue5 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Warning,
            kind: IssueKind::NoPhasesCreated,
        };
        let issue6 = ProjectReportIssue {
            message: "DIFFERENT".to_string(),
            severity: IssueSeverity::Warning,
            kind: IssueKind::NoPhasesCreated,
        };

        let mut issues: Vec<ProjectReportIssue> = vec![
            issue1.clone(),
            issue2.clone(),
            issue3.clone(),
            issue4.clone(),
            issue5.clone(),
            issue6.clone(),
        ];
        let expected_issues: Vec<ProjectReportIssue> = vec![
            issue1.clone(),
            issue4.clone(),
            issue3.clone(),
            issue2.clone(),
            issue6.clone(),
            issue5.clone(),
        ];

        // when
        project_report_sort_issues(&mut issues);

        // then
        assert_eq!(&issues, &expected_issues);
    }
}

fn find_unit_assignments(project: &Project, unit_path: &ObjectPath) -> Vec<PcbUnitAssignmentItem> {
    let unit_assignments = project
        .unit_assignments
        .iter()
        .filter_map(
            |(
                assignment_unit_path,
                DesignVariant {
                    design_name,
                    variant_name,
                },
            )| {
                let mut result = None;

                if assignment_unit_path.eq(unit_path) {
                    result = Some(PcbUnitAssignmentItem {
                        unit_path: unit_path.clone(),
                        design_name: design_name.clone(),
                        variant_name: variant_name.clone(),
                    })
                }
                result
            },
        )
        .collect();

    unit_assignments
}

#[derive(serde::Serialize, Default)]
pub struct ProjectReport {
    pub name: String,
    pub status: ProjectStatus,
    pub phase_overviews: Vec<PhaseOverview>,
    pub phase_specifications: Vec<PhaseSpecification>,
    /// A list of unique issues.
    /// Note: Using a Vec doesn't prevent duplicates, duplicates must be filtered before adding them.
    pub issues: Vec<ProjectReportIssue>,
}

#[derive(Clone, serde::Serialize)]
pub enum ProjectStatus {
    Incomplete,
    Complete,
}

impl Default for ProjectStatus {
    fn default() -> Self {
        Self::Incomplete
    }
}

#[derive(Clone, serde::Serialize, PartialEq)]
pub enum PhaseStatus {
    Incomplete,
    Complete,
    Abandoned,
}

#[derive(serde::Serialize)]
pub struct PhaseOverview {
    pub phase_name: String,
    pub status: PhaseStatus,
    pub process: String,
    pub operations_overview: Vec<PhaseOperationOverview>,
}

#[derive(Clone, serde::Serialize)]
pub struct PhaseSpecification {
    pub phase_name: String,
    pub operations: Vec<OperationItem>,
    pub load_out_assignments: Vec<PhaseLoadOutAssignmentItem>,
}

#[derive(Clone, serde::Serialize)]
pub struct PhaseOperationOverview {
    pub operation: OperationReference,
    pub status: OperationStatus,
    pub tasks: Vec<(TaskReference, Option<Box<dyn TaskOverview>>)>,
}

#[derive(Clone, serde::Serialize)]
pub struct OperationItem {
    pub operation: OperationReference,
    pub task_specifications: Vec<(TaskReference, Option<Box<dyn TaskSpecification>>)>,
}

#[typetag::serialize(tag = "type")]
pub trait TaskSpecification: DynClone {}
dyn_clone::clone_trait_object!(TaskSpecification);

macro_rules! generic_task_specification {
    ($name:ident, $key:literal) => {
        #[typetag::serialize(name = $key)]
        impl TaskSpecification for $name {}

        #[derive(Clone, serde::Serialize)]
        pub struct $name {}
    };
}

generic_task_specification!(ManualSolderingTaskSpecification, "manual_soldering_specification");
generic_task_specification!(AutomatedSolderingTaskSpecification, "automated_soldering_specification");
generic_task_specification!(PlaceComponentsTaskSpecification, "place_components_specification");

#[typetag::serialize(name = "load_pcbs_specification")]
impl TaskSpecification for LoadPcbsTaskSpecification {}

#[derive(Clone, serde::Serialize)]
struct LoadPcbsTaskSpecification {
    pub pcbs: Vec<PcbReportItem>,
}

#[typetag::serialize(tag = "type")]
pub trait TaskOverview: DynClone {}
dyn_clone::clone_trait_object!(TaskOverview);

macro_rules! generic_task_overview {
    ($name:ident, $key:literal) => {
        #[typetag::serialize(name = $key)]
        impl TaskOverview for $name {}

        #[derive(Clone, serde::Serialize)]
        pub struct $name {}
    };
}

generic_task_overview!(LoadPcbsTaskOverview, "load_pcbs_overview");
generic_task_overview!(ManualSolderingTaskOverview, "manual_soldering_overview");
generic_task_overview!(AutomatedSolderingTaskOverview, "automated_soldering_overview");

#[typetag::serialize(name = "place_components_overview")]
impl TaskOverview for PlaceComponentsTaskOverview {}

#[derive(Clone, serde::Serialize)]
pub struct PlaceComponentsTaskOverview {
    pub placed: usize,
    pub skipped: usize,
    pub total: usize,
}

#[serde_as]
#[derive(Clone, serde::Serialize)]
pub struct PcbUnitAssignmentItem {
    #[serde_as(as = "DisplayFromStr")]
    unit_path: ObjectPath,
    design_name: DesignName,
    variant_name: VariantName,
}

#[derive(Clone, serde::Serialize)]
pub enum PcbReportItem {
    /// there should be one or more assignments, but the assignment might not have been made yet.
    Panel {
        name: String,
        unit_assignments: Vec<PcbUnitAssignmentItem>,
    },
    /// there should be exactly one assignment, but the assignment might not have been made yet.
    Single {
        name: String,
        unit_assignment: Option<PcbUnitAssignmentItem>,
    },
}

#[derive(Clone, serde::Serialize)]
pub struct PhaseLoadOutAssignmentItem {
    pub feeder_reference: String,
    pub manufacturer: String,
    pub mpn: String,
    pub quantity: u32,
}

// FUTURE implement `Display` and improve info logging
#[derive(Clone, serde::Serialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectReportIssue {
    pub message: String,
    pub severity: IssueSeverity,
    pub kind: IssueKind,
}

#[derive(Clone, serde::Serialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum IssueSeverity {
    Severe,
    Warning,
}

#[serde_as]
#[derive(Clone, serde::Serialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum IssueKind {
    NoPcbsAssigned,
    NoPhasesCreated,
    InvalidUnitAssignment {
        #[serde_as(as = "DisplayFromStr")]
        object_path: ObjectPath,
    },
    UnassignedPlacement {
        #[serde_as(as = "DisplayFromStr")]
        object_path: ObjectPath,
    },
    UnassignedPartFeeder {
        part: Part,
    },
}

pub(crate) fn build_report_file_path(name: &str, directory: &Path) -> PathBuf {
    let mut report_file_path: PathBuf = PathBuf::from(directory);
    report_file_path.push(format!("{}_report.json", name));
    report_file_path
}

pub(crate) fn project_report_save_as_json(report: &ProjectReport, report_file_path: &PathBuf) -> anyhow::Result<()> {
    let report_file = File::create(report_file_path)?;
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(report_file, formatter);
    report.serialize(&mut ser)?;

    let mut report_file = ser.into_inner();
    let _written = report_file.write(b"\n")?;

    info!("Generated JSON report. path: {:?}", report_file_path);

    Ok(())
}

#[cfg(feature = "markdown")]
pub fn project_report_json_to_markdown(json_report_file_name: &PathBuf) -> anyhow::Result<()> {
    let json_string = fs::read_to_string(json_report_file_name)?;

    let json = serde_json::from_str(&json_string)?;

    let renderer = MarkdownRenderer::default();
    let markdown = renderer.render(&json);

    /// Replace the file extension with a different one.
    ///
    /// * New extension should not start with a '.'.
    /// * If there's no existing extension, it adds the new one
    /// * If there is an extension, it replaces it.
    fn replace_extension(mut path: PathBuf, extension: &str) -> PathBuf {
        assert!(!extension.starts_with('.'));
        if let Some(file_name) = path
            .file_name()
            .and_then(|n| n.to_str())
        {
            if let Some(new_name) = Path::new(file_name)
                .with_extension(extension)
                .to_str()
            {
                path.set_file_name(new_name);
            }
        }
        path
    }

    let markdown_file_name = replace_extension(json_report_file_name.clone(), "md");
    fs::write(&markdown_file_name, markdown)?;

    info!("Generated Markdown report. path: {:?}", markdown_file_name);

    Ok(())
}
