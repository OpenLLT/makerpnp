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
    use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
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

        let design_x = panel_sizing.edge_rails.top
            + panel_sizing.design_sizings[0]
                .placement_offset
                .x;
        let design_y = panel_sizing.edge_rails.left
            + panel_sizing.design_sizings[0]
                .placement_offset
                .y;
        let unit_1_x = panel_sizing.pcb_unit_positionings[0]
            .offset
            .x;
        let unit_1_y = panel_sizing.pcb_unit_positionings[0]
            .offset
            .y;

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
        let expectations: [[Decimal; 3]; 1] = [[
            Decimal::from_f64(design_x + unit_1_x + 1.6).unwrap(),
            Decimal::from_f64(design_y + unit_1_y + 1.6).unwrap(),
            dec!(90.0),
        ]];

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
    use nalgebra::{Point2, Vector2};
    use pnp::panel::DesignSizing;
    use pnp::part::Part;
    use pnp::pcb::PcbSide;
    use pnp::placement::Placement;
    use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    use crate::pcb::{PcbAssemblyFlip, PcbSideAssemblyOrientation, PcbUnitTransform, UnitPlacementPosition};

    #[test]
    pub fn apply_to_placement_matrix() {
        // given

        let edge_rail_top_bottom = 5.0;
        let edge_rail_left_right = 5.0;
        let routing_gap = 2.0;
        let eda_placement_export_offset = Vector2::new(dec!(10), dec!(10));

        let panel_size = Vector2::new(135.0, 94.0);
        let panel_center = panel_size / 2.0;
        let panel_rotation = 45.0_f64;
        let panel_rotation_radians = panel_rotation.to_radians();

        println!("panel_center: {:?}", panel_center);

        let design_sizing = DesignSizing {
            size: Vector2::new(39.0, 39.0),
            gerber_offset: Vector2::new(-5.0, -5.0),
            placement_offset: Vector2::new(-10.0, -10.0),
            origin: Vector2::new(19.5, 19.5),
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

        let unit1_placement1_coords = Point2::new(
            unit_offset.x
                + placement1.x.to_f64().unwrap()
                + -eda_placement_export_offset
                    .x
                    .to_f64()
                    .unwrap(),
            unit_offset.y
                + placement1.y.to_f64().unwrap()
                + -eda_placement_export_offset
                    .y
                    .to_f64()
                    .unwrap(),
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
            println!("panel_corners: {:?}", panel_corners);

            for point in panel_corners.iter_mut() {
                *point = rotate_point_around_center(*point, panel_center, angle_radians)
            }

            panel_corners
        }

        let rotated_corners = rotate_panel_around_center(panel_size, panel_rotation_radians);
        println!("rotated_corners: {:?}", rotated_corners);

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
        println!("shift: {:?}", shift);

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

        let transform = PcbUnitTransform {
            unit_offset,
            unit_rotation: dec!(0.0),
            design_sizing,
            orientation: PcbSideAssemblyOrientation {
                flip: PcbAssemblyFlip::None,
                rotation: dec!(90.0),
            },
            panel_size,
        };

        // and
        let expected_result = UnitPlacementPosition {
            x: Decimal::from_f64(unit1_placement1_coords_after_rotation_and_translation.x).unwrap(),
            y: Decimal::from_f64(unit1_placement1_coords_after_rotation_and_translation.y).unwrap(),
            rotation: dec!(90.0),
        };

        // when
        let result = transform.apply_to_placement_matrix(&placement1);

        // then
        assert_eq!(result, expected_result);
    }
}
