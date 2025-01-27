use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::path::PathBuf;
use cushy::channel::Sender;
use cushy::dialog::ShouldClose;
use cushy::localization::Localize;
use cushy::localize;
use cushy::value::{Destination, Dynamic, Source};
use cushy::widget::{MakeWidget, WidgetInstance, WidgetList};
use cushy::widgets::label::Displayable;
use cushy::widgets::list::ListStyle;
use cushy::widgets::pile::{Pile, PiledWidget};
use cushy::widgets::Space;
use cushy::widgets::layers::{Modal, ModalHandle};
use slotmap::new_key_type;
use tracing::{debug, error, info, trace};
use planner_app::{Arg, Event, PcbSide, PhaseOverview, PhasePlacements, ProjectTreeView, ProjectView, Reference};
use planner_gui::action::Action;
use crate::app_core::CoreService;
use planner_gui::task::Task;
use cushy::widgets::tree::{Tree, TreeNodeKey};
use fluent_bundle::FluentValue;
use fluent_bundle::types::FluentNumber;
use petgraph::visit::{depth_first_search, Control, DfsEvent};
use regex::Regex;
use planner_gui::widgets::properties::{Properties, PropertiesItem};
use crate::app_tabs::project::ProjectTabMessage;
use crate::project::dialogs::add_pcb::AddPcbForm;
use crate::project::toolbar::{make_toolbar, ToolbarMessage};

mod toolbar;
mod dialogs;

new_key_type! {
    /// A key for a project
    pub struct ProjectKey;
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectPath(String);

impl ProjectPath {
    pub fn new(path: String) -> Self {
        Self(path)
    }
}

impl Deref for ProjectPath {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for ProjectPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone)]
pub struct AddPcbArgs {
    pub name: String,
    pub kind: planner_app::PcbKind
}

#[derive(Debug, Clone)]
pub enum ProjectMessage {
    None,
    
    //
    // User interactions
    //
    
    Load,
    Navigate(ProjectPath),
    
    AddPcb(AddPcbArgs),
    
    //
    // Internal messages
    //
    Error(String),
    UpdateView(ProjectView),
    Loaded,
    Create,
    Created,
    RequestView(ProjectViewRequest),
    Save,
    Saved,
    ToolbarMessage(ToolbarMessage),
}

#[derive(Debug, Clone)]
pub enum ProjectViewRequest {
    Overview,
    ProjectTree,
    PhaseOverview { phase: String },
    PhasePlacements { phase: String },
}


#[derive(Default)]
pub enum ProjectAction {
    #[default]
    None,
    Task(Task<ProjectMessage>),
    ShowError(String),
    NameChanged(String),
}

struct PhaseWidgetState {
    overview: Option<Dynamic<PhaseOverview>>,
    placements: Option<Dynamic<PhasePlacements>>,
    handle: PiledWidget,
}

pub struct Project {
    pub(crate) name: Dynamic<Option<String>>,
    pub(crate) path: PathBuf,
    core_service: CoreService,
    project_tree: Dynamic<Tree>,
    project_tree_path: Dynamic<ProjectPath>,
    
    sender: Sender<ProjectMessage>,
    
    phase_widgets: HashMap<Reference, PhaseWidgetState>,
    
    default_content_handle: Option<PiledWidget>,
    pile: Pile,

    add_pcb_modal: Option<ModalHandle>,
}

impl Project {
    pub fn new(name: String, path: PathBuf, project_message_sender: Sender<ProjectMessage>) -> (Self, ProjectMessage) {
        let project_tree = Dynamic::new(Tree::default());
        
        let core_service = CoreService::new();
        let instance = Self {
            name: Dynamic::new(Some(name)),
            path,
            core_service,
            project_tree,
            project_tree_path: Dynamic::new(ProjectPath("/".to_string())),
            sender: project_message_sender,
            pile: Pile::default(),
            phase_widgets: HashMap::new(),
            default_content_handle: None,
            add_pcb_modal: None,
        };

        (instance, ProjectMessage::Create)
    }

    pub fn from_path(path: PathBuf, project_message_sender: Sender<ProjectMessage>) -> (Self, ProjectMessage) {
        let project_tree = Dynamic::new(Tree::default());
        let core_service = CoreService::new();
        let instance = Self {
            name: Dynamic::default(),
            path,
            core_service,
            project_tree,
            project_tree_path: Dynamic::new(ProjectPath("/".to_string())),
            sender: project_message_sender,
            pile: Pile::default(),
            phase_widgets: HashMap::new(),
            default_content_handle: None,
            add_pcb_modal: None,
        };

        (instance, ProjectMessage::Load)
    }

    pub fn make_widget(&mut self) -> WidgetInstance {

        let modal = Modal::new();
        self.add_pcb_modal.replace(modal.new_handle());

        let project_tree_widget = self.project_tree.lock().make_widget();
        let project_explorer = "Project Explorer".contain()
            .and(project_tree_widget.contain())
            .into_rows()
            .contain()
            .make_widget();
        
        let default_content = "content-pane"
            .to_label()
            .centered()
            .and(self.project_tree_path.to_label().centered())
            .into_rows();
        
        let default_content_handle = self.pile
            .push(default_content);
        
        // TODO improve this workaround for https://github.com/khonsulabs/cushy/issues/231
        //      we have to store the default_content_handle otherwise the widget
        //      will be dropped at the end of this method, and when it does the widget will be removed from the pile
        //      so we have to hold on to the handle, but doing so required changing this method to accept `&mut self` instead of `&self`
        self.default_content_handle.replace(default_content_handle);
        
        let (toolbar_message_sender, toolbar_message_receiver) = cushy::channel::build().finish();
        
        toolbar_message_receiver.on_receive({
            let project_message_sender = self.sender.clone();
            move |toolbar_message| {
                debug!("project_toolbar_message: {:?}", toolbar_message);
                project_message_sender.send(ProjectMessage::ToolbarMessage(toolbar_message))
                    .map_err(|message| error!("unable to forward toolbar message. message: {:?}", message))
                    .ok();
            }
        });
        
        let toolbar = make_toolbar(toolbar_message_sender);
        
        let content_pane = toolbar
            .and(self.pile.clone())
            .into_rows()
            .expand()
            .contain();
        
        let project_ui = project_explorer
            .and(content_pane)
            .into_columns()
            .expand_horizontally();
        
        project_ui
            .and(modal)
            .into_layers()
            .make_widget()
    }

    pub fn update(&mut self, message: ProjectMessage) -> Action<ProjectAction> {
        let action = match message {
            ProjectMessage::None => {
                ProjectAction::None
            }
            ProjectMessage::Load => {
                let task = self.core_service
                    .update(Event::Load { path: self.path.clone() })
                    .chain(Task::done(ProjectMessage::Loaded));
                ProjectAction::Task(task)
            },
            ProjectMessage::Loaded => {
                let task = self.core_service
                    .update(Event::RequestOverviewView {})
                    .chain(Task::done(ProjectMessage::RequestView(ProjectViewRequest::ProjectTree)));
                ProjectAction::Task(task)
            }
            ProjectMessage::Create => {
                let task = self.core_service
                    .update(Event::CreateProject { name: self.name.get().unwrap(), path: self.path.clone() })
                    .chain(Task::done(ProjectMessage::Created));
                ProjectAction::Task(task)
            },
            ProjectMessage::Created => {
                let task = self.core_service
                    .update(Event::RequestOverviewView {})
                    .chain(Task::done(ProjectMessage::RequestView(ProjectViewRequest::ProjectTree)));
                ProjectAction::Task(task)
            },
            ProjectMessage::Save => {
                let task = self.core_service
                    .update(Event::Save { })
                    .chain(Task::done(ProjectMessage::Saved));
                ProjectAction::Task(task)
            },
            ProjectMessage::Saved => {
                info!("Saved project. path: {:?}", self.path);
                ProjectAction::None
            }
            ProjectMessage::RequestView(view) => {
                let event = match view {
                    ProjectViewRequest::Overview => Event::RequestOverviewView {},
                    ProjectViewRequest::ProjectTree => Event::RequestProjectTreeView {},
                    ProjectViewRequest::PhaseOverview { phase } => Event::RequestPhaseOverviewView { phase_reference: Reference(phase) },
                    ProjectViewRequest::PhasePlacements { phase } => Event::RequestPhasePlacementsView { phase_reference: Reference(phase) },
                };
                
                let task = self.core_service
                    .update( event);
                ProjectAction::Task(task)
            }
            ProjectMessage::Navigate(path) => {
                // if the path starts with `/project/` then show/hide UI elements based on the path, 
                // e.g. update a dynamic that controls a per-project-tab-bar dynamic selector
                info!("ProjectMessage::Navigate. path: {}", path);
                self.project_tree_path.set(path.clone());

                let phase_pattern = Regex::new(r"/project/phases/(?<phase>.*){1}").unwrap();
                if let Some(captures) = phase_pattern.captures(&path) {
                    let phase_reference: String = captures.name("phase").unwrap().as_str().to_string();
                    debug!("phase_reference: {}", phase_reference);

                    let tasks: Vec<_> = vec![
                        Task::done(ProjectMessage::RequestView(ProjectViewRequest::PhaseOverview { phase:
                            phase_reference.clone()
                        })),
                        Task::done(ProjectMessage::RequestView(ProjectViewRequest::PhasePlacements { phase:
                            phase_reference.clone()
                        })),
                    ];
                    
                    ProjectAction::Task(Task::batch(tasks))
                } else {
                    ProjectAction::None
                }
            }
            ProjectMessage::Error(error) => {
                ProjectAction::ShowError(error)
            }
            ProjectMessage::UpdateView(view) => {
                // update the GUI using the view
                match view {
                    ProjectView::Overview(project_overview) => {
                        debug!("project overview: {:?}", project_overview);
                        self.name.set(Some(project_overview.name.clone()));
                        
                        ProjectAction::NameChanged(project_overview.name)
                    }
                    ProjectView::ProjectTree(project_tree) => {
                        debug!("project tree: {:?}", project_tree);

                        self.update_tree(project_tree);
                        
                        ProjectAction::None
                    }
                    ProjectView::Placements(placements) => {
                        ProjectAction::None
                    }
                    ProjectView::PhaseOverview(phase_overview) => {
                        debug!("phase overview: {:?}", phase_overview);
                        let phase = phase_overview.phase_reference.clone();
                        
                        self.update_phase_state(phase, Some(phase_overview), None);
                        
                        ProjectAction::None
                    }
                    ProjectView::PhasePlacements(phase_placements) => {
                        debug!("phase placements: {:?}", phase_placements);
                        let phase = phase_placements.phase_reference.clone();

                        self.update_phase_state(phase, None, Some(phase_placements));

                        ProjectAction::None
                    }
                    ProjectView::PhasePlacementOrderings(phase_placement_orderings) => {
                        ProjectAction::None
                    }
                }
            }
            ProjectMessage::ToolbarMessage(message) => {
                debug!("toolbar message: {:?}", message);
                
                self.on_toolbar_message(message)
            }
            ProjectMessage::AddPcb(args) => {
                debug!("Add Pcb. args: {:?}", args);

                let task = self.core_service
                    .update(Event::AddPcb { kind: args.kind, name: args.name })
                    .chain(Task::done(ProjectMessage::RequestView(ProjectViewRequest::ProjectTree)));
                ProjectAction::Task(task)
            }
        };

        Action::new(action)
    }

    fn update_phase_state(&mut self, phase_reference: Reference, mut phase_overview: Option<PhaseOverview>, mut phase_placements: Option<PhasePlacements>) {

        let maybe_state = self.phase_widgets.get_mut(&phase_reference);
        let handle = match maybe_state {
            None => {
                let dyn_overview = phase_overview.map(Dynamic::new);
                let dyn_placements = phase_placements.map(Dynamic::new);

                let widget = update_stuff(&dyn_overview, &dyn_placements);
                let handle = self.pile.push(widget);

                let state = PhaseWidgetState {
                    overview: dyn_overview,
                    placements: dyn_placements,
                    handle: handle.clone(),
                };

                let _ = self.phase_widgets.insert(phase_reference, state);

                handle
            }
            Some(state) => {
                match &state.overview {
                    None => {
                        state.overview = phase_overview.map(Dynamic::new);
                    }
                    Some(overview) => {
                        if phase_overview.is_some() {
                            overview.replace(phase_overview.take().unwrap());
                        }
                    }
                }
                match &state.placements {
                    None => {
                        state.placements = phase_placements.map(Dynamic::new);
                    }
                    Some(placements) => {
                        if phase_placements.is_some() {
                            placements.replace(phase_placements.take().unwrap());
                        }
                    }
                }

                let widget = update_stuff(&state.overview, &state.placements);
                let handle = self.pile.push(widget);
                
                // replacing the handle causes the old pile instance to be removed from the pile
                state.handle = handle.clone();

                state.handle.clone()
            },
        };
        handle.show();
    }
    
    fn update_tree(&mut self, project_tree_view: ProjectTreeView) {

        // TODO maybe synchronize instead of rebuild, when we need to show a selected tree item this will be a problem
        //      as the selection will be lost and need to be re-determined.
        //      instead of syncronization, maybe just remember the 'path' and re-select a tree item that has the same path  
        let mut project_tree = self.project_tree.lock();
        project_tree.clear();

        //
        // Create the tree widget nodes from the project tree view
        //
        // Assumes the only relationships in the tree are parent->child, i.e. parent->grandchild is catered handled.

        use petgraph::graph::node_index as n;

        let start = n(0);
        
        let mut stack: Vec<(Option<TreeNodeKey>, Option<TreeNodeKey>)> = vec![];

        let mut current_parent_key: Option<TreeNodeKey> = None;
        let mut current_node_key: Option<TreeNodeKey> = None;
        
        // FIXME depth_first_search doesn't emit (Discover) nodes in the same order they were added to the tree.
        //       the order *is* important here, so we need to replace this with a different tree visitor solution.
        
        depth_first_search(&project_tree_view.tree, Some(start),{

            |event| {

                trace!("dfs. event: {:?}", event);
                match event {
                    DfsEvent::Discover(node, _) => {
                        let item = &project_tree_view.tree[node];
                        
                        let path = ProjectPath(format!("/project{}", item.path).to_string());
                        
                        
                        let key = format!("project-explorer-node-{}", item.key);

                        let mut title = Localize::new(key);
                        
                        for (key, value) in item.args.iter() {
                            match value {
                                Arg::String(value) => {
                                    title = title.arg(key, value.clone());
                                }
                            }
                        }

                        let sender = self.sender.clone();
                        let node_widget = title
                            .to_button()
                            .on_click(move |_event|{
                                sender.send(ProjectMessage::Navigate(path.clone())).expect("sent");
                            })
                            .make_widget();

                        let child_key = project_tree.insert_child(node_widget, current_parent_key.as_ref()).unwrap();

                        current_node_key.replace(child_key);
                    }
                    DfsEvent::TreeEdge(_from, _to) => {
                        stack.push((current_node_key.clone(), current_parent_key.clone()));
                        current_parent_key.replace(current_node_key.take().unwrap());
                        current_node_key.take();
                    }
                    DfsEvent::Finish(_from, _time) => {
                        if let Some((node_key, parent_key)) = stack.pop() {
                            current_node_key.replace(node_key.unwrap_or_default());
                            current_parent_key.replace(parent_key.unwrap_or_default());
                        }
                    }
                    _ => {
                    }
                }
                Control::<()>::Continue
            }
        });
    }

    fn on_toolbar_message(&self, message: ToolbarMessage) -> ProjectAction {
        
        match message {
            ToolbarMessage::AddPcb => {
                self.display_add_pcb_modal();
            }
        }
        
        ProjectAction::None
    }

    fn display_add_pcb_modal(&self) {
        let handle = self.add_pcb_modal.as_ref().unwrap();
        let form = AddPcbForm::default();
        handle
            .build_dialog(&form)
            .with_default_button(localize!("form-button-ok"), {
                let sender = self.sender.clone();
                move || {
                    info!("Add pcb. Ok clicked");
                    let validations = form.validations();
                    match validations.is_valid() {
                        false => ShouldClose::Remain,
                        true => {
                            
                            let args = form.result().unwrap();
                            info!("Add pcb. name: {}, kind: {}", args.name, args.kind);
                            
                            sender.send(ProjectMessage::AddPcb(args)).expect("sent");
                            
                            ShouldClose::Close
                        },
                    }
                }
            })
            .with_cancel_button(localize!("form-button-cancel"), {
                || {
                    info!("Add pcb. Cancel clicked");
                    ShouldClose::Close
                }
            })

            .show();
    }
}

fn make_phase_overview_widget(dyn_overview: &Dynamic<PhaseOverview>) -> impl MakeWidget + Sized {
    
    let mut items: Vec<PropertiesItem> = vec![];

    let reference_item = PropertiesItem::from_optional_value(
        Localize::new("phase-reference"),
        dyn_overview.map_each(|phase_overview|Some(phase_overview.phase_reference.to_string()))
    );
    items.push(reference_item);

    let load_out_source_item = PropertiesItem::from_optional_value(
        Localize::new("phase-load-out-source"),
        dyn_overview.map_each(|phase_overview|Some(phase_overview.load_out_source.clone()))
    );
    items.push(load_out_source_item);

    let pcb_side_item = PropertiesItem::from_field(
        Localize::new("phase-pcb-side"),
        dyn_overview.map_each(|phase_overview|{
            let pcb_side = match phase_overview.pcb_side {
                PcbSide::Top => Localize::new("pcb-side-top"),
                PcbSide::Bottom => Localize::new("pcb-side-bottom"),
            };
            pcb_side.make_widget()
        })
    );
    items.push(pcb_side_item);

    let process_item = PropertiesItem::from_optional_value(
        Localize::new("phase-process"),
        dyn_overview.map_each(|phase_overview|Some(phase_overview.process.to_string()))
    );
    items.push(process_item);

    let properties = Properties::default()
        .with_header_label(Localize::new("phase-properties-header"))
        .with_footer_label(
            Localize::new("phase-properties-footer")
                .arg("count", FluentValue::Number(FluentNumber::from(items.len()))
                )
        )
        .with_items(items);

    properties
        .make_widget()
        .expand_horizontally()
}

fn make_phase_placements_widget(dyn_placements: &Dynamic<PhasePlacements>) -> impl MakeWidget + Sized {
    dyn_placements.map_each(|phase_placements| {
        phase_placements.placements.iter().map(|state|
            format!("{:?}", state.placement)
        ).collect::<WidgetList>()
    })
        .into_list()
        .style(ListStyle::Decimal)
        .vertical_scroll()
        .expand()
        .contain()
}

fn update_stuff(dyn_overview: &Option<Dynamic<PhaseOverview>>, dyn_placements: &Option<Dynamic<PhasePlacements>>) -> WidgetInstance {

    let overview_widget = match dyn_overview {
        Some(overview) => make_phase_overview_widget(overview).make_widget(),
        None => Space::default().make_widget(),
    };

    let placements_widget = match dyn_placements {
        Some(placements) => make_phase_placements_widget(placements).make_widget(),
        None => Space::default().make_widget(),
    };

    let widgets = overview_widget
        .and(placements_widget)
        .into_rows();

    widgets
        .make_widget()
}
