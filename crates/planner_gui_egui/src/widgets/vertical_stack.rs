use egui::{Frame, Id, Rect, Sense, Stroke, Ui, Vec2};

/// A component that displays multiple panels stacked vertically with resize handles.
pub struct VerticalStack {
    min_height: f32,
    id_source: Id,
    panel_heights: Vec<f32>,
    default_panel_height: f32,
}

impl VerticalStack {
    /// Creates a new empty vertical stack.
    pub fn new() -> Self {
        Self {
            min_height: 50.0,
            id_source: Id::new("vertical_stack"),
            panel_heights: Vec::new(),
            default_panel_height: 100.0,
        }
    }

    /// Sets the minimum height for panels.
    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = height;
        self
    }

    /// Sets a custom ID source for the stack.
    pub fn id_source(&mut self, id: impl std::hash::Hash) -> &mut Self {
        self.id_source = Id::new(id);
        self
    }

    /// Set the default height for new panels.
    pub fn default_panel_height(mut self, height: f32) -> Self {
        self.default_panel_height = height;
        self
    }

    /// The main function to render the stack and add panels.
    pub fn body<F>(&mut self, ui: &mut Ui, add_contents: F)
    where
        F: FnOnce(&mut StackBody),
    {
        // Create a scroll area to contain all panels
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut body = StackBody {
                    ui,
                    min_height: self.min_height,
                    id_source: self.id_source,
                    panel_heights: &mut self.panel_heights,
                    default_panel_height: self.default_panel_height,
                    panel_count: 0,
                };

                add_contents(&mut body);
            });
    }
}

/// Helper struct to manage the stack body content.
pub struct StackBody<'a> {
    ui: &'a mut Ui,
    min_height: f32,
    id_source: Id,
    panel_heights: &'a mut Vec<f32>,
    default_panel_height: f32,
    panel_count: usize,
}

impl<'a> StackBody<'a> {
    /// Add a panel to the stack with the given content.
    pub fn add_panel<F>(&mut self, add_contents: F)
    where
        F: FnOnce(&mut Ui),
    {
        let panel_idx = self.panel_count;

        // Ensure we have a height for this panel
        if self.panel_heights.len() <= panel_idx {
            self.panel_heights.push(self.default_panel_height);
        }

        let panel_height = self.panel_heights[panel_idx].max(self.min_height);

        // Create a unique ID for this panel
        let panel_id = self.id_source.with(panel_idx);

        // Create a frame with a border for the panel
        Frame::default()
            .stroke(Stroke::new(1.0, self.ui.visuals().widgets.noninteractive.bg_stroke.color))
            .show(self.ui, |ui| {
                let available_width = ui.available_width();

                // Create a sizing area for the panel with the current height
                let panel_rect = Rect::from_min_size(
                    ui.cursor().min,
                    Vec2::new(available_width, panel_height),
                );

                // Allocate the space
                ui.allocate_rect(panel_rect, Sense::hover());

                // Add a child UI confined to the panel area
                let mut child_ui = ui.child_ui(panel_rect, *ui.layout(), None);

                // Add the panel content
                add_contents(&mut child_ui);
            });

        // Add resize handle after the panel (unless it's the last panel)
        if panel_idx < self.panel_count || panel_idx == 0 {
            self.add_resize_handle(panel_idx);
        }

        self.panel_count += 1;
    }

    /// Add a resize handle between panels.
    fn add_resize_handle(&mut self, panel_idx: usize) {
        let handle_id = self.id_source.with("resize_handle").with(panel_idx);
        let handle_height = 8.0;
        let handle_rect = Rect::from_min_size(
            self.ui.cursor().min,
            Vec2::new(self.ui.available_width(), handle_height),
        );

        let handle_response = self.ui.interact(handle_rect, handle_id, Sense::drag());

        // Draw the handle
        let handle_visuals = self.ui.style().noninteractive();
        let handle_stroke = if handle_response.hovered() || handle_response.dragged() {
            Stroke::new(2.0, handle_visuals.bg_stroke.color)
        } else {
            Stroke::new(1.0, handle_visuals.bg_stroke.color)
        };

        // Draw the handle line
        let center_y = handle_rect.center().y;
        let left = handle_rect.left();
        let right = handle_rect.right();
        self.ui.painter().line_segment(
            [egui::Pos2::new(left, center_y), egui::Pos2::new(right, center_y)],
            handle_stroke,
        );

        // Handle dragging to resize
        if handle_response.dragged() && panel_idx < self.panel_heights.len() {
            let delta = handle_response.drag_delta().y;
            if delta != 0.0 {
                self.panel_heights[panel_idx] = (self.panel_heights[panel_idx] + delta).max(self.min_height);

                // If there's a next panel, adjust its height too
                if panel_idx + 1 < self.panel_heights.len() {
                    self.panel_heights[panel_idx + 1] = (self.panel_heights[panel_idx + 1] - delta).max(self.min_height);
                }
            }
        }

        // Allocate space for the handle
        self.ui.allocate_rect(handle_rect, Sense::hover());
    }
}

impl Default for VerticalStack {
    fn default() -> Self {
        Self::new()
    }
}