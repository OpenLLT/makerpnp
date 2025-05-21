use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::SystemTime;

use anyhow::anyhow;
use crux_core::macros::effect;
use crux_core::render::RenderOperation;
pub use crux_core::Core;
use crux_core::{render, App, Command};
use petgraph::Graph;
pub use planning::actions::{AddOrRemoveAction, SetOrClearAction};
pub use planning::design::{DesignIndex, DesignName, DesignNumber, DesignVariant};
pub use planning::file::{FileReference, FileReferenceError};
pub use planning::gerber::GerberPurpose;
use planning::pcb::Pcb;
pub use planning::phase::PhaseReference;
use planning::phase::{Phase, PhaseState};
pub use planning::placement::PlacementSortingItem;
pub use planning::placement::PlacementSortingMode;
pub use planning::placement::PlacementStatus;
pub use planning::placement::ProjectPlacementStatus;
pub use planning::placement::{PlacementOperation, PlacementState};
pub use planning::process::ProcessReference;
pub use planning::process::TaskReference;
pub use planning::process::TaskStatus;
pub use planning::process::{OperationReference, OperationStatus, ProcessDefinition, TaskAction};
use planning::project::{PartStateError, PcbOperationError, ProcessFactory, Project, ProjectRefreshResult};
pub use planning::variant::VariantName;
use planning::{file, project};
pub use pnp::load_out::LoadOutItem;
pub use pnp::object_path::ObjectPath;
pub use pnp::part::Part;
pub use pnp::pcb::PcbSide;
pub use pnp::pcb::{PcbUnitIndex, PcbUnitNumber};
pub use pnp::placement::Placement;
pub use pnp::placement::RefDes;
pub use pnp::reference::Reference;
use regex::Regex;
use serde_with::serde_as;
pub use stores::load_out::LoadOutSource;
use stores::load_out::{LoadOutOperationError, LoadOutSourceError};
use thiserror::Error;
use tracing::{debug, info, trace, warn};

use crate::effects::pcb_view_renderer::PcbViewRendererOperation;
use crate::effects::project_view_renderer::ProjectViewRendererOperation;
use crate::effects::{pcb_view_renderer, project_view_renderer};

pub mod effects;

extern crate serde_regex;

#[derive(Default)]
pub struct Planner;

#[derive(Default)]
pub struct ModelProject {
    path: PathBuf,
    project: Project,
    modified: bool,
}

impl ModelProject {
    fn pcbs<'a, 'b>(&'a self, model_pcbs: &'b ModelPcbs) -> impl Iterator<Item = Option<&'b ModelPcb>> + use<'a, 'b> {
        self.project
            .pcbs
            .iter()
            .map(|project_pcb| model_pcbs.get(&project_pcb.pcb_file))
    }
}

pub struct ModelPcb {
    path: PathBuf,
    pcb: Pcb,
    modified: bool,
}

type ModelPcbs = BTreeMap<FileReference, ModelPcb>;

#[derive(Default)]
pub struct Model {
    model_project: Option<ModelProject>,
    /// PCBs that have been created/loaded.
    ///
    /// Important: Can contain instances of [`ModelPcb`] that have been created or loaded, but not assigned to a project yet.
    model_pcbs: ModelPcbs,

    error: Option<(chrono::DateTime<chrono::Utc>, String)>,
}

impl Model {
    /// an iterator over the pcbs for the project
    ///
    /// will return Some(None) for any PCB that hasn't been loaded.
    #[allow(dead_code)]
    fn project_model_pcbs(&self) -> impl Iterator<Item = Option<&ModelPcb>> + '_ {
        Self::project_model_pcbs_maybe(&self.model_project, &self.model_pcbs)
    }

    fn project_model_pcbs_maybe<'a, 'b>(
        model_project: &'a Option<ModelProject>,
        model_pcbs: &'b ModelPcbs,
    ) -> impl Iterator<Item = Option<&'b ModelPcb>> + use<'a, 'b> {
        model_project
            .as_ref()
            .into_iter()
            .flat_map(|model_project| model_project.pcbs(model_pcbs))
    }

    // FUTURE consider adding a 'refresh_project_pcbs' that re-loads them.

    /// Load PCBs for the project that are not already loaded.
    ///
    /// * Requires a project to be loaded.
    /// * Stops and returns on the first error.
    fn load_unloaded_project_pcbs(&mut self, root: &PathBuf) -> Result<(), AppError> {
        let Some(model_project) = &mut self.model_project else {
            return Err(AppError::OperationRequiresProject);
        };

        Self::load_project_pcbs_inner(&mut model_project.project, &mut self.model_pcbs, root)
    }

    fn load_project_pcbs_inner(
        project: &mut Project,
        model_pcbs: &mut ModelPcbs,
        root: &PathBuf,
    ) -> Result<(), AppError> {
        let pcbs_to_load = project
            .pcbs
            .iter_mut()
            .filter(|pcb| !model_pcbs.contains_key(&pcb.pcb_file))
            .collect::<Vec<_>>();

        // Then, process one-by-one: load and insert
        for pcb in pcbs_to_load {
            let (pcb_file, pcb_data, path) = pcb
                .load_pcb(root)
                .map_err(AppError::IoError)?;
            model_pcbs.insert(pcb_file, ModelPcb {
                path,
                pcb: pcb_data,
                modified: false,
            });
        }

        Ok(())
    }

    #[allow(dead_code)]
    fn project_pcbs(&self) -> Vec<&Pcb> {
        let iter = self.project_model_pcbs();
        Self::project_pcbs_inner(iter)
    }

    /// Return the loaded PCBs for the project.
    ///
    /// The iterator here should be an iterator that returns `Some(None)` for any PCB that hasn't been loaded.
    /// See [`project_pcbs`], [`project_model_pcbs`]
    fn project_pcbs_inner<'a>(iter: impl Iterator<Item = Option<&'a ModelPcb>>) -> Vec<&'a Pcb> {
        iter.filter_map(|model_pcb| {
            model_pcb
                .as_ref()
                .map(|model_pcb| &model_pcb.pcb)
        })
        .collect::<Vec<_>>()
    }

    /// Load a PCB, no project required.
    fn load_pcb(&mut self, pcb_file: FileReference, root: &Option<PathBuf>) -> Result<(), AppError> {
        let path = pcb_file
            .try_build_path(root.as_ref())
            .map_err(AppError::FileReferenceError)?;

        let pcb = file::load::<Pcb>(&path).map_err(AppError::IoError)?;

        self.model_pcbs
            .insert(pcb_file, ModelPcb {
                path,
                pcb,
                modified: false,
            });

        Ok(())
    }

    /// Save a PCB, no project required.
    ///
    /// PCB is saved using the path build from the file reference when it was loaded.
    fn save_pcb(&mut self, pcb_file: &FileReference) -> Result<(), AppError> {
        info!("Saving PCB. pcb_file: {}", pcb_file);
        let model_pcb = self
            .model_pcbs
            .get_mut(pcb_file)
            .ok_or(AppError::OperationError(anyhow!(
                "PCB not loaded. pcb_file: {:?}",
                pcb_file
            )))?;

        file::save::<Pcb>(&model_pcb.pcb, &model_pcb.path).map_err(AppError::IoError)?;

        model_pcb.modified = false;

        Ok(())
    }
}

#[effect]
pub enum Effect {
    Render(RenderOperation),
    ProjectView(ProjectViewRendererOperation),
    PcbView(PcbViewRendererOperation),
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PcbGerberItem {
    pub path: PathBuf,
    /// if `None` then the gerber applies to both sides, e.g. 'pcb outline'
    pub pcb_side: Option<PcbSide>,
    pub purpose: GerberPurpose,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct ProjectPcbOverview {
    pub index: u16,
    pub pcb_file: FileReference,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PcbOverview {
    pub path: PathBuf,
    pub pcb_file: FileReference,

    pub name: String,
    pub units: u16,
    /// A list of unique designs, a panel can have multiple designs.
    pub designs: Vec<DesignName>,

    /// A map of design to units, some units may be un-assigned
    /// The name of the design can be obtained by looking indexing into `designs` with the `DesignIndex`
    pub unit_map: HashMap<PcbUnitIndex, DesignIndex>,

    /// each design can have multiple gerbers
    pub gerbers: Vec<Vec<PcbGerberItem>>,
    // FUTURE add dimensions (per design)
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PcbUnitAssignments {
    /// the design name for the pcb unit index can be obtained via the PCB overview
    /// not all pcb units may be assigned
    pub unit_assignments: HashMap<PcbUnitIndex, VariantName>,
    pub index: u16,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct LoadOut {
    pub phase_reference: PhaseReference,
    pub source: LoadOutSource,
    pub items: Vec<LoadOutItem>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct Phases {
    pub phases: Vec<PhaseOverview>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PhaseOverview {
    pub phase_reference: PhaseReference,
    pub process: ProcessReference,
    pub load_out_source: LoadOutSource,
    pub pcb_side: PcbSide,
    pub phase_placement_orderings: Vec<PlacementSortingItem>,
    pub state: PhaseState,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PlacementsItem {
    pub path: ObjectPath,
    pub state: PlacementState,
    pub ordering: usize,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PhasePlacements {
    pub phase_reference: PhaseReference,

    pub placements: Vec<PlacementsItem>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PartWithState {
    pub part: Part,
    pub processes: Vec<ProcessReference>,
    pub ref_des_set: BTreeSet<RefDes>,
    pub quantity: usize,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PartStates {
    pub parts: Vec<PartWithState>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PlacementsList {
    pub placements: Vec<PlacementsItem>,
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
    Boolean(bool),
    String(String),
    Integer(i64),
    // Add other types, like 'Number' here as required.
}

#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug, Clone, Eq)]
pub struct ProjectOverview {
    pub name: String,
    pub processes: Vec<ProcessReference>,
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
    Parts(PartStates),
    Phases(Phases),
    PhaseLoadOut(LoadOut),
    PhaseOverview(PhaseOverview),
    PhasePlacements(PhasePlacements),
    Placements(PlacementsList),
    ProjectTree(ProjectTreeView),
    Process(ProcessDefinition),
    PcbOverview(ProjectPcbOverview),
    PcbUnitAssignments(PcbUnitAssignments),
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum ProjectViewRequest {
    Overview,
    Parts,
    Phases,
    PhaseLoadOut { phase: PhaseReference },
    PhaseOverview { phase: PhaseReference },
    PhasePlacements { phase: PhaseReference },
    Placements,
    ProjectTree,
    PcbOverview { pcb: u16 },
    PcbUnitAssignments { pcb: u16 },
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub enum PcbView {
    PcbOverview(PcbOverview),
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum PcbViewRequest {
    Overview { pcb_file: FileReference },
}

#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug)]
pub struct PlannerOperationViewModel {
    pub project_modified: bool,
    pub pcbs_modified: bool,
    pub error: Option<(chrono::DateTime<chrono::Utc>, String)>,
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
        pcb_file: FileReference,
    },
    CreateProjectPcb {
        name: String,
        units: u16,
        unit_map: BTreeMap<PcbUnitNumber, DesignName>,
    },
    SaveAllPcbs,
    AssignVariantToUnit {
        unit: ObjectPath,
        variant: VariantName,
    },
    RefreshFromDesignVariants,
    AssignProcessToParts {
        process: ProcessReference,
        operation: AddOrRemoveAction,
        #[serde(with = "serde_regex")]
        manufacturer: Regex,
        #[serde(with = "serde_regex")]
        mpn: Regex,
    },
    CreatePhase {
        process: ProcessReference,
        reference: PhaseReference,
        load_out: LoadOutSource,
        pcb_side: PcbSide,
    },
    AssignPlacementsToPhase {
        phase: PhaseReference,
        operation: SetOrClearAction,

        /// to apply to object path (not refdes)
        #[serde(with = "serde_regex")]
        placements: Regex,
    },
    AddPartsToLoadout {
        phase: PhaseReference,
        #[serde(with = "serde_regex")]
        manufacturer: Regex,
        #[serde(with = "serde_regex")]
        mpn: Regex,
    },
    AssignFeederToLoadOutItem {
        phase: PhaseReference,
        feeder_reference: Reference,
        #[serde(with = "serde_regex")]
        manufacturer: Regex,
        #[serde(with = "serde_regex")]
        mpn: Regex,
    },
    SetPlacementOrdering {
        phase: PhaseReference,
        placement_orderings: Vec<PlacementSortingItem>,
    },
    GenerateArtifacts,
    RecordPhaseOperation {
        phase: PhaseReference,
        operation: OperationReference,
        task: TaskReference,
        action: TaskAction,
    },
    /// Record placements operation
    RecordPlacementsOperation {
        #[serde(with = "serde_regex")]
        object_path_patterns: Vec<Regex>,
        operation: PlacementOperation,
    },
    RemoveUsedPlacements {
        phase: Option<PhaseReference>,
    },
    /// Reset operations
    ResetOperations {},

    //
    // Project Views
    //
    RequestOverviewView {},
    RequestPlacementsView {},
    RequestProjectTreeView {},
    RequestPhasesView {},
    RequestPhaseOverviewView {
        phase_reference: PhaseReference,
    },
    RequestPhasePlacementsView {
        phase_reference: PhaseReference,
    },
    RequestProcessView {
        process_reference: String,
    },
    RequestPartStatesView,
    RequestPhaseLoadOutView {
        phase_reference: PhaseReference,
    },
    RequestProjectPcbOverviewView {
        /// index, 0-based
        pcb: u16,
    },
    RequestPcbUnitAssignmentsView {
        /// index, 0-based
        pcb: u16,
    },

    //
    // PCB operations
    //
    LoadPcb {
        pcb_file: FileReference,
        /// The directory, for relative paths. e.g. the project's directory
        /// if this is None, only Absolute paths can be used.
        root: Option<PathBuf>,
    },

    AddGerberFiles {
        pcb_file: FileReference,
        design: DesignName,
        // TODO use FileReferences, not paths?
        files: Vec<(PathBuf, Option<PcbSide>, GerberPurpose)>,
    },
    RemoveGerberFiles {
        pcb_file: FileReference,
        design: DesignName,
        files: Vec<PathBuf>,
    },

    //
    // PCB views
    //
    RequestPcbOverviewView {
        pcb_file: FileReference,
    },
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
            } => Box::new(move |model: &mut Model| {
                info!("Load project. path: {:?}", &path);

                let project: Project = file::load(&path).map_err(AppError::IoError)?;

                model
                    .model_project
                    .replace(ModelProject {
                        path: path.clone(),
                        project,
                        modified: false,
                    });

                let project_directory = path.parent().unwrap();
                model.load_unloaded_project_pcbs(&project_directory.to_path_buf())?;

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

                file::save(project, path).map_err(AppError::IoError)?;

                info!("Saved. path: {:?}", path);
                *modified = false;

                Ok(render::render())
            }),
            Event::CreateProjectPcb {
                name,
                units,
                unit_map,
            } => Box::new(move |model: &mut Model| {
                // project required so that relative file references can be created.
                let ModelProject {
                    path, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let project_directory = path.parent().unwrap();

                let (pcb, pcb_file, pcb_path) = planning::pcb::create_pcb(project_directory, name, units, unit_map)
                    .map_err(AppError::PcbOperationError)?;

                model
                    .model_pcbs
                    .insert(pcb_file.clone(), ModelPcb {
                        path: pcb_path,
                        pcb: pcb.clone(),
                        // not saved, yet
                        modified: true,
                    });

                model.save_pcb(&pcb_file)?;

                // TODO tell to UI to navigate to the newly created file, don't use a view
                Ok(render::render())
            }),
            Event::LoadPcb {
                pcb_file,
                root,
            } => Box::new(move |model: &mut Model| {
                // Note: doesn't require a project.
                info!("Load pcb. pcb_file: {:?}", &pcb_file);

                model.load_pcb(pcb_file, &root)?;

                Ok(render::render())
            }),
            Event::SaveAllPcbs => Box::new(|model: &mut Model| {
                for (_file_reference, model_pcb) in model.model_pcbs.iter_mut() {
                    let ModelPcb {
                        path,
                        pcb,
                        modified,
                    } = model_pcb;

                    info!("Save pcb. path: {:?}", path);

                    file::save(pcb, &path).map_err(AppError::IoError)?;

                    info!("Saved. path: {:?}", path);
                    *modified = false;
                }

                Ok(render::render())
            }),
            Event::AddPcb {
                pcb_file,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    modified,
                    path,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let project_directory = path.parent().unwrap();
                let pcb_path = pcb_file.build_path(&project_directory.to_path_buf());

                let pcb = file::load::<Pcb>(&pcb_path).map_err(AppError::IoError)?;

                project::add_pcb(project, &pcb_file).map_err(AppError::PcbOperationError)?;

                model
                    .model_pcbs
                    .insert(pcb_file, ModelPcb {
                        path: pcb_path,
                        pcb,
                        modified: false,
                    });

                *modified |= true;

                Ok(render::render())
            }),
            Event::AssignVariantToUnit {
                variant: variant_name,
                unit,
            } => Box::new(move |model: &mut Model| {
                let (
                    ModelProject {
                        project,
                        path,
                        modified,
                        ..
                    },
                    pcbs,
                    ..,
                ) = { Self::model_project_and_pcbs(model) }?;

                project
                    .update_assignment(&pcbs, unit.clone(), variant_name)
                    .map_err(AppError::OperationError)?;
                *modified |= true;

                let refresh_result = Self::refresh_project(project, &pcbs, path).map_err(AppError::OperationError)?;
                *modified |= refresh_result.modified;

                Ok(render::render())
            }),
            Event::RefreshFromDesignVariants => Box::new(|model: &mut Model| {
                let (
                    ModelProject {
                        project,
                        path,
                        modified,
                        ..
                    },
                    pcbs,
                    ..,
                ) = { Self::model_project_and_pcbs(model) }?;
                let refresh_result = Self::refresh_project(project, &pcbs, path).map_err(AppError::OperationError)?;
                *modified |= refresh_result.modified;

                Ok(render::render())
            }),
            Event::AssignProcessToParts {
                process: process_name,
                operation,
                manufacturer: manufacturer_pattern,
                mpn: mpn_pattern,
            } => Box::new(move |model: &mut Model| {
                let (
                    ModelProject {
                        project,
                        path,
                        modified,
                        ..
                    },
                    pcbs,
                    ..,
                ) = { Self::model_project_and_pcbs(model) }?;

                let process = project
                    .find_process(&process_name)
                    .map_err(|cause| AppError::ProcessError(cause.into()))?
                    .clone();

                let refresh_result = Self::refresh_project(project, &pcbs, path).map_err(AppError::OperationError)?;
                *modified |= refresh_result.modified;

                *modified |= project::update_applicable_processes(
                    project,
                    refresh_result.unique_parts.as_slice(),
                    process,
                    operation,
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
                    .update_phase(reference, process.reference.clone(), load_out.to_string(), pcb_side)
                    .map_err(AppError::OperationError)?;

                Ok(render::render())
            }),
            Event::AssignPlacementsToPhase {
                phase: phase_reference,
                operation,
                placements: placements_pattern,
            } => Box::new(move |model: &mut Model| {
                let (
                    ModelProject {
                        project,
                        path,
                        modified,
                        ..
                    },
                    pcbs,
                    ..,
                ) = { Self::model_project_and_pcbs(model) }?;

                let refresh_result = Self::refresh_project(project, &pcbs, path).map_err(AppError::OperationError)?;
                *modified |= refresh_result.modified;

                let phase = project
                    .phases
                    .get_mut(&phase_reference)
                    .ok_or(AppError::UnknownPhaseReference(phase_reference.clone()))?
                    .clone();

                let parts = project::assign_placements_to_phase(project, &phase, operation.clone(), placements_pattern);
                trace!("Required load_out parts: {:?}", parts);

                *modified |= project::refresh_phase_operation_states(project);

                let load_out_source =
                    try_build_phase_load_out_source(&path, &phase).map_err(AppError::LoadoutSourceError)?;

                match operation {
                    SetOrClearAction::Set => {
                        for part in parts.iter() {
                            let part_state = project
                                .part_states
                                .get_mut(&part)
                                .ok_or_else(|| PartStateError::NoPartStateFound {
                                    part: part.clone(),
                                })
                                .map_err(AppError::PartError)?;

                            *modified |= project::add_process_to_part(part_state, part, phase.process.clone());
                        }
                        stores::load_out::add_parts_to_load_out(&load_out_source, parts)
                            .map_err(AppError::LoadoutError)?;
                    }
                    SetOrClearAction::Clear => {
                        // FUTURE not currently sure if cleanup should happen automatically or if it should be explicit.
                    }
                }
                Ok(render::render())
            }),
            Event::AddPartsToLoadout {
                phase: phase_reference,
                manufacturer: manufacturer_pattern,
                mpn: mpn_pattern,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    path,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let phase = project
                    .phases
                    .get_mut(&phase_reference)
                    .ok_or(AppError::UnknownPhaseReference(phase_reference.clone()))?;

                let load_out_source =
                    try_build_phase_load_out_source(&path, &phase).map_err(AppError::LoadoutSourceError)?;

                let parts = project::find_phase_parts(project, &phase_reference, manufacturer_pattern, mpn_pattern);

                stores::load_out::add_parts_to_load_out(&load_out_source, parts).map_err(AppError::LoadoutError)?;

                Ok(render::render())
            }),
            Event::RemoveUsedPlacements {
                phase: phase_reference,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                *modified |= project.remove_unused_placements(phase_reference);

                Ok(render::render())
            }),
            Event::AssignFeederToLoadOutItem {
                phase: phase_reference,
                feeder_reference,
                manufacturer,
                mpn,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    path,
                    project,
                    ..
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

                let load_out_source =
                    try_build_phase_load_out_source(path, phase).map_err(AppError::LoadoutSourceError)?;

                stores::load_out::assign_feeder_to_load_out_item(
                    &load_out_source,
                    &process,
                    feeder_reference,
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
                let (
                    ModelProject {
                        project,
                        path,
                        modified,
                        ..
                    },
                    pcbs,
                    ..,
                ) = { Self::model_project_and_pcbs(model) }?;

                let refresh_result = Self::refresh_project(project, &pcbs, path).map_err(AppError::OperationError)?;
                *modified |= refresh_result.modified;

                *modified |= project::update_placement_orderings(project, &reference, &placement_orderings)
                    .map_err(AppError::OperationError)?;

                Ok(render::render())
            }),
            Event::GenerateArtifacts => Box::new(|model: &mut Model| {
                let (
                    ModelProject {
                        project,
                        modified,
                        ..
                    },
                    pcbs,
                    project_directory,
                ) = { Self::model_project_and_pcbs(model) }?;

                *modified |= project::refresh_phase_operation_states(project);

                let phase_load_out_item_map = project
                    .phases
                    .iter()
                    .try_fold(
                        BTreeMap::<Reference, Vec<LoadOutItem>>::new(),
                        |mut map, (reference, phase)| {
                            let load_out_items = stores::load_out::load_items(&LoadOutSource::try_from_path(
                                &project_directory,
                                PathBuf::from_str(&phase.load_out_source)?,
                            )?)?;
                            map.insert(reference.clone(), load_out_items);
                            Ok::<BTreeMap<Reference, Vec<LoadOutItem>>, anyhow::Error>(map)
                        },
                    )
                    .map_err(AppError::OperationError)?;

                project::generate_artifacts(project, &pcbs, &project_directory, phase_load_out_item_map)
                    .map_err(|cause| AppError::OperationError(cause.into()))?;
                Ok(render::render())
            }),
            Event::RecordPhaseOperation {
                phase: reference,
                operation,
                task,
                action,
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
                *modified |=
                    project::apply_phase_operation_task_action(project, directory, &reference, operation, task, action)
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
            // Gerber file management
            //
            Event::AddGerberFiles {
                pcb_file,
                design,
                files,
            } => Box::new(move |model: &mut Model| {
                let ModelPcb {
                    modified,
                    pcb,
                    ..
                } = model
                    .model_pcbs
                    .get_mut(&pcb_file)
                    .ok_or(AppError::PcbOperationError(PcbOperationError::PcbNotLoaded))?;

                debug!(
                    "Adding gerbers to pcb. pcb_file: {}, design: {} files: {:?}",
                    pcb_file, design, files
                );

                *modified |= pcb
                    .add_gerbers(design, files)
                    .map_err(|e| AppError::PcbOperationError(PcbOperationError::PcbError(e)))?;

                Ok(render::render())
            }),
            Event::RemoveGerberFiles {
                pcb_file,
                design,
                files,
            } => Box::new(move |model: &mut Model| {
                let ModelPcb {
                    modified,
                    pcb,
                    ..
                } = model
                    .model_pcbs
                    .get_mut(&pcb_file)
                    .ok_or(AppError::PcbOperationError(PcbOperationError::PcbNotLoaded))?;

                debug!(
                    "Removing gerbers from pcb. pcb_file: {}, design: {} files: {:?}",
                    pcb_file, design, files
                );
                let (was_modified, unremoved_files) = pcb
                    .remove_gerbers(design, files)
                    .map_err(|e| AppError::PcbOperationError(PcbOperationError::PcbError(e)))?;

                if !unremoved_files.is_empty() {
                    warn!("Unable to remove the following gerbers. files: {:?}", unremoved_files);
                }
                *modified |= was_modified;

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
                    processes: project
                        .processes
                        .iter()
                        .map(|process| process.reference.clone())
                        .collect(),
                };
                Ok(project_view_renderer::view(ProjectView::Overview(overview)))
            }),
            Event::RequestProjectPcbOverviewView {
                pcb: pcb_index,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                // we need to make sure the index is valid before attempting to get the corresponding PCB from the model.
                let project_pcb = project
                    .pcbs
                    .get(pcb_index as usize)
                    .ok_or(AppError::PcbOperationError(PcbOperationError::Unknown))?;

                let pcb_overview = ProjectPcbOverview {
                    index: pcb_index,
                    pcb_file: project_pcb.pcb_file.clone(),
                };

                Ok(project_view_renderer::view(ProjectView::PcbOverview(pcb_overview)))
            }),
            Event::RequestPcbOverviewView {
                pcb_file,
            } => Box::new(move |model: &mut Model| {
                let ModelPcb {
                    pcb,
                    path,
                    ..
                } = &model
                    .model_pcbs
                    .get(&pcb_file)
                    .ok_or(AppError::PcbOperationError(PcbOperationError::PcbNotLoaded))?;

                let designs = pcb
                    .unique_designs_iter()
                    .cloned()
                    .collect::<Vec<_>>();

                let gerbers = designs
                    .iter()
                    .enumerate()
                    .map(|(index, _design_name)| {
                        let design_index = DesignIndex::from(index);

                        let design_gerbers = pcb
                            .design_gerbers
                            .get(&design_index)
                            .map_or(Vec::new(), |v| {
                                v.iter()
                                    .map(|gerber_file| {
                                        // convert from project type to view type
                                        PcbGerberItem {
                                            path: gerber_file.file.clone(),
                                            pcb_side: gerber_file.pcb_side.clone(),
                                            purpose: gerber_file.purpose,
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            });

                        design_gerbers
                    })
                    .collect::<Vec<_>>();

                let unit_map = pcb
                    .unit_map
                    .iter()
                    .map(|(a, b)| (*a, *b))
                    .collect::<HashMap<PcbUnitIndex, DesignIndex>>();

                let pcb_overview = PcbOverview {
                    path: path.clone(),
                    pcb_file: pcb_file.clone(),
                    name: pcb.name.clone(),
                    units: pcb.units,
                    designs,
                    unit_map,
                    gerbers,
                };

                Ok(pcb_view_renderer::view(PcbView::PcbOverview(pcb_overview)))
            }),
            Event::RequestPcbUnitAssignmentsView {
                pcb: pcb_index,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let project_pcb = project
                    .pcbs
                    .get(pcb_index as usize)
                    .ok_or(AppError::PcbOperationError(PcbOperationError::Unknown))?;

                let unit_assignments = project_pcb
                    .unit_assignments
                    .iter()
                    .map(|(pcb_unit_index, (_design_index, variant_name))| (*pcb_unit_index, variant_name.clone()))
                    .collect::<HashMap<_, _>>();

                let pcb_unit_assignments = PcbUnitAssignments {
                    index: pcb_index,
                    unit_assignments,
                };
                Ok(project_view_renderer::view(ProjectView::PcbUnitAssignments(
                    pcb_unit_assignments,
                )))
            }),
            Event::RequestPlacementsView {} => Box::new(|model: &mut Model| {
                let ModelProject {
                    project, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let placements = project
                    .placements
                    .iter()
                    .enumerate()
                    .map(|(ordering, (path, state))| PlacementsItem {
                        path: path.clone(),
                        state: state.clone(),
                        ordering,
                    })
                    .collect();

                let placements = PlacementsList {
                    placements,
                };

                Ok(project_view_renderer::view(ProjectView::Placements(placements)))
            }),
            Event::RequestProjectTreeView {} => Box::new(|model: &mut Model| {
                let (
                    ModelProject {
                        project, ..
                    },
                    pcbs,
                    ..,
                ) = { Self::model_project_and_pcbs(model) }?;

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

                for (pcb_index, (project_pcb, pcb)) in project
                    .pcbs
                    .iter()
                    .zip(pcbs)
                    .enumerate()
                {
                    let pcb_node = project_tree
                        .tree
                        .add_node(ProjectTreeItem {
                            key: "pcb".to_string(),
                            args: HashMap::from([("name".to_string(), Arg::String(pcb.name.clone()))]),
                            path: format!("/pcbs/{}", pcb_index).to_string(),
                        });
                    project_tree
                        .tree
                        .add_edge(pcbs_node, pcb_node, ());

                    let unit_assignments_node = project_tree
                        .tree
                        .add_node(ProjectTreeItem {
                            key: "unit-assignments".to_string(),
                            path: format!("/pcbs/{}/units", pcb_index).to_string(),
                            ..ProjectTreeItem::default()
                        });
                    project_tree
                        .tree
                        .add_edge(pcb_node, unit_assignments_node, ());

                    for (pcb_unit_index, design_index) in pcb.unit_map.iter() {
                        let mut object_path = ObjectPath::default();
                        object_path.set_pcb_instance((pcb_index + 1) as u16);
                        object_path.set_pcb_unit(pcb_unit_index + 1);

                        let mut args = HashMap::from([("name".to_string(), Arg::String(object_path.to_string()))]);

                        let design_name = pcb
                            .design_names
                            .iter()
                            .nth(*design_index as usize)
                            .unwrap();
                        args.insert("design_name".to_string(), Arg::String(design_name.to_string()));

                        if let Some((_design_index, variant_name)) = project_pcb
                            .unit_assignments
                            .get(pcb_unit_index)
                        {
                            // it's invalid for these to be mismatched
                            debug_assert!(_design_index == design_index);
                            args.insert("variant_name".to_string(), Arg::String(variant_name.to_string()));
                        }

                        let unit_assignment_node = project_tree
                            .tree
                            .add_node(ProjectTreeItem {
                                key: "unit-assignment".to_string(),
                                args,
                                path: format!("/pcbs/{}/units/{}", pcb_index, pcb_unit_index).to_string(),
                            });

                        project_tree
                            .tree
                            .add_edge(unit_assignments_node, unit_assignment_node, ());
                    }
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
                            args: HashMap::from([("name".to_string(), Arg::String(process.reference.to_string()))]),
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

                for (reference, phase) in &project.phases {
                    //
                    // add phase node
                    //
                    let phase_path = format!("/phases/{}", reference).to_string();
                    let phase_node = project_tree
                        .tree
                        .add_node(ProjectTreeItem {
                            key: "phase".to_string(),
                            args: HashMap::from([
                                ("reference".to_string(), Arg::String(reference.to_string())),
                                ("process".to_string(), Arg::String(phase.process.to_string())),
                            ]),
                            path: phase_path.clone(),
                        });
                    project_tree
                        .tree
                        .add_edge(phases_node, phase_node, ());

                    //
                    // add loadout node to the phase
                    //
                    let loadout_node = project_tree
                        .tree
                        .add_node(ProjectTreeItem {
                            key: "phase-loadout".to_string(),
                            args: HashMap::from([(
                                "source".to_string(),
                                Arg::String(phase.load_out_source.to_string()),
                            )]),
                            path: format!("{}/loadout", phase_path),
                        });
                    project_tree
                        .tree
                        .add_edge(phase_node, loadout_node, ());

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

                Ok(project_view_renderer::view(ProjectView::ProjectTree(project_tree)))
            }),
            Event::RequestPhasesView {} => Box::new(|model: &mut Model| {
                let ModelProject {
                    path,
                    project,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let phases = project
                    .phases
                    .iter()
                    .map(|(phase_reference, phase)| {
                        let phase_state = project
                            .phase_states
                            .get(phase_reference)
                            .unwrap();
                        // FUTURE try and avoid the [`unwrap`] here, ideally by ensuring load-out sources are always correct
                        //        for every situation instead of using [`try_build_phase_load_out_source`]
                        try_build_phase_overview(path, phase_reference.clone(), phase, phase_state).unwrap()
                    })
                    .collect::<Vec<PhaseOverview>>();

                let phases = Phases {
                    phases,
                };

                Ok(project_view_renderer::view(ProjectView::Phases(phases)))
            }),

            Event::RequestPhaseOverviewView {
                phase_reference,
            } => Box::new(|model: &mut Model| {
                let ModelProject {
                    path,
                    project,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;
                let phase = project
                    .phases
                    .get(&phase_reference)
                    .ok_or(AppError::UnknownPhaseReference(phase_reference.clone()))?;
                let phase_state = project
                    .phase_states
                    .get(&phase_reference)
                    .unwrap();

                let phase_overview = try_build_phase_overview(path, phase_reference, phase, phase_state)
                    .map_err(AppError::LoadoutSourceError)?;

                Ok(project_view_renderer::view(ProjectView::PhaseOverview(phase_overview)))
            }),
            Event::RequestPhasePlacementsView {
                phase_reference,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    path,
                    project,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;
                let phase = project
                    .phases
                    .get(&phase_reference)
                    .ok_or(AppError::UnknownPhaseReference(phase_reference.clone()))?;

                let load_out_source =
                    try_build_phase_load_out_source(path, &phase).map_err(AppError::LoadoutSourceError)?;

                let loadout_items = stores::load_out::load_items(&load_out_source).map_err(AppError::OperationError)?;

                let mut placements: Vec<(&ObjectPath, &PlacementState)> = project
                    .placements
                    .iter()
                    .filter(|(_path, state)| match &state.phase {
                        Some(candidate_phase) if phase_reference == *candidate_phase => true,
                        _ => false,
                    })
                    .collect();

                project::sort_placements(&mut placements, &phase.placement_orderings, &loadout_items);

                let placements = placements
                    .into_iter()
                    .enumerate()
                    .map(|(ordering, (path, state))| PlacementsItem {
                        path: path.clone(),
                        state: state.clone(),
                        ordering,
                    })
                    .collect();

                let phase_placements = PhasePlacements {
                    phase_reference,
                    placements,
                };
                Ok(project_view_renderer::view(ProjectView::PhasePlacements(
                    phase_placements,
                )))
            }),
            Event::RequestProcessView {
                process_reference,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let process_reference =
                    ProcessReference::try_from(process_reference).map_err(|err| AppError::ProcessError(err.into()))?;

                let process = project
                    .find_process(&process_reference)
                    .map_err(|err| AppError::ProcessError(err.into()))?;

                Ok(project_view_renderer::view(ProjectView::Process(process.clone())))
            }),
            Event::RequestPartStatesView {} => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let mut parts = project
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
                            ref_des_set: Default::default(),
                            quantity: 0,
                        }
                    })
                    .collect::<Vec<_>>();

                //
                // add the set of ref_des and count the quantity for each part.
                //
                for (_object_path, placement_state) in project.placements.iter_mut() {
                    if let Some(part) = parts
                        .iter_mut()
                        .find(|part_with_state| {
                            part_with_state
                                .part
                                .eq(&placement_state.placement.part)
                        })
                    {
                        part.quantity += 1;
                        let _inserted = part.ref_des_set.insert(
                            placement_state
                                .placement
                                .ref_des
                                .clone(),
                        );
                    }
                }

                let part_states_view = PartStates {
                    parts,
                };

                Ok(project_view_renderer::view(ProjectView::Parts(part_states_view)))
            }),
            Event::RequestPhaseLoadOutView {
                phase_reference,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    path,
                    project,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;
                let phase = project
                    .phases
                    .get(&phase_reference)
                    .ok_or(AppError::UnknownPhaseReference(phase_reference.clone()))?;

                let load_out_source =
                    try_build_phase_load_out_source(path, &phase).map_err(AppError::LoadoutSourceError)?;

                let items = stores::load_out::load_items(&load_out_source).map_err(AppError::OperationError)?;

                let load_out_view = LoadOut {
                    phase_reference,
                    source: load_out_source,
                    items,
                };

                Ok(project_view_renderer::view(ProjectView::PhaseLoadOut(load_out_view)))
            }),
        }
    }

    fn model_project_and_pcbs(model: &mut Model) -> Result<(&mut ModelProject, Vec<&Pcb>, PathBuf), AppError> {
        let Some(model_project) = model.model_project.as_mut() else {
            return Err(AppError::OperationRequiresProject);
        };

        let project_directory = model_project
            .path
            .parent()
            .unwrap()
            .to_path_buf();
        let model_pcbs = &mut model.model_pcbs;
        Model::load_project_pcbs_inner(&mut model_project.project, model_pcbs, &project_directory)?;

        let iter = model_project.pcbs(&model.model_pcbs);
        let pcbs = Model::project_pcbs_inner(iter);

        Ok((model_project, pcbs, project_directory))
    }
}

impl App for Planner {
    type Event = Event;
    type Model = Model;
    type ViewModel = PlannerOperationViewModel;
    type Capabilities = ();
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
                model
                    .error
                    .replace((chrono::DateTime::from(SystemTime::now()), format!("{:?}", e)));
                render::render()
            }
            Ok(command) => {
                model.error.take();
                command
            }
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        let project_modified = model
            .model_project
            .as_ref()
            .map_or(false, |project| project.modified);

        let pcbs_modified = model
            .model_pcbs
            .iter()
            .any(|(_file_reference, pcb)| pcb.modified);

        let view_model = PlannerOperationViewModel {
            project_modified,
            pcbs_modified,
            error: model.error.clone(),
        };

        trace!("view model: {:?}", view_model);

        view_model
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
    #[error("Loadout source error. cause: {0}")]
    LoadoutSourceError(LoadOutSourceError),
    #[error("Loadout error. cause: {0}")]
    LoadoutError(LoadOutOperationError),
    #[error("PCB error. cause: {0}")]
    PcbOperationError(PcbOperationError),
    #[error("IO error. cause: {0}")]
    IoError(std::io::Error),

    #[error("Unknown phase reference. reference: {0}")]
    UnknownPhaseReference(Reference),

    #[error("File reference error")]
    FileReferenceError(FileReferenceError),
}

impl Planner {
    fn refresh_project(project: &mut Project, pcbs: &[&Pcb], path: &PathBuf) -> anyhow::Result<ProjectRefreshResult> {
        let directory = path.parent().unwrap();

        let unique_design_variants = project.unique_design_variants(pcbs);
        let design_variant_placement_map = stores::placements::load_all_placements(unique_design_variants, directory)?;
        let refresh_result = project::refresh_from_design_variants(project, pcbs, design_variant_placement_map);

        trace!("Refreshed from design variants. modified: {}", refresh_result.modified);

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
        let expected_view = PlannerOperationViewModel::default();
        assert_eq!(actual_view, &expected_view);
    }
}

/// Build a load-out source, where the load-out source *may* be a relative or absolute path.
///
/// 'project_path' is the project FILE (not directory).
fn try_build_phase_load_out_source(project_path: &PathBuf, phase: &Phase) -> Result<LoadOutSource, LoadOutSourceError> {
    assert!(project_path.is_file());

    let directory = project_path
        .parent()
        .unwrap()
        .to_path_buf();

    LoadOutSource::try_from_path(&directory, PathBuf::from(&phase.load_out_source))
}

fn try_build_phase_overview(
    project_path: &PathBuf,
    phase_reference: PhaseReference,
    phase: &Phase,
    state: &PhaseState,
) -> Result<PhaseOverview, LoadOutSourceError> {
    let load_out_source = try_build_phase_load_out_source(project_path, phase)?;

    Ok(PhaseOverview {
        phase_reference,
        process: phase.process.clone(),
        load_out_source,
        pcb_side: phase.pcb_side.clone(),
        phase_placement_orderings: phase.placement_orderings.clone(),
        state: state.clone(),
    })
}
