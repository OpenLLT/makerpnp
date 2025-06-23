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
    last_available_height: f32,
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
            last_available_height: 0.0,
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

        println!("Stack body - panel count: {}, available height: {}", panel_count, available_height);

        // Ensure we have enough heights for all panels
        while self.panel_heights.len() < panel_count {
            self.panel_heights.push(self.default_panel_height);
        }

        // Truncate if we have too many heights
        if self.panel_heights.len() > panel_count {
            self.panel_heights.truncate(panel_count);
        }

        // Check if we need to redistribute heights
        let height_changed = (self.last_available_height - available_height).abs() > 1.0;
        let panel_count_changed = !self.initialized || panel_count != self.panel_heights.len();

        // Need to redistribute heights if:
        // 1. First initialization
        // 2. Panel count changed
        // 3. Available height changed significantly
        // 4. Drag just ended (reset distribution)
        let need_redistribution = !self.initialized ||
            panel_count_changed ||
            height_changed ||
            (self.drag_in_progress && !ui.input(|i| i.pointer.any_down()));

        // Reset drag state if no pointer is down
        if self.drag_in_progress && !ui.input(|i| i.pointer.any_down()) {
            self.drag_in_progress = false;
        }

        if need_redistribution {
            println!("Redistributing heights - initialized: {}, panel_count_changed: {}, height_changed: {}",
                     self.initialized, panel_count_changed, height_changed);
            self.distribute_panel_heights(panel_count, available_height);
            self.last_available_height = available_height;
        }

        // Reserve exact space for the stack to prevent scrollbars
        let total_height: f32 = self.panel_heights.iter().sum();
        let handle_height = 8.0;
        let handles_height = if panel_count > 1 { (panel_count - 1) as f32 * handle_height } else { 0.0 };
        let stack_height = total_height + handles_height;

        println!("Stack height: {} (panels: {}, handles: {})", stack_height, total_height, handles_height);

        // Use a frame to contain the entire stack and reserve exactly the right amount of space
        Frame::none()
            .fill(Color32::TRANSPARENT)
            .show(ui, |ui| {
                // Reserve exact height to prevent scrollbars
                ui.set_min_height(stack_height);
                ui.set_max_height(stack_height);

                // Now render all panels with the calculated heights
                self.render_panels(ui, body.panel_functions);
            });

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
                self.add_resize_handle(ui, idx - 1, handle_height);
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
                            ui.set_max_height(panel_height);
                            ui.expand_to_include_rect(ui.max_rect());
                            panel_fn(ui);
                        }
                    );
                });
        }
    }

    /// Add a resize handle between panels.
    fn add_resize_handle(&mut self, ui: &mut Ui, panel_idx: usize, handle_height: f32) {
        let handle_id = self.id_source.with("resize_handle").with(panel_idx);
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
            Stroke::new(2.0, Color32::WHITE) // More visible when hovered/dragged
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

                // Calculate new heights with constraints
                let new_current = (current_height + delta).max(self.min_height);
                let new_next = (next_height - delta).max(self.min_height);

                // Apply the new heights
                self.panel_heights[panel_idx] = new_current;
                self.panel_heights[panel_idx + 1] = new_next;

                println!("  After resize: panel {} height = {}, panel {} height = {}",
                         panel_idx, self.panel_heights[panel_idx],
                         panel_idx + 1, self.panel_heights[panel_idx + 1]);

                // After any resize, we need to ensure all panels fit in the available space
                // This will be done on the next frame when need_redistribution is checked
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

        // If first frame or total panel height is significantly different from available space
        if !self.initialized || (total_panel_height - available_for_panels).abs() > 1.0 {
            if !self.initialized || self.drag_in_progress == false {
                // First frame or after drag ended - distribute evenly
                let height_per_panel = (available_for_panels / panel_count as f32).max(self.min_height);
                for i in 0..panel_count {
                    self.panel_heights[i] = height_per_panel;
                }
                println!("Distributing evenly: {} per panel", height_per_panel);
            } else if total_panel_height > available_for_panels {
                // Need to shrink panels
                self.scale_panels_to_fit(panel_count, available_for_panels);
            } else {
                // Need to expand panels
                self.scale_panels_to_fit(panel_count, available_for_panels);
            }
        }

        println!("Panel heights after distribution: {:?}", self.panel_heights);
    }

    // Helper to scale panels proportionally to fit available space
    fn scale_panels_to_fit(&mut self, panel_count: usize, available_height: f32) {
        // Get total of current heights
        let total_height: f32 = self.panel_heights.iter().sum();

        if total_height <= 0.0 {
            // No valid height to scale from, distribute evenly
            let height_per_panel = (available_height / panel_count as f32).max(self.min_height);
            for i in 0..panel_count {
                self.panel_heights[i] = height_per_panel;
            }
            return;
        }

        // Calculate how much space panels would take if we respect min_height
        let min_total: f32 = (0..panel_count).map(|_| self.min_height).sum();

        if available_height <= min_total {
            // Can't fit even with minimum heights, set all to minimum
            for i in 0..panel_count {
                self.panel_heights[i] = self.min_height;
            }
            return;
        }

        // Scale factor to fit available space
        let scale_factor = available_height / total_height;

        // First pass: scale all panels proportionally
        for i in 0..panel_count {
            self.panel_heights[i] = (self.panel_heights[i] * scale_factor).max(self.min_height);
        }

        // Second pass: check if we need to adjust to exactly match available height
        let new_total: f32 = self.panel_heights.iter().sum();
        let remaining = available_height - new_total;

        if remaining.abs() > 0.1 {
            // Find panels that can be adjusted (not at minimum height)
            let adjustable_panels: Vec<usize> = (0..panel_count)
                .filter(|&i| self.panel_heights[i] > self.min_height + 0.1)
                .collect();

            if !adjustable_panels.is_empty() {
                // Distribute remaining space evenly among adjustable panels
                let per_panel_adjustment = remaining / adjustable_panels.len() as f32;
                for &idx in &adjustable_panels {
                    self.panel_heights[idx] += per_panel_adjustment;
                    // Ensure we don't go below minimum
                    self.panel_heights[idx] = self.panel_heights[idx].max(self.min_height);
                }
            } else if remaining > 0.0 {
                // If all panels are at minimum and we have extra space,
                // just add it to the first panel
                if panel_count > 0 {
                    self.panel_heights[0] += remaining;
                }
            }
        }
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