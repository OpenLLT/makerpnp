pub mod add_pcb;
pub mod create_unit_assignment;
pub mod errors;

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
