use egui::{Id, Rect, Sense, Stroke, Ui, Vec2, ScrollArea, CornerRadius, UiBuilder, Color32, StrokeKind, Frame};
use std::boxed::Box;
use std::collections::HashMap;
use egui::scroll_area::ScrollBarVisibility;

/// A component that displays multiple panels stacked vertically with resize handles contained within a scroll area.
/// 
/// This is very useful for layouts that need multiple 'panel' views.
///
/// The amount of panels is dynamic and can be added or removed at runtime.
///
/// Example use: 
/// ```
/// use egui_vertical_stack::vertical_stack::VerticalStack;
///
/// struct MyApp {
///     vertical_stack: VerticalStack,
/// }
///
/// impl MyApp {
///     pub fn new() -> Self {
///         Self {
///             vertical_stack: VerticalStack::new()
///                 .min_panel_height(150.0)
///                 .default_panel_height(50.0),
///         }
///     }
/// }
///
/// impl eframe::App for MyApp {
///     fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
///        egui::SidePanel::left("left_panel").show(ctx, |ui| {
///             self.vertical_stack
///                 .id_salt(ui.id().with("vertical_stack"))
///                 .body(ui, |body|{
///                     body.add_panel(|ui|{
///                         ui.label("top");
///                     });
///                     body.add_panel(|ui|{
///                         ui.label("middle with some very long text");
///                     });
///                     body.add_panel(|ui|{
///                         ui.label("bottom");
///                     });
///                 });
///         });
///         egui::CentralPanel::default().show(ctx, |ui| {
///             ui.label("main content");
///         });
///     }
/// }
/// ```
#[derive(Debug)]
pub struct VerticalStack {
    min_panel_height: f32,
    id_source: Id,
    panel_heights: Vec<f32>,
    default_panel_height: f32,
    drag_in_progress: bool,
    active_drag_handle: Option<usize>,
    drag_start_y: Option<f32>,
    drag_start_height: Option<f32>,
    initialized: bool,
    last_available_height: f32,
    max_height: Option<f32>,
    max_panel_height: Option<f32>,
    content_sizes: Vec<(f32, f32)>,
    need_sizing_pass: bool,
    scroll_bar_visibility: ScrollBarVisibility,
}

impl VerticalStack {
    pub fn new() -> Self {
        Self {
            min_panel_height: 50.0,
            id_source: Id::new("vertical_stack"),
            panel_heights: Vec::new(),
            default_panel_height: 100.0,
            drag_in_progress: false,
            active_drag_handle: None,
            drag_start_y: None,
            drag_start_height: None,
            initialized: false,
            last_available_height: 0.0,
            max_height: None,
            max_panel_height: None,
            content_sizes: Vec::new(),
            need_sizing_pass: true,
            scroll_bar_visibility: ScrollBarVisibility::VisibleWhenNeeded,
        }
    }

    /// Sets a custom ID salt for the stack.
    pub fn id_salt(&mut self, id: impl std::hash::Hash) -> &mut Self {
        self.id_source = Id::new(id);
        self
    }

    /// Sets the maximum height for the scroll area.
    /// If None, will use all available height.
    pub fn max_height(mut self, height: Option<f32>) -> Self {
        self.max_height = height;
        self
    }

    /// Sets the minimum height for panels.
    pub fn min_panel_height(mut self, height: f32) -> Self {
        self.min_panel_height = height;
        self
    }

    /// Sets the maximum height for individual panels.
    /// If None, panels can grow as large as needed (or up to the entire scroll area).
    pub fn max_panel_height(mut self, height: Option<f32>) -> Self {
        self.max_panel_height = height;
        self
    }


    /// Set the default height for new panels.
    pub fn default_panel_height(mut self, height: f32) -> Self {
        self.default_panel_height = height;
        self
    }
    
    /// Adjust the scroll-area's scrollbar visibility, the default is 'when needed', but using 'always visible' will
    /// likely yield a better UX.
    pub fn scroll_bar_visibility(mut self, visibility: ScrollBarVisibility) -> Self {
        self.scroll_bar_visibility = visibility;
        self
    }

    /// The main function to render the stack and add panels.
    pub fn body<F>(&mut self, ui: &mut Ui, collect_panels: F)
    where
        F: FnOnce(&mut StackBodyBuilder) + Clone,
    {
        let available_rect = ui.available_rect_before_wrap();
        let available_height = match self.max_height {
            Some(max_height) => max_height.min(available_rect.height()),
            None => available_rect.height(),
        };

        // Check if we need a sizing pass (first frame or available height changed)
        if self.need_sizing_pass || !self.initialized || (self.last_available_height - available_height).abs() > 1.0 {
            self.last_available_height = available_height;
            self.need_sizing_pass = false;

            // Run a sizing pass to measure content heights
            self.do_sizing_pass(ui, collect_panels.clone());

            ui.ctx().request_discard("sizing");
        }

        // Now do the actual rendering with known content heights
        self.do_render_pass(ui, collect_panels, available_height);

        // Mark as initialized
        self.initialized = true;
    }

    /// Perform a sizing pass to measure content heights without rendering
    fn do_sizing_pass<F>(&mut self, ui: &mut Ui, collect_panels: F)
    where
        F: FnOnce(&mut StackBodyBuilder),
    {
        self.content_sizes.clear();
        // Create a temporary UI for measuring content heights
        ui.allocate_ui(Vec2::new(ui.available_width(), 0.0), |ui| {
            let mut body = StackBodyBuilder {
                panels: Vec::new(),
            };

            // Collect panel functions
            collect_panels(&mut body);

            // Get panel count
            let panel_count = body.panels.len();

            // Early return if no panels
            if panel_count == 0 {
                return;
            }

            // Ensure we have enough heights for all panels
            while self.panel_heights.len() < panel_count {
                // Always respect the minimum height from the beginning
                self.panel_heights.push(self.default_panel_height.max(self.min_panel_height));
            }

            // Truncate if we have too many heights
            if self.panel_heights.len() > panel_count {
                self.panel_heights.truncate(panel_count);
            }

            // Get the available width for panels
            let panel_width = ui.available_width();

            // Measure each panel's content
            for (idx, panel_fn) in body.panels.into_iter().enumerate() {
                // Create a temporary rect with max height for measurement
                let panel_rect = Rect::from_min_size(
                    ui.cursor().min,
                    Vec2::new(panel_width, f32::MAX)
                );

                // Create a new child UI with sizing pass enabled
                let mut measuring_ui = ui.new_child(
                    UiBuilder::new()
                        .max_rect(panel_rect)
                        .sizing_pass()
                );

                // Call the panel function to measure its size
                panel_fn(&mut measuring_ui);

                // Get the measured content height
                let content_rect = measuring_ui.min_rect();
                self.content_sizes.push((content_rect.width(), content_rect.height()));
            }
        });
    }

    /// Render the actual UI with known content heights
    fn do_render_pass<F>(&mut self, ui: &mut Ui, collect_panels: F, available_height: f32)
    where
        F: FnOnce(&mut StackBodyBuilder),
    {
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

        // Handle drag state
        let pointer_is_down = ui.input(|i| i.pointer.any_down());

        if self.drag_in_progress && !pointer_is_down {
            self.drag_in_progress = false;
            self.active_drag_handle = None;
        }


        let (max_content_width, max_content_height) = self.content_sizes.iter().fold((0.0_f32, 0.0_f32), |acc, (w, h)| (acc.0.max(*w), acc.1.max(*h)));

        // let mut clip_rect = ui.clip_rect();
        // clip_rect.max.y = available_height;
        // ui.set_clip_rect(clip_rect);
        // 
        // Create a ScrollArea with the available height
        ScrollArea::both()
            //.id_salt(self.id_source.with("scroll_area"))
            .max_height(available_height)
            //.scroll_bar_visibility(self.scroll_bar_visibility)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                // Create a clip rect that exactly matches the ScrollArea's constraints
                let scroll_area_rect = ui.max_rect();
                #[cfg(feature = "layout_debugging")] {
                    let debug_stroke = Stroke::new(1.0, Color32::ORANGE);
                    ui.painter().rect(
                        scroll_area_rect,
                        CornerRadius::ZERO,
                        Color32::TRANSPARENT,
                        debug_stroke,
                        StrokeKind::Outside
                    );
                }

                let scroll_area_rect = ui.available_rect_before_wrap();
                #[cfg(feature = "layout_debugging")] {
                    let debug_stroke = Stroke::new(1.0, Color32::PURPLE);
                    ui.painter().rect(
                        scroll_area_rect,
                        CornerRadius::ZERO,
                        Color32::TRANSPARENT,
                        debug_stroke,
                        StrokeKind::Outside
                    );
                }

                // let mut clip_rect = scroll_area_rect;
                // 
                // // Set this clip rect to ensure content doesn't visually overflow
                // ui.set_clip_rect(clip_rect);
                // 

                // Use vertical layout with no spacing
                ui.spacing_mut().item_spacing.y = 0.0;

                // Get the available rect for the panel content
                let panel_rect = ui.available_rect_before_wrap();

                for (idx, panel_fn) in body.panels.into_iter().enumerate() {
                    // Get panel height (already has min_height applied above)
                    let panel_height = self.panel_heights[idx];

                    // Create a fixed size for this panel
                    let panel_size = Vec2::new(panel_rect.width(), panel_height);

                    let desired_size = Vec2::new(panel_rect.width(), panel_height);
                    ui.allocate_ui(desired_size, |ui| {
                        // Set a min height for the panel content
                        ui.set_min_height(panel_height);

                        // Call panel function in a slightly inset area
                        let inner_margin = 2.0;

                        let frame_width = max_content_width.max(scroll_area_rect.width());

                        // Draw the panel frame
                        let mut frame_rect = ui.max_rect();
                        frame_rect.max.x = frame_rect.min.x + frame_width + (inner_margin * 2.0);

                        let stroke = Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color);
                        ui.painter().rect(
                            frame_rect,
                            CornerRadius::ZERO,
                            Color32::TRANSPARENT,
                            stroke,
                            StrokeKind::Outside
                        );

                        let content_rect = frame_rect.shrink(inner_margin);

                        #[cfg(feature = "layout_debugging")] {
                            let debug_stroke = Stroke::new(1.0, Color32::GREEN);
                            ui.painter().rect(
                                frame_rect,
                                CornerRadius::ZERO,
                                Color32::TRANSPARENT,
                                debug_stroke,
                                StrokeKind::Outside
                            );
                        }

                        let mut clip_rect = content_rect;
                        clip_rect.max.y = scroll_area_rect.max.y.min(clip_rect.max.y);

                        #[cfg(feature = "layout_debugging")]
                        {
                            let debug_stroke = Stroke::new(1.0, Color32::RED);
                            ui.painter().rect(
                                clip_rect,
                                CornerRadius::ZERO,
                                Color32::TRANSPARENT,
                                debug_stroke,
                                StrokeKind::Outside
                            );
                        }

                        ui.allocate_ui_at_rect(content_rect, |ui| {

                            Frame::NONE.show(ui, |ui| {
                                let mut clip_rect = ui.clip_rect();
                                println!("before clip_rect={:?}", clip_rect);

                                clip_rect.max.x = clip_rect.min.x + content_rect.width();
                                println!("after clip_rect={:?}", clip_rect);

                                #[cfg(feature = "layout_debugging")]
                                {
                                    let debug_stroke = Stroke::new(1.0, Color32::CYAN);
                                    ui.painter().rect(
                                        clip_rect,
                                        CornerRadius::ZERO,
                                        Color32::TRANSPARENT,
                                        debug_stroke,
                                        StrokeKind::Outside
                                    );
                                }

                                //ui.set_clip_rect(content_rect);


                                panel_fn(ui);
                            })

                        });
                    });
                    // 
                    // // Draw frame
                    // let frame_rect = rect;
                    // let stroke = Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color);
                    // ui.painter().rect(
                    //     frame_rect,
                    //     CornerRadius::ZERO,
                    //     Color32::TRANSPARENT,
                    //     stroke,
                    //     StrokeKind::Outside
                    // );
                    // 
                    // // Create content area with padding
                    // let inner_margin = 2.0;
                    // let content_rect = frame_rect.shrink(inner_margin);
                    // 
                    // #[cfg(feature = "layout_debugging")]
                    // {
                    //     // Debug stroke for content rect (RED)
                    //     let mut inner_stroke = stroke;
                    //     inner_stroke.color = Color32::RED;
                    //     ui.painter().rect(
                    //         content_rect,
                    //         CornerRadius::ZERO,
                    //         Color32::TRANSPARENT,
                    //         inner_stroke,
                    //         StrokeKind::Outside
                    //     );
                    // }
                    // 
                    // // Allocate space for content area
                    // let _child_response = ui.allocate_rect(content_rect, Sense::hover());
                    // 
                    // // Create a child UI inside this exact rect
                    // let mut child_ui = ui.new_child(
                    //     UiBuilder::new()
                    //         .max_rect(content_rect)
                    //         .layout(*ui.layout())
                    // );
                    // 
                    // // Set clip rect to prevent overflow
                    // child_ui.set_clip_rect(content_rect);
                    // 
                    // // Call panel function
                    // panel_fn(&mut child_ui);
                    // 
                    // #[cfg(feature = "layout_debugging")]
                    // {
                    //     // Debug visualization of response rect (GREEN)
                    //     let mut debug_stroke = stroke;
                    //     debug_stroke.color = Color32::GREEN;
                    //     ui.painter().rect(
                    //         _child_response.rect,
                    //         CornerRadius::ZERO,
                    //         Color32::TRANSPARENT,
                    //         debug_stroke,
                    //         StrokeKind::Outside
                    //     );
                    // }
                    // Add resize handle after each panel but with spacing adjustment
                    // First, add 2px spacing to correctly position the handle
                    ui.allocate_exact_size(Vec2::new(panel_rect.width(), 2.0), Sense::hover());

                    // Now add the resize handle (without overlapping the panel)
                    self.add_resize_handle_no_gap(ui, idx);
                }
            });
    }
    
    fn add_resize_handle_no_gap(&mut self, ui: &mut Ui, panel_idx: usize) {
        let handle_height = 7.0;  // Keep this as 7 pixels total

        // For the last panel handle, we don't need to check the next panel's index
        let is_last_panel = panel_idx == self.panel_heights.len() - 1;

        // Skip if not valid (except for last panel)
        if !is_last_panel && (panel_idx >= self.panel_heights.len() || panel_idx + 1 >= self.panel_heights.len()) {
            return;
        }

        // Calculate handle rect
        let available_rect = ui.available_rect_before_wrap();
        let handle_rect = Rect::from_min_size(
            available_rect.min,
            Vec2::new(available_rect.width(), handle_height)
        );

        // Allocate exact size with drag sense
        let (rect, response) = ui.allocate_exact_size(
            handle_rect.size(),
            Sense::drag()
        );

        // Set the resize cursor when hovering or dragging
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }

        // Draw a small horizontal rule to indicate the drag handle
        // Calculate exact position for the 1px line with 3px padding above and below
        let painter = ui.painter();
        let handle_stroke = if response.hovered() || response.dragged() {
            Stroke::new(1.0, Color32::WHITE)
        } else {
            Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color)
        };

        // Position line exactly in the middle of the handle area
        let line_y = rect.min.y + (rect.height() / 2.0);

        // Draw a thin line that's always visible
        painter.line_segment(
            [egui::Pos2::new(rect.left(), line_y), egui::Pos2::new(rect.right(), line_y)],
            handle_stroke,
        );

        #[cfg(feature = "layout_debugging")]
        {
            // Debug visualization of handle rect to see its exact bounds (can be removed in production)
            let debug_stroke = Stroke::new(1.0, Color32::YELLOW);
            painter.rect_stroke(rect, 0.0, debug_stroke, StrokeKind::Outside);
        }

        // Check if this is the active drag handle
        let is_active_handle = self.active_drag_handle == Some(panel_idx);

        // Handle drag start
        if response.drag_started() {
            self.drag_in_progress = true;
            self.active_drag_handle = Some(panel_idx);

            // Store initial values
            if let Some(pointer_pos) = ui.ctx().pointer_latest_pos() {
                self.drag_start_y = Some(pointer_pos.y);
                self.drag_start_height = Some(self.panel_heights[panel_idx]);
            }
        }

        // Handle ongoing drag
        if is_active_handle && self.drag_in_progress && response.dragged() {
            if let (Some(start_y), Some(start_height)) = (self.drag_start_y, self.drag_start_height) {
                if let Some(current_pos) = ui.ctx().pointer_latest_pos() {
                    // Calculate delta from initial position
                    let total_delta = current_pos.y - start_y;
                    let current_height = self.panel_heights[panel_idx];
                    let target_height = (start_height + total_delta).max(self.min_panel_height);

                    println!("Drag: panel={}, current_height={}, delta={}, target_height={}, pointer_y={}",
                             panel_idx, current_height, total_delta, target_height, current_pos.y);

                    // Apply delta to the initial height
                    let mut new_height = (start_height + total_delta).max(self.min_panel_height);

                    // Apply maximum constraint if set
                    if let Some(max_panel_height) = self.max_panel_height {
                        new_height = new_height.min(max_panel_height);
                    }

                    // Update the panel height
                    self.panel_heights[panel_idx] = new_height;
                    self.need_sizing_pass = true;
                }
            }
        }

        // Handle drag end
        if is_active_handle && response.drag_stopped() {
            self.drag_in_progress = false;
            self.active_drag_handle = None;
            self.drag_start_y = None;
            self.drag_start_height = None;
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