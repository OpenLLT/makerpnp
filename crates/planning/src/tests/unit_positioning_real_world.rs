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

    #[rstest]
    #[case(5.0, 5.0, 2.0, Vector2::new(dec!(10), dec!(10)), Vector2::new(135.0, 94.0), 45.0, 180.0, PcbAssemblyFlip::None, Vector2::new(39.0, 39.0))]
    pub fn apply_to_placement_matrix(
        #[case] edge_rail_left_right: f64,
        #[case] edge_rail_top_bottom: f64,
        #[case] routing_gap: f64,
        #[case] eda_placement_export_offset: Vector2<Decimal>,
        #[case] panel_size: Vector2<f64>,
        #[case] panel_rotation: f64,
        #[case] unit_rotation: f64,
        #[case] assembly_flip: PcbAssemblyFlip,
        #[case] design_size: Vector2<f64>,
    ) {
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
            placement_offset: Vector2::new(-10.0, -10.0),
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
            x: eda_placement_export_offset.x + dec!(1.6),
            y: eda_placement_export_offset.y + dec!(1.6),
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
            "unit1_placement1_coords (before rotation): {:?}",
            unit1_placement1_coords
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
            rotate_point_around_center(unit1_placement1_coords, panel_center, panel_rotation_radians);

        println!(
            "unit1_placement1_coords (after rotation): {:?}",
            unit1_placement1_coords_after_rotation
        );

        let unit1_placement1_coords_after_rotation_and_translation = unit1_placement1_coords_after_rotation - shift;
        println!(
            "unit1_placement1_coords (after rotation and translation): {:?}",
            unit1_placement1_coords_after_rotation_and_translation
        );

        let mut new_rotation = placement1.rotation + panel_rotation_decimal + unit_rotation_decimal;

        // If flip the rotation
        if !matches!(orientation.flip, PcbAssemblyFlip::None) {
            new_rotation = dec!(180.0) - new_rotation;
        }

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
            x: Decimal::from_f64(unit1_placement1_coords_after_rotation_and_translation.x).unwrap(),
            y: Decimal::from_f64(unit1_placement1_coords_after_rotation_and_translation.y).unwrap(),
            rotation: unit1_placement_rotation_decimal,
        };

        // when
        let result = transform.apply_to_placement_matrix(&placement1);

        // then

        // Use approximate equality with a small epsilon
        let epsilon = Decimal::from_str("0.000000001").unwrap();

        // Assert with our custom comparison
        assert!(
            approx_eq_position(&result, &expected_result, epsilon),
            "Expected position close to {:?}, got {:?}",
            expected_result,
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
