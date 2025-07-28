use rust_decimal::Decimal;

use crate::pcb::UnitPlacementPosition;

#[cfg(test)]
mod placement_unit_positioning_tests {
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
    use rust_decimal::prelude::ToPrimitive;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use tap::Tap;

    use crate::design::{DesignIndex, DesignName};
    use crate::file::FileReference;
    use crate::pcb::{Pcb, PcbAssemblyFlip, PcbAssemblyOrientation, PcbSideAssemblyOrientation, UnitPlacementPosition};
    use crate::phase::PhaseReference;
    use crate::placement::{PlacementState, PlacementStatus, ProjectPlacementStatus};
    use crate::project::{build_placement_unit_positions, Project, ProjectPcb};

    #[test]
    fn test_build_placement_unit_positions() {
        // given
        let pcb_orientation: PcbAssemblyOrientation = PcbAssemblyOrientation {
            top: PcbSideAssemblyOrientation {
                flip: PcbAssemblyFlip::None,
                rotation: dec!(90),
            },
            bottom: PcbSideAssemblyOrientation {
                flip: PcbAssemblyFlip::Pitch,
                rotation: dec!(90),
            },
        };
        let unit_rotation_degrees: Decimal = dec!(0.0);

        let mut project = Project::new("test".to_string());

        let eda_gerber_export_offset = Vector2::new(dec!(5), dec!(5));
        let eda_placement_export_offset = Vector2::new(dec!(10), dec!(10));

        let placement1 = Placement {
            ref_des: "SP60".into(),
            part: Part {
                manufacturer: "MFR1".to_string(),
                mpn: "MPN1".to_string(),
            },
            place: true,
            pcb_side: PcbSide::Top,
            x: eda_placement_export_offset.x + dec!(1.6),
            y: eda_placement_export_offset.y + dec!(1.6),
            rotation: Decimal::from(0),
        };

        let placement_state1 = PlacementState {
            unit_path: ObjectPath::default(),
            placement: placement1,
            unit_position: UnitPlacementPosition::default(),
            operation_status: PlacementStatus::Pending,
            project_status: ProjectPlacementStatus::Used,
            phase: Some(PhaseReference::from_raw_str("Top_SMT")),
        };

        project.placements.insert(
            ObjectPath::from_str("pcb=1::unit=1::ref_des=SP60").unwrap(),
            placement_state1
                .clone()
                .tap_mut(|ps| ps.unit_path = ObjectPath::from_str("pcb=1::unit=1").unwrap()),
        );
        project.placements.insert(
            ObjectPath::from_str("pcb=1::unit=2::ref_des=SP60").unwrap(),
            placement_state1
                .clone()
                .tap_mut(|ps| ps.unit_path = ObjectPath::from_str("pcb=1::unit=2").unwrap()),
        );
        project.placements.insert(
            ObjectPath::from_str("pcb=1::unit=3::ref_des=SP60").unwrap(),
            placement_state1
                .clone()
                .tap_mut(|ps| ps.unit_path = ObjectPath::from_str("pcb=1::unit=3").unwrap()),
        );
        project.placements.insert(
            ObjectPath::from_str("pcb=1::unit=4::ref_des=SP60").unwrap(),
            placement_state1
                .clone()
                .tap_mut(|ps| ps.unit_path = ObjectPath::from_str("pcb=1::unit=4").unwrap()),
        );
        project.placements.insert(
            ObjectPath::from_str("pcb=1::unit=5::ref_des=SP60").unwrap(),
            placement_state1
                .clone()
                .tap_mut(|ps| ps.unit_path = ObjectPath::from_str("pcb=1::unit=5").unwrap()),
        );
        project.placements.insert(
            ObjectPath::from_str("pcb=1::unit=6::ref_des=SP60").unwrap(),
            placement_state1
                .clone()
                .tap_mut(|ps| ps.unit_path = ObjectPath::from_str("pcb=1::unit=6").unwrap()),
        );

        let design_names: IndexSet<DesignName> = IndexSet::from(["Design1".into()]);
        let unit_map: BTreeMap<PcbUnitIndex, DesignIndex> =
            BTreeMap::from_iter([(0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0)]);

        let edge_rails = Dimensions {
            left: 5.0,
            right: 5.0,
            top: 5.0,
            bottom: 5.0,
        };

        let design_size: Vector2<f64> = [39.0, 39.0].into();
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

        let edge_routing_gap = 2.0;
        let x_count = 3;
        let y_count = 2;

        let mut pcb1 = Pcb::new("PCB1".to_string(), 6, design_names, unit_map);
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
                PcbUnitPositioning {
                    offset: [
                        edge_rails.left + edge_routing_gap + ((design_sizing.size.x + edge_routing_gap) * 2 as f64),
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
                PcbUnitPositioning {
                    offset: [
                        edge_rails.left + edge_routing_gap + ((design_sizing.size.x + edge_routing_gap) * 2 as f64),
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
        project_pcb
            .unit_assignments
            .insert(4, (0, "Variant1".into()));
        project_pcb
            .unit_assignments
            .insert(5, (0, "Variant1".into()));
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
        let expectations: [[Decimal; 3]; 6] = [
            [dec!(85.4), dec!(8.6), dec!(90.0)],
            [dec!(85.4), dec!(49.6), dec!(90.0)],
            [dec!(85.4), dec!(90.6), dec!(90.0)],
            [dec!(44.40000000000001), dec!(8.6), dec!(90.0)],
            [dec!(44.40000000000001), dec!(49.6), dec!(90.0)],
            [dec!(44.40000000000001), dec!(90.6), dec!(90.0)],
        ];

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
}

#[cfg(test)]
mod pcb_unit_transform_tests {
    use std::str::FromStr;

    use math::angle::normalize_angle_deg_signed_decimal;
    use nalgebra::{Point2, Vector2};
    use pnp::panel::DesignSizing;
    use pnp::part::Part;
    use pnp::pcb::PcbSide;
    use pnp::placement::Placement;
    use rstest::rstest;
    use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    use crate::pcb::{PcbAssemblyFlip, PcbSideAssemblyOrientation, PcbUnitTransform, UnitPlacementPosition};
    use crate::tests::unit_positioning_real_world::approx_eq_position;

    // Single structure for all test parameters
    struct TransformTestCase {
        edge_rail_left_right: f64,
        edge_rail_top_bottom: f64,
        routing_gap: f64,
        eda_placement_export_offset: Vector2<Decimal>,
        panel_size: Vector2<f64>,
        panel_rotation: f64,
        unit_rotation: f64,
        assembly_flip: PcbAssemblyFlip,
        design_size: Vector2<f64>,
    }

    const PLACEMENT_POINT_TEST: Point2<Decimal> = Point2::new(dec!(10.0), dec!(20.0));
    // SPRacingH7NEO, RefDes: SP52
    const PLACEMENT_POINT_REAL_WORLD: Point2<Decimal> = Point2::new(dec!(10.8), dec!(14.25));

    #[rstest]
    #[case::single_top(TransformTestCase {
        edge_rail_left_right: 0.0,
        edge_rail_top_bottom: 0.0,
        routing_gap: 0.0,
        eda_placement_export_offset: Vector2::new(dec!(0), dec!(0)),
        panel_size: Vector2::new(50.0, 50.0),
        panel_rotation: 0.0,
        unit_rotation: 0.0,
        assembly_flip: PcbAssemblyFlip::None,
        design_size: Vector2::new(50.0, 50.0),
    }, PLACEMENT_POINT_TEST, UnitPlacementPosition { x: dec!(10.0), y: dec!(20.0), rotation: dec!(0) })]
    #[case::single_bottom_pitch_flipped(TransformTestCase {
        edge_rail_left_right: 0.0,
        edge_rail_top_bottom: 0.0,
        routing_gap: 0.0,
        eda_placement_export_offset: Vector2::new(dec!(0), dec!(0)),
        panel_size: Vector2::new(50.0, 50.0),
        panel_rotation: 0.0,
        unit_rotation: 0.0,
        assembly_flip: PcbAssemblyFlip::Pitch,
        design_size: Vector2::new(50.0, 50.0),
    }, PLACEMENT_POINT_TEST, UnitPlacementPosition { x: dec!(10.0), y: dec!(30.0), rotation: dec!(180) })]
    #[case::single_bottom_roll_flipped(TransformTestCase {
        edge_rail_left_right: 0.0,
        edge_rail_top_bottom: 0.0,
        routing_gap: 0.0,
        eda_placement_export_offset: Vector2::new(dec!(0), dec!(0)),
        panel_size: Vector2::new(50.0, 50.0),
        panel_rotation: 0.0,
        unit_rotation: 0.0,
        assembly_flip: PcbAssemblyFlip::Roll,
        design_size: Vector2::new(50.0, 50.0),
    }, PLACEMENT_POINT_TEST, UnitPlacementPosition { x: dec!(40.0), y: dec!(20.0), rotation: dec!(180) })]
    #[case::single_top_placement_offset(TransformTestCase {
        edge_rail_left_right: 0.0,
        edge_rail_top_bottom: 0.0,
        routing_gap: 0.0,
        eda_placement_export_offset: Vector2::new(dec!(10), dec!(10)),
        panel_size: Vector2::new(50.0, 50.0),
        panel_rotation: 0.0,
        unit_rotation: 0.0,
        assembly_flip: PcbAssemblyFlip::None,
        design_size: Vector2::new(50.0, 50.0),
    }, PLACEMENT_POINT_TEST, UnitPlacementPosition { x: dec!(10.0), y: dec!(20.0), rotation: dec!(0) })]
    #[case::rectangular_panel_top_with_rails_and_routing_gap_and_placement_offset(TransformTestCase {
        edge_rail_left_right: 5.0,
        edge_rail_top_bottom: 10.0,
        routing_gap: 2.0,
        eda_placement_export_offset: Vector2::new(dec!(10), dec!(10)),
        panel_size: Vector2::new(150.0, 100.0),
        panel_rotation: 0.0,
        unit_rotation: 0.0,
        assembly_flip: PcbAssemblyFlip::None,
        design_size: Vector2::new(50.0, 50.0),
    }, PLACEMENT_POINT_TEST, UnitPlacementPosition { x: dec!(17.0), y: dec!(32.0), rotation: dec!(0) })]
    #[case::rectangular_panel_bottom_pitch_flipped_with_rails_and_routing_gap_and_placement_offset(TransformTestCase {
        edge_rail_left_right: 5.0,
        edge_rail_top_bottom: 10.0,
        routing_gap: 2.0,
        eda_placement_export_offset: Vector2::new(dec!(10), dec!(10)),
        panel_size: Vector2::new(150.0, 100.0),
        panel_rotation: 0.0,
        unit_rotation: 0.0,
        assembly_flip: PcbAssemblyFlip::Pitch,
        design_size: Vector2::new(50.0, 50.0),
    }, PLACEMENT_POINT_TEST, UnitPlacementPosition { x: dec!(17.0), y: dec!(68.0), rotation: dec!(180) })]
    #[case::rectangular_panel_bottom_roll_flipped_with_rails_and_routing_gap_and_placement_offset(TransformTestCase {
        edge_rail_left_right: 5.0,
        edge_rail_top_bottom: 10.0,
        routing_gap: 2.0,
        eda_placement_export_offset: Vector2::new(dec!(10), dec!(10)),
        panel_size: Vector2::new(150.0, 100.0),
        panel_rotation: 0.0,
        unit_rotation: 0.0,
        assembly_flip: PcbAssemblyFlip::Roll,
        design_size: Vector2::new(50.0, 50.0),
    }, PLACEMENT_POINT_TEST, UnitPlacementPosition { x: dec!(133.0), y: dec!(32.0), rotation: dec!(180) })]
    #[case::rotated_rectangular_panel_top_with_rails_and_routing_gap_and_placement_offset(TransformTestCase {
        edge_rail_left_right: 5.0,
        edge_rail_top_bottom: 10.0,
        routing_gap: 2.0,
        eda_placement_export_offset: Vector2::new(dec!(10), dec!(10)),
        panel_size: Vector2::new(150.0, 100.0),
        panel_rotation: 45.0,
        unit_rotation: 0.0,
        assembly_flip: PcbAssemblyFlip::None,
        design_size: Vector2::new(50.0, 50.0),
    }, PLACEMENT_POINT_TEST, UnitPlacementPosition { x: dec!(60.10407640085654), y: dec!(34.648232278140824), rotation: dec!(45) })]
    #[case::rotated_rectangular_panel_bottom_pitch_flipped_with_rails_and_routing_gap_and_placement_offset(TransformTestCase {
        edge_rail_left_right: 5.0,
        edge_rail_top_bottom: 10.0,
        routing_gap: 2.0,
        eda_placement_export_offset: Vector2::new(dec!(10), dec!(10)),
        panel_size: Vector2::new(150.0, 100.0),
        panel_rotation: 45.0,
        unit_rotation: 0.0,
        assembly_flip: PcbAssemblyFlip::Pitch,
        design_size: Vector2::new(50.0, 50.0),
    }, PLACEMENT_POINT_TEST, UnitPlacementPosition { x: dec!(34.648232278140824), y: dec!(60.10407640085654), rotation: dec!(-135) })]
    #[case::rotated_rectangular_panel_bottom_roll_flipped_with_rails_and_routing_gap_and_placement_offset(TransformTestCase {
        edge_rail_left_right: 5.0,
        edge_rail_top_bottom: 10.0,
        routing_gap: 2.0,
        eda_placement_export_offset: Vector2::new(dec!(10), dec!(10)),
        panel_size: Vector2::new(150.0, 100.0),
        panel_rotation: 45.0,
        unit_rotation: 0.0,
        assembly_flip: PcbAssemblyFlip::Roll,
        design_size: Vector2::new(50.0, 50.0),
    }, PLACEMENT_POINT_TEST, UnitPlacementPosition { x: dec!(142.12846301849606), y: dec!(116.67261889578035), rotation: dec!(-135) })]
    #[case::rotated_rectangular_panel_top_with_rails_and_routing_gap_and_placement_offset_and_rotated_units(TransformTestCase {
        edge_rail_left_right: 5.0,
        edge_rail_top_bottom: 10.0,
        routing_gap: 2.0,
        eda_placement_export_offset: Vector2::new(dec!(10), dec!(10)),
        panel_size: Vector2::new(150.0, 100.0),
        panel_rotation: 45.0,
        unit_rotation: 270.0,
        assembly_flip: PcbAssemblyFlip::None,
        design_size: Vector2::new(50.0, 50.0),
    }, PLACEMENT_POINT_TEST, UnitPlacementPosition { x: dec!(53.033008588991066), y: dec!(55.86143571373725), rotation: dec!(-45) })]
    #[case::rotated_rectangular_panel_bottom_pitch_flipped_with_rails_and_routing_gap_and_placement_offset_and_rotated_units(TransformTestCase {
        edge_rail_left_right: 5.0,
        edge_rail_top_bottom: 10.0,
        routing_gap: 2.0,
        eda_placement_export_offset: Vector2::new(dec!(10), dec!(10)),
        panel_size: Vector2::new(150.0, 100.0),
        panel_rotation: 45.0,
        unit_rotation: 270.0,
        assembly_flip: PcbAssemblyFlip::Pitch,
        design_size: Vector2::new(50.0, 50.0),
    }, PLACEMENT_POINT_TEST, UnitPlacementPosition { x: dec!(55.86143571373725), y: dec!(53.033008588991066), rotation: dec!(135) })]
    #[case::rotated_rectangular_panel_bottom_roll_flipped_with_rails_and_routing_gap_and_placement_offset_and_rotated_units(TransformTestCase {
        edge_rail_left_right: 5.0,
        edge_rail_top_bottom: 10.0,
        routing_gap: 2.0,
        eda_placement_export_offset: Vector2::new(dec!(10), dec!(10)),
        panel_size: Vector2::new(150.0, 100.0),
        panel_rotation: 45.0,
        unit_rotation: 270.0,
        assembly_flip: PcbAssemblyFlip::Roll,
        design_size: Vector2::new(50.0, 50.0),
    }, PLACEMENT_POINT_TEST, UnitPlacementPosition { x: dec!(120.91525958289964), y: dec!(123.74368670764582), rotation: dec!(135) })]
    #[case::real_world(TransformTestCase {
        edge_rail_left_right: 5.0,
        edge_rail_top_bottom: 5.0,
        routing_gap: 2.0,
        eda_placement_export_offset: Vector2::new(dec!(10), dec!(10)),
        panel_size: Vector2::new(135.0, 94.0),
        panel_rotation: 90.0,
        unit_rotation: 90.0,
        assembly_flip: PcbAssemblyFlip::None,
        design_size: Vector2::new(39.0, 39.0),
    }, PLACEMENT_POINT_REAL_WORLD, UnitPlacementPosition { x: dec!(76.2), y: dec!(31.75), rotation: dec!(180) })]
    #[case::real_world_pitch_flipped(TransformTestCase {
        edge_rail_left_right: 5.0,
        edge_rail_top_bottom: 5.0,
        routing_gap: 2.0,
        eda_placement_export_offset: Vector2::new(dec!(10), dec!(10)),
        panel_size: Vector2::new(135.0, 94.0),
        panel_rotation: 90.0,
        unit_rotation: 90.0,
        assembly_flip: PcbAssemblyFlip::Pitch,
        design_size: Vector2::new(39.0, 39.0),
    }, PLACEMENT_POINT_REAL_WORLD, UnitPlacementPosition { x: dec!(17.799999999999997), y: dec!(31.750000000000007), rotation: dec!(0) })]
    #[case::real_world_pitch_flipped(TransformTestCase {
        edge_rail_left_right: 5.0,
        edge_rail_top_bottom: 5.0,
        routing_gap: 2.0,
        eda_placement_export_offset: Vector2::new(dec!(10), dec!(10)),
        panel_size: Vector2::new(135.0, 94.0),
        panel_rotation: 90.0,
        unit_rotation: 90.0,
        assembly_flip: PcbAssemblyFlip::Roll,
        design_size: Vector2::new(39.0, 39.0),
    }, PLACEMENT_POINT_REAL_WORLD, UnitPlacementPosition { x: dec!(76.2), y: dec!(103.25), rotation: dec!(0) })]
    fn apply_to_placement_matrix(
        #[case] test_case: TransformTestCase,
        #[case] placement_point: Point2<Decimal>,
        #[case] known_good: UnitPlacementPosition,
    ) {
        let TransformTestCase {
            edge_rail_left_right,
            edge_rail_top_bottom,
            routing_gap,
            eda_placement_export_offset,
            panel_size,
            panel_rotation,
            unit_rotation,
            assembly_flip,
            design_size,
        } = test_case;

        // given
        let panel_center = panel_size / 2.0;
        let panel_rotation_decimal = Decimal::from_f64(panel_rotation).unwrap();
        let panel_rotation_radians = panel_rotation.to_radians();

        let unit_rotation_decimal = Decimal::from_f64(unit_rotation).unwrap();
        let unit_rotation_radians = unit_rotation.to_radians();

        let orientation = PcbSideAssemblyOrientation {
            flip: assembly_flip,
            rotation: panel_rotation_decimal,
        };

        println!("panel_center: {:?}", panel_center);

        let design_sizing = DesignSizing {
            size: design_size,
            gerber_offset: Vector2::new(-5.0, -5.0),
            placement_offset: Vector2::new(
                -eda_placement_export_offset
                    .x
                    .to_f64()
                    .unwrap(),
                -eda_placement_export_offset
                    .y
                    .to_f64()
                    .unwrap(),
            ),
            origin: Vector2::new(design_size.x / 2.0, design_size.y / 2.0),
        };

        let unit_offset = Vector2::new(edge_rail_left_right + routing_gap, edge_rail_top_bottom + routing_gap);

        let placement1 = Placement {
            ref_des: "SP60".into(),
            part: Part {
                manufacturer: "MFR1".to_string(),
                mpn: "MPN1".to_string(),
            },
            place: true,
            pcb_side: PcbSide::Top,
            x: eda_placement_export_offset.x + placement_point.x,
            y: eda_placement_export_offset.y + placement_point.y,
            rotation: Decimal::from(0),
        };

        // Update to account for unit rotation in our calculation
        let placement_coords = Point2::new(
            placement1.x.to_f64().unwrap()
                + -eda_placement_export_offset
                    .x
                    .to_f64()
                    .unwrap(),
            placement1.y.to_f64().unwrap()
                + -eda_placement_export_offset
                    .y
                    .to_f64()
                    .unwrap(),
        );

        // First rotate around design origin for unit rotation
        let placement_coords_after_unit_rotation =
            rotate_point_around_center(placement_coords, design_sizing.origin, unit_rotation_radians);

        // Then translate to unit position
        let unit1_placement1_coords = Point2::new(
            unit_offset.x + placement_coords_after_unit_rotation.x,
            unit_offset.y + placement_coords_after_unit_rotation.y,
        );

        println!(
            "unit1_placement1_coords (before rotation and flipping): {:?}",
            unit1_placement1_coords
        );

        // Apply the appropriate flip transformation
        let unit1_placement1_coords_flipped = match orientation.flip {
            PcbAssemblyFlip::None => unit1_placement1_coords,
            PcbAssemblyFlip::Pitch => {
                // Mirror y-coordinate around the center of the panel
                let panel_center_y = panel_size.y / 2.0;
                let distance_from_center = unit1_placement1_coords.y - panel_center_y;
                Point2::new(unit1_placement1_coords.x, panel_center_y - distance_from_center)
            }
            PcbAssemblyFlip::Roll => {
                // Mirror x-coordinate around the center of the panel
                let panel_center_x = panel_size.x / 2.0;
                let distance_from_center = unit1_placement1_coords.x - panel_center_x;
                Point2::new(panel_center_x - distance_from_center, unit1_placement1_coords.y)
            }
        };
        println!(
            "unit1_placement1_coords (after flipping): {:?}",
            unit1_placement1_coords_flipped
        );

        fn rotate_point_around_center(pt: Point2<f64>, center: Vector2<f64>, angle_radians: f64) -> Point2<f64> {
            let cos_theta = angle_radians.cos();
            let sin_theta = angle_radians.sin();

            Point2::new(
                center.x + (pt.x - center.x) * cos_theta - (pt.y - center.y) * sin_theta,
                center.y + (pt.x - center.x) * sin_theta + (pt.y - center.y) * cos_theta,
            )
        }

        fn rotate_panel_around_center(panel_size: Vector2<f64>, angle_radians: f64) -> [Point2<f64>; 4] {
            let panel_center = panel_size / 2.0;

            let mut panel_corners = [
                Point2::new(0.0, 0.0),
                Point2::new(panel_size.x, 0.0),
                Point2::new(panel_size.x, panel_size.y),
                Point2::new(0.0, panel_size.y),
            ];
            println!("panel_corners (test): {:?}", panel_corners);

            for point in panel_corners.iter_mut() {
                *point = rotate_point_around_center(*point, panel_center, angle_radians)
            }

            panel_corners
        }

        let rotated_corners = rotate_panel_around_center(panel_size, panel_rotation_radians);
        println!("rotated_corners (test): {:?}", rotated_corners);

        let shift = Vector2::new(
            rotated_corners
                .iter()
                .map(|p| p.x)
                .fold(f64::INFINITY, f64::min),
            rotated_corners
                .iter()
                .map(|p| p.y)
                .fold(f64::INFINITY, f64::min),
        );
        println!("shift (test): {:?}", shift);

        let unit1_placement1_coords_after_rotation =
            rotate_point_around_center(unit1_placement1_coords_flipped, panel_center, panel_rotation_radians);

        println!(
            "unit1_placement1_coords (after rotation): {:?}",
            unit1_placement1_coords_after_rotation
        );

        let unit1_placement1_coords_final = unit1_placement1_coords_after_rotation - shift;
        println!("unit1_placement1_coords_final: {:?}", unit1_placement1_coords_final);

        // For rotation calculation, first apply flip effect on angle if needed (after rotation)

        let mut flipped_rotation = placement1.rotation;
        if !matches!(orientation.flip, PcbAssemblyFlip::None) {
            flipped_rotation = dec!(180.0) - flipped_rotation;
        }

        // Then apply the panel and unit rotations
        let new_rotation = flipped_rotation + panel_rotation_decimal + unit_rotation_decimal;

        // Normalize rotation to be within -180 to 180 degrees
        let normalized_rotation = normalize_angle_deg_signed_decimal(new_rotation).normalize();

        let unit1_placement_rotation_decimal = normalized_rotation;
        println!(
            "unit1_placement_rotation_decimal: {:?}",
            unit1_placement_rotation_decimal
        );

        let transform = PcbUnitTransform {
            unit_offset,
            unit_rotation: unit_rotation_decimal,
            design_sizing,
            orientation,
            panel_size,
        };

        // and
        let expected_result = UnitPlacementPosition {
            x: Decimal::from_f64(unit1_placement1_coords_final.x).unwrap(),
            y: Decimal::from_f64(unit1_placement1_coords_final.y).unwrap(),
            rotation: unit1_placement_rotation_decimal,
        };

        // when
        let result = transform.apply_to_placement_matrix(&placement1);

        // then

        let equality_epsilon = Decimal::from_str("0.000000001").unwrap();

        assert!(
            approx_eq_position(&result, &expected_result, equality_epsilon),
            "Expected position close to {:?}, got {:?}",
            expected_result,
            result
        );

        // Assert with our known-good value.
        // This is a sanity check to ensure that our test and production code don't both become broken at the same time.
        assert!(
            approx_eq_position(&result, &known_good, equality_epsilon),
            "Expected position is known-good {:?}, got {:?}",
            known_good,
            result
        );
    }
}

// Helper function for floating-point equality with tolerance
fn approx_eq_decimal(a: Decimal, b: Decimal, epsilon: Decimal) -> bool {
    (a - b).abs() <= epsilon
}

// Helper function to compare UnitPlacementPosition with tolerance
fn approx_eq_position(a: &UnitPlacementPosition, b: &UnitPlacementPosition, epsilon: Decimal) -> bool {
    approx_eq_decimal(a.x, b.x, epsilon)
        && approx_eq_decimal(a.y, b.y, epsilon)
        && approx_eq_decimal(a.rotation, b.rotation, epsilon)
}
