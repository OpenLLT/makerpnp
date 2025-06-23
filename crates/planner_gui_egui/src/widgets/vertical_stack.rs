use eframe::epaint::Color32;
use egui::{Frame, Id, Rect, Sense, Stroke, Ui, Vec2, ScrollArea};
use std::boxed::Box;

/// A component that displays multiple panels stacked vertically with resize handles.
pub struct VerticalStack {
    min_height: f32,
    id_source: Id,
    panel_heights: Vec<f32>,
    default_panel_height: f32,
    drag_in_progress: bool,
    active_drag_handle: Option<usize>,
    initialized: bool,
    last_available_height: f32,
    max_height: Option<f32>,
}

impl VerticalStack {
    /// Creates a new empty vertical stack.
    pub fn new() -> Self {
        Self {
            min_height: 50.0,
            id_source: Id::new("vertical_stack"),
            panel_heights: Vec::new(),
            default_panel_height: 100.0,
            drag_in_progress: false,
            active_drag_handle: None,
            initialized: false,
            last_available_height: 0.0,
            max_height: None,
        }
    }

    /// Sets the minimum height for panels.
    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = height;
        self
    }

    /// Sets the maximum height for the scroll area.
    /// If None, will use all available height.
    pub fn max_height(mut self, height: Option<f32>) -> Self {
        self.max_height = height;
        self
    }

    /// Sets a custom ID salt for the stack.
    pub fn id_salt(&mut self, id: impl std::hash::Hash) -> &mut Self {
        self.id_source = Id::new(id);
        self
    }

    /// Set the default height for new panels.
    pub fn default_panel_height(mut self, height: f32) -> Self {
        self.default_panel_height = height;
        self
    }

    /// The main function to render the stack and add panels.
    pub fn body<F>(&mut self, ui: &mut Ui, collect_panels: F)
    where
        F: FnOnce(&mut StackBodyBuilder),
    {
        let available_rect = ui.available_rect_before_wrap();
        let available_height = match self.max_height {
            Some(max_height) => max_height.min(available_rect.height()),
            None => available_rect.height(),
        };

        let mut body = StackBodyBuilder {
            panels: Vec::new(),
        };

        // Collect panel functions (this doesn't render anything yet)
        collect_panels(&mut body);

        // Get panel count
        let panel_count = body.panels.len();

        // Early return if no panels
        if panel_count == 0 {
            return;
        }

        // Ensure we have enough heights for all panels
        while self.panel_heights.len() < panel_count {
            self.panel_heights.push(self.default_panel_height);
        }

        // Truncate if we have too many heights
        if self.panel_heights.len() > panel_count {
            self.panel_heights.truncate(panel_count);
        }

        // Handle drag state
        let pointer_is_down = ui.input(|i| i.pointer.any_down());

        if self.drag_in_progress && !pointer_is_down {
            self.drag_in_progress = false;
            self.active_drag_handle = None;
        }

        self.last_available_height = available_height;

        // Create a ScrollArea with the available height
        ScrollArea::vertical()
            .id_source(self.id_source.with("scroll_area"))
            .max_height(available_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Render each panel with its calculated height
                for (idx, panel_fn) in body.panels.into_iter().enumerate() {
                    let panel_height = self.panel_heights[idx].max(self.min_height);

                    // Create a panel with fixed height
                    Frame::default()
                        .stroke(Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color))
                        .show(ui, |ui| {
                            // Constrain the height of this panel
                            ui.set_min_height(panel_height);
                            ui.set_max_height(panel_height);

                            // Call the panel content function
                            panel_fn(ui);
                        });

                    // Add a resize handle after each panel (except the last one)
                    if idx < panel_count - 1 {
                        self.add_resize_handle(ui, idx);
                    }
                }
            });

        // Mark as initialized
        self.initialized = true;
    }

    /// Add a resize handle between panels.
    fn add_resize_handle(&mut self, ui: &mut Ui, panel_idx: usize) {
        let handle_id = self.id_source.with("resize_handle").with(panel_idx);
        let handle_height = 8.0;

        // Make sure we have the next panel's index available
        if panel_idx >= self.panel_heights.len() || panel_idx + 1 >= self.panel_heights.len() {
            ui.allocate_space(Vec2::new(ui.available_width(), handle_height));
            return;
        }

        // Allocate the space for the handle
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), handle_height),
            Sense::drag()
        );

        // Style for the handle
        let handle_stroke = if response.hovered() || response.dragged() {
            Stroke::new(2.0, Color32::WHITE)
        } else {
            Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color)
        };

        // Draw the handle line
        let center_y = rect.center().y;
        ui.painter().line_segment(
            [egui::Pos2::new(rect.left(), center_y), egui::Pos2::new(rect.right(), center_y)],
            handle_stroke,
        );

        // Handle dragging to resize - ONLY affecting the panel above
        if response.dragged() {
            // If this is the first drag or a different handle than before
            if !self.drag_in_progress || self.active_drag_handle != Some(panel_idx) {
                self.drag_in_progress = true;
                self.active_drag_handle = Some(panel_idx);
            }

            let delta = response.drag_delta().y;

            // Only process if there's an actual delta
            if delta != 0.0 {
                // Adjust only the panel above the handle
                let new_height = (self.panel_heights[panel_idx] + delta).max(self.min_height);

                // Apply the new height ONLY to the panel above the handle
                self.panel_heights[panel_idx] = new_height;
            }
        }
    }
}

// The body that collects panel functions
pub struct StackBodyBuilder {
    panels: Vec<Box<dyn FnOnce(&mut Ui)>>,
}

impl StackBodyBuilder {
    /// Add a panel to the stack with the given content.
    pub fn add_panel<F>(&mut self, add_contents: F)
    where
        F: FnOnce(&mut Ui) + 'static,
    {
        // Box the function and store it for later execution
        self.panels.push(Box::new(add_contents));
    }
}

impl Default for VerticalStack {
    fn default() -> Self {
        Self::new()
    }
}