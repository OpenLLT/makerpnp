use cushy::figures::units::Px;
use cushy::styles::components::IntrinsicPadding;
use cushy::value::{Dynamic, IntoValue, Source, Validations, Value};
use cushy::widget::{MakeWidget, WidgetInstance};
use cushy::widgets::grid::{GridDimension, GridWidgets};
use cushy::widgets::{Grid, Input, Space};
use cushy::{localize, MaybeLocalized};
use planner_app::ObjectPath;

use crate::project::dialogs::PcbKind;
use crate::project::CreateUnitAssignmentArgs;

#[derive(Default)]
pub struct CreateUnitAssignmentForm {
    design_name: Dynamic<String>,
    variant_name: Dynamic<String>,

    // object path
    pcb_kind: Dynamic<PcbKind>,
    pcb_instance: Dynamic<String>,
    pcb_unit: Dynamic<String>,

    validations: Validations,
}

impl CreateUnitAssignmentForm {
    pub fn validations(&self) -> &Validations {
        &self.validations
    }

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

    pub fn result(&self) -> Result<CreateUnitAssignmentArgs, ()> {
        if !self.validations.is_valid() {
            return Err(());
        }

        let pcb_kind = self.pcb_kind.get().try_into()?;
        let pcb_instance: usize = self
            .pcb_instance
            .get()
            .parse::<usize>()
            .unwrap();
        let pcb_unit: usize = self
            .pcb_unit
            .get()
            .parse::<usize>()
            .unwrap();

        let mut object_path = ObjectPath::default();

        object_path.set_pcb_kind_and_instance(pcb_kind, pcb_instance);
        object_path.set_pcb_unit(pcb_unit);

        Ok(CreateUnitAssignmentArgs {
            design_name: self.design_name.get(),
            variant_name: self.variant_name.get(),
            object_path,
        })
    }
}

impl MakeWidget for &CreateUnitAssignmentForm {
    fn make_widget(self) -> WidgetInstance {
        let validations = self.validations.clone();

        let design_name_label = localize!("form-create-unit-assignment-input-design-name").align_left();
        let design_name_input = Input::new(self.design_name.clone())
            .placeholder(localize!("form-create-unit-assignment-input-design-name-placeholder"))
            .validation(validations.validate(
                &self.design_name.clone(),
                CreateUnitAssignmentForm::validate_design_name,
            ))
            .hint(localize!("form-field-required"));

        let design_name_row = (design_name_label, design_name_input);

        // FIXME remove this workaround for lack of grid gutter support.
        let gutter_row_1 = (Space::clear().height(Px::new(5)), Space::clear().height(Px::new(5)));

        let variant_name_label = localize!("form-create-unit-assignment-input-variant-name").align_left();
        let variant_name_input = Input::new(self.variant_name.clone())
            .placeholder(localize!("form-create-unit-assignment-input-variant-name-placeholder"))
            .validation(validations.validate(
                &self.variant_name.clone(),
                CreateUnitAssignmentForm::validate_variant_name,
            ))
            .hint(localize!("form-field-required"));

        let variant_name_row = (variant_name_label, variant_name_input);

        // FIXME remove this workaround for lack of grid gutter support.
        let gutter_row_2 = (Space::clear().height(Px::new(5)), Space::clear().height(Px::new(5)));

        let pcb_kind_label = localize!("form-common-choice-pcb-kind").align_left();

        let pcb_kind_choices = self
            .pcb_kind
            .new_radio(PcbKind::Single)
            .labelled_by(localize!("form-common-choice-pcb-kind-single"))
            .and(
                self.pcb_kind
                    .new_radio(PcbKind::Panel)
                    .labelled_by(localize!("form-common-choice-pcb-kind-panel")),
            )
            .into_columns()
            .validation(
                self.validations
                    .validate(&self.pcb_kind, CreateUnitAssignmentForm::validate_pcb_kind),
            )
            .hint(localize!("form-field-required"));

        let pcb_kind_row = (pcb_kind_label, pcb_kind_choices);

        // FIXME remove this workaround for lack of grid gutter support.
        let gutter_row_3 = (Space::clear().height(Px::new(5)), Space::clear().height(Px::new(5)));

        let pcb_instance_label = localize!("form-create-unit-assignment-input-pcb-instance").align_left();
        let pcb_instance_input = Input::new(self.pcb_instance.clone())
            .placeholder(localize!("form-create-unit-assignment-input-pcb-instance-placeholder"))
            .validation(validations.validate(
                &self.pcb_instance.clone(),
                CreateUnitAssignmentForm::validate_pcb_instance,
            ))
            .hint(localize!("form-field-required"));

        let pcb_instance_row = (pcb_instance_label, pcb_instance_input);

        // FIXME remove this workaround for lack of grid gutter support.
        let gutter_row_4 = (Space::clear().height(Px::new(5)), Space::clear().height(Px::new(5)));

        let pcb_unit_label = localize!("form-create-unit-assignment-input-pcb-unit").align_left();
        let pcb_unit_input = Input::new(self.pcb_unit.clone())
            .placeholder(localize!("form-create-unit-assignment-input-pcb-unit-placeholder"))
            .validation(validations.validate(&self.pcb_unit.clone(), CreateUnitAssignmentForm::validate_pcb_unit))
            .hint(localize!("form-field-required"));

        let pcb_unit_row = (pcb_unit_label, pcb_unit_input);

        // FIXME remove this workaround for lack of grid gutter support.
        let gutter_row_5 = (Space::clear().height(Px::new(5)), Space::clear().height(Px::new(5)));

        let grid_widgets = GridWidgets::from(design_name_row)
            .and(gutter_row_1)
            .and(variant_name_row)
            .and(gutter_row_2)
            .and(pcb_kind_row)
            .and(gutter_row_3)
            .and(pcb_instance_row)
            .and(gutter_row_4)
            .and(pcb_unit_row)
            .and(gutter_row_5);

        let grid = Grid::from_rows(grid_widgets)
            .dimensions([GridDimension::FitContent, GridDimension::Fractional {
                weight: 1,
            }])
            // FIXME failing to set a gutter between the rows
            .with(&IntrinsicPadding, Px::new(5)); // no visible effect.

        grid.make_widget()
    }
}
