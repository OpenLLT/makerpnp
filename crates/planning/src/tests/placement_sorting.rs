use std::collections::BTreeMap;

use eda_units::eda_units::dimension_unit::{DimensionUnitVector2, DimensionUnitVector2Ext};
use eda_units::eda_units::unit_system::UnitSystem;
use pnp::object_path::ObjectPath;
use pnp::package::Package;
use pnp::part::Part;
use pnp::placement::Placement;
use util::sorting::SortOrder;

use crate::placement::{PlacementSortingMode, PlacementState};
use crate::project::sort_placements;

#[test]
fn test_placement_sorting_pcb_unit_xy() {
    // given
    let unit_configurations = build_2x2_panel();

    // and
    let placement_states = build_placements_for_unit_position_testing();
    let mut sortable_placement_states = placement_states
        .iter()
        .map(|(object_path, placement_state)| (object_path, placement_state))
        .collect::<Vec<_>>();
    let placement_orderings = vec![(PlacementSortingMode::PcbUnitXY, SortOrder::Asc).into()];
    let load_out_items = vec![];
    let pcb_1_unit_positions = unit_configurations
        .iter()
        .map(|(_, position, _)| *position)
        .collect::<Vec<_>>();
    let part_packages: BTreeMap<&Part, &Package> = BTreeMap::new();
    let pcb_unit_positioning_map = vec![pcb_1_unit_positions];

    // when
    sort_placements(
        &mut sortable_placement_states,
        &placement_orderings,
        &load_out_items,
        &part_packages,
        &pcb_unit_positioning_map,
    );

    // then
    let placement_units = sortable_placement_states
        .iter()
        .map(|(object_path, _)| object_path.pcb_unit().unwrap())
        .collect::<Vec<_>>();

    dump_unit_position_sorting_results(unit_configurations, &sortable_placement_states);

    // sorted first vertically, then horizontally (by x, then by y)
    assert_eq!(placement_units, vec![
        // these numbers correspond to first number in the unit_configurations
        1, 3, 2, 4,
    ])
}

#[test]
fn test_placement_sorting_pcb_unit_yx() {
    // given
    let unit_configurations = build_2x2_panel();

    // and
    let placement_states = build_placements_for_unit_position_testing();
    let mut sortable_placement_states = placement_states
        .iter()
        .map(|(object_path, placement_state)| (object_path, placement_state))
        .collect::<Vec<_>>();
    let placement_orderings = vec![(PlacementSortingMode::PcbUnitYX, SortOrder::Asc).into()];
    let load_out_items = vec![];
    let pcb_1_unit_positions = unit_configurations
        .iter()
        .map(|(_, position, _)| *position)
        .collect::<Vec<_>>();
    let part_packages: BTreeMap<&Part, &Package> = BTreeMap::new();
    let pcb_unit_positioning_map = vec![pcb_1_unit_positions];

    // when
    sort_placements(
        &mut sortable_placement_states,
        &placement_orderings,
        &load_out_items,
        &part_packages,
        &pcb_unit_positioning_map,
    );

    // then
    let placement_units = sortable_placement_states
        .iter()
        .map(|(object_path, _)| object_path.pcb_unit().unwrap())
        .collect::<Vec<_>>();

    dump_unit_position_sorting_results(unit_configurations, &sortable_placement_states);

    // sorted first horizontally, then vertically (by y, then by x)
    assert_eq!(placement_units, vec![
        // these numbers correspond to first number in the unit_configurations
        1, 2, 3, 4,
    ])
}

fn build_2x2_panel() -> Vec<(i32, DimensionUnitVector2, &'static str)> {
    // two of the units have their coordinates inches, to ensure that the sorting correctly
    // handles conversions.
    vec![
        (
            1,
            DimensionUnitVector2::new_dim_f64(0.0, 0.0, UnitSystem::Millimeters),
            "front/bottom left",
        ),
        (
            2,
            DimensionUnitVector2::new_dim_f64(50.0, 0.0, UnitSystem::Millimeters),
            "front/bottom right",
        ),
        (
            3,
            DimensionUnitVector2::new_dim_f64(0.0, 50.0, UnitSystem::Millimeters).in_unit_system(UnitSystem::Inches),
            "rear/top left",
        ),
        (
            4,
            DimensionUnitVector2::new_dim_f64(50.0, 50.0, UnitSystem::Millimeters).in_unit_system(UnitSystem::Inches),
            "rear/top right",
        ),
    ]
}

fn build_placements_for_unit_position_testing() -> Vec<(ObjectPath, PlacementState)> {
    macro_rules! build_placement {
        ($unit_path:literal, $ref_des:literal) => {
            PlacementState {
                unit_path: ObjectPath::from_raw_str($unit_path),
                placement: Placement {
                    ref_des: $ref_des.into(),
                    ..Placement::default()
                },
                ..PlacementState::default()
            }
        };
    }

    // unit grid: 2x2 grid
    // ordering: diagonally, starting at top left, not starting with 1, not ending with 4.
    vec![
        (
            ObjectPath::from_raw_str("pcb=1::unit=3::ref_des=R1"),
            build_placement!("pcb=1::unit=3", "R1"),
        ),
        (
            ObjectPath::from_raw_str("pcb=1::unit=2::ref_des=R1"),
            build_placement!("pcb=1::unit=2", "R1"),
        ),
        (
            ObjectPath::from_raw_str("pcb=1::unit=4::ref_des=R1"),
            build_placement!("pcb=1::unit=4", "R1"),
        ),
        (
            ObjectPath::from_raw_str("pcb=1::unit=1::ref_des=R1"),
            build_placement!("pcb=1::unit=1", "R1"),
        ),
    ]
}

fn dump_unit_position_sorting_results(
    unit_configurations: Vec<(i32, DimensionUnitVector2, &str)>,
    sortable_placement_states: &Vec<(&ObjectPath, &PlacementState)>,
) {
    for (index, (path, _placement_state)) in sortable_placement_states
        .iter()
        .enumerate()
    {
        let placement_unit = path.pcb_unit().unwrap();
        let uc = unit_configurations[placement_unit as usize - 1];
        println!("{}, {} => ({}, [{}, {}], {})", index, path, uc.0, uc.1.x, uc.1.y, uc.2);
    }
}
