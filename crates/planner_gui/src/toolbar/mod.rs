use cushy::channel::Sender;
use cushy::figures::units::Lp;
use cushy::localization::Localize;
use cushy::styles::components::IntrinsicPadding;
use cushy::styles::Dimension;
use cushy::value::Dynamic;
use cushy::widget::{IntoWidgetList, MakeWidget, WidgetInstance};
use cushy::widgets::{Expand, Radio};
use unic_langid::LanguageIdentifier;

#[derive(Clone, Debug)]
pub enum ToolbarMessage {
    OpenClicked,
    HomeClicked,
    NewClicked,
    CloseAllClicked,
    SaveClicked,
}

pub(crate) fn make_toolbar(
    toolbar_message_sender: Sender<ToolbarMessage>,
    language_identifier: Dynamic<LanguageIdentifier>,
    languages: &Vec<LanguageIdentifier>,
) -> WidgetInstance {
    let button_padding = Dimension::Lp(Lp::points(4));

    let home_button = Localize::new("toolbar-button-home")
        .into_button()
        .on_click({
            let toolbar_message_sender = toolbar_message_sender.clone();
            move |_event| {
                let _ = toolbar_message_sender.send(ToolbarMessage::HomeClicked);
            }
        })
        .with(&IntrinsicPadding, button_padding);

    let new_button = Localize::new("toolbar-button-new")
        .into_button()
        .on_click({
            let toolbar_message_sender = toolbar_message_sender.clone();
            move |_event| {
                let _ = toolbar_message_sender.send(ToolbarMessage::NewClicked);
            }
        })
        .with(&IntrinsicPadding, button_padding);

    let open_button = Localize::new("toolbar-button-open")
        .into_button()
        .on_click({
            let toolbar_message_sender = toolbar_message_sender.clone();
            move |_event| {
                let _ = toolbar_message_sender.send(ToolbarMessage::OpenClicked);
            }
        })
        .with(&IntrinsicPadding, button_padding);

    let save_button = Localize::new("toolbar-button-save")
        .into_button()
        .on_click({
            let toolbar_message_sender = toolbar_message_sender.clone();
            move |_event| {
                let _ = toolbar_message_sender.send(ToolbarMessage::SaveClicked);
            }
        })
        .with(&IntrinsicPadding, button_padding);

    let close_all_button = Localize::new("toolbar-button-close-all")
        .into_button()
        .on_click({
            let toolbar_message_sender = toolbar_message_sender.clone();
            move |_event| {
                let _ = toolbar_message_sender.send(ToolbarMessage::CloseAllClicked);
            }
        })
        .with(&IntrinsicPadding, button_padding);

    // TODO use a drop-down/pop-up instead of a radio group
    let language_radio_buttons: Vec<WidgetInstance> = languages
        .iter()
        .map(|language| {
            Radio::new(language.clone(), language_identifier.clone())
                // TODO show human-readable language names
                //      in the format "<Country in current locale> (<Language in current locale>) - <Country in native locale> (<Languge in native locale>)
                //      e.g. given the current locale of "en-US", display: "Spanish (Spain) - Español (España)" for "es-ES"
                .labelled_by(language.to_string())
                .make_widget()
        })
        .collect();

    let language_selector = language_radio_buttons.into_columns();

    let toolbar_widgets: [WidgetInstance; 7] = [
        home_button.make_widget(),
        new_button.make_widget(),
        open_button.make_widget(),
        save_button.make_widget(),
        close_all_button.make_widget(),
        Expand::empty().make_widget(),
        language_selector.make_widget(),
    ];

    let toolbar = toolbar_widgets
        .into_columns()
        .contain()
        .make_widget();

    toolbar
}
