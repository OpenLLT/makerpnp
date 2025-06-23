use eframe::epaint::{Color32, StrokeKind};
use egui::{Frame, Id, Rect, Sense, Stroke, Ui, Vec2, ScrollArea, Rounding, CornerRadius, UiBuilder};
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
            .id_salt(self.id_source.with("scroll_area"))
            .max_height(available_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Use vertical layout with no spacing
                ui.spacing_mut().item_spacing.y = 0.0;

                // Get the available rect for the panel content
                // if this is done INSIDE the loop below, and one of the panels overflows, then the width of remaining panels will be wrong.
                let panel_rect = ui.available_rect_before_wrap();

                // Render each panel with its calculated height
                for (idx, panel_fn) in body.panels.into_iter().enumerate() {
                    // Create a panel that spans the full width
                    let panel_height = self.panel_heights[idx].max(self.min_height);
                    let panel_size = Vec2::new(panel_rect.width(), panel_height);

                    // Allocate space with a sense for interaction but without consuming input
                    let (rect, _) = ui.allocate_exact_size(panel_size, Sense::hover());

                    // Draw the panel frame manually
                    let frame_rect = rect;
                    let stroke = Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color);
                    ui.painter().rect(
                        frame_rect,
                        CornerRadius::ZERO,
                        Color32::TRANSPARENT,
                        stroke,
                        StrokeKind::Outside
                    );

                    // Create content with padding inside the frame
                    let inner_margin = 2.0; // Adjust this for desired spacing
                    let content_rect = frame_rect.shrink(inner_margin);

                    let mut inner_stroke = stroke;
                    inner_stroke.color = Color32::RED;

                    // Use Frame::NONE instead of the deprecated Frame::none()
                    Frame::NONE
                        .fill(Color32::TRANSPARENT)
                        .stroke(inner_stroke)
                        .inner_margin(0.0)
                        .outer_margin(0.0)
                        .show(ui, |ui| {
                            // Use allocate_new_ui instead of the deprecated allocate_ui_at_rect
                            let meh = ui.allocate_new_ui(UiBuilder::new().max_rect(content_rect), |ui| {
                                // without this, the content will overflow to the right and below the frame.
                                ui.set_clip_rect(content_rect);
                                // Call the panel content function
                                panel_fn(ui);
                            });

                            let mut debug_stroke = stroke;
                            debug_stroke.color = Color32::GREEN;

                            ui.painter().rect(
                                meh.response.rect,
                                CornerRadius::ZERO,
                                Color32::TRANSPARENT,
                                debug_stroke,
                                StrokeKind::Outside
                            );
                        });

                    // Add a resize handle after each panel (except the last one)
                    if idx < panel_count - 1 {
                        self.add_resize_handle_no_gap(ui, idx);
                    }
                }
            });

        // Mark as initialized
        self.initialized = true;
    }

    /// Add a resize handle between panels with no gap.
    fn add_resize_handle_no_gap(&mut self, ui: &mut Ui, panel_idx: usize) {
        let handle_id = self.id_source.with("resize_handle").with(panel_idx);
        let handle_height = 7.0;

        // Make sure we have the next panel's index available
        if panel_idx >= self.panel_heights.len() || panel_idx + 1 >= self.panel_heights.len() {
            return;
        }

        // Calculate handle rect directly where we need it
        let available_rect = ui.available_rect_before_wrap();
        
        // Create a thin rectangle for the handle
        let handle_rect = Rect::from_min_size(
            available_rect.min,
            Vec2::new(available_rect.width(), handle_height)
        );

        // Allocate exact size with no default spacing
        let (rect, response) = ui.allocate_exact_size(
            handle_rect.size(),
            Sense::drag()
        );

        // Create a custom painter that doesn't add spacing
        let painter = ui.painter();

        // Style for the handle
        let handle_stroke = if response.hovered() || response.dragged() {
            Stroke::new(2.0, Color32::WHITE)
        } else {
            Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color)
        };

        // Draw the handle line
        let center_y = rect.center().y;
        painter.line_segment(
            [egui::Pos2::new(rect.left(), center_y), egui::Pos2::new(rect.right(), center_y)],
            handle_stroke,
        );

        // Handle dragging to resize
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