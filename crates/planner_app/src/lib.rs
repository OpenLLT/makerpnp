use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;
use crux_core::App;
use crux_core::macros::Effect;
use crux_core::render::Render;
use regex::Regex;
use serde_with::serde_as;
use tracing::{info, trace};
use planning::design::{DesignName, DesignVariant};
use planning::phase::PhaseError;
use planning::placement::{PlacementOperation, PlacementSortingItem, PlacementState};
use planning::process::{ProcessName, ProcessOperationKind, ProcessOperationSetItem};
use planning::project;
use planning::project::{PartStateError, ProcessFactory, Project};
use planning::variant::VariantName;
use pnp::load_out::LoadOutItem;
use pnp::object_path::ObjectPath;
use pnp::part::Part;
pub use pnp::pcb::{PcbKind, PcbSide};
use stores::load_out::LoadOutSource;

pub use crux_core::Core;
use petgraph::Graph;
use thiserror::Error;
use pnp::placement::Placement;
use crate::capabilities::navigator::Navigator;
use crate::capabilities::view_renderer::ViewRenderer;

pub use planning::reference::Reference;

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
 
    error: Option<String>
}

#[derive(Effect)]
pub struct Capabilities {
    // TODO remove 'render'? perhaps put the latest view enum in the Model?
    render: Render<Event>,
    view: ViewRenderer<Event>,

    // TODO consider removing this as it's currently no-longer used.
    #[allow(dead_code)]
    navigate: Navigator<Event>
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
    pub placements: Vec<PlacementState>
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PhasePlacementOrderings {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub placement_orderings: Vec<PlacementSortingItem>
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PlacementsList {
    // FUTURE consider introducing PlacementListItem, a subset of Placement
    placements: Vec<Placement>
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone, Eq)]
pub struct ProjectTreeItem {
    pub name: String,
    /// "/" = root, paths are "/" separated.
    // FIXME path elements that contain a `/` need to be escaped and un-escaped.  e.g. a phase reference of `top/1`
    pub path: String,
}

#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug, Clone, Eq)]
pub struct ProjectOverview {
    pub name: String
}

#[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
pub struct ProjectTreeView {
    
    /// A directed graph of ProjectTreeItem.
    /// 
    /// The only relationships in the tree are parent->child, i.e. not parent->grandchild
    /// the first element is the only root element
    pub tree: Graph<ProjectTreeItem, ()>
}

impl ProjectTreeView {
    fn new() -> Self {
        Self {
            tree: Graph::new()
        }
    }
}

impl PartialEq for ProjectTreeView {
    fn eq(&self, other: &ProjectTreeView) -> bool {

        /// Acknowledgement: https://github.com/petgraph/petgraph/issues/199#issuecomment-484077775
        fn graph_eq<N, E, Ty, Ix>(
            a: &petgraph::Graph<N, E, Ty, Ix>,
            b: &petgraph::Graph<N, E, Ty, Ix>,
        ) -> bool
        where
            N: PartialEq,
            E: PartialEq,
            Ty: petgraph::EdgeType,
            Ix: petgraph::graph::IndexType + PartialEq,
        {
            let a_ns = a.raw_nodes().iter().map(|n| &n.weight);
            let b_ns = b.raw_nodes().iter().map(|n| &n.weight);
            let a_es = a.raw_edges().iter().map(|e| (e.source(), e.target(), &e.weight));
            let b_es = b.raw_edges().iter().map(|e| (e.source(), e.target(), &e.weight));
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
    pub error: Option<String>
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
        placement_orderings: Vec<PlacementSortingItem>
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
    ResetOperations {
    },
    
    //
    // Views
    //
    RequestOverviewView { },
    RequestProjectTreeView { },
    RequestPhaseOverviewView { 
        phase_reference: Reference
    },
    RequestPhasePlacementsView { 
        phase_reference: Reference
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
            Event::CreateProject { name, path } => {
                info!("Creating project. path: {:?}", &path);
                
                let project = Project::new(name);
                model.model_project.replace(ModelProject{
                    path,
                    project,
                    modified: true,
                });

                info!("Created project successfully.");
            },
            Event::Load { path } => {
                info!("Load project. path: {:?}", &path);

                match project::load(&path) {
                    Ok(project) => {
                        model.model_project.replace(ModelProject {
                            path,
                            project,
                            modified: false,
                        });
                    },
                    Err(e) => {
                        model.error.replace(format!("{:?}", e));
                    }
                }
            },
            Event::Save => {
                if let Some(ModelProject { path, project, modified, .. }) = &mut model.model_project {
                    info!("Save project. path: {:?}", &path);
                    
                    match project::save(project, &path) {
                        Ok(_) => {
                            info!("Saved. path: {:?}", path);
                            *modified = false;
                        },
                        Err(e) => {
                            model.error.replace(format!("{:?}", e));
                        },
                    }
                } else {
                    model.error.replace("project required".to_string());
                }
            },
            Event::AddPcb { kind, name } => {
                if let Some(ModelProject { project, modified, .. }) = &mut model.model_project {
                    match project::add_pcb(project, kind.clone().into(), name) {
                        Ok(_) => {
                            *modified = true;
                        },
                        Err(e) => {
                            model.error.replace(format!("{:?}", e));
                        },
                    }
                    self.update(Event::Save {}, model, caps);
                } else {
                    model.error.replace("project required".to_string());
                }
            },
            Event::AssignVariantToUnit { design, variant, unit } => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { path, project, modified, .. }) = &mut model.model_project {
                        project.update_assignment(unit.clone(), DesignVariant { design_name: design.clone(), variant_name: variant.clone() })?;
                        *modified = true;
                        let _all_parts = Self::refresh_project(project, path)?;
                    } else {
                        model.error.replace("project required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            Event::RefreshFromDesignVariants => {
                if let Some(ModelProject { path, project, modified, .. }) = &mut model.model_project {
                    if let Err(e) = Self::refresh_project(project, path) {
                        model.error.replace(format!("{:?}", e));
                    };
                    *modified = true;
                } else {
                    model.error.replace("project required".to_string());
                }
            },
            Event::AssignProcessToParts { process: process_name, manufacturer: manufacturer_pattern, mpn: mpn_pattern } => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { path, project, modified, .. }) = &mut model.model_project {
                        let process = project.find_process(&process_name)?.clone();
                        let all_parts = Self::refresh_project(project, path)?;
                        *modified = true;

                        project::update_applicable_processes(project, all_parts.as_slice(), process, manufacturer_pattern, mpn_pattern);
                    } else {
                        model.error.replace("project required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            Event::CreatePhase { process: process_name, reference, load_out, pcb_side } => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { project, modified, .. }) = &mut model.model_project {
                        let process_name_str = process_name.to_string();
                        let process = ProcessFactory::by_name(process_name_str.as_str())?;

                        project.ensure_process(&process)?;
                        *modified = true;

                        stores::load_out::ensure_load_out(&load_out)?;

                        project.update_phase(reference, process.name.clone(), load_out.to_string(), pcb_side)?;
                    } else {
                        model.error.replace("project required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            Event::AssignPlacementsToPhase { phase: reference, placements: placements_pattern } => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { path, project, modified, .. }) = &mut model.model_project {
                        let _all_parts = Self::refresh_project(project, path)?;
                        *modified = true;

                        let phase = project.phases.get(&reference)
                            .ok_or(PhaseError::UnknownPhase(reference))?.clone();

                        let parts = project::assign_placements_to_phase(project, &phase, placements_pattern);
                        trace!("Required load_out parts: {:?}", parts);

                        *modified |= project::update_phase_operation_states(project);

                        for part in parts.iter() {
                            let part_state = project.part_states.get_mut(&part)
                                .ok_or_else(|| PartStateError::NoPartStateFound { part: part.clone() })?;

                            project::add_process_to_part(part_state, part, phase.process.clone());
                        }

                        stores::load_out::add_parts_to_load_out(&LoadOutSource::from_str(&phase.load_out_source).unwrap(), parts)?;
                    } else {
                        model.error.replace("project and path required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            Event::AssignFeederToLoadOutItem { phase: reference, feeder_reference, manufacturer, mpn } => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { project, .. }) = &mut model.model_project {
                        let phase = project.phases.get(&reference)
                            .ok_or(PhaseError::UnknownPhase(reference))?.clone();

                        let process = project.find_process(&phase.process)?.clone();

                        stores::load_out::assign_feeder_to_load_out_item(&phase, &process, &feeder_reference, manufacturer, mpn)?;
                    } else {
                        model.error.replace("project and path required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            Event::SetPlacementOrdering { phase: reference, placement_orderings } => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { path, project, modified, .. }) = &mut model.model_project {
                        let _all_parts = Self::refresh_project(project, path)?;
                        *modified = true;

                        *modified |= project::update_placement_orderings(project, &reference, &placement_orderings)?;
                    } else {
                        model.error.replace("project and path required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            Event::GenerateArtifacts => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { path, project, modified, .. }) = &mut model.model_project {
                        *modified = project::update_phase_operation_states(project);

                        let phase_load_out_item_map = project.phases.iter().try_fold(BTreeMap::<Reference, Vec<LoadOutItem>>::new(), |mut map, (reference, phase) | {
                            let load_out_items = stores::load_out::load_items(&LoadOutSource::from_str(&phase.load_out_source).unwrap())?;
                            map.insert(reference.clone(), load_out_items);
                            Ok::<BTreeMap<Reference, Vec<LoadOutItem>>, anyhow::Error>(map)
                        })?;

                        let directory = path.parent().unwrap();
                        project::generate_artifacts(&project, directory, phase_load_out_item_map)?;
                    } else {
                        model.error.replace("project required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            Event::RecordPhaseOperation { phase: reference, operation, set } => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { path, project, modified, .. }) = &mut model.model_project {
                        let directory = path.parent().unwrap();
                        *modified = project::update_phase_operation(project, directory, &reference, operation.into(), set.into())?;
                    } else {
                        model.error.replace("project required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            Event::RecordPlacementsOperation { object_path_patterns, operation } => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { path, project, modified, .. }) = &mut model.model_project {
                        let directory = path.parent().unwrap();
                        *modified = project::update_placements_operation(project, directory, object_path_patterns, operation.into())?;
                    } else {
                        model.error.replace("project required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            Event::ResetOperations { } => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { project, modified, .. }) = &mut model.model_project {
                        project::reset_operations(project)?;
                        *modified = true;
                    } else {
                        model.error.replace("project required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            
            //
            // Views
            // 
            Event::RequestOverviewView { } => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { project, .. }) = &mut model.model_project {

                        let overview = ProjectOverview {
                            name: project.name.clone(),
                        };
                        caps.view.view(ProjectView::Overview(overview), |_|Event::None)

                    } else {
                        model.error.replace("project required".to_string());
                    }
                    Ok(())
                };
                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            Event::RequestProjectTreeView { } => {
                default_render = false;
                
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { project, .. }) = &mut model.model_project {

                        let add_test_nodes = false;
                        
                        let mut project_tree = ProjectTreeView::new();
                        
                        let root_node = project_tree.tree.add_node(ProjectTreeItem { name: "Root".to_string(), path: "/".to_string() });
                        
                        let phases_node = project_tree.tree.add_node(ProjectTreeItem { name: "Phases".to_string(), path: "/phases".to_string() });
                        project_tree.tree.add_edge(root_node.clone(), phases_node.clone(), ());
                        
                        for (reference, ..) in &project.phases {
                            let phase_node = project_tree.tree.add_node(ProjectTreeItem {
                                name: reference.to_string(),
                                path: format!("/phases/{}", reference).to_string()
                            });
                            project_tree.tree.add_edge(phases_node.clone(), phase_node, ());
                            
                            if add_test_nodes {
                                let test_node = project_tree.tree.add_node(ProjectTreeItem {
                                    name: "Test".to_string(),
                                    path: format!("/phases/{}/test", reference).to_string()
                                });
                                project_tree.tree.add_edge(phase_node, test_node, ());
                            }
                        }

                        if add_test_nodes {
                            let test_node = project_tree.tree.add_node(ProjectTreeItem {
                                name: "Test".to_string(),
                                path: "/test".to_string()
                            });
                            project_tree.tree.add_edge(root_node.clone(), test_node, ());
                        }


                        caps.view.view(ProjectView::ProjectTree(project_tree), |_|Event::None)    
                        
                    } else {
                        model.error.replace("project required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            Event::RequestPhaseOverviewView { phase_reference } => {
                default_render = false;

                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { project, .. }) = &mut model.model_project {
                        
                        if let Some(phase) = project.phases.get(&phase_reference) {
                            let phase_overview = PhaseOverview {
                                phase_reference,
                                process: phase.process.clone(),
                                load_out_source: phase.load_out_source.clone(),
                                pcb_side: phase.pcb_side.clone(),
                            };

                            caps.view.view(ProjectView::PhaseOverview(phase_overview), |_| Event::None)
                        } else {
                            model.error.replace("unknown reference".to_string());
                        }
                    } else {
                        model.error.replace("project required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            Event::RequestPhasePlacementsView { phase_reference } => {
                default_render = false;

                let try_fn = |model: &mut Model| -> Result<(), AppError> {
                    // TODO use this style of returning early (using .ok_or()) in other event handlers to reduce nesting.
                    let ModelProject { project, .. } = model.model_project.as_mut().ok_or(AppError::OperationRequiresProject)?;

                    let _phase = project.phases.get(&phase_reference).ok_or(AppError::UnknownPhaseReference(phase_reference.clone()))?;

                    let placements = project.placements.iter().filter_map(|(path, state)|{
                        match &state.phase {
                            Some(candidate_phase) if phase_reference == *candidate_phase => Some(state.clone()),
                            _ => None
                        }
                    }).collect();
                    
                    let phase_placements = PhasePlacements {
                        phase_reference,
                        placements
                    };

                    caps.view.view(ProjectView::PhasePlacements(phase_placements), |_|Event::None);
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
        let modified = model.model_project
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
    #[error("Unknown phase reference. reference: {0}")]
    UnknownPhaseReference(Reference),
    #[error("Operation requires a project")]
    OperationRequiresProject,
}

impl Planner {
    fn refresh_project(project: &mut Project, path: &PathBuf) -> anyhow::Result<Vec<Part>> {
        let directory = path.parent().unwrap();
        
        let unique_design_variants = project.unique_design_variants();
        let design_variant_placement_map = stores::placements::load_all_placements(
            &unique_design_variants,
            directory
        )?;
        let all_parts = project::refresh_from_design_variants(project, design_variant_placement_map);

        // TODO make this return a 'modified' flag too
        Ok(all_parts)
    }
}

#[cfg(test)]
mod app_tests {
    use super::*;
    use crux_core::{assert_effect, testing::AppTester};

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
