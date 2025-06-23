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
            panel_count: 0,
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
        let available_height = ui.available_height();

        // Reset panel count before adding new panels
        self.panel_count = 0;
        self.drag_in_progress = false;

        // If we're not initialized, do a preliminary pass to count panels without rendering
        if !self.initialized {
            // Create a temporary invisible UI to count panels
            let temp_rect = ui.available_rect_before_wrap();
            let mut temp_ui = ui.child_ui(temp_rect, ui.layout().clone(), None);

            // Set clip rect to zero to ensure nothing is drawn
            temp_ui.set_clip_rect(Rect::NOTHING);

            // Create a temporary body to count panels
            let mut temp_body = StackBody {
                ui: &mut temp_ui,
                min_height: self.min_height,
                id_source: self.id_source,
                panel_heights: &mut Vec::new(), // Temporary vector
                default_panel_height: self.default_panel_height,
                panel_count: &mut self.panel_count,
                drag_in_progress: &mut false,
                available_height,
                initialized: false,
                measuring_only: true,
            };

            // Count panels without rendering
            add_contents(&mut temp_body);

            // Now we know the panel count, prepare heights
            self.initialize_panel_heights(available_height);

            // Reset panel count for actual rendering
            self.panel_count = 0;
        }

        // Create the body for actual rendering
        let mut body = StackBody {
            ui,
            min_height: self.min_height,
            id_source: self.id_source,
            panel_heights: &mut self.panel_heights,
            default_panel_height: self.default_panel_height,
            panel_count: &mut self.panel_count,
            drag_in_progress: &mut self.drag_in_progress,
            available_height,
            initialized: self.initialized,
            measuring_only: false,
        };

        // Render the contents
        add_contents(&mut body);

        // Only redistribute heights if not dragging
        if !self.drag_in_progress {
            self.distribute_panel_heights(available_height);
        }

        // Mark as initialized after first frame
        self.initialized = true;
    }

    // Initialize panel heights for the first time
    fn initialize_panel_heights(&mut self, available_height: f32) {
        if self.panel_count == 0 {
            return;
        }

        // Clear any existing heights
        self.panel_heights.clear();

        // Calculate space needed for resize handles
        let handle_height = 8.0;
        let handles_height = if self.panel_count > 1 {
            (self.panel_count - 1) as f32 * handle_height
        } else {
            0.0
        };

        // Calculate even distribution of available space
        let available_for_panels = available_height - handles_height;
        let height_per_panel = (available_for_panels / self.panel_count as f32).max(self.min_height);

        // Set all panels to same height initially
        for _ in 0..self.panel_count {
            self.panel_heights.push(height_per_panel);
        }

        // If using minimum heights exceeds available space, accept scrolling
        let total_height: f32 = self.panel_heights.iter().sum::<f32>() + handles_height;
        if total_height > available_height {
            // We can't make it fit, but at least ensure we're using minimum heights
            for i in 0..self.panel_heights.len() {
                self.panel_heights[i] = self.min_height;
            }
        }
    }

    // Helper method to distribute heights to fill available space
    fn distribute_panel_heights(&mut self, available_height: f32) {
        if self.panel_count == 0 {
            return;
        }

        // Make sure we have the right number of panel heights
        if self.panel_heights.len() != self.panel_count {
            self.initialize_panel_heights(available_height);
            return;
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

        // If we need to scale panels (too big or too small)
        if (total_panel_height - available_for_panels).abs() > 1.0 {
            if total_panel_height > available_for_panels {
                // Need to shrink panels
                let excess = total_panel_height - available_for_panels;

                // Calculate how much space we can free up before hitting minimum heights
                let freeable_space: f32 = self.panel_heights.iter()
                    .map(|h| (h - self.min_height).max(0.0))
                    .sum();

                if freeable_space >= excess {
                    // We can free up enough space while respecting minimum heights
                    let mut remaining_excess = excess;

                    // Proportionally reduce panels based on height above minimum
                    for i in 0..self.panel_count {
                        let freeable = (self.panel_heights[i] - self.min_height).max(0.0);
                        if freeable > 0.0 {
                            let reduction = excess * (freeable / freeable_space);
                            self.panel_heights[i] -= reduction;
                            remaining_excess -= reduction;
                        }
                    }

                    // If there's any rounding error, apply to first panel that can take it
                    if remaining_excess > 0.01 {
                        for i in 0..self.panel_count {
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
                    for i in 0..self.panel_count {
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
                    for i in 0..self.panel_count {
                        let ratio = self.panel_heights[i] / total_current;
                        let addition = extra_space * ratio;
                        self.panel_heights[i] += addition;
                        remaining -= addition;
                    }

                    // Distribute any remaining space to the first panel
                    if remaining > 0.01 && self.panel_count > 0 {
                        self.panel_heights[0] += remaining;
                    }
                } else {
                    // No existing height, distribute evenly
                    let per_panel = extra_space / self.panel_count as f32;
                    for i in 0..self.panel_count {
                        self.panel_heights[i] = per_panel.max(self.min_height);
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
    initialized: bool,
    measuring_only: bool,
}

impl<'a> StackBody<'a> {
    /// Add a panel to the stack with the given content.
    pub fn add_panel<F>(&mut self, add_contents: F)
    where
        F: FnOnce(&mut Ui),
    {
        let panel_idx = *self.panel_count;

        // If we're just measuring, increment count and return
        if self.measuring_only {
            *self.panel_count += 1;
            return;
        }

        // Determine panel height
        let panel_height = if panel_idx < self.panel_heights.len() {
            self.panel_heights[panel_idx].max(self.min_height)
        } else {
            // This shouldn't happen normally but as a fallback
            self.default_panel_height.max(self.min_height)
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

                // Use the allocated height exactly as specified
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

        // Handle dragging to resize
        if handle_response.dragged() {
            *self.drag_in_progress = true;
            let delta = handle_response.drag_delta().y;

            // Only process if there's an actual delta
            if delta != 0.0 {
                // Calculate constraints
                let handle_count = (*self.panel_count - 1).max(0) as f32;
                let handles_height = handle_count * handle_height;

                // Calculate sum of all panel heights except the two we're adjusting
                let other_panels_sum: f32 = self.panel_heights.iter().enumerate()
                    .filter(|(i, _)| *i != panel_idx && *i != panel_idx + 1)
                    .map(|(_, &h)| h)
                    .sum();

                // Max available height for these two panels to avoid scrollbar
                let max_available = self.available_height - other_panels_sum - handles_height;

                // Current heights
                let current_height = self.panel_heights[panel_idx];
                let next_height = self.panel_heights[panel_idx + 1];

                println!("  Before resize: panel {} height = {}, panel {} height = {}",
                         panel_idx, current_height, panel_idx + 1, next_height);

                // Proposed new heights
                let new_current = (current_height + delta).max(self.min_height);
                let new_next = (next_height - delta).max(self.min_height);

                // Total of new heights
                let new_total = new_current + new_next;

                if new_total <= max_available {
                    // We have enough space, apply changes directly
                    self.panel_heights[panel_idx] = new_current;
                    self.panel_heights[panel_idx + 1] = new_next;
                } else {
                    // We'd exceed available space, need to scale back
                    // Check if both panels can be at least minimum height
                    let min_total = self.min_height * 2.0;

                    if max_available >= min_total {
                        // Preserve the proportion of change as much as possible
                        let desired_ratio = new_current / new_total;
                        let adjusted_current = (max_available * desired_ratio).max(self.min_height);
                        let adjusted_next = (max_available - adjusted_current).max(self.min_height);

                        // If we still exceed due to minimums, final adjustment
                        let final_total = adjusted_current + adjusted_next;
                        if final_total > max_available {
                            // Just distribute evenly at this point
                            self.panel_heights[panel_idx] = max_available / 2.0;
                            self.panel_heights[panel_idx + 1] = max_available / 2.0;
                        } else {
                            self.panel_heights[panel_idx] = adjusted_current;
                            self.panel_heights[panel_idx + 1] = adjusted_next;
                        }
                    } else {
                        // Can't even fit minimum heights, use minimums
                        self.panel_heights[panel_idx] = self.min_height;
                        self.panel_heights[panel_idx + 1] = self.min_height;
                    }
                }

                println!("  After resize: panel {} height = {}, panel {} height = {}",
                         panel_idx, self.panel_heights[panel_idx],
                         panel_idx + 1, self.panel_heights[panel_idx + 1]);
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