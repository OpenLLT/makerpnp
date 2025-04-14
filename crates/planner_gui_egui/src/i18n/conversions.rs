use planner_app::{PcbSide, PlacementSortingMode, PlacementStatus, ProjectPlacementStatus, TaskStatus};
use util::sorting::SortOrder;

pub fn pcb_side_to_i18n_key(pcb_side: &PcbSide) -> &'static str {
    match pcb_side {
        PcbSide::Top => "pcb-side-top",
        PcbSide::Bottom => "pcb-side-bottom",
    }
}

pub fn placement_operation_status_to_i18n_key(placement_operation_status: &PlacementStatus) -> &'static str {
    match placement_operation_status {
        PlacementStatus::Placed => "placement-placed",
        PlacementStatus::Skipped => "placement-skipped",
        PlacementStatus::Pending => "placement-pending",
    }
}

pub fn placement_place_to_i18n_key(placed: bool) -> &'static str {
    match placed {
        true => "placement-place",
        false => "placement-no-place",
    }
}

pub fn placement_project_status_to_i18n_key(status: &ProjectPlacementStatus) -> &'static str {
    match status {
        ProjectPlacementStatus::Used => "placement-project-status-used",
        ProjectPlacementStatus::Unused => "placement-project-status-unused",
    }
}

pub fn placement_sorting_mode_to_i18n_key(mode: &PlacementSortingMode) -> &'static str {
    match mode {
        PlacementSortingMode::FeederReference => "sort-mode-feeder-reference",
        PlacementSortingMode::PcbUnit => "sort-mode-pcb-unit",
        PlacementSortingMode::RefDes => "sort-mode-ref-des",
    }
}

pub fn sort_order_to_i18n_key(sort_order: &SortOrder) -> &'static str {
    match sort_order {
        SortOrder::Asc => "sort-order-ascending",
        SortOrder::Desc => "sort-order-descending",
    }
}

pub fn process_operation_status_to_i18n_key(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "process-operation-status-pending",
        TaskStatus::Started => "process-operation-status-incomplete",
        TaskStatus::Complete => "process-operation-status-complete",
        TaskStatus::Abandoned => "process-operation-status-abandoned",
    }
}
