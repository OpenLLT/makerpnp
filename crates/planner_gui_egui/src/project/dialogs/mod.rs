pub mod add_pcb;
pub mod add_phase;
pub mod errors;
pub mod manage_gerbers;
pub mod placement_orderings;

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum PcbKindChoice {
    Single,
    Panel,
}

impl From<PcbKindChoice> for planner_app::PcbKind {
    fn from(value: PcbKindChoice) -> Self {
        match value {
            PcbKindChoice::Single => planner_app::PcbKind::Single,
            PcbKindChoice::Panel => planner_app::PcbKind::Panel,
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum PcbSideChoice {
    Top,
    Bottom,
}

impl From<PcbSideChoice> for planner_app::PcbSide {
    fn from(value: PcbSideChoice) -> Self {
        match value {
            PcbSideChoice::Top => planner_app::PcbSide::Top,
            PcbSideChoice::Bottom => planner_app::PcbSide::Bottom,
        }
    }
}
