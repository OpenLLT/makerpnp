use std::fmt::Debug;
use cushy::styles::ContainerLevel;
use cushy::value::{Dynamic, Switchable};
use cushy::widget::{MakeWidget, WidgetInstance};
use cushy::widgets::{Grid, Label, Space};
use cushy::widgets::grid::{GridDimension, GridWidgets};
use cushy::widgets::label::{Displayable, DynamicDisplay};

pub struct PropertiesItem {
    label: WidgetInstance,
    field: WidgetInstance,
}

pub struct Properties {
    items: Vec<PropertiesItem>,
    grid_dimensions: Dynamic<[GridDimension;2]>,
    header: WidgetInstance,
    footer: WidgetInstance,
    expand_vertically: bool,
}

impl Default for Properties {
    fn default() -> Self {
        Self {
            items: Default::default(),
            grid_dimensions: Default::default(),
            header: Space::default().make_widget(),
            footer: Space::default().make_widget(),
            expand_vertically: false,
        }
    }
}

impl Properties {

    pub fn with_items(mut self, items: Vec<PropertiesItem>) -> Self {
        self.items = items;
        self
    }

    pub fn with_header_widget(mut self, header: WidgetInstance) -> Self {
        self.header = header;
        self
    }
    pub fn with_footer_widget(mut self, footer: WidgetInstance) -> Self {
        self.footer = footer;
        self
    }

    pub fn with_header_label<T>(mut self, label: Label<T>) -> Self
    where
        T: Debug + DynamicDisplay + Send + 'static,
    {
        let properties_header = label
            .centered()
            .align_left()
            .contain_level(ContainerLevel::Highest);

        self.header = properties_header.make_widget();
        self
    }
    
    pub fn expand_vertically(mut self) -> Self {
        self.expand_vertically = true;
        self
    }

    pub fn with_footer_label<T>(mut self, label: Label<T>) -> Self
    where
        T: Debug + DynamicDisplay + Send + 'static,
    {
        let properties_footer = label
            .centered()
            .align_left()
            .contain_level(ContainerLevel::Highest);

        self.footer = properties_footer.make_widget();
        self
    }

    pub fn push(&mut self, item: PropertiesItem) {
        self.items.push(item);
    }

    pub fn make_widget(&self) -> WidgetInstance {

        let grid_rows: Vec<(WidgetInstance, WidgetInstance)> = self.items.iter().map(|item|{
            (
                item.label.clone(),
                item.field.clone()
            )
        }).collect();

        let grid_row_widgets = GridWidgets::from(grid_rows);

        let grid = Grid::from_rows(grid_row_widgets);

        let grid_widget = grid
            .dimensions(self.grid_dimensions.clone())
            .align_top()
            .align_left()
            .make_widget();

        let scrollable_content = grid_widget
            .vertical_scroll()
            .contain_level(ContainerLevel::High);
        
        let scrollable_content = if self.expand_vertically {
            scrollable_content
                .expand_vertically()
                .make_widget()
            
        } else {
            scrollable_content
                .make_widget()
        };
        
        let properties_widget = self.header.clone()
            .and(scrollable_content)
            .and(self.footer.clone())
            .into_rows();
        
        let properties_widget = if self.expand_vertically {
            properties_widget
                .expand_vertically()
                .make_widget()
        } else {
            properties_widget
                .make_widget()
        };
        
        properties_widget
    }
}

impl PropertiesItem {
    pub fn from_field(label: impl MakeWidget, field: impl MakeWidget) -> Self {
        Self {
            label: label.make_widget(),
            field: field.make_widget(),
        }
    }

    pub fn from_optional_value(label: impl MakeWidget, value: Dynamic<Option<String>>) -> Self {
        let field = value.clone().switcher({
            move |value, _| {
                match value.clone() {
                    Some(value) =>
                        value
                            .into_label()
                            .make_widget()
                    ,
                    None =>
                        Space::clear()
                            .make_widget(),
                }
            }
        })
            .align_left()
            .make_widget();

        Self {
            label: label.make_widget(),
            field,
        }
    }
}

