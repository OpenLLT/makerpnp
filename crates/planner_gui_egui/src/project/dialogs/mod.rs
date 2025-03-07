pub mod errors;
pub mod add_pcb;

#[derive(Default, Eq, PartialEq, Debug, Clone, Copy)]
pub enum PcbKind {
    #[default]
    None,
    Single,
    Panel,
}

impl TryFrom<PcbKind> for planner_app::PcbKind {
    type Error = ();

    fn try_from(value: PcbKind) -> Result<Self, Self::Error> {
        match value {
            PcbKind::None => Err(()),
            PcbKind::Single => Ok(planner_app::PcbKind::Single),
            PcbKind::Panel => Ok(planner_app::PcbKind::Panel),
        }
    }
}
