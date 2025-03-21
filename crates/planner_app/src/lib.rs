use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::str::FromStr;

use crux_core::macros::Effect;
use crux_core::render::Render;
pub use crux_core::Core;
use crux_core::{render, App, Command};
use petgraph::Graph;
pub use planning::design::{DesignName, DesignVariant};
pub use planning::placement::PlacementState;
use planning::placement::{PlacementOperation, PlacementSortingItem};
use planning::process::{ProcessName, ProcessOperationKind, ProcessOperationSetItem};
use planning::project;
use planning::project::{PartStateError, PcbOperationError, ProcessFactory, Project, ProjectRefreshResult};
pub use planning::reference::Reference;
pub use planning::variant::VariantName;
use pnp::load_out::LoadOutItem;
pub use pnp::object_path::ObjectPath;
pub use pnp::part::Part;
pub use pnp::pcb::{PcbKind, PcbSide};
use regex::Regex;
use serde_with::serde_as;
use stores::load_out::{LoadOutOperationError, LoadOutSource};
use thiserror::Error;
use tracing::{info, trace};

use crate::capabilities::view_renderer;
use crate::capabilities::view_renderer::ProjectViewRenderer;

pub mod capabilities;

extern crate serde_regex;

#[derive(Default)]
pub struct Planner;

#[derive(Default)]
pub struct ModelProject {
    path: PathBuf,
    project: Project,
    modified: bool,
}

#[derive(Default)]
pub struct Model {
    model_project: Option<ModelProject>,

    error: Option<String>,
}

#[derive(Effect)]
#[allow(unused)]
pub struct Capabilities {
    /// the default render operation, see `ProjectOperationViewModel`
    render: Render<Event>,
    /// a custom capability for use with `ProjectView`
    project_view: ProjectViewRenderer<Event>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PhaseOverview {
    pub phase_reference: Reference,
    pub process: ProcessName,
    pub load_out_source: String,
    pub pcb_side: PcbSide,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PhasePlacements {
    pub phase_reference: Reference,
    pub placements: Vec<PlacementState>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct Process {
    pub name: ProcessName,
    pub operations: Vec<ProcessOperationKind>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PartWithState {
    pub part: Part,
    pub processes: Vec<ProcessName>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PartStates {
    pub parts: Vec<PartWithState>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PhasePlacementOrderings {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub placement_orderings: Vec<PlacementSortingItem>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PlacementsList {
    pub placements: Vec<PlacementState>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone, Eq)]
pub struct ProjectTreeItem {
    pub key: String,
    pub args: HashMap<String, Arg>,

    /// "/" = root, paths are "/" separated.
    // FIXME path elements that contain a `/` need to be escaped and un-escaped.  e.g. a phase reference of `top/1`
    pub path: String,
}

impl Default for ProjectTreeItem {
    fn default() -> Self {
        Self {
            key: "unknown".to_string(),
            args: HashMap::new(),
            path: "/".to_string(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone, Eq)]
pub enum Arg {
    String(String),
    // Add other types, like 'Number' here as required.
}

#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug, Clone, Eq)]
pub struct ProjectOverview {
    pub name: String,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
pub struct ProjectTreeView {
    /// A directed graph of ProjectTreeItem.
    ///
    /// The only relationships in the tree are parent->child, i.e. not parent->grandchild
    /// the first element is the only root element
    pub tree: Graph<ProjectTreeItem, ()>,
}

impl ProjectTreeView {
    fn new() -> Self {
        Self {
            tree: Graph::new(),
        }
    }
}

impl PartialEq for ProjectTreeView {
    fn eq(&self, other: &ProjectTreeView) -> bool {
        /// Acknowledgement: https://github.com/petgraph/petgraph/issues/199#issuecomment-484077775
        fn graph_eq<N, E, Ty, Ix>(a: &petgraph::Graph<N, E, Ty, Ix>, b: &petgraph::Graph<N, E, Ty, Ix>) -> bool
        where
            N: PartialEq,
            E: PartialEq,
            Ty: petgraph::EdgeType,
            Ix: petgraph::graph::IndexType + PartialEq,
        {
            let a_ns = a.raw_nodes().iter().map(|n| &n.weight);
            let b_ns = b.raw_nodes().iter().map(|n| &n.weight);
            let a_es = a
                .raw_edges()
                .iter()
                .map(|e| (e.source(), e.target(), &e.weight));
            let b_es = b
                .raw_edges()
                .iter()
                .map(|e| (e.source(), e.target(), &e.weight));
            a_ns.eq(b_ns) && a_es.eq(b_es)
        }

        graph_eq(&self.tree, &other.tree)
    }
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub enum ProjectView {
    Overview(ProjectOverview),
    ProjectTree(ProjectTreeView),
    Placements(PlacementsList),
    PhaseOverview(PhaseOverview),
    PhasePlacements(PhasePlacements),
    PhasePlacementOrderings(PhasePlacementOrderings),
    Process(Process),
    Parts(PartStates),
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum ProjectViewRequest {
    // TODO add all the views and use this
    ProjectTree,
    Overview,
    Placements,
    PhaseOverview { phase: String },
    PhasePlacements { phase: String },
    Parts,
}

#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug)]
pub struct ProjectOperationViewModel {
    pub modified: bool,
    pub error: Option<String>,
}

#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum Event {
    None,
    CreateProject {
        name: String,
        /// The name of the project file
        path: PathBuf,
    },

    // TODO consider if the 'shell' should be loading and saving the project, not the core?
    //      currently the core does all loading/saving and uses stores too, this might not be how
    //      crux is intended to be used.
    Save,
    Load {
        /// The name of the project file
        path: PathBuf,
    },

    AddPcb {
        kind: PcbKind,
        name: String,
    },
    AssignVariantToUnit {
        design: DesignName,
        variant: VariantName,
        unit: ObjectPath,
    },
    RefreshFromDesignVariants,
    AssignProcessToParts {
        process: ProcessName,
        #[serde(with = "serde_regex")]
        manufacturer: Regex,
        #[serde(with = "serde_regex")]
        mpn: Regex,
    },
    CreatePhase {
        process: ProcessName,
        reference: Reference,
        load_out: LoadOutSource,
        pcb_side: PcbSide,
    },
    AssignPlacementsToPhase {
        phase: Reference,
        #[serde(with = "serde_regex")]
        placements: Regex,
    },
    AssignFeederToLoadOutItem {
        phase: Reference,
        feeder_reference: Reference,
        #[serde(with = "serde_regex")]
        manufacturer: Regex,
        #[serde(with = "serde_regex")]
        mpn: Regex,
    },
    SetPlacementOrdering {
        phase: Reference,
        placement_orderings: Vec<PlacementSortingItem>,
    },
    GenerateArtifacts,
    RecordPhaseOperation {
        phase: Reference,
        operation: ProcessOperationKind,
        set: ProcessOperationSetItem,
    },
    /// Record placements operation
    RecordPlacementsOperation {
        #[serde(with = "serde_regex")]
        object_path_patterns: Vec<Regex>,
        operation: PlacementOperation,
    },
    /// Reset operations
    ResetOperations {},

    //
    // Views
    //
    RequestOverviewView {},
    RequestPlacementsView {},
    RequestProjectTreeView {},
    RequestPhaseOverviewView {
        phase_reference: Reference,
    },
    RequestPhasePlacementsView {
        phase_reference: Reference,
    },
    RequestProcessView {
        process_name: String,
    },
    RequestPartStatesView,
}

impl Planner {
    fn update_inner(
        &self,
        event: <Planner as App>::Event,
    ) -> Box<
        dyn FnOnce(
            &mut <Planner as App>::Model,
        ) -> Result<Command<<Planner as App>::Effect, <Planner as App>::Event>, AppError>,
    > {
        match event {
            Event::None => Box::new(|_model: &mut Model| Ok(render::render())),
            Event::CreateProject {
                name,
                path,
            } => Box::new(|model: &mut Model| {
                info!("Creating project. path: {:?}", &path);

                let project = Project::new(name);
                model
                    .model_project
                    .replace(ModelProject {
                        path,
                        project,
                        modified: true,
                    });

                info!("Created project successfully.");
                Ok(render::render())
            }),
            Event::Load {
                path,
            } => Box::new(|model: &mut Model| {
                info!("Load project. path: {:?}", &path);

                let project = project::load(&path).map_err(AppError::IoError)?;

                model
                    .model_project
                    .replace(ModelProject {
                        path,
                        project,
                        modified: false,
                    });

                Ok(render::render())
            }),
            Event::Save => Box::new(|model: &mut Model| {
                let ModelProject {
                    project,
                    path,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                info!("Save project. path: {:?}", &path);

                project::save(project, path).map_err(AppError::IoError)?;

                info!("Saved. path: {:?}", path);
                *modified = false;

                Ok(render::render())
            }),
            Event::AddPcb {
                kind,
                name,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                project::add_pcb(project, kind.clone().into(), name)
                    .map_err(|cause| AppError::PcbError(cause.into()))?;

                *modified |= true;

                Ok(render::render())
            }),
            Event::AssignVariantToUnit {
                design,
                variant,
                unit,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    path,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;
                project
                    .update_assignment(unit.clone(), DesignVariant {
                        design_name: design.clone(),
                        variant_name: variant.clone(),
                    })
                    .map_err(|cause| AppError::OperationError(cause.into()))?;
                *modified |= true;
                let _refresh_result = Self::refresh_project(project, path).map_err(AppError::OperationError)?;
                Ok(render::render())
            }),
            Event::RefreshFromDesignVariants => Box::new(|model: &mut Model| {
                let ModelProject {
                    project,
                    path,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let refresh_result = Self::refresh_project(project, path).map_err(AppError::OperationError)?;
                trace!("Refreshed from design variants. modified: {}", refresh_result.modified);

                *modified |= refresh_result.modified;

                Ok(render::render())
            }),
            Event::AssignProcessToParts {
                process: process_name,
                manufacturer: manufacturer_pattern,
                mpn: mpn_pattern,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    path,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;
                let process = project
                    .find_process(&process_name)
                    .map_err(|cause| AppError::ProcessError(cause.into()))?
                    .clone();

                let refresh_result = Self::refresh_project(project, path).map_err(AppError::OperationError)?;
                *modified |= true;

                project::update_applicable_processes(
                    project,
                    refresh_result.unique_parts.as_slice(),
                    process,
                    manufacturer_pattern,
                    mpn_pattern,
                );

                Ok(render::render())
            }),
            Event::CreatePhase {
                process: process_name,
                reference,
                load_out,
                pcb_side,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;
                let process_name_str = process_name.to_string();
                let process = ProcessFactory::by_name(process_name_str.as_str())
                    .map_err(|cause| AppError::ProcessError(cause.into()))?;

                project
                    .ensure_process(&process)
                    .map_err(AppError::OperationError)?;
                *modified |= true;

                stores::load_out::ensure_load_out(&load_out).map_err(AppError::OperationError)?;

                project
                    .update_phase(reference, process.name.clone(), load_out.to_string(), pcb_side)
                    .map_err(AppError::OperationError)?;

                Ok(render::render())
            }),
            Event::AssignPlacementsToPhase {
                phase: phase_reference,
                placements: placements_pattern,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    path,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;
                let _refresh_result = Self::refresh_project(project, path).map_err(AppError::OperationError)?;
                *modified |= true;

                let phase = project
                    .phases
                    .get_mut(&phase_reference)
                    .ok_or(AppError::UnknownPhaseReference(phase_reference.clone()))?
                    .clone();

                let parts = project::assign_placements_to_phase(project, &phase, placements_pattern);
                trace!("Required load_out parts: {:?}", parts);

                *modified |= project::update_phase_operation_states(project);

                for part in parts.iter() {
                    let part_state = project
                        .part_states
                        .get_mut(&part)
                        .ok_or_else(|| PartStateError::NoPartStateFound {
                            part: part.clone(),
                        })
                        .map_err(AppError::PartError)?;

                    project::add_process_to_part(part_state, part, phase.process.clone());
                }

                stores::load_out::add_parts_to_load_out(
                    &LoadOutSource::from_str(&phase.load_out_source).unwrap(),
                    parts,
                )
                .map_err(AppError::LoadoutError)?;

                Ok(render::render())
            }),
            Event::AssignFeederToLoadOutItem {
                phase: phase_reference,
                feeder_reference,
                manufacturer,
                mpn,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let phase = project
                    .phases
                    .get(&phase_reference)
                    .ok_or(AppError::UnknownPhaseReference(phase_reference.clone()))?;

                let process = project
                    .find_process(&phase.process)
                    .map_err(|cause| AppError::ProcessError(cause.into()))?
                    .clone();

                stores::load_out::assign_feeder_to_load_out_item(
                    &phase,
                    &process,
                    &feeder_reference,
                    manufacturer,
                    mpn,
                )
                .map_err(AppError::OperationError)?;
                Ok(render::render())
            }),
            Event::SetPlacementOrdering {
                phase: reference,
                placement_orderings,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    path,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;
                let _refresh_result = Self::refresh_project(project, path).map_err(AppError::OperationError)?;
                *modified |= true;

                *modified |= project::update_placement_orderings(project, &reference, &placement_orderings)
                    .map_err(AppError::OperationError)?;

                Ok(render::render())
            }),
            Event::GenerateArtifacts => Box::new(|model: &mut Model| {
                let ModelProject {
                    project,
                    path,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                *modified |= project::update_phase_operation_states(project);

                let phase_load_out_item_map = project
                    .phases
                    .iter()
                    .try_fold(
                        BTreeMap::<Reference, Vec<LoadOutItem>>::new(),
                        |mut map, (reference, phase)| {
                            let load_out_items = stores::load_out::load_items(
                                &LoadOutSource::from_str(&phase.load_out_source).unwrap(),
                            )?;
                            map.insert(reference.clone(), load_out_items);
                            Ok::<BTreeMap<Reference, Vec<LoadOutItem>>, anyhow::Error>(map)
                        },
                    )
                    .map_err(AppError::OperationError)?;

                let directory = path.parent().unwrap();
                project::generate_artifacts(project, directory, phase_load_out_item_map)
                    .map_err(|cause| AppError::OperationError(cause.into()))?;
                Ok(render::render())
            }),
            Event::RecordPhaseOperation {
                phase: reference,
                operation,
                set,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    path,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let directory = path.parent().unwrap();
                *modified |= project::update_phase_operation(project, directory, &reference, operation, set)
                    .map_err(AppError::OperationError)?;
                Ok(render::render())
            }),
            Event::RecordPlacementsOperation {
                object_path_patterns,
                operation,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    path,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;
                let directory = path.parent().unwrap();
                *modified |= project::update_placements_operation(project, directory, object_path_patterns, operation)
                    .map_err(AppError::OperationError)?;
                Ok(render::render())
            }),
            Event::ResetOperations {} => Box::new(|model: &mut Model| {
                let ModelProject {
                    project,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;
                project::reset_operations(project).map_err(AppError::OperationError)?;

                *modified |= true;
                Ok(render::render())
            }),

            //
            // Views
            //
            Event::RequestOverviewView {} => Box::new(|model: &mut Model| {
                let ModelProject {
                    project, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let overview = ProjectOverview {
                    name: project.name.clone(),
                };
                Ok(view_renderer::view(ProjectView::Overview(overview)))
            }),
            Event::RequestPlacementsView {} => Box::new(|model: &mut Model| {
                let ModelProject {
                    project, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let placements = PlacementsList {
                    placements: project
                        .placements
                        .values()
                        .cloned()
                        .collect(),
                };

                Ok(view_renderer::view(ProjectView::Placements(placements)))
            }),
            Event::RequestProjectTreeView {} => Box::new(|model: &mut Model| {
                let ModelProject {
                    project, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let add_test_nodes = false;

                let mut project_tree = ProjectTreeView::new();

                let root_node = project_tree
                    .tree
                    .add_node(ProjectTreeItem {
                        key: "root".to_string(),
                        path: "/".to_string(),
                        ..ProjectTreeItem::default()
                    });

                let parts_node = project_tree
                    .tree
                    .add_node(ProjectTreeItem {
                        key: "parts".to_string(),
                        path: "/parts".to_string(),
                        ..ProjectTreeItem::default()
                    });
                project_tree
                    .tree
                    .add_edge(root_node, parts_node, ());

                let placements_node = project_tree
                    .tree
                    .add_node(ProjectTreeItem {
                        key: "placements".to_string(),
                        path: "/placements".to_string(),
                        ..ProjectTreeItem::default()
                    });
                project_tree
                    .tree
                    .add_edge(root_node, placements_node, ());

                let pcbs_node = project_tree
                    .tree
                    .add_node(ProjectTreeItem {
                        key: "pcbs".to_string(),
                        path: "/pcbs".to_string(),
                        ..ProjectTreeItem::default()
                    });
                project_tree
                    .tree
                    .add_edge(root_node, pcbs_node, ());

                for (index, pcb) in project.pcbs.iter().enumerate() {
                    let pcb_node = project_tree
                        .tree
                        .add_node(ProjectTreeItem {
                            key: "pcb".to_string(),
                            args: HashMap::from([
                                ("name".to_string(), Arg::String(pcb.name.clone())),
                                ("kind".to_string(), Arg::String(pcb.kind.to_string())),
                            ]),
                            path: format!("/pcbs/{}", index).to_string(),
                        });
                    project_tree
                        .tree
                        .add_edge(pcbs_node, pcb_node, ());
                }

                let unit_assignments_node = project_tree
                    .tree
                    .add_node(ProjectTreeItem {
                        key: "unit-assignments".to_string(),
                        path: "/units".to_string(),
                        ..ProjectTreeItem::default()
                    });
                project_tree
                    .tree
                    .add_edge(root_node, unit_assignments_node, ());

                for (index, (path, design_variant)) in project
                    .unit_assignments
                    .iter()
                    .enumerate()
                {
                    let unit_assignment_node = project_tree
                        .tree
                        .add_node(ProjectTreeItem {
                            key: "unit-assignment".to_string(),
                            args: HashMap::from([
                                ("name".to_string(), Arg::String(path.to_string())),
                                (
                                    "design_name".to_string(),
                                    Arg::String(design_variant.design_name.to_string()),
                                ),
                                (
                                    "variant_name".to_string(),
                                    Arg::String(design_variant.variant_name.to_string()),
                                ),
                            ]),
                            path: format!("/units/{}", index).to_string(),
                        });

                    project_tree
                        .tree
                        .add_edge(unit_assignments_node, unit_assignment_node, ());
                }

                let processes_node = project_tree
                    .tree
                    .add_node(ProjectTreeItem {
                        key: "processes".to_string(),
                        path: "/processes".to_string(),
                        ..ProjectTreeItem::default()
                    });
                project_tree
                    .tree
                    .add_edge(root_node, processes_node, ());

                for (index, process) in project.processes.iter().enumerate() {
                    let process_node = project_tree
                        .tree
                        .add_node(ProjectTreeItem {
                            key: "process".to_string(),
                            args: HashMap::from([("name".to_string(), Arg::String(process.name.to_string()))]),
                            path: format!("/processes/{}", index).to_string(),
                        });

                    project_tree
                        .tree
                        .add_edge(processes_node, process_node, ());
                }

                let phases_node = project_tree
                    .tree
                    .add_node(ProjectTreeItem {
                        key: "phases".to_string(),
                        path: "/phases".to_string(),
                        ..ProjectTreeItem::default()
                    });
                project_tree
                    .tree
                    .add_edge(root_node, phases_node, ());

                for (reference, ..) in &project.phases {
                    let phase_node = project_tree
                        .tree
                        .add_node(ProjectTreeItem {
                            key: "phase".to_string(),
                            args: HashMap::from([("reference".to_string(), Arg::String(reference.to_string()))]),
                            path: format!("/phases/{}", reference).to_string(),
                        });
                    project_tree
                        .tree
                        .add_edge(phases_node.clone(), phase_node, ());

                    if add_test_nodes {
                        let test_node = project_tree
                            .tree
                            .add_node(ProjectTreeItem {
                                key: "test".to_string(),
                                path: format!("/phases/{}/test", reference).to_string(),
                                ..ProjectTreeItem::default()
                            });
                        project_tree
                            .tree
                            .add_edge(phase_node, test_node, ());
                    }
                }

                if add_test_nodes {
                    let test_node = project_tree
                        .tree
                        .add_node(ProjectTreeItem {
                            key: "test".to_string(),
                            path: "/test".to_string(),
                            ..ProjectTreeItem::default()
                        });
                    project_tree
                        .tree
                        .add_edge(root_node, test_node, ());
                }

                Ok(view_renderer::view(ProjectView::ProjectTree(project_tree)))
            }),
            Event::RequestPhaseOverviewView {
                phase_reference,
            } => Box::new(|model: &mut Model| {
                let ModelProject {
                    project, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;
                let phase = project
                    .phases
                    .get(&phase_reference)
                    .ok_or(AppError::UnknownPhaseReference(phase_reference.clone()))?;

                let phase_overview = PhaseOverview {
                    phase_reference,
                    process: phase.process.clone(),
                    load_out_source: phase.load_out_source.clone(),
                    pcb_side: phase.pcb_side.clone(),
                };

                Ok(view_renderer::view(ProjectView::PhaseOverview(phase_overview)))
            }),
            Event::RequestPhasePlacementsView {
                phase_reference,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;
                let _phase = project
                    .phases
                    .get(&phase_reference)
                    .ok_or(AppError::UnknownPhaseReference(phase_reference.clone()))?;

                let placements = project
                    .placements
                    .iter()
                    .filter_map(|(_path, state)| match &state.phase {
                        Some(candidate_phase) if phase_reference == *candidate_phase => Some(state.clone()),
                        _ => None,
                    })
                    .collect();

                let phase_placements = PhasePlacements {
                    phase_reference,
                    placements,
                };
                Ok(view_renderer::view(ProjectView::PhasePlacements(phase_placements)))
            }),
            Event::RequestProcessView {
                process_name,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let process_name = ProcessName(process_name);

                let process = project
                    .find_process(&process_name)
                    .map_err(|err| AppError::ProcessError(err.into()))?;

                let process_view = Process {
                    name: process_name,
                    operations: process.operations.clone(),
                };

                Ok(view_renderer::view(ProjectView::Process(process_view)))
            }),
            Event::RequestPartStatesView {} => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let parts = project
                    .part_states
                    .iter()
                    .map(|(part, state)| {
                        let processes = state
                            .applicable_processes
                            .iter()
                            .cloned()
                            .collect();
                        PartWithState {
                            part: part.clone(),
                            processes,
                        }
                    })
                    .collect::<Vec<_>>();

                let part_states_view = PartStates {
                    parts,
                };

                Ok(view_renderer::view(ProjectView::Parts(part_states_view)))
            }),
        }
    }
}

impl App for Planner {
    type Event = Event;
    type Model = Model;
    type ViewModel = ProjectOperationViewModel;
    type Capabilities = Capabilities;
    type Effect = Effect;

    fn update(
        &self,
        event: Self::Event,
        model: &mut Self::Model,
        _caps: &Self::Capabilities,
    ) -> Command<Self::Effect, Self::Event> {
        let try_fn = self.update_inner(event);

        match try_fn(model) {
            Err(e) => {
                model.error.replace(format!("{:?}", e));
                render::render()
            }
            Ok(command) => command,
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        let modified = model
            .model_project
            .as_ref()
            .map_or(false, |project| project.modified);

        ProjectOperationViewModel {
            modified,
            error: model.error.clone(),
        }
    }
}

#[derive(Error, Debug)]
enum AppError {
    #[error("Operation requires a project")]
    OperationRequiresProject,
    #[error("Operation error, cause: {0}")]
    OperationError(anyhow::Error),
    #[error("Process error. cause: {0}")]
    ProcessError(anyhow::Error),
    #[error("Part error. cause: {0}")]
    PartError(PartStateError),
    #[error("Loadout error. cause: {0}")]
    LoadoutError(LoadOutOperationError),
    #[error("PCB error. cause: {0}")]
    PcbError(PcbOperationError),
    #[error("IO error. cause: {0}")]
    IoError(std::io::Error),

    #[error("Unknown phase reference. reference: {0}")]
    UnknownPhaseReference(Reference),
}

impl Planner {
    fn refresh_project(project: &mut Project, path: &PathBuf) -> anyhow::Result<ProjectRefreshResult> {
        let directory = path.parent().unwrap();

        let unique_design_variants = project.unique_design_variants();
        let design_variant_placement_map = stores::placements::load_all_placements(&unique_design_variants, directory)?;
        let refresh_result = project::refresh_from_design_variants(project, design_variant_placement_map);

        Ok(refresh_result)
    }
}

#[cfg(test)]
mod app_tests {
    use crux_core::{assert_effect, testing::AppTester};

    use super::*;

    #[test]
    fn minimal() {
        let hello = AppTester::<Planner>::default();
        let mut model = Model::default();

        // Call 'update' and request effects
        let update = hello.update(Event::None, &mut model);

        // Check update asked us to `Render`
        assert_effect!(update, Effect::Render(_));

        // Make sure the view matches our expectations
        let actual_view = &hello.view(&model);
        let expected_view = ProjectOperationViewModel::default();
        assert_eq!(actual_view, &expected_view);
    }
}
