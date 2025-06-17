use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;

use indexmap::{IndexMap, IndexSet};
use nalgebra::{Point2, Vector2};
use planning::design::{DesignIndex, DesignName};
use planning::file::FileReference;
use planning::placement::{PlacementSortingMode, PlacementStatus, ProjectPlacementStatus};
use planning::process::{OperationReference, ProcessReference, ProcessRuleReference, TaskReference, TaskStatus};
use planning::variant::VariantName;
use pnp::object_path::ObjectPath;
use pnp::panel::{DesignSizing, Dimensions, FiducialParameters, PcbUnitPositioning, Unit};
use pnp::pcb::{PcbSide, PcbUnitIndex};
use pnp::placement::RefDes;
use pnp::reference::Reference;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use util::dynamic::as_any::AsAny;
use util::sorting::SortOrder;

use crate::common::serde::ToFormattedJson;

#[serde_as]
#[derive(Debug, serde::Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct TestProject {
    pub name: String,

    /// The *definition* of the processes used by this project.
    pub processes: Vec<TestProcessDefinition>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub pcbs: Vec<TestProjectPcb>,

    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub part_states: BTreeMap<TestPart, TestPartState>,

    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub phases: BTreeMap<Reference, TestPhase>,

    #[serde(skip_serializing_if = "IndexSet::is_empty")]
    #[serde(default)]
    pub phase_orderings: IndexSet<Reference>,

    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub phase_states: BTreeMap<Reference, TestPhaseState>,

    #[serde_as(as = "Vec<(DisplayFromStr, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub placements: BTreeMap<ObjectPath, TestPlacementState>,
}

impl TestProject {
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();

        self
    }

    pub fn with_default_processes(mut self) -> Self {
        self.processes.clear();

        self.processes
            .push(TestProcessDefinition {
                reference: Reference::from_raw_str("pnp"),
                operations: vec![
                    TestOperationDefinition {
                        reference: Reference::from_raw_str("load_pcbs"),
                        tasks: vec![TaskReference::from_raw_str("core::load_pcbs")],
                    },
                    TestOperationDefinition {
                        reference: Reference::from_raw_str("automated_pnp"),
                        tasks: vec![TaskReference::from_raw_str("core::place_components")],
                    },
                    TestOperationDefinition {
                        reference: Reference::from_raw_str("reflow_oven_soldering"),
                        tasks: vec![TaskReference::from_raw_str("core::automated_soldering")],
                    },
                ],
                rules: vec![ProcessRuleReference::from_raw_str("core::unique_feeder_references")],
            });

        self.processes
            .push(TestProcessDefinition {
                reference: Reference::from_raw_str("manual"),
                operations: vec![
                    TestOperationDefinition {
                        reference: Reference::from_raw_str("load_pcbs"),
                        tasks: vec![TaskReference::from_raw_str("core::load_pcbs")],
                    },
                    TestOperationDefinition {
                        reference: Reference::from_raw_str("manually_solder_components"),
                        tasks: vec![
                            TaskReference::from_raw_str("core::place_components"),
                            TaskReference::from_raw_str("core::manual_soldering"),
                        ],
                    },
                ],
                rules: vec![],
            });

        self
    }

    pub fn with_pcbs(mut self, pcbs: Vec<TestProjectPcb>) -> Self {
        self.pcbs = pcbs;
        self
    }

    pub fn with_part_states(mut self, part_states: Vec<(TestPart, TestPartState)>) -> Self {
        self.part_states = BTreeMap::from_iter(part_states.into_iter());
        self
    }

    pub fn with_placements(mut self, placements: Vec<(&str, TestPlacementState)>) -> Self {
        self.placements = BTreeMap::from_iter(
            placements
                .into_iter()
                .map(|(a, b)| (ObjectPath::from_str(a).unwrap(), b)),
        );
        self
    }

    pub fn with_phases(mut self, phases: Vec<TestPhase>) -> Self {
        self.phases = BTreeMap::from_iter(
            phases
                .into_iter()
                .map(|phase| (phase.reference.clone(), phase)),
        );
        self
    }

    pub fn with_phase_orderings(mut self, phase_orderings: &[&str]) -> Self {
        self.phase_orderings = IndexSet::from_iter(
            phase_orderings
                .into_iter()
                .map(|a| Reference::from_raw_str(a)),
        );
        self
    }

    pub fn with_phase_states(mut self, phase_states: Vec<(&str, Vec<TestOperationState>)>) -> Self {
        self.phase_states = BTreeMap::from_iter(
            phase_states
                .into_iter()
                .map(|(reference, operation_states)| {
                    (Reference::from_raw_str(reference), TestPhaseState {
                        operation_states,
                    })
                }),
        );
        self
    }
}

#[derive(Debug, serde::Serialize)]
pub struct TestProcessDefinition {
    pub reference: ProcessReference,
    pub operations: Vec<TestOperationDefinition>,
    pub rules: Vec<ProcessRuleReference>,
}

#[derive(Debug, serde::Serialize)]
pub struct TestOperationDefinition {
    pub reference: OperationReference,
    pub tasks: Vec<TaskReference>,
}

#[serde_as]
#[derive(Debug, serde::Serialize)]
pub struct TestPcb {
    pub name: String,
    pub units: u16,
    pub design_names: Vec<DesignName>,

    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub unit_map: BTreeMap<PcbUnitIndex, DesignIndex>,

    pub panel_sizing: TestPanelSizing,
}

#[serde_as]
#[derive(Debug, serde::Serialize)]
pub struct TestPanelSizing {
    pub units: Unit,
    pub size: Point2<f64>,
    pub edge_rails: Dimensions<f64>,
    pub fiducials: Vec<TestFiducialParameters>,
    pub design_sizings: Vec<TestDesignSizing>,
    pub pcb_unit_positionings: Vec<TestPcbUnitPositioning>,
}

#[derive(serde::Serialize, Debug)]
pub struct TestFiducialParameters {
    pub position: Vector2<f64>,
    pub mask_diameter: f64,
    pub copper_diameter: f64,
}

#[derive(serde::Serialize, Debug)]
pub struct TestDesignSizing {
    pub origin: Vector2<f64>,
    pub offset: Vector2<f64>,
    pub size: Vector2<f64>,
}

#[derive(serde::Serialize, Debug)]
pub struct TestPcbUnitPositioning {
    pub offset: Vector2<f64>,
    /// clockwise positive radians
    pub rotation: f64,
}

#[serde_as]
#[derive(Debug, serde::Serialize)]
pub struct TestProjectPcb {
    pub pcb_file: FileReference,

    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub unit_assignments: BTreeMap<PcbUnitIndex, (DesignIndex, VariantName)>,
}

#[derive(Debug, serde::Serialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct TestPart {
    pub manufacturer: String,
    pub mpn: String,
}

impl TestPart {
    pub fn new(manufacturer: &str, mpn: &str) -> Self {
        Self {
            manufacturer: manufacturer.to_string(),
            mpn: mpn.to_string(),
        }
    }
}

#[derive(Debug, serde::Serialize, Default)]
pub struct TestPartState {
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    #[serde(default)]
    pub applicable_processes: BTreeSet<ProcessReference>,
}

impl TestPartState {
    pub fn new(references: &[&str]) -> Self {
        Self {
            applicable_processes: BTreeSet::from_iter(
                references
                    .into_iter()
                    .map(|reference| ProcessReference::from_raw_str(reference)),
            ),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct TestPhase {
    pub reference: Reference,
    pub process: ProcessReference,
    pub load_out_source: String,
    pub pcb_side: PcbSide,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub placement_orderings: Vec<TestPlacementSortingItem>,
}

impl TestPhase {
    pub fn new(
        reference: &str,
        process: &str,
        path: &str,
        pcb_side: PcbSide,
        placement_orderings: &[(&str, &str)],
    ) -> Self {
        Self {
            reference: Reference::from_raw_str(reference),
            process: Reference::from_raw_str(process),
            load_out_source: path.to_string(),
            pcb_side,
            placement_orderings: placement_orderings
                .into_iter()
                .map(|(mode, sort_order)| TestPlacementSortingItem {
                    mode: PlacementSortingMode::deserialize(
                        serde::de::value::StrDeserializer::<serde::de::value::Error>::new(mode),
                    )
                    .unwrap(),
                    sort_order: SortOrder::deserialize(
                        serde::de::value::StrDeserializer::<serde::de::value::Error>::new(sort_order),
                    )
                    .unwrap(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct TestPhaseState {
    pub operation_states: Vec<TestOperationState>,
}

#[derive(Debug, serde::Serialize)]
pub struct TestOperationState {
    pub reference: OperationReference,
    pub task_states: IndexMap<TaskReference, Box<dyn TestSerializableTaskState>>,
}

impl TestOperationState {
    pub fn new(reference: &str, task_states: Vec<(&str, Box<dyn TestSerializableTaskState>)>) -> TestOperationState {
        TestOperationState {
            reference: Reference::from_raw_str(reference),
            task_states: IndexMap::from_iter(
                task_states
                    .into_iter()
                    .map(|(reference, task_state)| (TaskReference::from_raw_str(reference), task_state)),
            ),
        }
    }
}

#[typetag::serialize(tag = "type")]
pub trait TestSerializableTaskState: TestTaskState + AsAny + Send + Sync + Debug {}

pub trait TestTaskState {}

#[derive(Debug, serde::Serialize, Default)]
pub struct TestPlacementTaskState {
    pub placed: usize,
    pub skipped: usize,
    pub total: usize,
    status: TaskStatus,
}

impl TestTaskState for TestPlacementTaskState {}

#[typetag::serialize(name = "core::placement_task_state")]
impl TestSerializableTaskState for TestPlacementTaskState {}

impl TestPlacementTaskState {
    pub fn new(status: TaskStatus) -> Self {
        Self {
            status,
            ..Default::default()
        }
    }

    pub fn with_placed(mut self, placed: usize) -> Self {
        self.placed = placed;
        self
    }

    pub fn with_skipped(mut self, skipped: usize) -> Self {
        self.skipped = skipped;
        self
    }

    pub fn with_total(mut self, total: usize) -> Self {
        self.total = total;
        self
    }
}

macro_rules! generic_test_task {
    ($name:ident, $key:literal) => {
        #[derive(Debug, serde::Serialize, Default)]
        pub struct $name {
            status: TaskStatus,
        }

        impl TestTaskState for $name {}

        #[typetag::serialize(name = $key)]
        impl TestSerializableTaskState for $name {}

        impl $name {
            pub fn new(status: TaskStatus) -> Self {
                Self {
                    status,
                }
            }
        }
    };
}

generic_test_task!(TestLoadPcbsTaskState, "core::load_pcbs_task_state");
generic_test_task!(TestAutomatedSolderingTaskState, "core::automated_soldering_task_state");
generic_test_task!(TestManualSolderingTaskState, "core::manual_soldering_task_state");

#[derive(Debug, serde::Serialize)]
pub struct TestPlacementSortingItem {
    pub mode: PlacementSortingMode,
    pub sort_order: SortOrder,
}

#[serde_as]
#[derive(Debug, serde::Serialize)]
pub struct TestPlacementState {
    #[serde_as(as = "DisplayFromStr")]
    pub unit_path: ObjectPath,
    pub placement: TestPlacement,
    pub operation_status: PlacementStatus,
    pub project_status: ProjectPlacementStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub phase: Option<Reference>,
}

impl TestPlacementState {
    pub fn new(
        unit_path: &str,
        placement: TestPlacement,
        operation_status: PlacementStatus,
        project_status: ProjectPlacementStatus,
        phase: Option<&str>,
    ) -> Self {
        TestPlacementState {
            unit_path: ObjectPath::from_str(unit_path).unwrap(),
            placement,
            operation_status,
            project_status,
            phase: phase.map(|phase| Reference::from_str(phase).unwrap()),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct TestPlacement {
    pub ref_des: RefDes,
    pub part: TestPart,
    pub place: bool,
    pub pcb_side: PcbSide,
    pub x: Decimal,
    pub y: Decimal,
    pub rotation: Decimal,
}

impl TestPlacement {
    pub fn new(
        ref_des: &str,
        manufacturer: &str,
        mpn: &str,
        place: bool,
        pcb_side: PcbSide,
        x: Decimal,
        y: Decimal,
        rotation: Decimal,
    ) -> Self {
        // FUTURE add test assertions
        Self {
            ref_des: RefDes::from(ref_des),
            part: TestPart {
                manufacturer: manufacturer.to_string(),
                mpn: mpn.to_string(),
            },
            place,
            pcb_side,
            x,
            y,
            rotation,
        }
    }
}

impl TestProject {
    pub fn content(&self) -> String {
        let content = self.to_formatted_json();

        println!("expected project: {}", content);

        content
    }

    pub fn new() -> Self {
        Default::default()
    }
}

impl TestPcb {
    pub fn content(&self) -> String {
        let content = self.to_formatted_json();

        println!("expected pcb: {}", content);

        content
    }
}

impl Display for TestProject {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let content = self.to_formatted_json();
        write!(f, "{}", content)
    }
}

impl Display for TestPcb {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let content = self.to_formatted_json();
        write!(f, "{}", content)
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub enum TestProcessOperationStatus {
    Incomplete,
    Complete,
    Abandoned,
    Pending,
}

impl Display for TestProcessOperationStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TestProcessOperationStatus::Pending => write!(f, "Pending"),
            TestProcessOperationStatus::Incomplete => write!(f, "Incomplete"),
            TestProcessOperationStatus::Complete => write!(f, "Complete"),
            TestProcessOperationStatus::Abandoned => write!(f, "Abandoned"),
        }
    }
}
