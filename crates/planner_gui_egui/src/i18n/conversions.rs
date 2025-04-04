use planner_app::{PcbSide, PlacementStatus};

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
