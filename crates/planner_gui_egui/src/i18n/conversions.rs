use planner_app::PcbSide;

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
