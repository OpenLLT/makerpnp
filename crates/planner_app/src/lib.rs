use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::str::FromStr;

use crux_core::macros::Effect;
use crux_core::render::Render;
use crux_core::App;
pub use crux_core::Core;
use petgraph::Graph;
pub use planning::design::{DesignName, DesignVariant};
use planning::placement::{PlacementOperation, PlacementSortingItem, PlacementState};
use planning::process::{ProcessName, ProcessOperationKind, ProcessOperationSetItem};
use planning::project;
use planning::project::{PartStateError, PcbOperationError, ProcessFactory, Project, ProjectRefreshResult};
pub use planning::reference::Reference;
pub use planning::variant::VariantName;
use pnp::load_out::LoadOutItem;
pub use pnp::object_path::ObjectPath;
pub use pnp::pcb::{PcbKind, PcbSide};
use pnp::placement::Placement;
use regex::Regex;
use serde_with::serde_as;
use stores::load_out::{LoadOutOperationError, LoadOutSource};
use thiserror::Error;
use tracing::{info, trace};

use crate::capabilities::navigator::Navigator;
use crate::capabilities::view_renderer::ViewRenderer;

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
pub struct Capabilities {
    // TODO remove 'render'? perhaps put the latest view enum in the Model?
    render: Render<Event>,
    view: ViewRenderer<Event>,

    // TODO consider removing this as it's currently no-longer used.
    #[allow(dead_code)]
    navigate: Navigator<Event>,
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
pub struct PhasePlacementOrderings {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub placement_orderings: Vec<PlacementSortingItem>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PlacementsList {
    // FUTURE consider introducing PlacementListItem, a subset of Placement
    placements: Vec<Placement>,
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
    RequestProjectTreeView {},
    RequestPhaseOverviewView {
        phase_reference: Reference,
    },
    RequestPhasePlacementsView {
        phase_reference: Reference,
    },
}

impl App for Planner {
    type Event = Event;
    type Model = Model;
    type ViewModel = ProjectOperationViewModel;
    type Capabilities = Capabilities;

    fn update(&self, event: Self::Event, model: &mut Self::Model, caps: &Self::Capabilities) {
        let mut default_render = true;
        match event {
            Event::None => {}
            Event::CreateProject {
                name,
                path,
            } => {
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
            }
            Event::Load {
                path,
            } => {
                let try_fn = |model: &mut Model| -> Result<(), AppError> {
                    info!("Load project. path: {:?}", &path);

                    let project = project::load(&path).map_err(AppError::IoError)?;

                    model
                        .model_project
                        .replace(ModelProject {
                            path,
                            project,
                            modified: false,
                        });

                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            Event::Save => {
                let try_fn = |model: &mut Model| -> Result<(), AppError> {
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

                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            Event::AddPcb {
                kind,
                name,
            } => {
                let try_fn = |model: &mut Model| -> Result<(), AppError> {
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

                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            Event::AssignVariantToUnit {
                design,
                variant,
                unit,
            } => {
                let try_fn = |model: &mut Model| -> Result<(), AppError> {
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
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            Event::RefreshFromDesignVariants => {
                let try_fn = |model: &mut Model| -> Result<(), AppError> {
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
                    *modified |= refresh_result.modified;

                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            Event::AssignProcessToParts {
                process: process_name,
                manufacturer: manufacturer_pattern,
                mpn: mpn_pattern,
            } => {
                let try_fn = |model: &mut Model| -> Result<(), AppError> {
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

                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            Event::CreatePhase {
                process: process_name,
                reference,
                load_out,
                pcb_side,
            } => {
                let try_fn = |model: &mut Model| -> Result<(), AppError> {
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

                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            Event::AssignPlacementsToPhase {
                phase: phase_reference,
                placements: placements_pattern,
            } => {
                let try_fn = |model: &mut Model| -> Result<(), AppError> {
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

                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            Event::AssignFeederToLoadOutItem {
                phase: phase_reference,
                feeder_reference,
                manufacturer,
                mpn,
            } => {
                let try_fn = |model: &mut Model| -> Result<(), AppError> {
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
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            Event::SetPlacementOrdering {
                phase: reference,
                placement_orderings,
            } => {
                let try_fn = |model: &mut Model| -> Result<(), AppError> {
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

                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            Event::GenerateArtifacts => {
                let try_fn = |model: &mut Model| -> Result<(), AppError> {
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
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            Event::RecordPhaseOperation {
                phase: reference,
                operation,
                set,
            } => {
                let try_fn = |model: &mut Model| -> Result<(), AppError> {
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
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            Event::RecordPlacementsOperation {
                object_path_patterns,
                operation,
            } => {
                let try_fn = |model: &mut Model| -> Result<(), AppError> {
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
                    *modified |=
                        project::update_placements_operation(project, directory, object_path_patterns, operation)
                            .map_err(AppError::OperationError)?;
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            Event::ResetOperations {} => {
                let try_fn = |model: &mut Model| -> Result<(), AppError> {
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
                    Ok(())
                };
                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }

            //
            // Views
            //
            Event::RequestOverviewView {} => {
                let try_fn = |model: &mut Model| -> Result<(), AppError> {
                    let ModelProject {
                        project, ..
                    } = model
                        .model_project
                        .as_mut()
                        .ok_or(AppError::OperationRequiresProject)?;

                    let overview = ProjectOverview {
                        name: project.name.clone(),
                    };
                    caps.view
                        .view(ProjectView::Overview(overview));
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            Event::RequestProjectTreeView {} => {
                default_render = false;

                let try_fn = |model: &mut Model| -> Result<(), AppError> {
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

                    let pcbs_node = project_tree
                        .tree
                        .add_node(ProjectTreeItem {
                            key: "pcbs".to_string(),
                            path: "/pcbs".to_string(),
                            ..ProjectTreeItem::default()
                        });
                    project_tree
                        .tree
                        .add_edge(root_node.clone(), pcbs_node.clone(), ());

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
                            .add_edge(pcbs_node.clone(), pcb_node, ());
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
                        .add_edge(root_node.clone(), unit_assignments_node.clone(), ());

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
                            .add_edge(unit_assignments_node.clone(), unit_assignment_node, ());
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
                        .add_edge(root_node.clone(), processes_node.clone(), ());

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
                            .add_edge(processes_node.clone(), process_node, ());
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
                        .add_edge(root_node.clone(), phases_node.clone(), ());

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
                            .add_edge(root_node.clone(), test_node, ());
                    }

                    caps.view
                        .view(ProjectView::ProjectTree(project_tree));
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            Event::RequestPhaseOverviewView {
                phase_reference,
            } => {
                default_render = false;

                let try_fn = |model: &mut Model| -> Result<(), AppError> {
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

                    caps.view
                        .view(ProjectView::PhaseOverview(phase_overview));
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            Event::RequestPhasePlacementsView {
                phase_reference,
            } => {
                default_render = false;

                let try_fn = |model: &mut Model| -> Result<(), AppError> {
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

                    caps.view
                        .view(ProjectView::PhasePlacements(phase_placements));
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
        }

        if default_render {
            // This causes the shell to request the view, via `view()`
            caps.render.render();
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
        let hello = AppTester::<Planner, _>::default();
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
