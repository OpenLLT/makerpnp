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
    active_drag_handle: Option<usize>,
    drag_start_heights: Vec<f32>,
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
            active_drag_handle: None,
            drag_start_heights: Vec::new(),
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
        // Measure the available space - very important for preventing scrollbars
        let available_rect = ui.available_rect_before_wrap();
        let available_height = available_rect.height();

        // Create the stack body to collect panel functions
        let mut body = StackBody {
            panel_functions: Vec::new(),
        };

        // Collect panel functions (this doesn't render anything yet)
        add_contents(&mut body);

        // Get panel count
        let panel_count = body.panel_functions.len();

        println!("Stack body - panel count: {}, available height: {}", panel_count, available_height);

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
            // Drag just released
            println!("Drag released");
            self.drag_in_progress = false;
            self.active_drag_handle = None;
            self.drag_start_heights.clear();
        }

        // Check if we need a full redistribution
        let height_changed = (self.last_available_height - available_height).abs() > 1.0;
        let panel_count_changed = !self.initialized || panel_count != self.panel_heights.len();
        let need_redistribution = (!self.initialized || panel_count_changed || height_changed) &&
            !self.drag_in_progress;

        if need_redistribution {
            println!("Redistributing heights - initialized: {}, panel_count_changed: {}, height_changed: {}",
                     self.initialized, panel_count_changed, height_changed);

            self.distribute_panel_heights(panel_count, available_height);
        } else if !self.drag_in_progress {
            // Just ensure the total height matches available space
            self.ensure_exact_total_height(available_height);
        }

        self.last_available_height = available_height;

        // Calculate exact space needed for the entire stack
        let handle_height = 8.0;
        let handles_height = (panel_count - 1) as f32 * handle_height;

        // Reserve the exact available rect
        let response = ui.allocate_rect(available_rect, Sense::hover());

        // Now render all panels with their calculated heights
        let mut current_pos = available_rect.min;

        // Render each panel with its calculated height
        for (idx, panel_fn) in body.panel_functions.into_iter().enumerate() {
            let panel_height = self.panel_heights[idx].max(self.min_height);

            // Determine panel rect
            let panel_rect = Rect::from_min_size(
                current_pos,
                Vec2::new(available_rect.width(), panel_height),
            );

            println!("Rendering panel {}: height = {}, rect = {:?}", idx, panel_height, panel_rect);

            // Create a frame with a border for the panel
            Frame::default()
                .stroke(Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color))
                .show(ui, |ui| {
                    // Set a clip rect to ensure content doesn't overflow
                    ui.set_clip_rect(panel_rect);

                    // Allocate the exact panel rect
                    ui.allocate_ui_at_rect(panel_rect, |ui| {
                        // Make sure the UI uses the full allocated space
                        ui.expand_to_include_rect(panel_rect);

                        // Call the panel content function
                        panel_fn(ui);
                    });
                });

            // Move the current position down
            current_pos.y += panel_height;

            // Add a resize handle after each panel (except the last one)
            if idx < panel_count - 1 {
                // Create the handle rect
                let handle_rect = Rect::from_min_size(
                    current_pos,
                    Vec2::new(available_rect.width(), handle_height),
                );

                self.add_resize_handle(ui, idx, handle_rect);

                // Move the position down past the handle
                current_pos.y += handle_height;
            }
        }

        // Mark as initialized
        self.initialized = true;
    }

    /// Add a resize handle between panels at the exact specified position.
    fn add_resize_handle(&mut self, ui: &mut Ui, panel_idx: usize, handle_rect: Rect) {
        let handle_id = self.id_source.with("resize_handle").with(panel_idx);

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

        // Handle dragging to resize - ONLY affecting the two adjacent panels
        if handle_response.dragged() {
            // If this is the first drag or a different handle than before
            if !self.drag_in_progress || self.active_drag_handle != Some(panel_idx) {
                // Start a new drag session
                println!("  Starting new drag for handle {}", panel_idx);
                self.drag_in_progress = true;
                self.active_drag_handle = Some(panel_idx);

                // Save ALL panel heights at drag start
                self.drag_start_heights = self.panel_heights.clone();
            }

            let delta = handle_response.drag_delta().y;

            // Only process if there's an actual delta
            if delta != 0.0 {
                println!("  Drag delta: {}", delta);

                // Get the original heights from when the drag started
                let original_heights = &self.drag_start_heights;
                let original_top = original_heights[panel_idx];
                let original_bottom = original_heights[panel_idx + 1];
                let original_sum = original_top + original_bottom;

                // Calculate accumulated delta from the original positions
                let top_panel_height = self.panel_heights[panel_idx];
                let current_delta = top_panel_height - original_top;
                let new_delta = current_delta + delta;

                // Constrain the delta to respect minimum heights
                let max_delta_up = original_top - self.min_height;
                let max_delta_down = original_bottom - self.min_height;
                let constrained_delta = new_delta.max(-max_delta_up).min(max_delta_down);

                // Calculate new heights while preserving the sum exactly
                let new_top = original_top + constrained_delta;
                let new_bottom = original_sum - new_top; // Ensure exact sum preservation

                println!("  Original: top={}, bottom={}, sum={}", original_top, original_bottom, original_sum);
                println!("  New: top={}, bottom={}, sum={}", new_top, new_bottom, new_top + new_bottom);

                // Apply the new heights ONLY to the two affected panels
                self.panel_heights[panel_idx] = new_top;
                self.panel_heights[panel_idx + 1] = new_bottom;

                // CRITICAL: Do not modify any other panel heights!
            }
        }
    }

    // Helper method to distribute heights to fill available space
    fn distribute_panel_heights(&mut self, panel_count: usize, available_height: f32) {
        if panel_count == 0 {
            return;
        }

        // Calculate space needed for resize handles
        let handle_height = 8.0;
        let handles_height = (panel_count - 1) as f32 * handle_height;

        // Space available for actual panels
        let available_for_panels = available_height - handles_height;

        // If first initialization, distribute evenly
        if !self.initialized {
            let height_per_panel = (available_for_panels / panel_count as f32).max(self.min_height);
            for i in 0..panel_count {
                self.panel_heights[i] = height_per_panel;
            }
            println!("Initial distribution: {} per panel", height_per_panel);
            return;
        }

        // Get the total height currently used by panels
        let total_panel_height: f32 = self.panel_heights.iter().sum();

        println!("Available height: {}, Handles: {}, Total panel height: {}",
                 available_height, handles_height, total_panel_height);

        // If total panel height is significantly different from available space
        if (total_panel_height - available_for_panels).abs() > 1.0 {
            // Scale all panels proportionally to fit the available space
            self.scale_panels_to_fit(available_for_panels);
        }

        println!("Panel heights after distribution: {:?}", self.panel_heights);
    }

    // Ensure the total panel height exactly matches the target height
    fn ensure_exact_total_height(&mut self, available_height: f32) {
        if self.panel_heights.is_empty() {
            return;
        }

        // Calculate space needed for resize handles
        let handle_height = 8.0;
        let handles_height = (self.panel_heights.len() - 1) as f32 * handle_height;

        // Space available for actual panels
        let available_for_panels = available_height - handles_height;

        // Get current total height
        let total_height: f32 = self.panel_heights.iter().sum();

        // If the difference is significant, scale to match exactly
        if (total_height - available_for_panels).abs() > 0.1 {
            let scale = available_for_panels / total_height;

            // Scale all heights proportionally
            for height in &mut self.panel_heights {
                *height *= scale;
                *height = height.max(self.min_height);
            }

            // Final adjustment for precision
            let new_total: f32 = self.panel_heights.iter().sum();
            let diff = available_for_panels - new_total;

            if diff.abs() > 0.1 {
                // Find a panel that can be adjusted
                for height in &mut self.panel_heights {
                    if *height > self.min_height + diff.abs() {
                        *height += diff;
                        break;
                    }
                }
            }
        }
    }

    // Helper to scale panels proportionally to fit available space
    fn scale_panels_to_fit(&mut self, available_height: f32) {
        // Get total of current heights
        let total_height: f32 = self.panel_heights.iter().sum();

        if total_height <= 0.0 {
            return;
        }

        // Scale factor to fit available space
        let scale_factor = available_height / total_height;

        // Apply scale factor to all panels
        for height in &mut self.panel_heights {
            *height = (*height * scale_factor).max(self.min_height);
        }

        // Ensure we match the exact available height
        let new_total: f32 = self.panel_heights.iter().sum();
        let diff = available_height - new_total;

        if diff.abs() > 0.1 {
            // Find a panel that can be adjusted
            for height in &mut self.panel_heights {
                if *height > self.min_height + diff.abs() {
                    *height += diff;
                    break;
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