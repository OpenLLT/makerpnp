use clap::ValueEnum;
use eda::EdaTool;
use planning::actions::{AddOrRemoveAction, SetOrClearAction};
use planning::placement::{PlacementStatus, PlacementSortingMode, PlacementOperation};
use planning::process::OperationAction;
use pnp::pcb::{PcbKind, PcbSide};
use util::sorting::SortOrder;

/// Args decouple of CLI arg handling requirements from the internal data structures

#[derive(Debug, Clone)]
#[derive(ValueEnum)]
#[value(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SortOrderArg {
    Asc,
    Desc,
}

impl SortOrderArg {
    pub fn to_sort_order(&self) -> SortOrder {
        match self {
            SortOrderArg::Asc => SortOrder::Asc,
            SortOrderArg::Desc => SortOrder::Desc,
        }
    }
}

#[derive(Debug, Clone)]
#[derive(ValueEnum)]
#[value(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlacementSortingModeArg {
    FeederReference,
    PcbUnit,
    RefDes,
    // FUTURE add other modes, such as COST, PART, AREA, HEIGHT, REFDES, ANGLE, DESIGN_X, DESIGN_Y, PANEL_X, PANEL_Y, DESCRIPTION
}

impl PlacementSortingModeArg {
    pub fn to_placement_sorting_mode(&self) -> PlacementSortingMode {
        match self {
            PlacementSortingModeArg::FeederReference => PlacementSortingMode::FeederReference,
            PlacementSortingModeArg::PcbUnit => PlacementSortingMode::PcbUnit,
            PlacementSortingModeArg::RefDes => PlacementSortingMode::RefDes,
        }
    }
}

#[derive(ValueEnum, Clone, Debug)]
#[value(rename_all = "lower")]
pub enum PcbSideArg {
    Top,
    Bottom,
}

impl From<PcbSideArg> for PcbSide {
    fn from(value: PcbSideArg) -> Self {
        match value {
            PcbSideArg::Top => Self::Top,
            PcbSideArg::Bottom => Self::Bottom,
        }
    }
}

#[derive(ValueEnum, Clone, Debug)]
#[value(rename_all = "lower")]
pub enum PcbKindArg {
    Single,
    Panel,
}

impl From<PcbKindArg> for PcbKind {
    fn from(value: PcbKindArg) -> Self {
        match value {
            PcbKindArg::Single => Self::Single,
            PcbKindArg::Panel => Self::Panel,
        }
    }
}

#[derive(ValueEnum, Debug, Clone)]
pub enum AddOrRemoveOperationArg {
    Add,
    Remove,
}

impl From<AddOrRemoveOperationArg> for AddOrRemoveAction {
    fn from(value: AddOrRemoveOperationArg) -> Self {
        match value {
            AddOrRemoveOperationArg::Add => Self::Add,
            AddOrRemoveOperationArg::Remove => Self::Remove,
        }
    }
}

#[derive(ValueEnum, Debug, Clone)]
pub enum SetOrClearOperationArg {
    Set,
    Clear,
}

impl From<SetOrClearOperationArg> for SetOrClearAction {
    fn from(value: SetOrClearOperationArg) -> Self {
        match value {
            SetOrClearOperationArg::Set => Self::Set,
            SetOrClearOperationArg::Clear => Self::Clear,
        }
    }
}

#[derive(Clone)]
#[derive(ValueEnum)]
pub enum EdaToolArg {
    #[value(name("diptrace"))]
    DipTrace,
    #[value(name("kicad"))]
    KiCad,
    #[value(name("easyeda"))]
    EasyEda,
}

impl EdaToolArg {
    pub fn build(&self) -> EdaTool {
        match self {
            EdaToolArg::DipTrace => EdaTool::DipTrace,
            EdaToolArg::KiCad => EdaTool::KiCad,
            EdaToolArg::EasyEda => EdaTool::EasyEda,
        }
    }
}

#[derive(Clone, Debug)]
#[derive(ValueEnum)]
pub enum PlacementOperationArg {
    #[value(name("placed"))]
    Placed,
    #[value(name("skipped"))]
    Skipped,
    #[value(name("reset"))]
    Reset,
}

impl From<PlacementOperationArg> for PlacementOperation {
    fn from(value: PlacementOperationArg) -> Self {
        match value {
            PlacementOperationArg::Placed => Self::Place,
            PlacementOperationArg::Skipped => Self::Skip,
            PlacementOperationArg::Reset => Self::Reset,
        }
    }
}


#[derive(Clone, Debug)]
#[derive(ValueEnum)]
pub enum OperationActionArg {
    #[value(name("started"))]
    Started,
    #[value(name("completed"))]
    Completed,
    #[value(name("abandoned"))]
    Abandoned,
}

impl From<OperationActionArg> for OperationAction {
    fn from(value: OperationActionArg) -> Self {
        match value {
            OperationActionArg::Started => OperationAction::Started,
            OperationActionArg::Completed => OperationAction::Completed,
            OperationActionArg::Abandoned => OperationAction::Abandoned,
        }
    }
}
