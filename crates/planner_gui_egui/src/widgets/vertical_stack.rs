use eframe::epaint::Color32;
use egui::{Frame, Id, Rect, Sense, Stroke, Ui, Vec2};

/// A component that displays multiple panels stacked vertically with resize handles.
pub struct VerticalStack {
    min_height: f32,
    id_source: Id,
    panel_heights: Vec<f32>,
    default_panel_height: f32,
    panel_count: usize,
    drag_in_progress: bool,
    first_frame: bool,
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
            first_frame: true,
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
            first_frame: self.first_frame,
        };

        // Render the contents
        add_contents(&mut body);

        // Update first_frame flag after first render
        self.first_frame = false;

        // Ensure heights after we know the panel count
        // Only normalize heights if no drag is in progress
        if !self.drag_in_progress {
            self.distribute_panel_heights(available_height);
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

        // Calculate space needed for resize handles
        let handle_height = 8.0;
        let handles_height = if self.panel_count > 1 {
            (self.panel_count - 1) as f32 * handle_height
        } else {
            0.0
        };

        // Get the total height currently used by panels
        let total_panel_height: f32 = self.panel_heights.iter().take(self.panel_count).sum();
        let available_for_panels = available_height - handles_height;

        println!("Available height: {}, Handles: {}, Total panel height: {}",
                 available_height, handles_height, total_panel_height);

        // If we need to scale panels (too big or too small)
        if (total_panel_height - available_for_panels).abs() > 1.0 {
            let extra_space = available_for_panels - total_panel_height;

            // Distribute extra space proportionally among panels
            if extra_space > 0.0 {
                // Expand panels proportionally
                let sum_weights: f32 = self.panel_heights.iter().take(self.panel_count)
                    .map(|h| h - self.min_height).sum();

                if sum_weights > 0.0 {
                    // Distribute proportionally based on existing size above minimum
                    let mut remaining = extra_space;
                    for i in 0..self.panel_count {
                        let weight = (self.panel_heights[i] - self.min_height) / sum_weights;
                        let addition = extra_space * weight;
                        self.panel_heights[i] += addition;
                        remaining -= addition;
                    }

                    // Distribute any remaining space evenly
                    if remaining > 0.0 {
                        let per_panel = remaining / self.panel_count as f32;
                        for i in 0..self.panel_count {
                            self.panel_heights[i] += per_panel;
                        }
                    }
                } else {
                    // All panels at minimum, distribute evenly
                    let per_panel = extra_space / self.panel_count as f32;
                    for i in 0..self.panel_count {
                        self.panel_heights[i] += per_panel;
                    }
                }
            } else if total_panel_height > available_for_panels {
                // Shrink panels but respect minimum height
                // First, calculate how much space we need to free up
                let mut to_free = total_panel_height - available_for_panels;

                // Calculate how much space we can free up before hitting minimum heights
                let freeable_space: f32 = self.panel_heights.iter().take(self.panel_count)
                    .map(|h| (h - self.min_height).max(0.0)).sum();

                if freeable_space >= to_free {
                    // We can free up enough space while respecting minimum heights
                    // Shrink proportionally to current size above minimum
                    for i in 0..self.panel_count {
                        let freeable = (self.panel_heights[i] - self.min_height).max(0.0);
                        if freeable > 0.0 {
                            let reduction = to_free * (freeable / freeable_space);
                            self.panel_heights[i] -= reduction;
                            to_free -= reduction;
                        }
                    }
                } else {
                    // Can't free enough space while respecting minimums
                    // Set all to minimum and accept scrolling
                    for i in 0..self.panel_count {
                        self.panel_heights[i] = self.min_height;
                    }
                }
            }
        }

        println!("Panel heights after distribution: {:?}",
                 &self.panel_heights[0..self.panel_count.min(self.panel_heights.len())]);
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
    first_frame: bool,
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
            // On first frame, distribute height more evenly based on panel count seen so far
            if self.first_frame {
                // For the first frame, calculate a reasonable height that won't cause scrolling
                let handle_height = 8.0;
                let estimated_panels = panel_idx + 1;  // Assume at least this many panels
                let handles_space = (estimated_panels - 1).max(0) as f32 * handle_height;
                let available_space = self.available_height - handles_space;
                let reasonable_height = (available_space / estimated_panels as f32).max(self.min_height);
                self.panel_heights.push(reasonable_height);
            } else {
                self.panel_heights.push(self.default_panel_height);
            }
        }

        // Determine if this is the last panel
        let is_last_panel = panel_idx == *self.panel_count;

        // Calculate remaining space for the last panel to fill the available height
        let panel_height = if is_last_panel && self.panel_heights.len() > 1 {
            // For the last panel, calculate remaining space
            let used_space: f32 = self.panel_heights.iter().take(panel_idx).sum();
            let handle_space = panel_idx as f32 * 8.0; // 8.0 is handle height
            let remaining = (self.available_height - used_space - handle_space).max(self.min_height);
            self.panel_heights[panel_idx] = remaining;
            remaining
        } else {
            self.panel_heights[panel_idx].max(self.min_height)
        };

        println!("Panel {}: height = {}", panel_idx, panel_height);

        // Add a resize handle before the panel (except for the first panel)
        if panel_idx > 0 {
            self.add_resize_handle(panel_idx - 1);
        }

        // Create a frame with a border for the panel that fills the available space
        Frame::default()
            .stroke(Stroke::new(1.0, self.ui.visuals().widgets.noninteractive.bg_stroke.color))
            .show(self.ui, |ui| {
                let available_width = ui.available_width();

                // Here's the key fix - use the allocated height exactly as specified
                ui.allocate_ui_with_layout(
                    Vec2::new(available_width, panel_height),
                    egui::Layout::top_down(egui::Align::LEFT).with_cross_justify(true),
                    |ui| {
                        // Use the full allocated space
                        ui.set_min_height(panel_height);
                        ui.expand_to_include_rect(ui.max_rect());
                        add_contents(ui);
                    }
                );
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

        // Handle dragging to resize with constraints to prevent scrollbars
        if handle_response.dragged() {
            *self.drag_in_progress = true;
            let delta = handle_response.drag_delta().y;
            println!("  Drag delta: {}", delta);

            // Only process if there's an actual delta
            if delta != 0.0 {
                // Calculate new heights while respecting constraints
                let current_height = self.panel_heights[panel_idx];
                let next_height = self.panel_heights[panel_idx + 1];

                println!("  Before resize: panel {} height = {}, panel {} height = {}",
                         panel_idx, current_height, panel_idx + 1, next_height);

                // Check if we're at the last handle (affecting the last panel)
                let is_last_handle = panel_idx + 2 == *self.panel_count;

                // Calculate maximum allowed heights to prevent scrollbars
                let max_total = if is_last_handle {
                    // For last handle, ensure we don't exceed available height
                    let other_panels_height: f32 = self.panel_heights.iter()
                        .take(*self.panel_count)
                        .enumerate()
                        .filter(|(i, _)| *i != panel_idx && *i != panel_idx + 1)
                        .map(|(_, h)| *h)
                        .sum();

                    let handles_height = (*self.panel_count - 1) as f32 * handle_height;
                    self.available_height - other_panels_height - handles_height
                } else {
                    // For other handles, we don't need special constraint
                    current_height + next_height
                };

                // Calculate new heights
                let new_current = (current_height + delta).max(self.min_height);
                let actual_delta = new_current - current_height;
                let new_next = (next_height - actual_delta).max(self.min_height);

                // Check if we hit the minimum height constraint on the next panel
                if new_next <= self.min_height && next_height > self.min_height {
                    // We can only take what's available above min_height
                    let available_delta = next_height - self.min_height;
                    self.panel_heights[panel_idx] = current_height + available_delta;
                    self.panel_heights[panel_idx + 1] = self.min_height;
                    println!("  Limited by min height: panel {} height = {}, panel {} height = {}",
                             panel_idx, self.panel_heights[panel_idx],
                             panel_idx + 1, self.panel_heights[panel_idx + 1]);
                } else if new_current <= self.min_height && current_height > self.min_height {
                    // We can only reduce current panel to min_height
                    let available_delta = current_height - self.min_height;
                    self.panel_heights[panel_idx] = self.min_height;
                    self.panel_heights[panel_idx + 1] = next_height + available_delta;
                    println!("  Limited by min height: panel {} height = {}, panel {} height = {}",
                             panel_idx, self.panel_heights[panel_idx],
                             panel_idx + 1, self.panel_heights[panel_idx + 1]);
                } else if new_current + new_next > max_total {
                    // Limit by max_total to prevent scrollbars
                    let excess = new_current + new_next - max_total;

                    // Distribute the excess reduction proportionally
                    let total = new_current + new_next;
                    let current_ratio = new_current / total;
                    let next_ratio = new_next / total;

                    let current_reduction = excess * current_ratio;
                    let next_reduction = excess * next_ratio;

                    self.panel_heights[panel_idx] = (new_current - current_reduction).max(self.min_height);
                    self.panel_heights[panel_idx + 1] = (new_next - next_reduction).max(self.min_height);

                    println!("  Limited by max_total: panel {} height = {}, panel {} height = {}",
                             panel_idx, self.panel_heights[panel_idx],
                             panel_idx + 1, self.panel_heights[panel_idx + 1]);
                } else {
                    // Normal case - we can apply the full delta
                    self.panel_heights[panel_idx] = new_current;
                    self.panel_heights[panel_idx + 1] = new_next;
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