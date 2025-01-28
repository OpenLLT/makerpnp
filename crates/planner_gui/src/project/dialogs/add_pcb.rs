use cushy::figures::units::Px;
use cushy::styles::components::IntrinsicPadding;
use cushy::value::{Dynamic, IntoValue, Source, Validations, Value};
use cushy::widget::MakeWidget;
use cushy::widgets::grid::{GridDimension, GridWidgets};
use cushy::widgets::{Grid, Input, Space};
use cushy::{localize, MaybeLocalized};

use crate::project::AddPcbArgs;

#[derive(Default, Eq, PartialEq, Debug, Clone, Copy)]
pub enum PcbKind {
    #[default]
    None,
    Single,
    Panel,
}

impl TryFrom<PcbKind> for planner_app::PcbKind {
    type Error = ();

    fn try_from(value: PcbKind) -> Result<Self, Self::Error> {
        match value {
            PcbKind::None => Err(()),
            PcbKind::Single => Ok(planner_app::PcbKind::Single),
            PcbKind::Panel => Ok(planner_app::PcbKind::Panel),
        }
    }
}

#[derive(Default)]
pub struct AddPcbForm {
    name: Dynamic<String>,
    kind: Dynamic<PcbKind>,

    validations: Validations,
}

impl AddPcbForm {
    pub fn validations(&self) -> &Validations {
        &self.validations
    }

    fn validate_name(name: &String) -> Result<(), Value<MaybeLocalized>> {
        if name.is_empty() {
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

    pub fn result(&self) -> Result<AddPcbArgs, ()> {
        if !self.validations.is_valid() {
            return Err(());
        }

        let kind = self.kind.get().try_into()?;

        Ok(AddPcbArgs {
            name: self.name.get(),
            kind,
        })
    }
}

impl MakeWidget for &AddPcbForm {
    fn make_widget(self) -> cushy::widget::WidgetInstance {
        let validations = self.validations.clone();

        let name_label = localize!("form-add-pcb-input-name").align_left();
        let name_input = Input::new(self.name.clone())
            .placeholder(localize!("form-add-pcb-input-name-placeholder"))
            .validation(validations.validate(&self.name.clone(), AddPcbForm::validate_name))
            .hint(localize!("form-field-required"));

        let name_row = (name_label, name_input);

        // FIXME remove this workaround for lack of grid gutter support.
        let gutter_row_1 = (Space::clear().height(Px::new(5)), Space::clear().height(Px::new(5)));

        let kind_label = localize!("form-add-pcb-choice-kind").align_left();

        let kind_choices = self
            .kind
            .new_radio(PcbKind::Single)
            .labelled_by(localize!("form-add-pcb-choice-kind-single"))
            .and(
                self.kind
                    .new_radio(PcbKind::Panel)
                    .labelled_by(localize!("form-add-pcb-choice-kind-panel")),
            )
            .into_columns()
            .validation(
                self.validations
                    .validate(&self.kind, AddPcbForm::validate_pcb_kind),
            )
            .hint(localize!("form-field-required"));

        let kind_row = (kind_label, kind_choices);

        let gutter_row_2 = (Space::clear().height(Px::new(5)), Space::clear().height(Px::new(5)));

        let grid_widgets = GridWidgets::from(name_row)
            .and(gutter_row_1)
            .and(kind_row)
            .and(gutter_row_2);

        let grid = Grid::from_rows(grid_widgets)
            .dimensions([GridDimension::FitContent, GridDimension::Fractional {
                weight: 1,
            }])
            // FIXME failing to set a gutter between the rows
            .with(&IntrinsicPadding, Px::new(5)); // no visible effect.

        grid.make_widget()
    }
}
