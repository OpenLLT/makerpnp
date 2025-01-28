use cushy::channel::Sender;
use cushy::figures::units::Lp;
use cushy::localization::Localize;
use cushy::styles::components::IntrinsicPadding;
use cushy::styles::Dimension;
use cushy::widget::{IntoWidgetList, MakeWidget, WidgetInstance};
use cushy::widgets::Expand;

#[derive(Clone, Debug)]
pub enum ToolbarMessage {
    AddPcb,
}

pub fn make_toolbar(toolbar_sender: Sender<ToolbarMessage>) -> WidgetInstance {
    let button_padding = Dimension::Lp(Lp::points(4));

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

    let toolbar_widgets: [WidgetInstance; 2] = [add_pcb_button.make_widget(), Expand::empty().make_widget()];

    let toolbar = toolbar_widgets
        .into_columns()
        .contain()
        .make_widget();

    toolbar
}
