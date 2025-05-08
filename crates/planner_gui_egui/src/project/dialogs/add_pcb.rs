use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::path::PathBuf;
use std::str::FromStr;

use egui::{Modal, TextEdit, Ui};
use egui_i18n::tr;
use egui_mobius::types::Value;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignContent, AlignItems, Display, FlexDirection, Style};
use egui_taffy::{Tui, TuiBuilderLogic, taffy, tui};
use planner_app::{DesignName, PcbUnitNumber};
use taffy::Size;
use validator::{Validate, ValidationError};

use crate::forms::Form;
use crate::ui_component::{ComponentState, UiComponent};
use crate::widgets::list_box::list_box_with_id_multi_tui;

#[derive(Debug)]
pub struct AddPcbModal {
    fields: Value<AddPcbFields>,

    path: PathBuf,

    pub component: ComponentState<AddPcbModalUiCommand>,
}

impl AddPcbModal {
    pub fn new(path: PathBuf) -> Self {
        Self {
            fields: Default::default(),
            path,
            component: Default::default(),
        }
    }

    fn show_form(&self, ui: &mut Ui, form: &Form<AddPcbFields, AddPcbModalUiCommand>) {
        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        tui(ui, ui.id().with("add_pcb_form"))
            .reserve_available_width()
            .style(Style {
                align_items: Some(AlignItems::Center),
                flex_direction: FlexDirection::Column,
                size: Size {
                    width: percent(1.),
                    height: auto(),
                },
                padding: length(8.),
                gap: length(8.),
                ..default_style()
            })
            .show(|tui| {
                form.show_fields_vertical(tui, |form, tui| {
                    form.add_field_ui("name", tr!("form-add-pcb-input-name"), tui, {
                        // NOTE text input does not resize with grid cell when using `.ui_add`, known issue - https://discord.com/channels/900275882684477440/904461220592119849/1338883750922293319
                        //      as a workaround we use `ui_add_manual` for now, with `no_transform`.
                        move |ui: &mut Ui, fields, sender| {
                            let mut name_clone = fields.name.clone();
                            let output = TextEdit::singleline(&mut name_clone)
                                .desired_width(ui.available_width())
                                .hint_text(tr!("form-add-pcb-input-name-placeholder"))
                                .show(ui);

                            if !fields.name.eq(&name_clone) {
                                sender
                                    .send(AddPcbModalUiCommand::NameChanged(name_clone))
                                    .expect("sent")
                            }

                            output.response
                        }
                    });

                    form.add_field_ui("units", tr!("form-add-pcb-input-units"), tui, {
                        move |ui: &mut Ui, fields, sender| {
                            let mut units = fields.units;
                            ui.add(egui::DragValue::new(&mut units).range(1..=u16::MAX));

                            if units != fields.units {
                                sender
                                    .send(AddPcbModalUiCommand::UnitsChanged(units))
                                    .expect("sent");
                            }

                            ui.response()
                        }
                    });

                    form.add_field_ui("design_name", tr!("form-add-pcb-input-design-name"), tui, {
                        // NOTE text input does not resize with grid cell when using `.ui_add`, known issue - https://discord.com/channels/900275882684477440/904461220592119849/1338883750922293319
                        //      as a workaround we use `ui_add_manual` for now, with `no_transform`.
                        move |ui: &mut Ui, fields, sender| {
                            let mut design_name_clone = fields.design_name.clone();
                            let output = TextEdit::singleline(&mut design_name_clone)
                                .desired_width(ui.available_width())
                                .hint_text(tr!("form-add-pcb-input-design-name-placeholder"))
                                .show(ui);

                            if !fields
                                .design_name
                                .eq(&design_name_clone)
                            {
                                sender
                                    .send(AddPcbModalUiCommand::DesignNameChanged(design_name_clone))
                                    .expect("sent")
                            }

                            output.response
                        }
                    });

                    form.add_field_tui("unit_map", tr!("form-add-pcb-unit-map"), tui, {
                        move |tui: &mut Tui, fields, sender| {
                            let id = tui.current_id();
                            let unit_map_id = id.with("unit_map");

                            let units = fields.units as usize;

                            let unit_map = fields
                                .unit_map
                                .iter()
                                .take(units)
                                .enumerate()
                                .map(|(index, design)| {
                                    format!(
                                        "{}: {}",
                                        index + 1,
                                        design
                                            .as_ref()
                                            .unwrap_or(&tr!("assignment-unassigned").to_string())
                                    )
                                })
                                .collect::<Vec<_>>();

                            tui.style(Style {
                                flex_grow: 1.0,
                                flex_direction: FlexDirection::Column,
                                min_size: Size {
                                    width: percent(0.2),
                                    height: length(200.0),
                                },
                                max_size: Size {
                                    width: auto(),
                                    height: length(200.0),
                                },
                                ..Style::default()
                            })
                            .add_with_border(|tui: &mut Tui| {
                                let (_changed, selection) = tui.ui_scroll_area_ext(None, |ui| {
                                    egui_taffy::tui(ui, ui.id().with("list_box_scroll_area"))
                                        .reserve_available_width()
                                        .style(Style {
                                            align_items: Some(AlignItems::Stretch),
                                            flex_direction: FlexDirection::Column,
                                            size: Size {
                                                width: percent(1.),
                                                height: length(200.0),
                                            },
                                            padding: length(8.),
                                            gap: length(8.),
                                            ..default_style()
                                        })
                                        .show(|tui| list_box_with_id_multi_tui(tui, unit_map_id, &unit_map))
                                });

                                // if the unit_map was grown, then the last item selected, then the unit count was
                                // decreased, the map will still be the size after it was grown and the selection
                                // will still contain an entry that is not visble or usable for selection operations
                                // thus we need to filter the selection
                                let selection = selection
                                    .into_iter()
                                    .filter(|&index| index < units)
                                    .collect::<Vec<_>>();

                                let is_design_name_ok = !fields.design_name.trim().is_empty();
                                let is_selection_ok = !selection.is_empty();
                                let is_unit_map_empty = fields.is_unit_map_empty();

                                //
                                // assign selection/assign all/unassign button row (three columns)
                                //
                                tui.style(Style {
                                    display: Display::Flex,
                                    align_content: Some(AlignContent::Stretch),
                                    flex_direction: FlexDirection::Row,
                                    flex_grow: 1.0,
                                    ..default_style()
                                })
                                .add(|tui| {
                                    if tui
                                        .style(Style {
                                            flex_grow: 0.25,
                                            ..default_style()
                                        })
                                        .enabled_ui(is_design_name_ok && is_selection_ok)
                                        .button(|tui| {
                                            tui.label(tr!("form-add-pcb-assign-selection"));
                                        })
                                        .clicked()
                                    {
                                        sender
                                            .send(AddPcbModalUiCommand::AssignDesignSelection(selection.clone()))
                                            .expect("sent");
                                    }
                                    if tui
                                        .style(Style {
                                            flex_grow: 0.25,
                                            ..default_style()
                                        })
                                        .enabled_ui(is_design_name_ok)
                                        .button(|tui| {
                                            tui.label(tr!("form-add-pcb-assign-all"));
                                        })
                                        .clicked()
                                    {
                                        sender
                                            .send(AddPcbModalUiCommand::AssignDesignToAllClicked)
                                            .expect("sent");
                                    }
                                    if tui
                                        .style(Style {
                                            flex_grow: 0.25,
                                            ..default_style()
                                        })
                                        .enabled_ui(is_selection_ok)
                                        .button(|tui| {
                                            tui.label(tr!("form-add-pcb-unassign-selection"));
                                        })
                                        .clicked()
                                    {
                                        sender
                                            .send(AddPcbModalUiCommand::UnassignDesignsSelection(selection))
                                            .expect("sent");
                                    }
                                    if tui
                                        .style(Style {
                                            flex_grow: 0.25,
                                            ..default_style()
                                        })
                                        .enabled_ui(!is_unit_map_empty)
                                        .button(|tui| {
                                            tui.label(tr!("form-add-pcb-unassign-all"));
                                        })
                                        .clicked()
                                    {
                                        sender
                                            .send(AddPcbModalUiCommand::UnassignAllDesignsClicked)
                                            .expect("sent");
                                    }
                                });
                            });
                        }
                    });
                });
            });
    }
}

#[derive(Clone, Debug, Validate, serde::Deserialize, serde::Serialize)]
#[validate(context = AddPcbValidationContext)]
pub struct AddPcbFields {
    #[validate(length(min = 1, code = "form-input-error-length"))]
    name: String,

    #[validate(range(min = 1, max = u16::MAX, code = "form-input-error-range"))]
    units: u16,

    /// index of the vec is the pcb unit index (0-based)
    #[validate(custom(function = "AddPcbFields::validate_unit_map", use_context))]
    unit_map: Vec<Option<String>>,

    /// allows the user to type in a design name, the current value is used when making assignments or
    /// sizing the unit_map
    #[validate(length(min = 1, code = "form-input-error-length"))]
    design_name: String,
}

impl Default for AddPcbFields {
    fn default() -> Self {
        const DEFAULT_UNITS: u16 = 6;

        Self {
            name: "".to_string(),
            units: DEFAULT_UNITS,

            // See [`Self::grow_unit_map`]
            unit_map: vec![None; DEFAULT_UNITS as usize],
            design_name: "".to_string(),
        }
    }
}

pub struct AddPcbValidationContext {
    units: u16,
}

impl AddPcbFields {
    fn validate_unit_map(
        unit_map: &Vec<Option<String>>,
        context: &AddPcbValidationContext,
    ) -> Result<(), ValidationError> {
        // code elsewhere should grow the vector to the amount of units, but never shorten it
        // we take the elements we need when it is submitted
        let len = unit_map.len();
        if len < context.units as usize {
            let mut error = ValidationError::new("form-input-error-map-incorrect-entry-count");
            error.add_param(Cow::from("required"), &context.units);
            error.add_param(Cow::from("actual"), &len);

            return Err(error);
        }

        if Self::is_unit_map_fully_populated_inner(unit_map, context.units as usize) {
            return Err(ValidationError::new("form-input-error-map-unassigned-entries"));
        }

        Ok(())
    }

    pub fn is_unit_map_fully_populated_inner(unit_map: &Vec<Option<String>>, units: usize) -> bool {
        unit_map
            .iter()
            .take(units)
            .any(|design| design.is_none())
    }

    pub fn is_unit_map_fully_populated(&self) -> bool {
        Self::is_unit_map_fully_populated_inner(&self.unit_map, self.units as usize)
    }

    pub fn is_unit_map_empty_inner(unit_map: &Vec<Option<String>>, units: usize) -> bool {
        unit_map
            .iter()
            .take(units)
            .all(|design| design.is_none())
    }

    pub fn is_unit_map_empty(&self) -> bool {
        Self::is_unit_map_empty_inner(&self.unit_map, self.units as usize)
    }

    /// Grow the unit map but never shrink it.  If we shrink it, we will lose the mappings already
    /// created if the user changes the number of units.
    fn grow_unit_map(&mut self, new_size: usize) {
        if self.unit_map.len() < new_size {
            self.unit_map.resize(new_size, None);
        }
    }
}

#[derive(Debug, Clone)]
pub enum AddPcbModalUiCommand {
    Submit,
    Cancel,

    NameChanged(String),
    UnitsChanged(u16),

    DesignNameChanged(String),
    AssignDesignSelection(Vec<usize>),
    UnassignDesignsSelection(Vec<usize>),
    AssignDesignToAllClicked,
    UnassignAllDesignsClicked,
}

#[derive(Debug, Clone)]
pub enum AddPcbModalAction {
    Submit(AddPcbArgs),
    CloseDialog,
}

/// Value object
#[derive(Debug, Clone)]
pub struct AddPcbArgs {
    pub name: String,
    pub units: u16,
    pub unit_map: BTreeMap<PcbUnitNumber, DesignName>,
}

impl UiComponent for AddPcbModal {
    type UiContext<'context> = ();
    type UiCommand = AddPcbModalUiCommand;
    type UiAction = AddPcbModalAction;

    fn ui<'context>(&self, ui: &mut egui::Ui, _context: &mut Self::UiContext<'context>) {
        ui.ctx().style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        let modal_id = ui.id().with("add_pcb_modal");

        Modal::new(modal_id).show(ui.ctx(), |ui| {
            ui.set_width(ui.available_width() * 0.8);

            let file_name = self
                .path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap();
            ui.heading(tr!("modal-add-pcb-title", {file: file_name}));

            let validation_context = AddPcbValidationContext {
                units: self.fields.lock().unwrap().units,
            };

            let form = Form::new(&self.fields, &self.component.sender, &validation_context);

            self.show_form(ui, &form);

            egui::Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    if ui
                        .button(tr!("form-button-cancel"))
                        .clicked()
                    {
                        self.component
                            .send(AddPcbModalUiCommand::Cancel);
                    }

                    if ui
                        .add_enabled(form.is_valid(), egui::Button::new(tr!("form-button-ok")))
                        .clicked()
                    {
                        self.component
                            .send(AddPcbModalUiCommand::Submit);
                    }
                },
            );
        });
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            AddPcbModalUiCommand::Submit => {
                let fields = self.fields.lock().unwrap();

                let units = fields.units;
                let unit_map_iter = fields
                    .unit_map
                    .iter()
                    .take(units as usize)
                    .enumerate()
                    .map(|(pcb_unit, design)| {
                        // Safety: from validation prevents 'design' from being None
                        (
                            pcb_unit as u16 + 1,
                            DesignName::from_str(design.as_ref().unwrap()).unwrap(),
                        )
                    });
                let unit_map = BTreeMap::from_iter(unit_map_iter);
                let args = AddPcbArgs {
                    name: fields.name.clone(),
                    // Safety: form validation prevents kind from being None
                    units,
                    unit_map,
                };

                Some(AddPcbModalAction::Submit(args))
            }
            AddPcbModalUiCommand::NameChanged(name) => {
                self.fields.lock().unwrap().name = name;
                None
            }
            AddPcbModalUiCommand::UnitsChanged(units) => {
                let mut fields = self.fields.lock().unwrap();

                fields.units = units;

                fields.grow_unit_map(units as usize);

                None
            }
            AddPcbModalUiCommand::Cancel => Some(AddPcbModalAction::CloseDialog),
            AddPcbModalUiCommand::DesignNameChanged(name) => {
                self.fields.lock().unwrap().design_name = name;
                None
            }
            AddPcbModalUiCommand::AssignDesignSelection(selection) => {
                let mut fields = self.fields.lock().unwrap();
                let design_name = fields.design_name.clone();

                for (index, entry) in fields.unit_map.iter_mut().enumerate() {
                    if selection.contains(&index) {
                        *entry = Some(design_name.clone());
                    }
                }

                None
            }
            AddPcbModalUiCommand::AssignDesignToAllClicked => {
                let mut fields = self.fields.lock().unwrap();
                let design_name = fields.design_name.clone();

                for entry in fields.unit_map.iter_mut() {
                    *entry = Some(design_name.clone());
                }

                None
            }
            AddPcbModalUiCommand::UnassignDesignsSelection(selection) => {
                let mut fields = self.fields.lock().unwrap();

                for (index, entry) in fields.unit_map.iter_mut().enumerate() {
                    if selection.contains(&index) {
                        *entry = None;
                    }
                }

                None
            }
            AddPcbModalUiCommand::UnassignAllDesignsClicked => {
                let mut fields = self.fields.lock().unwrap();

                for entry in fields.unit_map.iter_mut() {
                    *entry = None;
                }

                None
            }
        }
    }
}
