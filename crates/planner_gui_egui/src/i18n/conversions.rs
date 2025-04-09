use planner_app::{PcbSide, PlacementSortingMode, PlacementStatus, ProcessOperationKind, ProcessOperationStatus};
use util::sorting::SortOrder;

pub fn pcb_side_to_i18n_key(pcb_side: &PcbSide) -> &'static str {
    match pcb_side {
        PcbSide::Top => "pcb-side-top",
        PcbSide::Bottom => "pcb-side-bottom",
    }
}

pub fn placement_placed_to_i18n_key(placed: bool) -> &'static str {
    match placed {
        true => "placement-placed",
        false => "placement-pending",
    }
}

pub fn placement_place_to_i18n_key(placed: bool) -> &'static str {
    match placed {
        true => "placement-place",
        false => "placement-no-place",
    }
}

pub fn placement_status_to_i18n_key(status: &PlacementStatus) -> &'static str {
    match status {
        PlacementStatus::Known => "placement-status-known",
        PlacementStatus::Unknown => "placement-status-unknown",
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

pub fn process_operation_status_to_i18n_key(status: &ProcessOperationStatus) -> &'static str {
    match status {
        ProcessOperationStatus::Pending => "process-operation-status-pending",
        ProcessOperationStatus::Incomplete => "process-operation-status-incomplete",
        ProcessOperationStatus::Complete => "process-operation-status-complete",
    }
}

pub fn process_operation_kind_to_i18n_key(kind: &ProcessOperationKind) -> &'static str {
    match kind {
        ProcessOperationKind::LoadPcbs => "process-operation-kind-load-pcbs",
        ProcessOperationKind::AutomatedPnp => "process-operation-kind-automated-pnp",
        ProcessOperationKind::ReflowComponents => "process-operation-kind-reflow-components",
        ProcessOperationKind::ManuallySolderComponents => "process-operation-kind-manually-solder-components",
    }
}
