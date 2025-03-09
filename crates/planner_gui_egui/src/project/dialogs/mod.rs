pub mod add_pcb;
pub mod errors;

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum PcbKindChoice {
    Single,
    Panel,
}

impl TryFrom<PcbKindChoice> for planner_app::PcbKind {
    type Error = ();

    fn try_from(value: PcbKindChoice) -> Result<Self, Self::Error> {
        match value {
            PcbKindChoice::Single => Ok(planner_app::PcbKind::Single),
            PcbKindChoice::Panel => Ok(planner_app::PcbKind::Panel),
        }
    }
}
