use cushy::figures::units::Lp;
use cushy::localization::Localize;
use cushy::reactive::channel::Sender;
use cushy::styles::components::IntrinsicPadding;
use cushy::styles::Dimension;
use cushy::widget::{IntoWidgetList, MakeWidget, WidgetInstance};
use cushy::widgets::Expand;

#[derive(Clone, Debug)]
pub enum ToolbarMessage {
    AddPcb,
    CreateUnitAssignment,
    RefreshFromVariants,
}

pub fn make_toolbar(toolbar_sender: Sender<ToolbarMessage>) -> WidgetInstance {
    let button_padding = Dimension::Lp(Lp::points(4));

    let refresh_from_variants_button = Localize::new("project-toolbar-button-refresh-from-variants")
        .into_button()
        .on_click({
            let toolbar_sender = toolbar_sender.clone();
            move |_event| {
                toolbar_sender
                    .send(ToolbarMessage::RefreshFromVariants)
                    .expect("sent")
            }
        })
        .with(&IntrinsicPadding, button_padding);

    let add_pcb_button = Localize::new("project-toolbar-button-add-pcb")
        .into_button()
        .on_click({
            let toolbar_sender = toolbar_sender.clone();
            move |_event| {
                toolbar_sender
                    .send(ToolbarMessage::AddPcb)
                    .expect("sent")
            }
        })
        .with(&IntrinsicPadding, button_padding);

    let create_unit_assignment_button = Localize::new("project-toolbar-button-create-unit-assignment")
        .into_button()
        .on_click({
            let toolbar_sender = toolbar_sender.clone();
            move |_event| {
                toolbar_sender
                    .send(ToolbarMessage::CreateUnitAssignment)
                    .expect("sent")
            }
        })
        .with(&IntrinsicPadding, button_padding);

    let toolbar_widgets: [WidgetInstance; 4] = [
        refresh_from_variants_button.make_widget(),
        add_pcb_button.make_widget(),
        create_unit_assignment_button.make_widget(),
        Expand::empty().make_widget(),
    ];

    let toolbar = toolbar_widgets
        .into_columns()
        .contain()
        .make_widget();

    toolbar
}
