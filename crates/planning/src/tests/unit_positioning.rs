//! This is an integration test for [`build_placement_unit_positions`]
//!
//! [`build_placement_unit_positions`] is one of the more fundamental functions in the planning module.  Given the
//! amount of test set-up code only some basic tests are included here, behind the scenes the
//! [`build_placement_unit_positions`] uses the [`PcbUnitTransform::apply_to_placement_matrix`] method.  
//! See the [`unit_transforms`] module which has extensive unit tests.
//!
//! Given the above, care must be taken to keep both sets of tests running during any refactoring.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;

use indexmap::IndexSet;
use nalgebra::Vector2;
use pnp::object_path::ObjectPath;
use pnp::panel::{DesignSizing, Dimensions, PanelSizing, PcbUnitPositioning, Unit};
use pnp::part::Part;
use pnp::pcb::{PcbSide, PcbUnitIndex};
use pnp::placement::Placement;
use rstest::rstest;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use tap::Tap;

use crate::design::{DesignIndex, DesignName};
use crate::file::FileReference;
use crate::pcb::{
    Pcb, PcbAssemblyFlip, PcbAssemblyOrientation, PcbSideAssemblyOrientation, PcbUnitTransform, UnitPlacementPosition,
};
use crate::phase::PhaseReference;
use crate::placement::{PlacementState, PlacementStatus, ProjectPlacementStatus};
use crate::project::{build_placement_unit_positions, Project, ProjectPcb};

macro_rules! make_x_y_rotation {
    ($x:expr, $y:expr, $rotation:expr) => {
        [dec!($x), dec!($y), dec!($rotation)]
    };
}

// abbreviations:
// CCW = counter clock-wise
// R = ROLL
// P = PITCH
// FLIP = flip (mirror)

const PCB_ORIENTATION_0_PFLIP: PcbAssemblyOrientation = PcbAssemblyOrientation {
    top: PcbSideAssemblyOrientation {
        flip: PcbAssemblyFlip::None,
        rotation: dec!(0),
    },
    bottom: PcbSideAssemblyOrientation {
        flip: PcbAssemblyFlip::Pitch,
        rotation: dec!(0),
    },
};

const PCB_ORIENTATION_CCW90_PFLIP: PcbAssemblyOrientation = PcbAssemblyOrientation {
    top: PcbSideAssemblyOrientation {
        flip: PcbAssemblyFlip::None,
        rotation: dec!(90),
    },
    bottom: PcbSideAssemblyOrientation {
        flip: PcbAssemblyFlip::Pitch,
        rotation: dec!(90),
    },
};

const UNIT_ROTATION_0: Decimal = dec!(0.0);
const UNIT_ROTATION_90: Decimal = dec!(90.0);

#[rustfmt::skip]
const CASE_1_EXPECTATIONS: [[Decimal; 3]; 8] = [
    make_x_y_rotation!(10, 10, 45),
    make_x_y_rotation!(10, 70, -135),
    make_x_y_rotation!(50, 10, 45),
    make_x_y_rotation!(50, 70, -135),
    make_x_y_rotation!(10, 50, 45),
    make_x_y_rotation!(10, 30, -135),
    make_x_y_rotation!(50, 50, 45),
    make_x_y_rotation!(50, 30, -135),
];

#[rustfmt::skip]
const CASE_2_EXPECTATIONS: [[Decimal; 3]; 8] = [
    make_x_y_rotation!(70, 10, 135),
    make_x_y_rotation!(10, 10, -45),
    make_x_y_rotation!(70, 50, 135),
    make_x_y_rotation!(10, 50, -45),
    make_x_y_rotation!(30, 10, 135),
    make_x_y_rotation!(50, 10, -45),
    make_x_y_rotation!(30, 50, 135),
    make_x_y_rotation!(50, 50, -45),
];

#[rustfmt::skip]
const CASE_3_EXPECTATIONS: [[Decimal; 3]; 8] = [
    make_x_y_rotation!(30, 10, 135),
    make_x_y_rotation!(30, 70, -45),
    make_x_y_rotation!(70, 10, 135),
    make_x_y_rotation!(70, 70, -45),
    make_x_y_rotation!(30, 50, 135),
    make_x_y_rotation!(30, 30, -45),
    make_x_y_rotation!(70, 50, 135),
    make_x_y_rotation!(70, 30, -45),
];

// TODO add test cases for roll flip

#[rstest]
#[case(PCB_ORIENTATION_0_PFLIP, UNIT_ROTATION_0, CASE_1_EXPECTATIONS)]
#[case(PCB_ORIENTATION_CCW90_PFLIP, UNIT_ROTATION_0, CASE_2_EXPECTATIONS)]
#[case(PCB_ORIENTATION_0_PFLIP, UNIT_ROTATION_90, CASE_3_EXPECTATIONS)]
fn test_build_placement_unit_positions(
    #[case] pcb_orientation: PcbAssemblyOrientation,
    #[case] unit_rotation_degrees: Decimal,
    #[case] expectations: [[Decimal; 3]; 8],
) {
    // given
    let mut project = Project::new("test".to_string());

    let eda_gerber_export_offset = Vector2::new(dec!(5), dec!(5));
    let eda_placement_export_offset = Vector2::new(dec!(10), dec!(10));

    let placement1 = Placement {
        ref_des: "R1".into(),
        part: Part {
            manufacturer: "MFR1".to_string(),
            mpn: "MPN1".to_string(),
        },
        place: true,
        pcb_side: PcbSide::Top,
        x: eda_placement_export_offset.x + dec!(10),
        y: eda_placement_export_offset.y + dec!(10),
        rotation: Decimal::from(45),
    };

    let placement2 = Placement {
        ref_des: "R2".into(),
        part: Part {
            manufacturer: "MFR1".to_string(),
            mpn: "MPN1".to_string(),
        },
        place: true,
        pcb_side: PcbSide::Bottom,
        x: eda_placement_export_offset.x + dec!(10),
        y: eda_placement_export_offset.y + dec!(10),
        rotation: Decimal::from(-45),
    };

    let placement_state1 = PlacementState {
        unit_path: ObjectPath::default(),
        placement: placement1,
        unit_position: UnitPlacementPosition::default(),
        operation_status: PlacementStatus::Pending,
        project_status: ProjectPlacementStatus::Used,
        phase: Some(PhaseReference::from_raw_str("Top_SMT")),
    };

    let placement_state2 = PlacementState {
        unit_path: ObjectPath::default(),
        placement: placement2,
        unit_position: UnitPlacementPosition::default(),
        operation_status: PlacementStatus::Pending,
        project_status: ProjectPlacementStatus::Used,
        phase: Some(PhaseReference::from_raw_str("Bottom_SMT")),
    };
    project.placements.insert(
        ObjectPath::from_str("pcb=1::unit=1::ref_des=R1").unwrap(),
        placement_state1
            .clone()
            .tap_mut(|ps| ps.unit_path = ObjectPath::from_str("pcb=1::unit=1").unwrap()),
    );
    project.placements.insert(
        ObjectPath::from_str("pcb=1::unit=2::ref_des=R1").unwrap(),
        placement_state1
            .clone()
            .tap_mut(|ps| ps.unit_path = ObjectPath::from_str("pcb=1::unit=2").unwrap()),
    );
    project.placements.insert(
        ObjectPath::from_str("pcb=1::unit=3::ref_des=R1").unwrap(),
        placement_state1
            .clone()
            .tap_mut(|ps| ps.unit_path = ObjectPath::from_str("pcb=1::unit=3").unwrap()),
    );
    project.placements.insert(
        ObjectPath::from_str("pcb=1::unit=4::ref_des=R1").unwrap(),
        placement_state1
            .clone()
            .tap_mut(|ps| ps.unit_path = ObjectPath::from_str("pcb=1::unit=4").unwrap()),
    );
    project.placements.insert(
        ObjectPath::from_str("pcb=1::unit=1::ref_des=R2").unwrap(),
        placement_state2
            .clone()
            .tap_mut(|ps| ps.unit_path = ObjectPath::from_str("pcb=1::unit=1").unwrap()),
    );
    project.placements.insert(
        ObjectPath::from_str("pcb=1::unit=2::ref_des=R2").unwrap(),
        placement_state2
            .clone()
            .tap_mut(|ps| ps.unit_path = ObjectPath::from_str("pcb=1::unit=2").unwrap()),
    );
    project.placements.insert(
        ObjectPath::from_str("pcb=1::unit=3::ref_des=R2").unwrap(),
        placement_state2
            .clone()
            .tap_mut(|ps| ps.unit_path = ObjectPath::from_str("pcb=1::unit=3").unwrap()),
    );
    project.placements.insert(
        ObjectPath::from_str("pcb=1::unit=4::ref_des=R2").unwrap(),
        placement_state2
            .clone()
            .tap_mut(|ps| ps.unit_path = ObjectPath::from_str("pcb=1::unit=4").unwrap()),
    );

    let design_names: IndexSet<DesignName> = IndexSet::from(["Design1".into()]);
    let unit_map: BTreeMap<PcbUnitIndex, DesignIndex> = BTreeMap::from_iter([(0, 0), (1, 0), (2, 0), (3, 0)]);

    // Panel layout (4 units), bottom left of panel is (0,0), top right is (x+, y+)
    //
    //                  pitch-flipped
    // top view:        bottom view:
    // +TTTTTTTTT+      +BBBBBBBBB+
    // L*********R      L*********R
    // L*###*###*R      L*###*###*R
    // L*#3#*#4#*R      L*#1#*#2#*R
    // L*###*###*R      L*###*###*R
    // L*********R      L*********R
    // L*###*###*R      L*###*###*R
    // L*#1#*#2#*R      L*#3#*#4#*R
    // L*###*###*R      L*###*###*R
    // +BBBBBBBBB+      +TTTTTTTTT+
    //
    // key: T = top rail, B = bottom rail, R = right rail, L = left rail, + = corner
    //      # = edge routing gap,
    //      <n> = unit n (origin)
    //
    // Note that when x axis flipped (y mirrored / pitch), the top edge becomes the bottom edge and the unit numbers are flipped too.
    //
    // This is so that when you place components ONLY on a single unit of a panel, e.g. number 1, when you come to
    // place components on the bottom of the panel, the components are placed on the same unit.
    //
    // When components are placed on the bottom, the component rotation needs to be adjusted too.

    let edge_rails = Dimensions {
        left: 0.0,
        right: 0.0,
        top: 0.0,
        bottom: 0.0,
    };

    let design_size: Vector2<f64> = [40.0, 40.0].into();
    // calculate the center of the design, to use as the origin for rotations/translations
    let design_center: Vector2<f64> = design_size / 2.0;
    // use the opposite of the eda_gerber_export_offset, note the leading `-` sign.
    let gerber_offset: Vector2<f64> = [
        -eda_gerber_export_offset
            .x
            .to_f64()
            .unwrap(),
        -eda_gerber_export_offset
            .y
            .to_f64()
            .unwrap(),
    ]
    .into();
    // use the opposite of the eda_gerber_export_offset, note the leading `-` sign.
    let placement_offset: Vector2<f64> = [
        -eda_placement_export_offset
            .x
            .to_f64()
            .unwrap(),
        -eda_placement_export_offset
            .y
            .to_f64()
            .unwrap(),
    ]
    .into();

    let design_sizing = DesignSizing {
        size: design_size,
        origin: design_center,
        gerber_offset,
        placement_offset,
    };

    let edge_routing_gap = 0.0;
    let x_count = 2;
    let y_count = 2;

    let mut pcb1 = Pcb::new("PCB1".to_string(), 4, design_names, unit_map);
    let panel_sizing = PanelSizing {
        units: Unit::Millimeters,
        size: [
            edge_rails.left
                + (design_sizing.size.x * x_count as f64)
                + (edge_routing_gap * (x_count + 1) as f64)
                + edge_rails.right,
            edge_rails.bottom
                + (design_sizing.size.y * y_count as f64)
                + (edge_routing_gap * (y_count + 1) as f64)
                + edge_rails.top,
        ]
        .into(),
        // Note: the edge rails are not used in the calculations, but are included for completeness.
        edge_rails: edge_rails.clone(),
        fiducials: vec![],
        design_sizings: vec![design_sizing.clone()],
        // Note: PcbUnitPositions are from the bottom left of the panel, rails are ignored by the calculations.
        pcb_unit_positionings: vec![
            // bottom/front row
            PcbUnitPositioning {
                offset: [
                    edge_rails.left + edge_routing_gap + ((design_sizing.size.x + edge_routing_gap) * 0 as f64),
                    edge_rails.bottom + edge_routing_gap + ((design_sizing.size.y + edge_routing_gap) * 0 as f64),
                ]
                .into(),
                rotation: unit_rotation_degrees,
            },
            PcbUnitPositioning {
                offset: [
                    edge_rails.left + edge_routing_gap + ((design_sizing.size.x + edge_routing_gap) * 1 as f64),
                    edge_rails.bottom + edge_routing_gap + ((design_sizing.size.y + edge_routing_gap) * 0 as f64),
                ]
                .into(),
                rotation: unit_rotation_degrees,
            },
            // top/rear row
            PcbUnitPositioning {
                offset: [
                    edge_rails.left + edge_routing_gap + ((design_sizing.size.x + edge_routing_gap) * 0 as f64),
                    edge_rails.bottom + edge_routing_gap + ((design_sizing.size.y + edge_routing_gap) * 1 as f64),
                ]
                .into(),
                rotation: unit_rotation_degrees,
            },
            PcbUnitPositioning {
                offset: [
                    edge_rails.left + edge_routing_gap + ((design_sizing.size.x + edge_routing_gap) * 1 as f64),
                    edge_rails.bottom + edge_routing_gap + ((design_sizing.size.y + edge_routing_gap) * 1 as f64),
                ]
                .into(),
                rotation: unit_rotation_degrees,
            },
        ],
    };
    pcb1.panel_sizing = panel_sizing;
    pcb1.orientation = pcb_orientation;

    let mut project_pcb = ProjectPcb::new(FileReference::Relative(PathBuf::from("pcb1.pcb.json")));
    project_pcb
        .unit_assignments
        .insert(0, (0, "Variant1".into()));
    project_pcb
        .unit_assignments
        .insert(1, (0, "Variant1".into()));
    project_pcb
        .unit_assignments
        .insert(2, (0, "Variant1".into()));
    project_pcb
        .unit_assignments
        .insert(3, (0, "Variant1".into()));
    project.pcbs.push(project_pcb);

    // and build args
    let pcbs = vec![&pcb1];
    println!("pcbs: {:?}", pcbs);

    let all_unit_assignments = project.all_unit_assignments(&pcbs);
    println!("all_unit_assignments: {:?}", all_unit_assignments);
    let placements = project
        .placements
        .iter()
        .map(|(path, state)| (path.clone(), &state.placement))
        .collect::<Vec<_>>();
    println!("placements: {:?}", placements);

    // and
    let expected_result = BTreeMap::from_iter(
        expectations
            .into_iter()
            .enumerate()
            .map(|(index, expectation)| {
                (placements[index].0.clone(), UnitPlacementPosition {
                    x: expectation[0],
                    y: expectation[1],
                    rotation: expectation[2],
                })
            }),
    );

    // when
    let result = build_placement_unit_positions(placements, &all_unit_assignments, &pcbs);

    // then
    let Ok(result) = result else {
        println!("result: {:?}", result);
        assert!(result.is_ok());
        unreachable!();
    };

    // and
    println!(
        "result:\n{}",
        result
            .iter()
            .map(|(op, upp)| format!("{} = {:?}", op, upp))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(result, expected_result);
}
