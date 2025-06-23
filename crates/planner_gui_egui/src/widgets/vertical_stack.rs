use eframe::epaint::Color32;
use egui::{Frame, Id, Rect, Sense, Stroke, Ui, Vec2};

/// A component that displays multiple panels stacked vertically with resize handles.
pub struct VerticalStack {
    min_height: f32,
    id_source: Id,
    panel_heights: Vec<f32>,
    default_panel_height: f32,
    panel_count: usize,  // Track the number of panels
    drag_in_progress: bool,
}

impl VerticalStack {
    /// Creates a new empty vertical stack.
    pub fn new() -> Self {
        Self {
            min_height: 50.0,
            id_source: Id::new("vertical_stack"),
            panel_heights: Vec::new(),
            default_panel_height: 100.0,
            panel_count: 0,
            drag_in_progress: false,
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
        let available_height = ui.available_height();

        // Reset panel count before adding new panels
        self.panel_count = 0;
        self.drag_in_progress = false;

        // Pre-allocate evenly distributed heights if needed
        if self.panel_heights.is_empty() {
            // Start with a reasonable default (3 panels)
            let estimated_panel_count = 3;
            let handle_height = 8.0;
            let handles_height = (estimated_panel_count - 1) as f32 * handle_height;
            let available_for_panels = available_height - handles_height;

            // Distribute evenly
            let panel_height = (available_for_panels / estimated_panel_count as f32).max(self.min_height);
            self.panel_heights = vec![panel_height; estimated_panel_count];
        }

        // Create the body for rendering
        let mut body = StackBody {
            ui,
            min_height: self.min_height,
            id_source: self.id_source,
            panel_heights: &mut self.panel_heights,
            default_panel_height: self.default_panel_height,
            panel_count: &mut self.panel_count,
            drag_in_progress: &mut self.drag_in_progress,
            available_height,
        };

        // Render the contents
        add_contents(&mut body);

        // After rendering, if we have too many or too few heights, adjust the vector
        if self.panel_heights.len() < self.panel_count {
            // Add heights for new panels
            let default_height = self.default_panel_height;
            self.panel_heights.resize(self.panel_count, default_height);
        }

        // Only normalize heights if no drag is in progress
        if !self.drag_in_progress {
            self.distribute_panel_heights(available_height);
        } else {
            // When dragging, ensure we don't exceed available height
            self.constrain_to_available_height(available_height);
        }
    }
    
    // New method to constrain total height during dragging
    fn constrain_to_available_height(&mut self, available_height: f32) {
        if self.panel_count == 0 {
            return;
        }

        // Calculate space needed for resize handles
        let handle_height = 8.0;
        let handles_height = if self.panel_count > 0 {
            (self.panel_count - 1) as f32 * handle_height
        } else {
            0.0
        };

        // Get the total height currently used by panels
        let total_panel_height: f32 = self.panel_heights.iter().take(self.panel_count).sum();
        let available_for_panels = available_height - handles_height;

        // If we exceed available height, scale down proportionally
        if total_panel_height > available_for_panels {
            // We need to scale down the panels to fit
            let scale_factor = available_for_panels / total_panel_height;

            // Scale all panels, respecting minimum height
            let mut scaled_heights = Vec::with_capacity(self.panel_count);

            for i in 0..self.panel_count {
                scaled_heights.push((self.panel_heights[i] * scale_factor).max(self.min_height));
            }

            // If scaled heights still exceed available space, we need to further adjust
            let scaled_total: f32 = scaled_heights.iter().sum();
            if scaled_total > available_for_panels {
                // We need to reduce further, prioritizing larger panels
                let excess = scaled_total - available_for_panels;

                // Sort panels by size (largest first) with their indices
                let mut panels_with_indices: Vec<(usize, f32)> = scaled_heights
                    .iter()
                    .enumerate()
                    .map(|(idx, &height)| (idx, height))
                    .collect();

                panels_with_indices.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

                // Reduce larger panels first until we've eliminated the excess
                let mut remaining_excess = excess;
                for (idx, height) in panels_with_indices {
                    if height > self.min_height && remaining_excess > 0.0 {
                        let reducible = height - self.min_height;
                        let reduction = remaining_excess.min(reducible);

                        scaled_heights[idx] -= reduction;
                        remaining_excess -= reduction;

                        if remaining_excess <= 0.0 {
                            break;
                        }
                    }
                }
            }

            // Update panel heights with constrained values
            for i in 0..self.panel_count {
                self.panel_heights[i] = scaled_heights[i];
            }
        }
    }

    // Helper method to distribute heights to fill available space
    fn distribute_panel_heights(&mut self, available_height: f32) {
        if self.panel_count == 0 {
            return;
        }

        // Make sure we have heights for all panels
        while self.panel_heights.len() < self.panel_count {
            self.panel_heights.push(self.default_panel_height);
        }

        // Trim any excess heights
        if self.panel_heights.len() > self.panel_count {
            self.panel_heights.truncate(self.panel_count);
        }

        // Calculate space needed for resize handles
        let handle_height = 8.0;
        let handles_height = if self.panel_count > 1 {
            (self.panel_count - 1) as f32 * handle_height
        } else {
            0.0
        };

        // Get the total height currently used by panels
        let total_panel_height: f32 = self.panel_heights.iter().sum();
        let available_for_panels = available_height - handles_height;

        println!("Available height: {}, Handles: {}, Total panel height: {}",
                 available_height, handles_height, total_panel_height);

        // If total height is significantly different from available space
        if (total_panel_height - available_for_panels).abs() > 1.0 {
            // Calculate the scaling factor
            let scale_factor = available_for_panels / total_panel_height;

            // Apply scaling to all panels, respecting minimum height
            for i in 0..self.panel_count {
                self.panel_heights[i] = (self.panel_heights[i] * scale_factor).max(self.min_height);
            }

            // After scaling, check if we still fit
            let scaled_total: f32 = self.panel_heights.iter().sum();

            if (scaled_total - available_for_panels).abs() > 1.0 {
                // We're still off - just distribute evenly
                let even_height = (available_for_panels / self.panel_count as f32).max(self.min_height);

                // If even distribution exceeds available space, reduce it
                if even_height * self.panel_count as f32 > available_for_panels {
                    // Calculate how much we're over
                    let excess = even_height * self.panel_count as f32 - available_for_panels;
                    let per_panel_reduction = excess / self.panel_count as f32;

                    // Apply reduction to each panel
                    for i in 0..self.panel_count {
                        self.panel_heights[i] = (even_height - per_panel_reduction).max(self.min_height);
                    }
                } else {
                    // Just use even distribution
                    for i in 0..self.panel_count {
                        self.panel_heights[i] = even_height;
                    }
                }
            }
        }

        println!("Panel heights after distribution: {:?}", self.panel_heights);
    }
}

pub struct StackBody<'a> {
    ui: &'a mut Ui,
    min_height: f32,
    id_source: Id,
    panel_heights: &'a mut Vec<f32>,
    default_panel_height: f32,
    panel_count: &'a mut usize,
    drag_in_progress: &'a mut bool,
    available_height: f32,
}

impl<'a> StackBody<'a> {
    /// Add a panel to the stack with the given content.
    pub fn add_panel<F>(&mut self, add_contents: F)
    where
        F: FnOnce(&mut Ui),
    {
        let panel_idx = *self.panel_count;

        // Ensure we have a height for this panel
        if self.panel_heights.len() <= panel_idx {
            self.panel_heights.push(self.default_panel_height);
        }

        // Use the current height from panel_heights
        let panel_height = self.panel_heights[panel_idx].max(self.min_height);
        println!("Panel {}: height = {}", panel_idx, panel_height);

        // Add a resize handle before the panel (except for the first panel)
        if panel_idx > 0 {
            self.add_resize_handle(panel_idx - 1);
        }

        // Create a frame with a border for the panel
        let frame = Frame::default()
            .stroke(Stroke::new(1.0, self.ui.visuals().widgets.noninteractive.bg_stroke.color));

        // Force the panel to use exactly the calculated height
        let rect = self.ui.available_rect_before_wrap();
        let panel_rect = Rect::from_min_size(
            rect.min,
            Vec2::new(rect.width(), panel_height),
        );

        // Allocate the space for this panel
        self.ui.allocate_rect(panel_rect, Sense::hover());

        // Show the frame in the allocated space
        frame.show(self.ui, |ui| {
            // Use the full width but constrain to the exact panel height
            ui.set_max_height(panel_height);
            ui.set_min_height(panel_height);

            // Add the contents
            add_contents(ui);
        });

        // Increment the panel count
        *self.panel_count += 1;
    }

    /// Add a resize handle between panels.
    fn add_resize_handle(&mut self, panel_idx: usize) {
        let handle_id = self.id_source.with("resize_handle").with(panel_idx);
        let handle_height = 8.0;
        let handle_rect = Rect::from_min_size(
            self.ui.cursor().min,
            Vec2::new(self.ui.available_width(), handle_height),
        );

        println!("Resize handle for panel {}: rect = {:?}", panel_idx, handle_rect);

        // Make sure we have the next panel's index available
        if panel_idx >= self.panel_heights.len() || panel_idx + 1 >= self.panel_heights.len() {
            println!("  Error: Panel index out of bounds");
            self.ui.allocate_rect(handle_rect, Sense::hover());
            return;
        }

        // Use drag sense explicitly to ensure dragging works
        let handle_response = self.ui.interact(handle_rect, handle_id, Sense::drag());

        println!("  Handle response: dragged = {}, hovered = {}",
                 handle_response.dragged(), handle_response.hovered());

        // Draw the handle
        let handle_visuals = self.ui.style().noninteractive();
        let handle_stroke = if handle_response.hovered() || handle_response.dragged() {
            Stroke::new(2.0, Color32::WHITE) // Make it more visible when hovered or dragged
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

        // Add some grip indicators
        for i in 0..5 {
            let x = left + (right - left) * (0.3 + 0.1 * i as f32);
            let y_top = center_y - 2.0;
            let y_bottom = center_y + 2.0;
            self.ui.painter().line_segment(
                [egui::Pos2::new(x, y_top), egui::Pos2::new(x, y_bottom)],
                handle_stroke,
            );
        }

        // Handle dragging to resize
        if handle_response.dragged() {
            *self.drag_in_progress = true;
            let delta = handle_response.drag_delta().y;
            println!("  Drag delta: {}", delta);

            // Only process if there's an actual delta
            if delta != 0.0 {
                // Calculate new heights while respecting min_height
                let current_height = self.panel_heights[panel_idx];
                let next_height = self.panel_heights[panel_idx + 1];

                println!("  Before resize: panel {} height = {}, panel {} height = {}",
                         panel_idx, current_height, panel_idx + 1, next_height);

                // Calculate new heights
                let new_current = (current_height + delta).max(self.min_height);
                let actual_delta = new_current - current_height;
                let new_next = (next_height - actual_delta).max(self.min_height);

                // Check if we hit the min_height constraint on the next panel
                if new_next == self.min_height && next_height > self.min_height {
                    // We can only take what's available above min_height
                    let available_delta = next_height - self.min_height;
                    self.panel_heights[panel_idx] = current_height + available_delta;
                    self.panel_heights[panel_idx + 1] = self.min_height;
                    println!("  Limited by min height: panel {} height = {}, panel {} height = {}",
                             panel_idx, self.panel_heights[panel_idx],
                             panel_idx + 1, self.panel_heights[panel_idx + 1]);
                } else if new_current == self.min_height && current_height > self.min_height {
                    // We can only reduce current panel to min_height
                    let available_delta = current_height - self.min_height;
                    self.panel_heights[panel_idx] = self.min_height;
                    self.panel_heights[panel_idx + 1] = next_height + available_delta;
                    println!("  Limited by min height: panel {} height = {}, panel {} height = {}",
                             panel_idx, self.panel_heights[panel_idx],
                             panel_idx + 1, self.panel_heights[panel_idx + 1]);
                } else {
                    // Normal case - we can apply the full delta
                    self.panel_heights[panel_idx] = new_current;
                    self.panel_heights[panel_idx + 1] = next_height - actual_delta;
                    println!("  After resize: panel {} height = {}, panel {} height = {}",
                             panel_idx, self.panel_heights[panel_idx],
                             panel_idx + 1, self.panel_heights[panel_idx + 1]);
                }
            }
        }

        // Make sure we allocate the rect with drag sense
        self.ui.allocate_rect(handle_rect, Sense::drag());
    }
}


impl Default for VerticalStack {
    fn default() -> Self {
        Self::new()
    }
}