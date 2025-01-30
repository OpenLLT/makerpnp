use std::path::PathBuf;

use cushy::channel::Sender;
use cushy::figures::units::Px;
use cushy::styles::components::IntrinsicPadding;
use cushy::value::{Destination, Dynamic, IntoValue, Source, Validations, Value};
use cushy::widget::{MakeWidget, WidgetInstance};
use cushy::widgets::grid::{GridDimension, GridWidgets};
use cushy::widgets::label::Displayable;
use cushy::widgets::{Grid, Input, Space};
use cushy::{localize, MaybeLocalized};
use planner_app::ObjectPath;
use tracing::debug;

use crate::project::dialogs::PcbKind;
use crate::project::CreateUnitAssignmentArgs;

#[derive(Debug, Clone)]
pub enum CreateUnitAssignmentFormMessage {
    UpdatePlacementsFilename,
}

#[derive(Default)]
pub struct CreateUnitAssignmentFormState {
    design_name: Dynamic<String>,
    variant_name: Dynamic<String>,

    placements_filename: Dynamic<String>,

    // object path
    pcb_kind: Dynamic<PcbKind>,
    pcb_instance: Dynamic<String>,
    pcb_unit: Dynamic<String>,

    validations: Validations,
}

pub struct CreateUnitAssignmentForm {
    state: CreateUnitAssignmentFormState,
    sender: Sender<CreateUnitAssignmentFormMessage>,
    project_path: PathBuf,
}

impl CreateUnitAssignmentFormState {
    fn validate_design_name(design_name: &String) -> Result<(), Value<MaybeLocalized>> {
        if design_name.is_empty() {
            Err(localize!("form-input-error-empty").into_value())
        } else {
            Ok(())
        }
    }

    fn validate_variant_name(design_name: &String) -> Result<(), Value<MaybeLocalized>> {
        if design_name.is_empty() {
            Err(localize!("form-input-error-empty").into_value())
        } else {
            Ok(())
        }
    }

    fn validate_pcb_kind(kind: &PcbKind) -> Result<(), Value<MaybeLocalized>> {
        match kind {
            PcbKind::None => Err(localize!("form-input-choice-empty").into_value()),
            _ => Ok(()),
        }
    }

    fn validate_pcb_instance(instance: &String) -> Result<(), Value<MaybeLocalized>> {
        match instance.parse::<usize>() {
            Ok(instance) => {
                if !(instance > 0) {
                    Err(localize!("form-input-number-require-greater-than-zero").into_value())
                } else {
                    Ok(())
                }
            }

            Err(_) => Err(localize!("form-input-number-require-positive-number").into_value()),
        }
    }

    fn validate_pcb_unit(unit: &String) -> Result<(), Value<MaybeLocalized>> {
        match unit.parse::<usize>() {
            Ok(instance) => {
                if !(instance > 0) {
                    Err(localize!("form-input-number-require-greater-than-zero").into_value())
                } else {
                    Ok(())
                }
            }

            Err(_) => Err(localize!("form-input-number-require-positive-number").into_value()),
        }
    }

    fn validate_placements_filename(
        placements_filename: &String,
        project_path: &PathBuf,
    ) -> Result<(), Value<MaybeLocalized>> {
        let mut placements_path = PathBuf::from(project_path);

        placements_path.push(placements_filename);
        if !placements_path.exists() {
            debug!("placements file does not exist. filename: {:?}", placements_path);
            Err(localize!("form-file-not-found").into_value())
        } else {
            Ok(())
        }
    }

    fn update_placements_filename(&self) {
        let filename = format!("{}_{}_placements.csv", self.design_name.get(), self.variant_name.get()).to_string();
        self.placements_filename.set(filename);
    }
}

impl CreateUnitAssignmentForm {
    pub fn new(sender: Sender<CreateUnitAssignmentFormMessage>, project_path: PathBuf) -> Self {
        Self {
            state: Default::default(),
            sender,
            project_path,
        }
    }

    pub fn update(&mut self, message: CreateUnitAssignmentFormMessage) {
        match message {
            CreateUnitAssignmentFormMessage::UpdatePlacementsFilename => self.state.update_placements_filename(),
        }
    }

    pub fn validations(&self) -> &Validations {
        &self.state.validations
    }
    pub fn result(&self) -> Result<CreateUnitAssignmentArgs, ()> {
        if !self.state.validations.is_valid() {
            return Err(());
        }

        let pcb_kind = self.state.pcb_kind.get().try_into()?;
        let pcb_instance: usize = self
            .state
            .pcb_instance
            .get()
            .parse::<usize>()
            .unwrap();
        let pcb_unit: usize = self
            .state
            .pcb_unit
            .get()
            .parse::<usize>()
            .unwrap();

        let mut object_path = ObjectPath::default();

        object_path.set_pcb_kind_and_instance(pcb_kind, pcb_instance);
        object_path.set_pcb_unit(pcb_unit);

        Ok(CreateUnitAssignmentArgs {
            design_name: self.state.design_name.get(),
            variant_name: self.state.variant_name.get(),
            object_path,
        })
    }
}

impl MakeWidget for &CreateUnitAssignmentForm {
    fn make_widget(self) -> WidgetInstance {
        let validations = self.state.validations.clone();

        self.state
            .design_name
            .for_each({
                let sender = self.sender.clone();
                move |_design_name| {
                    sender
                        .send(CreateUnitAssignmentFormMessage::UpdatePlacementsFilename)
                        .expect("sent");
                }
            })
            .persist();

        let design_name_label = localize!("form-create-unit-assignment-input-design-name").align_left();
        let design_name_input = Input::new(self.state.design_name.clone())
            .placeholder(localize!("form-create-unit-assignment-input-design-name-placeholder"))
            .validation(validations.validate(
                &self.state.design_name.clone(),
                CreateUnitAssignmentFormState::validate_design_name,
            ))
            .hint(localize!("form-field-required"));

        let design_name_row = (design_name_label, design_name_input);

        // FIXME remove this workaround for lack of grid gutter support.
        let design_name_row_gutter = (Space::clear().height(Px::new(5)), Space::clear().height(Px::new(5)));

        self.state
            .variant_name
            .for_each({
                let sender = self.sender.clone();
                move |_variant_name| {
                    sender
                        .send(CreateUnitAssignmentFormMessage::UpdatePlacementsFilename)
                        .expect("sent");
                }
            })
            .persist();

        let variant_name_label = localize!("form-create-unit-assignment-input-variant-name").align_left();
        let variant_name_input = Input::new(self.state.variant_name.clone())
            .placeholder(localize!("form-create-unit-assignment-input-variant-name-placeholder"))
            .validation(validations.validate(
                &self.state.variant_name.clone(),
                CreateUnitAssignmentFormState::validate_variant_name,
            ))
            .hint(localize!("form-field-required"));

        let variant_name_row = (variant_name_label, variant_name_input);

        // FIXME remove this workaround for lack of grid gutter support.
        let variant_name_row_gutter = (Space::clear().height(Px::new(5)), Space::clear().height(Px::new(5)));

        let placements_filename_label = localize!("form-create-unit-assignment-input-placements-filename").align_left();

        let placements_filename_input = self
            .state
            .placements_filename
            .clone()
            .into_label()
            .validation(validations.validate(&self.state.placements_filename.clone(), {
                let project_path = self.project_path.clone();
                move |placements_filename| {
                    CreateUnitAssignmentFormState::validate_placements_filename(placements_filename, &project_path)
                }
            }));

        let placements_filename_row = (placements_filename_label, placements_filename_input);

        // FIXME remove this workaround for lack of grid gutter support.
        let placements_filename_row_gutter = (Space::clear().height(Px::new(5)), Space::clear().height(Px::new(5)));

        let pcb_kind_label = localize!("form-common-choice-pcb-kind").align_left();

        let pcb_kind_choices = self
            .state
            .pcb_kind
            .new_radio(PcbKind::Single)
            .labelled_by(localize!("form-common-choice-pcb-kind-single"))
            .and(
                self.state
                    .pcb_kind
                    .new_radio(PcbKind::Panel)
                    .labelled_by(localize!("form-common-choice-pcb-kind-panel")),
            )
            .into_columns()
            .validation(
                self.state
                    .validations
                    .validate(&self.state.pcb_kind, CreateUnitAssignmentFormState::validate_pcb_kind),
            )
            .hint(localize!("form-field-required"));

        let pcb_kind_row = (pcb_kind_label, pcb_kind_choices);

        // FIXME remove this workaround for lack of grid gutter support.
        let pcb_kind_row_gutter = (Space::clear().height(Px::new(5)), Space::clear().height(Px::new(5)));

        let pcb_instance_label = localize!("form-create-unit-assignment-input-pcb-instance").align_left();
        let pcb_instance_input = Input::new(self.state.pcb_instance.clone())
            .placeholder(localize!("form-create-unit-assignment-input-pcb-instance-placeholder"))
            .validation(validations.validate(
                &self.state.pcb_instance.clone(),
                CreateUnitAssignmentFormState::validate_pcb_instance,
            ))
            .hint(localize!("form-field-required"));

        let pcb_instance_row = (pcb_instance_label, pcb_instance_input);

        // FIXME remove this workaround for lack of grid gutter support.
        let pcb_instance_row_gutter = (Space::clear().height(Px::new(5)), Space::clear().height(Px::new(5)));

        let pcb_unit_label = localize!("form-create-unit-assignment-input-pcb-unit").align_left();
        let pcb_unit_input = Input::new(self.state.pcb_unit.clone())
            .placeholder(localize!("form-create-unit-assignment-input-pcb-unit-placeholder"))
            .validation(validations.validate(
                &self.state.pcb_unit.clone(),
                CreateUnitAssignmentFormState::validate_pcb_unit,
            ))
            .hint(localize!("form-field-required"));

        let pcb_unit_row = (pcb_unit_label, pcb_unit_input);

        // FIXME remove this workaround for lack of grid gutter support.
        let pcb_unit_row_gutter = (Space::clear().height(Px::new(5)), Space::clear().height(Px::new(5)));

        let grid_widgets = GridWidgets::from(design_name_row)
            .and(design_name_row_gutter)
            .and(variant_name_row)
            .and(variant_name_row_gutter)
            .and(placements_filename_row)
            .and(placements_filename_row_gutter)
            .and(pcb_kind_row)
            .and(pcb_kind_row_gutter)
            .and(pcb_instance_row)
            .and(pcb_instance_row_gutter)
            .and(pcb_unit_row)
            .and(pcb_unit_row_gutter);

        let grid = Grid::from_rows(grid_widgets)
            .dimensions([GridDimension::FitContent, GridDimension::Fractional {
                weight: 1,
            }])
            // FIXME failing to set a gutter between the rows
            .with(&IntrinsicPadding, Px::new(5)); // no visible effect.

        grid.make_widget()
    }
}
