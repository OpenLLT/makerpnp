use eframe::epaint::Color32;
use egui::{Frame, Id, Rect, Sense, Stroke, Ui, Vec2};
use std::boxed::Box;

/// A component that displays multiple panels stacked vertically with resize handles.
pub struct VerticalStack {
    min_height: f32,
    id_source: Id,
    panel_heights: Vec<f32>,
    default_panel_height: f32,
    drag_in_progress: bool,
    initialized: bool,
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
            initialized: false,
        }
    }

    /// Sets the minimum height for panels.
    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = height;
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
    pub fn body<F>(&mut self, ui: &mut Ui, add_contents: F)
    where
        F: FnOnce(&mut StackBody),
    {
        // Create the stack body to collect panel functions
        let mut body = StackBody {
            panel_functions: Vec::new(),
        };

        // Collect panel functions (this doesn't render anything yet)
        add_contents(&mut body);

        // Get panel count and available height
        let panel_count = body.panel_functions.len();
        let available_height = ui.available_height();

        // Ensure we have enough heights for all panels
        while self.panel_heights.len() < panel_count {
            self.panel_heights.push(self.default_panel_height);
        }

        // Truncate if we have too many heights
        if self.panel_heights.len() > panel_count {
            self.panel_heights.truncate(panel_count);
        }

        // Distribute heights if needed (first frame, panel count changed, or no drag in progress)
        let need_redistribution = !self.initialized ||
            (panel_count != 0 && self.panel_heights.len() != panel_count) ||
            !self.drag_in_progress;

        if need_redistribution {
            self.distribute_panel_heights(panel_count, available_height);
        }

        // Now render all panels with the calculated heights
        self.render_panels(ui, body.panel_functions);

        // Mark as initialized
        self.initialized = true;
    }

    /// Render all panels with the calculated heights
    fn render_panels(&mut self, ui: &mut Ui, panel_functions: Vec<Box<dyn FnOnce(&mut Ui)>>) {
        // Skip if no panels
        if panel_functions.is_empty() {
            return;
        }

        // Handle height for resize handles
        let handle_height = 8.0;

        // Render each panel with its calculated height
        for (idx, panel_fn) in panel_functions.into_iter().enumerate() {
            // Add a resize handle before each panel (except the first one)
            if idx > 0 {
                self.add_resize_handle(ui, idx - 1);
            }

            // Get this panel's height
            let panel_height = self.panel_heights[idx].max(self.min_height);
            println!("Rendering panel {}: height = {}", idx, panel_height);

            // Create a frame with a border for the panel
            Frame::default()
                .stroke(Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color))
                .show(ui, |ui| {
                    let available_width = ui.available_width();

                    // Use the allocated height exactly as specified
                    ui.allocate_ui_with_layout(
                        Vec2::new(available_width, panel_height),
                        egui::Layout::top_down(egui::Align::LEFT).with_cross_justify(true),
                        |ui| {
                            // Use the full allocated space
                            ui.set_min_height(panel_height);
                            ui.expand_to_include_rect(ui.max_rect());
                            panel_fn(ui);
                        }
                    );
                });
        }
    }

    /// Add a resize handle between panels.
    fn add_resize_handle(&mut self, ui: &mut Ui, panel_idx: usize) {
        let handle_id = self.id_source.with("resize_handle").with(panel_idx);
        let handle_height = 8.0;
        let handle_rect = Rect::from_min_size(
            ui.cursor().min,
            Vec2::new(ui.available_width(), handle_height),
        );

        println!("Resize handle for panel {}: rect = {:?}", panel_idx, handle_rect);

        // Make sure we have the next panel's index available
        if panel_idx >= self.panel_heights.len() || panel_idx + 1 >= self.panel_heights.len() {
            println!("  Error: Panel index out of bounds");
            ui.allocate_rect(handle_rect, Sense::hover());
            return;
        }

        // Use drag sense explicitly to ensure dragging works
        let handle_response = ui.interact(handle_rect, handle_id, Sense::drag());

        println!("  Handle response: dragged = {}, hovered = {}",
                 handle_response.dragged(), handle_response.hovered());

        // Draw the handle
        let handle_visuals = ui.style().noninteractive();
        let handle_stroke = if handle_response.hovered() || handle_response.dragged() {
            Stroke::new(2.0, Color32::WHITE) // Make it more visible when hovered or dragged
        } else {
            Stroke::new(1.0, handle_visuals.bg_stroke.color)
        };

        // Draw the handle line
        let center_y = handle_rect.center().y;
        let left = handle_rect.left();
        let right = handle_rect.right();
        ui.painter().line_segment(
            [egui::Pos2::new(left, center_y), egui::Pos2::new(right, center_y)],
            handle_stroke,
        );

        // Add some grip indicators
        for i in 0..5 {
            let x = left + (right - left) * (0.3 + 0.1 * i as f32);
            let y_top = center_y - 2.0;
            let y_bottom = center_y + 2.0;
            ui.painter().line_segment(
                [egui::Pos2::new(x, y_top), egui::Pos2::new(x, y_bottom)],
                handle_stroke,
            );
        }

        // Handle dragging to resize
        if handle_response.dragged() {
            self.drag_in_progress = true;
            let delta = handle_response.drag_delta().y;
            println!("  Drag delta: {}", delta);

            // Only process if there's an actual delta
            if delta != 0.0 {
                // Calculate new heights while respecting min_height
                let current_height = self.panel_heights[panel_idx];
                let next_height = self.panel_heights[panel_idx + 1];

                println!("  Before resize: panel {} height = {}, panel {} height = {}",
                         panel_idx, current_height, panel_idx + 1, next_height);

                // Calculate maximum available height to prevent scrollbars
                let panel_count = self.panel_heights.len();
                let other_panels_sum: f32 = self.panel_heights.iter().enumerate()
                    .filter(|(i, _)| *i != panel_idx && *i != panel_idx + 1)
                    .map(|(_, &h)| h)
                    .sum();

                let handle_count = (panel_count - 1).max(0) as f32;
                let handles_height = handle_count * handle_height;
                let available_height = ui.available_rect_before_wrap().height();
                let max_available = available_height - other_panels_sum - handles_height;

                // Calculate new heights with constraints
                let mut new_current = (current_height + delta).max(self.min_height);
                let mut new_next = (next_height - delta).max(self.min_height);

                // Enforce the constraint that both panels combined can't exceed max_available
                if new_current + new_next > max_available {
                    // Scale both panels proportionally to fit
                    let ratio = max_available / (new_current + new_next);
                    new_current = (new_current * ratio).max(self.min_height);
                    new_next = (new_next * ratio).max(self.min_height);

                    // If we still exceed due to minimum heights, adjust the larger one
                    if new_current + new_next > max_available {
                        if new_current > new_next {
                            new_current = (max_available - new_next).max(self.min_height);
                        } else {
                            new_next = (max_available - new_current).max(self.min_height);
                        }
                    }
                }

                // Apply the new heights
                self.panel_heights[panel_idx] = new_current;
                self.panel_heights[panel_idx + 1] = new_next;

                println!("  After resize: panel {} height = {}, panel {} height = {}",
                         panel_idx, self.panel_heights[panel_idx],
                         panel_idx + 1, self.panel_heights[panel_idx + 1]);
            }
        }

        // Make sure we allocate the rect with drag sense
        ui.allocate_rect(handle_rect, Sense::drag());
    }

    // Helper method to distribute heights to fill available space
    fn distribute_panel_heights(&mut self, panel_count: usize, available_height: f32) {
        if panel_count == 0 {
            return;
        }

        // Calculate space needed for resize handles
        let handle_height = 8.0;
        let handles_height = if panel_count > 1 {
            (panel_count - 1) as f32 * handle_height
        } else {
            0.0
        };

        // Get the total height currently used by panels
        let total_panel_height: f32 = self.panel_heights.iter().sum();
        let available_for_panels = available_height - handles_height;

        println!("Available height: {}, Handles: {}, Total panel height: {}",
                 available_height, handles_height, total_panel_height);

        // If we need to scale panels (too big or too small)
        if (total_panel_height - available_for_panels).abs() > 1.0 {
            if !self.initialized {
                // First frame - distribute evenly
                let height_per_panel = (available_for_panels / panel_count as f32).max(self.min_height);
                for i in 0..panel_count {
                    self.panel_heights[i] = height_per_panel;
                }
            } else if total_panel_height > available_for_panels {
                // Need to shrink panels
                let excess = total_panel_height - available_for_panels;

                // Calculate how much space we can free up before hitting minimum heights
                let freeable_space: f32 = self.panel_heights.iter()
                    .map(|h| (h - self.min_height).max(0.0))
                    .sum();

                if freeable_space >= excess {
                    // We can free up enough space while respecting minimum heights
                    let mut remaining_excess = excess;

                    // First pass - proportionally reduce panels based on height above minimum
                    for i in 0..panel_count {
                        let freeable = (self.panel_heights[i] - self.min_height).max(0.0);
                        if freeable > 0.0 {
                            let reduction = excess * (freeable / freeable_space);
                            self.panel_heights[i] -= reduction;
                            remaining_excess -= reduction;
                        }
                    }

                    // Second pass - if there's any rounding error, apply to first panel that can take it
                    if remaining_excess > 0.01 {
                        for i in 0..panel_count {
                            let can_reduce = self.panel_heights[i] - self.min_height;
                            if can_reduce >= remaining_excess {
                                self.panel_heights[i] -= remaining_excess;
                                break;
                            }
                        }
                    }
                } else {
                    // Can't free enough space while respecting minimums
                    // Set all to minimum
                    for i in 0..panel_count {
                        self.panel_heights[i] = self.min_height;
                    }
                }
            } else {
                // Need to expand panels
                let extra_space = available_for_panels - total_panel_height;

                // Distribute extra space proportionally to current panel heights
                let total_current: f32 = self.panel_heights.iter().sum();
                if total_current > 0.0 {
                    let mut remaining = extra_space;
                    for i in 0..panel_count {
                        let ratio = self.panel_heights[i] / total_current;
                        let addition = extra_space * ratio;
                        self.panel_heights[i] += addition;
                        remaining -= addition;
                    }

                    // Distribute any remaining space to the first panel
                    if remaining > 0.01 && panel_count > 0 {
                        self.panel_heights[0] += remaining;
                    }
                } else {
                    // No existing height, distribute evenly
                    let per_panel = extra_space / panel_count as f32;
                    for i in 0..panel_count {
                        self.panel_heights[i] = per_panel.max(self.min_height);
                    }
                }
            }
        }

        println!("Panel heights after distribution: {:?}", self.panel_heights);
    }
}

// The body that collects panel functions
pub struct StackBody {
    panel_functions: Vec<Box<dyn FnOnce(&mut Ui)>>,
}

impl StackBody {
    /// Add a panel to the stack with the given content.
    pub fn add_panel<F>(&mut self, add_contents: F)
    where
        F: FnOnce(&mut Ui) + 'static,
    {
        // Box the function and store it for later execution
        self.panel_functions.push(Box::new(add_contents));
    }
}

impl Default for VerticalStack {
    fn default() -> Self {
        Self::new()
    }
}