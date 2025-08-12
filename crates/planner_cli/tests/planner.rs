#[macro_use]
extern crate util;

use std::collections::BTreeMap;

use nalgebra::Vector2;
use planning::design::DesignIndex;
use pnp::panel::Dimensions;
use pnp::pcb::PcbUnitIndex;

use crate::common::project_builder::TestDesignSizing;

pub mod common;

mod operation_sequence_1 {
    use std::collections::BTreeMap;
    use std::fs::{read_to_string, File};
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::str::FromStr;

    use assert_cmd::Command;
    use indoc::indoc;
    use nalgebra::Vector2;
    use planning::design::DesignVariant;
    use planning::file::FileReference;
    use planning::pcb::PcbAssemblyOrientation;
    use planning::placement::{PlacementOperation, PlacementStatus, ProjectPlacementStatus};
    use planning::process::TaskStatus;
    use pnp::object_path::ObjectPath;
    use pnp::panel::Dimensions;
    use pnp::pcb::PcbSide;
    use rust_decimal_macros::dec;
    use stores::test::load_out_builder::{LoadOutCSVBuilder, TestLoadOutRecord};
    use tempfile::tempdir;
    use util::test::{build_temp_file, prepare_args, print};

    use crate::common::operation_history::{
        TestLoadPcbsOperationTaskHistoryKind, TestOperationHistoryItem, TestOperationHistoryKind,
        TestPlaceComponentsOperationTaskHistoryKind, TestPlacementOperationHistoryKind,
    };
    use crate::common::phase_placement_builder::{PhasePlacementsCSVBuilder, TestPhasePlacementRecord};
    use crate::common::project_builder as project;
    use crate::common::project_builder::{
        TestAutomatedSolderingTaskState, TestDesignSizing, TestLoadPcbsTaskState, TestManualSolderingTaskState,
        TestOperationState, TestPartState, TestPcbUnitPositioning, TestPhase, TestPlacement, TestPlacementState,
        TestPlacementTaskState, TestProcessOperationStatus, TestProject, TestSerializableTaskState, TestUnitPosition,
    };
    use crate::common::project_report_builder as report;
    use crate::common::project_report_builder::{
        ProjectReportBuilder, TestAutomatedSolderingTaskOverview, TestAutomatedSolderingTaskSpecification, TestIssue,
        TestIssueKind, TestIssueSeverity, TestLoadPcbsTaskOverview, TestLoadPcbsTaskSpecification,
        TestManualSolderingTaskOverview, TestManualSolderingTaskSpecification, TestPart, TestPcbUnitAssignment,
        TestPhaseLoadOutAssignmentItem, TestPhaseOperation, TestPhaseOperationOverview, TestPhaseOverview,
        TestPhaseSpecification, TestPlaceComponentsTaskOverview, TestPlaceComponentsTaskSpecification,
        TestTaskOverview, TestTaskSpecification,
    };
    use crate::{calculate_offset, calculate_size};

    /// A context, which will be dropped when the tests are completed.
    mod context {
        use std::fs;
        use std::path::Path;
        use std::sync::{Mutex, MutexGuard};
        use std::thread::sleep;
        use std::time::Duration;

        use super::*;

        #[derive(Debug)]
        pub struct Context {
            pub temp_dir: tempfile::TempDir,

            pub trace_log_arg: String,
            pub path_arg: String,
            pub project_arg: String,
            pub test_trace_log_path: PathBuf,
            pub test_project_path: PathBuf,
            pub test_pcb_1_path: PathBuf,
            pub test_pcb_1_arg: String,
            pub phase_1_load_out_path: PathBuf,
            pub phase_2_load_out_path: PathBuf,
            pub phase_1_log_path: PathBuf,
        }

        impl Context {
            pub fn new() -> Self {
                let temp_dir = tempdir().unwrap();

                let path_arg = format!("--path {}", temp_dir.path().to_str().unwrap());

                let (test_trace_log_path, test_trace_log_file_name) = build_temp_file(&temp_dir, "trace", "log");
                let trace_log_arg = format!(
                    "--trace {}",
                    test_trace_log_file_name
                        .to_str()
                        .unwrap()
                );

                let (test_project_path, _test_project_file_name) =
                    build_temp_file(&temp_dir, "project-job1", "mpnp.json");

                let (test_pcb_1_path, _test_pcb_1_file_name) = build_temp_file(&temp_dir, "panel_a", "pcb.json");
                let test_pcb_1_arg = format!("--pcb-file {}", test_pcb_1_path.to_str().unwrap());

                let project_arg = "--project job1".to_string();

                let mut phase_1_load_out_path = PathBuf::from(temp_dir.path());
                phase_1_load_out_path.push("phase_1_top_1_load_out.csv");

                let mut phase_1_log_path = PathBuf::from(temp_dir.path());
                phase_1_log_path.push("top_1_log.json");

                let mut phase_2_load_out_path = PathBuf::from(temp_dir.path());
                phase_2_load_out_path.push("phase_2_bottom_1_load_out.csv");

                Context {
                    temp_dir,
                    path_arg,
                    project_arg,
                    trace_log_arg,
                    test_trace_log_path,
                    test_project_path,
                    test_pcb_1_path,
                    test_pcb_1_arg,
                    phase_1_load_out_path,
                    phase_1_log_path,
                    phase_2_load_out_path,
                }
            }

            pub fn delete_trace_log(&self) {
                if Path::new(&self.test_trace_log_path).exists() {
                    println!(
                        "deleting trace log: {}",
                        self.test_trace_log_path
                            .to_str()
                            .unwrap()
                    );
                    fs::remove_file(&self.test_trace_log_path).unwrap();
                }
            }
        }

        impl Drop for Context {
            fn drop(&mut self) {
                println!(
                    "destroying context. temp_dir: {}",
                    self.temp_dir.path().to_str().unwrap()
                );
            }
        }

        /// IMPORTANT: lock content must be dropped manually, as static items are never dropped.
        static LOCK: Mutex<(usize, Option<Context>)> = Mutex::new((0, None));

        /// Use a mutex to prevent multiple test threads interacting with the same static state.
        /// This can happen when tests use the same mock context.  Without this mechanism tests will
        /// interact with each other causing unexpected results and test failures.
        pub fn acquire(sequence: usize) -> MutexGuard<'static, (usize, Option<Context>)> {
            let mut lock = loop {
                let mut lock = LOCK
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if lock.0 == sequence - 1 {
                    lock.0 += 1;
                    break lock;
                }
                drop(lock);

                sleep(Duration::from_millis(100));
            };

            if lock.1.is_none() {
                lock.1.replace(Context::new());
            }

            lock
        }
    }

    pub fn read_and_show_file(path: &Path) -> std::io::Result<String> {
        println!("File: {:?}", path);
        let content: String = read_to_string(path)?;
        println!("{}", content);

        Ok(content)
    }

    #[test]
    fn sequence_01_create_job() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(1);
        let ctx = ctx_guard.1.as_mut().unwrap();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_default_processes()
            .content();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "-vvv",
            "create",
        ]);
        println!("args: {:?}", args);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        assert_contains_inorder!(trace_content, ["Created project successfully",]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_02_create_pcb() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(2);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_pcb_1_content = project::TestPcb {
            name: "panel_a".to_string(),
            units: 4,
            design_names: vec!["design_a".into(), "design_b".into()],
            unit_map: BTreeMap::from_iter([(0, 0), (1, 1), (2, 0), (3, 1)]),
            gerber_offset: Default::default(),
            panel_sizing: project::TestPanelSizing {
                units: Default::default(),
                size: Default::default(),
                edge_rails: Default::default(),
                fiducials: vec![],
                design_sizings: vec![
                    TestDesignSizing {
                        origin: Default::default(),
                        gerber_offset: Default::default(),
                        placement_offset: Default::default(),
                        size: Default::default(),
                    },
                    TestDesignSizing {
                        origin: Default::default(),
                        gerber_offset: Default::default(),
                        placement_offset: Default::default(),
                        size: Default::default(),
                    },
                ],
                pcb_unit_positionings: vec![
                    TestPcbUnitPositioning {
                        offset: Default::default(),
                        rotation: dec!(0),
                    },
                    TestPcbUnitPositioning {
                        offset: Default::default(),
                        rotation: dec!(0),
                    },
                    TestPcbUnitPositioning {
                        offset: Default::default(),
                        rotation: dec!(0),
                    },
                    TestPcbUnitPositioning {
                        offset: Default::default(),
                        rotation: dec!(0),
                    },
                ],
            },
            orientation: PcbAssemblyOrientation::default(),
        }
        .content();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "pcb",
            "-vvv",
            ctx.test_pcb_1_arg.as_str(),
            "create",
            "--name panel_a",
            "--units 4",
            "--design 1=design_a,2=design_b,3=design_a,4=design_b",
        ]);
        println!("args: {:?}", args);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and

        let saving_pcb_message = &format!("Saving PCB. path: {:?}\n", ctx.test_pcb_1_path);

        assert_contains_inorder!(trace_content, [
            "Creating PCB. name: 'panel_a'\n",
            "Added designs to PCB. design: [design_a, design_b]\n",
            saving_pcb_message,
        ]);

        // and
        let pcb_1_content: String = read_and_show_file(&ctx.test_pcb_1_path)?;

        assert_eq!(pcb_1_content, expected_pcb_1_content);

        Ok(())
    }

    #[test]
    fn sequence_03_configure_panel_sizing() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(3);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        let edge_rails = Dimensions {
            left: 10.0,
            right: 10.0,
            top: 5.0,
            bottom: 5.0,
        };
        let routing_gap = 2.0;

        let design_names = vec!["design_a".into(), "design_b".into()];
        let design_a_sizing = TestDesignSizing {
            size: Vector2::new(60.0, 50.0),
            origin: Vector2::new(30.0, 25.0),
            gerber_offset: Vector2::new(-20.0, -20.0),
            placement_offset: Vector2::new(-10.0, -10.0),
        };
        let design_b_sizing = TestDesignSizing {
            size: Vector2::new(40.0, 50.0),
            origin: Vector2::new(20.0, 25.0),
            gerber_offset: Vector2::new(-20.0, -20.0),
            placement_offset: Vector2::new(-10.0, -10.0),
        };
        let design_sizings = vec![design_a_sizing, design_b_sizing];

        let unit_map = BTreeMap::from_iter([(0, 0), (1, 1), (2, 0), (3, 1)]);

        let size = calculate_size(&unit_map, &design_sizings, &edge_rails, routing_gap, 2, 2);

        let pcb_unit_positions = vec![
            TestPcbUnitPositioning {
                offset: calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 2, 2, 0, 0),
                rotation: dec!(0),
            },
            TestPcbUnitPositioning {
                offset: calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 2, 2, 1, 0),
                rotation: dec!(0),
            },
            TestPcbUnitPositioning {
                offset: calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 2, 2, 0, 1),
                rotation: dec!(0),
            },
            TestPcbUnitPositioning {
                offset: calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 2, 2, 1, 1),
                rotation: dec!(0),
            },
        ];

        // and
        let expected_pcb_1_content = project::TestPcb {
            name: "panel_a".to_string(),
            units: 4,
            design_names: design_names.clone(),
            unit_map: unit_map.clone(),
            gerber_offset: Default::default(),
            panel_sizing: project::TestPanelSizing {
                units: Default::default(),
                size,
                edge_rails: edge_rails.clone(),
                fiducials: vec![],
                design_sizings: design_sizings.clone(),
                pcb_unit_positionings: pcb_unit_positions.clone(),
            },
            orientation: PcbAssemblyOrientation::default(),
        }
        .content();

        // and
        let edge_rails_arg = format!(
            "--edge-rails left={},right={},top={},bottom={}",
            edge_rails.left, edge_rails.right, edge_rails.top, edge_rails.bottom
        );
        let size_arg = format!("--size x={},y={}", size.x, size.y);

        let design_sizings_args = design_sizings
            .iter()
            .zip(design_names)
            .map(|(design_sizing, design_name)| {
                format!(
                    "--design-sizing {}:origin=x={},y={}:g_offset=x={},y={}:p_offset=x={},y={}:size=x={},y={}",
                    design_name,
                    design_sizing.origin.x,
                    design_sizing.origin.y,
                    design_sizing.gerber_offset.x,
                    design_sizing.gerber_offset.y,
                    design_sizing.placement_offset.x,
                    design_sizing.placement_offset.y,
                    design_sizing.size.x,
                    design_sizing.size.y,
                )
            })
            .collect::<Vec<_>>();

        let pcb_unit_position_args = pcb_unit_positions
            .iter()
            .enumerate()
            .map(|(index, pcb_unit_position)| {
                format!(
                    "--pcb-unit-position {}:offset=x={},y={}:rotation={}",
                    index + 1,
                    pcb_unit_position.offset.x,
                    pcb_unit_position.offset.y,
                    pcb_unit_position.rotation,
                )
            })
            .collect::<Vec<_>>();

        let mut unprepared_args = vec![
            ctx.trace_log_arg.as_str(),
            "pcb",
            ctx.test_pcb_1_arg.as_str(),
            "-vvv",
            "configure-panel-sizing",
            edge_rails_arg.as_str(),
            size_arg.as_str(),
        ];
        unprepared_args.extend(
            design_sizings_args
                .iter()
                .map(|arg| arg.as_str()),
        );
        unprepared_args.extend(
            pcb_unit_position_args
                .iter()
                .map(|arg| arg.as_str()),
        );

        let args = prepare_args(unprepared_args);

        println!("args: {:?}", args);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and

        let saving_pcb_message = &format!("Saved PCB. path: {:?}\n", ctx.test_pcb_1_path);

        assert_contains_inorder!(trace_content, [saving_pcb_message,]);

        // and
        let pcb_1_content: String = read_and_show_file(&ctx.test_pcb_1_path)?;

        assert_eq!(pcb_1_content, expected_pcb_1_content);

        Ok(())
    }

    #[test]
    fn sequence_04_add_pcb() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(4);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::default(),
            }])
            .content();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "-vvv",
            "add-pcb",
            "--file relative=panel_a.pcb.json",
        ]);
        println!("args: {:?}", args);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        assert_contains_inorder!(trace_content, [
            "Added PCB to project. pcb_file: relative='panel_a.pcb.json'\n",
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_05_assign_variant_to_unit() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(5);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let design_a_variant_a_placements_csv_content = indoc! {r#"
            "RefDes","Manufacturer","Mpn","Place","PcbSide","X","Y","Rotation"
            "R3","RES_MFR1","RES1","true","Top","10","15","90"
            "C1","CAP_MFR1","CAP1","true","Bottom","40","45","180"
            "J1","CONN_MFR1","CONN1","true","Bottom","30","35","-90"
            "R1","RES_MFR1","RES1","true","Top","20","25","0"
        "#};
        // two refdes on the same side should use the same part (R1, R3)
        // all placements should be within the configured design size (design_a = 60x50) + export offset (10,10)

        let mut placements_path = ctx.temp_dir.path().to_path_buf();
        placements_path.push("design_a_variant_a_placements.csv");

        println!("creating placements, path: {:?}", &placements_path);

        let mut placements_file = File::create(placements_path)?;
        let _written = placements_file.write(design_a_variant_a_placements_csv_content.as_bytes())?;
        placements_file.flush()?;

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (project::TestPart::new("CAP_MFR1", "CAP1"), TestPartState::default()),
                (project::TestPart::new("CONN_MFR1", "CONN1"), TestPartState::default()),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::default()),
            ])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(30),
                            dec!(35),
                            dec!(-90),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(84), dec!(-90)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(25),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(22), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(10),
                            dec!(15),
                            dec!(90),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(12), dec!(90)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
            ])
            .content();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "assign-variant-to-unit",
            "--variant variant_a",
            "--unit pcb=1::unit=1",
        ]);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        assert_contains_inorder!(trace_content, [
            "Unit assignment added. unit: 'pcb=1::unit=1', variant_name: variant_a\n",
            "New part. part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }\n",
            "New part. part: Part { manufacturer: \"CAP_MFR1\", mpn: \"CAP1\" }\n",
            "New part. part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }\n",
            "New placement. placement: Placement { ref_des: \"R3\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 10, y: 15, rotation: 90 }",
            "New placement. placement: Placement { ref_des: \"C1\", part: Part { manufacturer: \"CAP_MFR1\", mpn: \"CAP1\" }, place: true, pcb_side: Bottom, x: 40, y: 45, rotation: 180 }",
            "New placement. placement: Placement { ref_des: \"J1\", part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }, place: true, pcb_side: Bottom, x: 30, y: 35, rotation: -90 }",
            "New placement. placement: Placement { ref_des: \"R1\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 20, y: 25, rotation: 0 }",
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_06_refresh_from_design_variants() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(6);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        let design_a_variant_a_placements_csv_content = indoc! {r#"
            "RefDes","Manufacturer","Mpn","Place","PcbSide","X","Y","Rotation"
            "R3","RES_MFR1","RES1","true","Top","40","40","135"
            "R2","RES_MFR2","RES2","true","Top","30","30","45"
            "J1","CONN_MFR1","CONN1","true","Bottom","10","10","0"
            "R1","RES_MFR1","RES1","true","Top","20","20","-45"
        "#};
        // R2 added (should be before R1). C1 deleted, all coordinates and rotations are changed.

        let mut placements_path = ctx.temp_dir.path().to_path_buf();
        placements_path.push("design_a_variant_a_placements.csv");

        let mut placments_file = File::create(placements_path)?;
        let _written = placments_file.write(design_a_variant_a_placements_csv_content.as_bytes())?;
        placments_file.flush()?;

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (project::TestPart::new("CONN_MFR1", "CONN1"), TestPartState::default()),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::default()),
                (project::TestPart::new("RES_MFR2", "RES2"), TestPartState::default()),
            ])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Unused,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(10),
                            dec!(10),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(109), dec!(180)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(20),
                            dec!(-45),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(17), dec!(-45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R2",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R2",
                            "RES_MFR2",
                            "RES2",
                            true,
                            PcbSide::Top,
                            dec!(30),
                            dec!(30),
                            dec!(45),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(27), dec!(45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(40),
                            dec!(40),
                            dec!(135),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(37), dec!(135)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
            ])
            .content();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "refresh-from-design-variants",
        ]);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        assert_contains_inorder!(trace_content, [
            "New part. part: Part { manufacturer: \"RES_MFR2\", mpn: \"RES2\" }\n",
            "Removing unused part. part: Part { manufacturer: \"CAP_MFR1\", mpn: \"CAP1\" }\n",
            "Updating placement. old: Placement { ref_des: \"R3\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 10, y: 15, rotation: 90 }, new: Placement { ref_des: \"R3\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 40, y: 40, rotation: 135 }",
            "New placement. placement: Placement { ref_des: \"R2\", part: Part { manufacturer: \"RES_MFR2\", mpn: \"RES2\" }, place: true, pcb_side: Top, x: 30, y: 30, rotation: 45 }",
            "Updating placement. old: Placement { ref_des: \"J1\", part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }, place: true, pcb_side: Bottom, x: 30, y: 35, rotation: -90 }, new: Placement { ref_des: \"J1\", part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }, place: true, pcb_side: Bottom, x: 10, y: 10, rotation: 0 }",
            "Updating placement. old: Placement { ref_des: \"R1\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 20, y: 25, rotation: 0 }, new: Placement { ref_des: \"R1\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 20, y: 20, rotation: -45 }",
            "Marking placement as unused. placement: Placement { ref_des: \"C1\", part: Part { manufacturer: \"CAP_MFR1\", mpn: \"CAP1\" }, place: true, pcb_side: Bottom, x: 40, y: 45, rotation: 180 }",
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_07_assign_process_to_parts() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(7);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (
                    project::TestPart::new("CONN_MFR1", "CONN1"),
                    TestPartState::new(&["manual"]),
                ),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::default()),
                (project::TestPart::new("RES_MFR2", "RES2"), TestPartState::default()),
            ])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Unused,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(10),
                            dec!(10),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(109), dec!(180)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(20),
                            dec!(-45),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(17), dec!(-45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R2",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R2",
                            "RES_MFR2",
                            "RES2",
                            true,
                            PcbSide::Top,
                            dec!(30),
                            dec!(30),
                            dec!(45),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(27), dec!(45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(40),
                            dec!(40),
                            dec!(135),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(37), dec!(135)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
            ])
            .content();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "assign-process-to-parts",
            "--process manual",
            "--operation add",
            "--manufacturer CONN_MFR.*",
            "--mpn .*",
        ]);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        assert_contains_inorder!(trace_content, [
            "Added process. part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }, applicable_processes: [\"manual\"]",
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_08_create_phase_top() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(8);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (
                    project::TestPart::new("CONN_MFR1", "CONN1"),
                    TestPartState::new(&["manual"]),
                ),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::default()),
                (project::TestPart::new("RES_MFR2", "RES2"), TestPartState::default()),
            ])
            .with_phases(vec![TestPhase::new(
                "top_1",
                "pnp",
                ctx.phase_1_load_out_path
                    .to_str()
                    .unwrap(),
                PcbSide::Top,
                &[],
            )])
            .with_phase_orderings(&["top_1"])
            .with_phase_states(vec![("top_1", vec![
                TestOperationState::new("load_pcbs", vec![(
                    "core::load_pcbs",
                    Box::new(TestLoadPcbsTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                )]),
                TestOperationState::new("automated_pnp", vec![(
                    "core::place_components",
                    Box::new(TestPlacementTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                )]),
                TestOperationState::new("reflow_oven_soldering", vec![(
                    "core::automated_soldering",
                    Box::new(TestAutomatedSolderingTaskState::new(TaskStatus::Pending))
                        as Box<dyn TestSerializableTaskState>,
                )]),
            ])])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Unused,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(10),
                            dec!(10),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(109), dec!(180)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(20),
                            dec!(-45),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(17), dec!(-45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R2",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R2",
                            "RES_MFR2",
                            "RES2",
                            true,
                            PcbSide::Top,
                            dec!(30),
                            dec!(30),
                            dec!(45),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(27), dec!(45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(40),
                            dec!(40),
                            dec!(135),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(37), dec!(135)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
            ])
            .content();

        // and
        let phase_1_load_out_arg = format!(
            "--load-out {}",
            ctx.phase_1_load_out_path
                .to_str()
                .unwrap()
        );

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "create-phase",
            "--reference top_1",
            "--process pnp",
            &phase_1_load_out_arg,
            "--pcb-side top",
        ]);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        let load_out_creation_message = format!(
            "Created load-out. source: '{}'",
            ctx.phase_1_load_out_path
                .to_str()
                .unwrap()
        );

        assert_contains_inorder!(trace_content, [
            &load_out_creation_message,
            "Created phase. reference: 'top_1', process: pnp",
            "Phase ordering: ['top_1']\n",
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_09_create_phase_bottom() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(9);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (
                    project::TestPart::new("CONN_MFR1", "CONN1"),
                    TestPartState::new(&["manual"]),
                ),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::default()),
                (project::TestPart::new("RES_MFR2", "RES2"), TestPartState::default()),
            ])
            .with_phases(vec![
                TestPhase::new(
                    "bottom_1",
                    "manual",
                    ctx.phase_2_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Bottom,
                    &[],
                ),
                TestPhase::new(
                    "top_1",
                    "pnp",
                    ctx.phase_1_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Top,
                    &[],
                ),
            ])
            .with_phase_orderings(&["top_1", "bottom_1"])
            .with_phase_states(vec![
                ("bottom_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "core::load_pcbs",
                        Box::new(TestLoadPcbsTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("manually_solder_components", vec![
                        (
                            "core::place_components",
                            Box::new(TestPlacementTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                        (
                            "core::manual_soldering",
                            Box::new(TestManualSolderingTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                    ]),
                ]),
                ("top_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "core::load_pcbs",
                        Box::new(TestLoadPcbsTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("automated_pnp", vec![(
                        "core::place_components",
                        Box::new(TestPlacementTaskState::new(TaskStatus::Pending))
                            as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("reflow_oven_soldering", vec![(
                        "core::automated_soldering",
                        Box::new(TestAutomatedSolderingTaskState::new(TaskStatus::Pending))
                            as Box<dyn TestSerializableTaskState>,
                    )]),
                ]),
            ])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Unused,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(10),
                            dec!(10),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(109), dec!(180)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(20),
                            dec!(-45),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(17), dec!(-45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R2",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R2",
                            "RES_MFR2",
                            "RES2",
                            true,
                            PcbSide::Top,
                            dec!(30),
                            dec!(30),
                            dec!(45),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(27), dec!(45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(40),
                            dec!(40),
                            dec!(135),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(37), dec!(135)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
            ])
            .content();

        // and
        let phase_2_load_out_arg = format!(
            "--load-out {}",
            ctx.phase_2_load_out_path
                .to_str()
                .unwrap()
        );

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "create-phase",
            "--reference bottom_1",
            "--process manual",
            &phase_2_load_out_arg,
            "--pcb-side bottom",
        ]);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        let load_out_creation_message = format!(
            "Created load-out. source: '{}'",
            ctx.phase_2_load_out_path
                .to_str()
                .unwrap()
        );

        assert_contains_inorder!(trace_content, [
            &load_out_creation_message,
            "Created phase. reference: 'bottom_1', process: manual",
            "Phase ordering: ['top_1', 'bottom_1']\n",
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_10_assign_placements_to_phase() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(10);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (
                    project::TestPart::new("CONN_MFR1", "CONN1"),
                    TestPartState::new(&["manual"]),
                ),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::new(&["pnp"])),
                (project::TestPart::new("RES_MFR2", "RES2"), TestPartState::new(&["pnp"])),
            ])
            .with_phases(vec![
                TestPhase::new(
                    "bottom_1",
                    "manual",
                    ctx.phase_2_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Bottom,
                    &[],
                ),
                TestPhase::new(
                    "top_1",
                    "pnp",
                    ctx.phase_1_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Top,
                    &[],
                ),
            ])
            .with_phase_orderings(&["top_1", "bottom_1"])
            .with_phase_states(vec![
                ("bottom_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "core::load_pcbs",
                        Box::new(TestLoadPcbsTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("manually_solder_components", vec![
                        (
                            "core::place_components",
                            Box::new(TestPlacementTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                        (
                            "core::manual_soldering",
                            Box::new(TestManualSolderingTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                    ]),
                ]),
                ("top_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "core::load_pcbs",
                        Box::new(TestLoadPcbsTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("automated_pnp", vec![(
                        "core::place_components",
                        Box::new(TestPlacementTaskState::new(TaskStatus::Pending).with_total(3))
                            as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("reflow_oven_soldering", vec![(
                        "core::automated_soldering",
                        Box::new(TestAutomatedSolderingTaskState::new(TaskStatus::Pending))
                            as Box<dyn TestSerializableTaskState>,
                    )]),
                ]),
            ])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Unused,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(10),
                            dec!(10),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(109), dec!(180)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(20),
                            dec!(-45),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(17), dec!(-45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R2",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R2",
                            "RES_MFR2",
                            "RES2",
                            true,
                            PcbSide::Top,
                            dec!(30),
                            dec!(30),
                            dec!(45),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(27), dec!(45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(40),
                            dec!(40),
                            dec!(135),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(37), dec!(135)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
            ])
            .content();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "-vv",
            "assign-placements-to-phase",
            "--phase top_1",
            "--operation set",
            // By placement path pattern
            //"--placements pcb=1::unit=1::ref_des=R1"
            "--placements pcb=1::unit=1::ref_des=R.*",
            //"--placements pcb=1::unit=1::ref_des=J1",
            //"--placements pcb=panel::instance=.*::unit=.*::ref_des=R1"
            //"--placements pcb=1::unit=.*::ref_des=.*"
            //"--placements .*::ref_des=R.*"
            //"--placements .*",

            // FUTURE By manufacturer and mpn
            // "--manufacturer RES_MFR.*",
            // "--mpn .*"
        ]);

        // and
        let expected_phase_1_load_out_content = LoadOutCSVBuilder::new()
            .with_items(&[
                TestLoadOutRecord {
                    reference: "".to_string(),
                    manufacturer: "RES_MFR1".to_string(),
                    mpn: "RES1".to_string(),
                },
                TestLoadOutRecord {
                    reference: "".to_string(),
                    manufacturer: "RES_MFR2".to_string(),
                    mpn: "RES2".to_string(),
                },
            ])
            .as_string();

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        let loading_load_out_message = format!(
            "Loading load-out. source: '{}'",
            ctx.phase_1_load_out_path
                .to_str()
                .unwrap()
        );
        let storing_load_out_message = format!(
            "Storing load-out. source: '{}'",
            ctx.phase_1_load_out_path
                .to_str()
                .unwrap()
        );

        assert_contains_inorder!(trace_content, [
            // assignments should be made
            "Assigning placement to phase. phase: top_1, placement_path: pcb=1::unit=1::ref_des=R1",
            "Assigning placement to phase. phase: top_1, placement_path: pcb=1::unit=1::ref_des=R2",
            "Assigning placement to phase. phase: top_1, placement_path: pcb=1::unit=1::ref_des=R3",
            // all phase status should be updated
            "Refreshed placement task state. phase: bottom_1, operation: manually_solder_components, task: core::place_components, status: Pending, updated: false\n",
            "Refreshed placement task state. phase: top_1, operation: automated_pnp, task: core::place_components, status: Pending, updated: true\n",
            // part process should be updated
            "Added process. part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, applicable_processes: [\"pnp\"]",
            "Added process. part: Part { manufacturer: \"RES_MFR2\", mpn: \"RES2\" }, applicable_processes: [\"pnp\"]",
            // load-out should be checked
            &loading_load_out_message,
            r#"Checking for part in load_out. part: Part { manufacturer: "RES_MFR1", mpn: "RES1" }"#,
            r#"Checking for part in load_out. part: Part { manufacturer: "RES_MFR2", mpn: "RES2" }"#,
            &storing_load_out_message,
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        // and
        let phase_1_load_out_content: String = read_to_string(ctx.phase_1_load_out_path.clone())?;
        println!("actual:\n{}", phase_1_load_out_content);
        println!("expected:\n{}", expected_phase_1_load_out_content);

        assert_eq!(phase_1_load_out_content, expected_phase_1_load_out_content);

        Ok(())
    }

    //noinspection MissingFeatures
    #[test]
    fn sequence_11_assign_feeder_to_load_out_item() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(11);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "assign-feeder-to-load-out-item",
            "--phase top_1",
            "--feeder-reference FEEDER_1",
            "--manufacturer .*",
            "--mpn RES1",
        ]);

        let expected_phase_1_load_out_content = LoadOutCSVBuilder::new()
            .with_items(&[
                TestLoadOutRecord {
                    reference: "FEEDER_1".to_string(),
                    manufacturer: "RES_MFR1".to_string(),
                    mpn: "RES1".to_string(),
                },
                TestLoadOutRecord {
                    reference: "".to_string(),
                    manufacturer: "RES_MFR2".to_string(),
                    mpn: "RES2".to_string(),
                },
            ])
            .as_string();

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        assert_contains_inorder!(trace_content, [
            r#"Assigned feeder to load-out item. feeder: FEEDER_1, part: Part { manufacturer: "RES_MFR1", mpn: "RES1" }"#,
        ]);

        // and
        let phase_1_load_out_content: String = read_to_string(ctx.phase_1_load_out_path.clone())?;
        println!("actual:\n{}", phase_1_load_out_content);
        println!("expected:\n{}", expected_phase_1_load_out_content);

        assert_eq!(phase_1_load_out_content, expected_phase_1_load_out_content);

        Ok(())
    }

    #[test]
    fn sequence_12_set_placement_ordering() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(12);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (
                    project::TestPart::new("CONN_MFR1", "CONN1"),
                    TestPartState::new(&["manual"]),
                ),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::new(&["pnp"])),
                (project::TestPart::new("RES_MFR2", "RES2"), TestPartState::new(&["pnp"])),
            ])
            .with_phases(vec![
                TestPhase::new(
                    "bottom_1",
                    "manual",
                    ctx.phase_2_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Bottom,
                    &[],
                ),
                TestPhase::new(
                    "top_1",
                    "pnp",
                    ctx.phase_1_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Top,
                    &[
                        ("Pcb", "Asc"),
                        ("PcbUnit", "Asc"),
                        ("FeederReference", "Asc"),
                        ("RefDes", "Desc"),
                    ],
                ),
            ])
            .with_phase_orderings(&["top_1", "bottom_1"])
            .with_phase_states(vec![
                ("bottom_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "core::load_pcbs",
                        Box::new(TestLoadPcbsTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("manually_solder_components", vec![
                        (
                            "core::place_components",
                            Box::new(TestPlacementTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                        (
                            "core::manual_soldering",
                            Box::new(TestManualSolderingTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                    ]),
                ]),
                ("top_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "core::load_pcbs",
                        Box::new(TestLoadPcbsTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("automated_pnp", vec![(
                        "core::place_components",
                        Box::new(TestPlacementTaskState::new(TaskStatus::Pending).with_total(3))
                            as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("reflow_oven_soldering", vec![(
                        "core::automated_soldering",
                        Box::new(TestAutomatedSolderingTaskState::new(TaskStatus::Pending))
                            as Box<dyn TestSerializableTaskState>,
                    )]),
                ]),
            ])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Unused,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(10),
                            dec!(10),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(109), dec!(180)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(20),
                            dec!(-45),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(17), dec!(-45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R2",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R2",
                            "RES_MFR2",
                            "RES2",
                            true,
                            PcbSide::Top,
                            dec!(30),
                            dec!(30),
                            dec!(45),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(27), dec!(45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(40),
                            dec!(40),
                            dec!(135),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(37), dec!(135)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
            ])
            .content();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "set-placement-ordering",
            "--phase top_1",
            "--placement-orderings PCB:ASC,PCB_UNIT:ASC,FEEDER_REFERENCE:ASC,REF_DES:DESC",
            // example for PnP machine placement
            //"--orderings PCB_UNIT:ASC,COST:ASC,AREA:ASC,HEIGHT;ASC,FEEDER_REFERENCE:ASC",
            // example for manual placement
            //"--orderings COST:ASC,AREA:ASC,HEIGHT;ASC,PART:ASC,PCB_UNIT:ASC",
        ]);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        assert_contains_inorder!(trace_content, [
            "Phase placement orderings set. phase: 'top_1', orderings: [PCB:ASC, PCB_UNIT:ASC, FEEDER_REFERENCE:ASC, REF_DES:DESC]",
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_13_generate_artifacts() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(13);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_phase_1_placements_content = PhasePlacementsCSVBuilder::new()
            .with_items(&[
                TestPhasePlacementRecord {
                    object_path: "pcb=1::unit=1::ref_des=R2".to_string(),
                    feeder_reference: "".to_string(),
                    manufacturer: "RES_MFR2".to_string(),
                    mpn: "RES2".to_string(),
                    x: dec!(32),
                    y: dec!(27),
                    rotation: dec!(45),
                },
                TestPhasePlacementRecord {
                    object_path: "pcb=1::unit=1::ref_des=R3".to_string(),
                    feeder_reference: "FEEDER_1".to_string(),
                    manufacturer: "RES_MFR1".to_string(),
                    mpn: "RES1".to_string(),
                    x: dec!(42),
                    y: dec!(37),
                    rotation: dec!(135),
                },
                TestPhasePlacementRecord {
                    object_path: "pcb=1::unit=1::ref_des=R1".to_string(),
                    feeder_reference: "FEEDER_1".to_string(),
                    manufacturer: "RES_MFR1".to_string(),
                    mpn: "RES1".to_string(),
                    x: dec!(22),
                    y: dec!(17),
                    rotation: dec!(-45),
                },
            ])
            .as_string();

        let expected_project_report_content = ProjectReportBuilder::default()
            .with_name("job1")
            .with_status("Incomplete")
            .with_phases_overview(&[
                TestPhaseOverview {
                    phase: "top_1".to_string(),
                    status: "Incomplete".to_string(),
                    process: "pnp".to_string(),
                    operations_overview: vec![
                        TestPhaseOperationOverview {
                            operation: "load_pcbs".to_string(),
                            status: TestProcessOperationStatus::Pending,
                            tasks: vec![(
                                "core::load_pcbs".to_string(),
                                Box::new(TestLoadPcbsTaskOverview {}) as Box<dyn TestTaskOverview>,
                            )],
                        },
                        TestPhaseOperationOverview {
                            operation: "automated_pnp".to_string(),
                            status: TestProcessOperationStatus::Pending,
                            tasks: vec![(
                                "core::place_components".to_string(),
                                Box::new(TestPlaceComponentsTaskOverview {
                                    placed: 0,
                                    skipped: 0,
                                    total: 3,
                                }) as Box<dyn TestTaskOverview>,
                            )],
                        },
                        TestPhaseOperationOverview {
                            operation: "reflow_oven_soldering".to_string(),
                            status: TestProcessOperationStatus::Pending,
                            tasks: vec![(
                                "core::automated_soldering".to_string(),
                                Box::new(TestAutomatedSolderingTaskOverview {}) as Box<dyn TestTaskOverview>,
                            )],
                        },
                    ],
                },
                TestPhaseOverview {
                    phase: "bottom_1".to_string(),
                    status: "Incomplete".to_string(),
                    process: "manual".to_string(),
                    operations_overview: vec![
                        TestPhaseOperationOverview {
                            operation: "load_pcbs".to_string(),
                            status: TestProcessOperationStatus::Pending,
                            tasks: vec![(
                                "core::load_pcbs".to_string(),
                                Box::new(TestLoadPcbsTaskOverview {}) as Box<dyn TestTaskOverview>,
                            )],
                        },
                        TestPhaseOperationOverview {
                            operation: "manually_solder_components".to_string(),
                            status: TestProcessOperationStatus::Pending,
                            tasks: vec![
                                (
                                    "core::place_components".to_string(),
                                    Box::new(TestPlaceComponentsTaskOverview {
                                        placed: 0,
                                        skipped: 0,
                                        total: 0,
                                    }) as Box<dyn TestTaskOverview>,
                                ),
                                (
                                    "core::manual_soldering".to_string(),
                                    Box::new(TestManualSolderingTaskOverview {}) as Box<dyn TestTaskOverview>,
                                ),
                            ],
                        },
                    ],
                },
            ])
            .with_phase_specification(&[
                TestPhaseSpecification {
                    phase: "top_1".to_string(),
                    operations: vec![
                        TestPhaseOperation {
                            operation: "load_pcbs".to_string(),
                            task_specifications: vec![(
                                "core::load_pcbs".to_string(),
                                Box::new(TestLoadPcbsTaskSpecification {
                                    pcbs: vec![report::TestPcb {
                                        name: "panel_a".to_string(),
                                        unit_assignments: vec![TestPcbUnitAssignment {
                                            unit_path: "pcb=1::unit=1".to_string(),
                                            design_name: "design_a".to_string(),
                                            variant_name: "variant_a".to_string(),
                                        }],
                                    }],
                                }) as Box<dyn TestTaskSpecification>,
                            )],
                        },
                        TestPhaseOperation {
                            operation: "automated_pnp".to_string(),
                            task_specifications: vec![(
                                "core::place_components".to_string(),
                                Box::new(TestPlaceComponentsTaskSpecification {}) as Box<dyn TestTaskSpecification>,
                            )],
                        },
                        TestPhaseOperation {
                            operation: "reflow_oven_soldering".to_string(),
                            task_specifications: vec![(
                                "core::automated_soldering".to_string(),
                                Box::new(TestAutomatedSolderingTaskSpecification {}) as Box<dyn TestTaskSpecification>,
                            )],
                        },
                    ],
                    load_out_assignments: vec![
                        TestPhaseLoadOutAssignmentItem {
                            feeder_reference: Some("FEEDER_1".to_string()),
                            manufacturer: "RES_MFR1".to_string(),
                            mpn: "RES1".to_string(),
                            quantity: 2, // R1 and R3
                        },
                        TestPhaseLoadOutAssignmentItem {
                            feeder_reference: None,
                            manufacturer: "RES_MFR2".to_string(),
                            mpn: "RES2".to_string(),
                            quantity: 1,
                        },
                    ],
                },
                TestPhaseSpecification {
                    phase: "bottom_1".to_string(),
                    operations: vec![
                        TestPhaseOperation {
                            operation: "load_pcbs".to_string(),
                            task_specifications: vec![(
                                "core::load_pcbs".to_string(),
                                Box::new(TestLoadPcbsTaskSpecification {
                                    pcbs: vec![report::TestPcb {
                                        name: "panel_a".to_string(),
                                        unit_assignments: vec![TestPcbUnitAssignment {
                                            unit_path: "pcb=1::unit=1".to_string(),
                                            design_name: "design_a".to_string(),
                                            variant_name: "variant_a".to_string(),
                                        }],
                                    }],
                                }) as Box<dyn TestTaskSpecification>,
                            )],
                        },
                        TestPhaseOperation {
                            operation: "manually_solder_components".to_string(),
                            task_specifications: vec![
                                (
                                    "core::place_components".to_string(),
                                    Box::new(TestPlaceComponentsTaskSpecification {}) as Box<dyn TestTaskSpecification>,
                                ),
                                (
                                    "core::manual_soldering".to_string(),
                                    Box::new(TestManualSolderingTaskSpecification {}) as Box<dyn TestTaskSpecification>,
                                ),
                            ],
                        },
                    ],
                    load_out_assignments: vec![],
                },
            ])
            .with_issues(&[
                TestIssue {
                    message: "A placement has not been assigned to a phase".to_string(),
                    severity: TestIssueSeverity::Warning,
                    kind: TestIssueKind::UnassignedPlacement {
                        object_path: "pcb=1::unit=1::ref_des=J1".to_string(),
                    },
                },
                TestIssue {
                    message: "A part has not been assigned to a feeder".to_string(),
                    severity: TestIssueSeverity::Warning,
                    kind: TestIssueKind::UnassignedPartFeeder {
                        part: TestPart {
                            manufacturer: "RES_MFR2".to_string(),
                            mpn: "RES2".to_string(),
                        },
                    },
                },
            ])
            .as_string();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "generate-artifacts",
            "-vv",
        ]);
        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        let load_out_message_1 = format!(
            "Loading load-out. source: '{}'",
            ctx.phase_1_load_out_path
                .to_str()
                .unwrap()
        );

        assert_eq!(
            trace_content
                .clone()
                .split('\n')
                .collect::<Vec<&str>>()
                .iter()
                .fold(0, |mut count, &line| {
                    if line.contains(&load_out_message_1) {
                        count += 1;
                    }
                    count
                }),
            1
        );

        // and
        let mut phase_1_placements_file_path = PathBuf::from(ctx.temp_dir.path());
        phase_1_placements_file_path.push("top_1_placements.csv");
        let phase_1_message = format!(
            "Generated phase placements. phase: 'top_1', path: {:?}\n",
            phase_1_placements_file_path
        );

        let mut phase_2_placements_file_path = PathBuf::from(ctx.temp_dir.path());
        phase_2_placements_file_path.push("bottom_1_placements.csv");
        let phase_2_message = format!(
            "Generated phase placements. phase: 'bottom_1', path: {:?}\n",
            phase_2_placements_file_path
        );

        let mut json_project_report_file_path = PathBuf::from(ctx.temp_dir.path());
        json_project_report_file_path.push("job1_report.json");
        let json_report_message = format!("Generated JSON report. path: {:?}\n", json_project_report_file_path);

        let mut markdown_project_report_file_path = PathBuf::from(ctx.temp_dir.path());
        markdown_project_report_file_path.push("job1_report.md");
        let markdown_report_message = format!(
            "Generated Markdown report. path: {:?}\n",
            markdown_project_report_file_path
        );

        assert_contains_inorder!(trace_content, [
            &phase_1_message,
            &phase_2_message,
            &json_report_message,
            &markdown_report_message,
            "Generated artifacts.\n",
        ]);

        // and
        let phase_1_placements_content: String = read_to_string(phase_1_placements_file_path)?;
        println!("{}", phase_1_placements_content);

        assert_eq!(phase_1_placements_content, expected_phase_1_placements_content);

        // and
        println!("expected:\n{}", expected_project_report_content);

        let project_report_content: String = read_to_string(json_project_report_file_path)?;
        println!("actual:\n{}", project_report_content);

        // content of markdown report file is not asserted because:
        // a) it's generated by an external crate from the json version.
        // b) it's currently not known which is the best external crate.
        // c) it's not known if it's a long term feature, hence the feature gate.

        assert_eq!(project_report_content, expected_project_report_content);

        Ok(())
    }

    #[test]
    fn sequence_14_record_phase_operations() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(14);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd_1 = Command::new(env!("CARGO_BIN_EXE_planner_cli"));
        let mut cmd_2 = Command::new(env!("CARGO_BIN_EXE_planner_cli"));
        let mut cmd_3 = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (
                    project::TestPart::new("CONN_MFR1", "CONN1"),
                    TestPartState::new(&["manual"]),
                ),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::new(&["pnp"])),
                (project::TestPart::new("RES_MFR2", "RES2"), TestPartState::new(&["pnp"])),
            ])
            .with_phases(vec![
                TestPhase::new(
                    "bottom_1",
                    "manual",
                    ctx.phase_2_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Bottom,
                    &[],
                ),
                TestPhase::new(
                    "top_1",
                    "pnp",
                    ctx.phase_1_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Top,
                    &[
                        ("Pcb", "Asc"),
                        ("PcbUnit", "Asc"),
                        ("FeederReference", "Asc"),
                        ("RefDes", "Desc"),
                    ],
                ),
            ])
            .with_phase_orderings(&["top_1", "bottom_1"])
            .with_phase_states(vec![
                ("bottom_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "core::load_pcbs",
                        Box::new(TestLoadPcbsTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("manually_solder_components", vec![
                        (
                            "core::place_components",
                            Box::new(TestPlacementTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                        (
                            "core::manual_soldering",
                            Box::new(TestManualSolderingTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                    ]),
                ]),
                ("top_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "core::load_pcbs",
                        Box::new(TestLoadPcbsTaskState::new(TaskStatus::Complete))
                            as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("automated_pnp", vec![(
                        "core::place_components",
                        Box::new(TestPlacementTaskState::new(TaskStatus::Started).with_total(3))
                            as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("reflow_oven_soldering", vec![(
                        "core::automated_soldering",
                        Box::new(TestAutomatedSolderingTaskState::new(TaskStatus::Pending))
                            as Box<dyn TestSerializableTaskState>,
                    )]),
                ]),
            ])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Unused,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(10),
                            dec!(10),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(109), dec!(180)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(20),
                            dec!(-45),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(17), dec!(-45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R2",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R2",
                            "RES_MFR2",
                            "RES2",
                            true,
                            PcbSide::Top,
                            dec!(30),
                            dec!(30),
                            dec!(45),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(27), dec!(45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(40),
                            dec!(40),
                            dec!(135),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(37), dec!(135)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
            ])
            .content();

        // and
        let operation_expectations = vec![
            (
                "require",
                Some((
                    "top_1".to_string(),
                    "load_pcbs".to_string(),
                    Box::new(TestLoadPcbsOperationTaskHistoryKind {
                        status: TaskStatus::Started,
                    }) as Box<dyn TestOperationHistoryKind>,
                )),
            ),
            (
                "require",
                Some((
                    "top_1".to_string(),
                    "load_pcbs".to_string(),
                    Box::new(TestLoadPcbsOperationTaskHistoryKind {
                        status: TaskStatus::Complete,
                    }) as Box<dyn TestOperationHistoryKind>,
                )),
            ),
            (
                "require",
                Some((
                    "top_1".to_string(),
                    "automated_pnp".to_string(),
                    Box::new(TestPlaceComponentsOperationTaskHistoryKind {
                        status: TaskStatus::Started,
                    }) as Box<dyn TestOperationHistoryKind>,
                )),
            ),
            ("eof", None),
        ];

        // and
        let args_1 = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "record-phase-operation",
            "--phase top_1",
            "--operation load_pcbs",
            "--task core::load_pcbs",
            "--action start",
        ]);
        let message_1 = "Marking task as started. phase: top_1, operation: load_pcbs, task: core::load_pcbs";

        let args_2 = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "record-phase-operation",
            "--phase top_1",
            "--operation load_pcbs",
            "--task core::load_pcbs",
            "--action complete",
        ]);
        let message_2 = "Marking task as completed. phase: top_1, operation: load_pcbs, task: core::load_pcbs";

        let args_3 = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "record-phase-operation",
            "--phase top_1",
            "--operation automated_pnp",
            "--task core::place_components",
            "--action start",
        ]);
        let message_3 = "Marking task as started. phase: top_1, operation: automated_pnp, task: core::place_components";

        // when
        cmd_1
            .args(args_1)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and when
        cmd_2
            .args(args_2)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));
        // and when
        cmd_3
            .args(args_3)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("trace:\n{}", trace_content);

        let initial_log_file_message = format!("Created operation history file. path: {:?}\n", ctx.phase_1_log_path);
        let log_file_message = format!("Updated operation history file. path: {:?}\n", ctx.phase_1_log_path);

        assert_contains_inorder!(trace_content, [
            message_1,
            &initial_log_file_message,
            message_2,
            &log_file_message,
            message_3,
            &log_file_message,
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        // and
        let operation_history_file = File::open(ctx.phase_1_log_path.clone())?;
        let operation_history: Vec<TestOperationHistoryItem> = serde_json::from_reader(operation_history_file)?;

        assert_operation_history(operation_history, operation_expectations);

        Ok(())
    }

    #[test]
    fn sequence_15_record_placements_operation() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(15);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (
                    project::TestPart::new("CONN_MFR1", "CONN1"),
                    TestPartState::new(&["manual"]),
                ),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::new(&["pnp"])),
                (project::TestPart::new("RES_MFR2", "RES2"), TestPartState::new(&["pnp"])),
            ])
            .with_phases(vec![
                TestPhase::new(
                    "bottom_1",
                    "manual",
                    ctx.phase_2_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Bottom,
                    &[],
                ),
                TestPhase::new(
                    "top_1",
                    "pnp",
                    ctx.phase_1_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Top,
                    &[
                        ("Pcb", "Asc"),
                        ("PcbUnit", "Asc"),
                        ("FeederReference", "Asc"),
                        ("RefDes", "Desc"),
                    ],
                ),
            ])
            .with_phase_orderings(&["top_1", "bottom_1"])
            .with_phase_states(vec![
                ("bottom_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "core::load_pcbs",
                        Box::new(TestLoadPcbsTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("manually_solder_components", vec![
                        (
                            "core::place_components",
                            Box::new(TestPlacementTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                        (
                            "core::manual_soldering",
                            Box::new(TestManualSolderingTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                    ]),
                ]),
                ("top_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "core::load_pcbs",
                        Box::new(TestLoadPcbsTaskState::new(TaskStatus::Complete))
                            as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("automated_pnp", vec![(
                        "core::place_components",
                        Box::new(
                            TestPlacementTaskState::new(TaskStatus::Complete)
                                .with_placed(3)
                                .with_total(3),
                        ) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("reflow_oven_soldering", vec![(
                        "core::automated_soldering",
                        Box::new(TestAutomatedSolderingTaskState::new(TaskStatus::Pending))
                            as Box<dyn TestSerializableTaskState>,
                    )]),
                ]),
            ])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Unused,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(10),
                            dec!(10),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(109), dec!(180)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(20),
                            dec!(-45),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(17), dec!(-45)),
                        PlacementStatus::Placed,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R2",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R2",
                            "RES_MFR2",
                            "RES2",
                            true,
                            PcbSide::Top,
                            dec!(30),
                            dec!(30),
                            dec!(45),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(27), dec!(45)),
                        PlacementStatus::Placed,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(40),
                            dec!(40),
                            dec!(135),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(37), dec!(135)),
                        PlacementStatus::Placed,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
            ])
            .content();

        // and

        let operation_expectations = vec![
            ("ignore", None),
            ("ignore", None),
            ("ignore", None),
            (
                "require",
                Some((
                    "top_1".to_string(),
                    "automated_pnp".to_string(),
                    Box::new(TestPlacementOperationHistoryKind {
                        object_path: ObjectPath::from_str("pcb=1::unit=1::ref_des=R1").unwrap(),
                        operation: PlacementOperation::Place,
                    }) as Box<dyn TestOperationHistoryKind>,
                )),
            ),
            (
                "require",
                Some((
                    "top_1".to_string(),
                    "automated_pnp".to_string(),
                    Box::new(TestPlacementOperationHistoryKind {
                        object_path: ObjectPath::from_str("pcb=1::unit=1::ref_des=R2").unwrap(),
                        operation: PlacementOperation::Place,
                    }) as Box<dyn TestOperationHistoryKind>,
                )),
            ),
            (
                "require",
                Some((
                    "top_1".to_string(),
                    "automated_pnp".to_string(),
                    Box::new(TestPlacementOperationHistoryKind {
                        object_path: ObjectPath::from_str("pcb=1::unit=1::ref_des=R3").unwrap(),
                        operation: PlacementOperation::Place,
                    }) as Box<dyn TestOperationHistoryKind>,
                )),
            ),
            ("eof", None),
        ];

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "record-placements-operation",
            "--object-path-patterns pcb=1::unit=1::ref_des=R([1-3])?,pcb=1::unit=2::ref_des=.*",
            "--operation placed",
        ]);
        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        let log_file_message = format!("Updated operation history file. path: {:?}\n", ctx.phase_1_log_path);

        assert_contains_inorder!(trace_content, [
            "Placement marked as placed. object_path: pcb=1::unit=1::ref_des=R1\n",
            "Placement marked as placed. object_path: pcb=1::unit=1::ref_des=R2\n",
            "Placement marked as placed. object_path: pcb=1::unit=1::ref_des=R3\n",
            "Unmatched object path pattern. object_path_pattern: pcb=1::unit=2::ref_des=.*\n",
            "Refreshed placement task state. phase: bottom_1, operation: manually_solder_components, task: core::place_components, status: Pending, updated: false\n",
            "Refreshed placement task state. phase: top_1, operation: automated_pnp, task: core::place_components, status: Complete, updated: true\n",
            &log_file_message,
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        // and
        let operation_history_file = File::open(ctx.phase_1_log_path.clone())?;
        let operation_history: Vec<TestOperationHistoryItem> = serde_json::from_reader(operation_history_file)?;
        println!("{:?}", operation_history);

        assert_operation_history(operation_history, operation_expectations);

        Ok(())
    }

    #[test]
    fn sequence_16_reset_operations() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(16);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (
                    project::TestPart::new("CONN_MFR1", "CONN1"),
                    TestPartState::new(&["manual"]),
                ),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::new(&["pnp"])),
                (project::TestPart::new("RES_MFR2", "RES2"), TestPartState::new(&["pnp"])),
            ])
            .with_phases(vec![
                TestPhase::new(
                    "bottom_1",
                    "manual",
                    ctx.phase_2_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Bottom,
                    &[],
                ),
                TestPhase::new(
                    "top_1",
                    "pnp",
                    ctx.phase_1_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Top,
                    &[
                        ("Pcb", "Asc"),
                        ("PcbUnit", "Asc"),
                        ("FeederReference", "Asc"),
                        ("RefDes", "Desc"),
                    ],
                ),
            ])
            .with_phase_orderings(&["top_1", "bottom_1"])
            .with_phase_states(vec![
                ("bottom_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "core::load_pcbs",
                        Box::new(TestLoadPcbsTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("manually_solder_components", vec![
                        (
                            "core::place_components",
                            Box::new(TestPlacementTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                        (
                            "core::manual_soldering",
                            Box::new(TestManualSolderingTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                    ]),
                ]),
                ("top_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "core::load_pcbs",
                        Box::new(TestLoadPcbsTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("automated_pnp", vec![(
                        "core::place_components",
                        Box::new(TestPlacementTaskState::new(TaskStatus::Pending).with_total(3))
                            as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("reflow_oven_soldering", vec![(
                        "core::automated_soldering",
                        Box::new(TestAutomatedSolderingTaskState::new(TaskStatus::Pending))
                            as Box<dyn TestSerializableTaskState>,
                    )]),
                ]),
            ])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Unused,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(10),
                            dec!(10),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(109), dec!(180)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(20),
                            dec!(-45),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(17), dec!(-45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R2",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R2",
                            "RES_MFR2",
                            "RES2",
                            true,
                            PcbSide::Top,
                            dec!(30),
                            dec!(30),
                            dec!(45),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(27), dec!(45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(40),
                            dec!(40),
                            dec!(135),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(37), dec!(135)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
            ])
            .content();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "reset-operations",
        ]);
        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        assert_contains_inorder!(trace_content, [
            "Placement operations reset.\n",
            "Phase operations reset. phase: bottom_1\n",
            "Phase operations reset. phase: top_1\n",
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_17_cleanup() {
        let mut ctx_guard = context::acquire(17);
        let ctx = ctx_guard.1.take().unwrap();
        drop(ctx);
    }

    fn assert_operation_history(
        mut operation_history: Vec<TestOperationHistoryItem>,
        operation_expectations: Vec<(&str, Option<(String, String, Box<dyn TestOperationHistoryKind>)>)>,
    ) {
        for (index, (&ref expectation_operation, expectation)) in operation_expectations
            .iter()
            .enumerate()
        {
            match expectation_operation {
                "eof" => {
                    assert!(operation_history.is_empty());
                    break;
                }
                _ => {}
            }

            let (item, remaining_operation_history) = operation_history.split_first().unwrap();
            println!(
                "index: {}, expectation: {}, item: {:?}",
                index, expectation_operation, item
            );

            match expectation_operation {
                "ignore" => {}
                "require" => {
                    assert_eq!(
                        &(
                            item.phase.clone(),
                            item.operation_reference.to_string(),
                            dyn_clone::clone_box(&*item.task_history)
                        ),
                        expectation.as_ref().unwrap()
                    );
                }
                _ => unreachable!(),
            }

            operation_history = Vec::from(remaining_operation_history);
        }
    }
}

mod help {
    use assert_cmd::Command;
    use indoc::indoc;
    use predicates::prelude::{predicate, PredicateBooleanExt};
    use util::test::print;

    #[test]
    fn no_args() {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_output = indoc! {"
            Usage: planner_cli [OPTIONS] <COMMAND>

            Commands:
              project  Project mode
              pcb      PCB mode
              help     Print this message or the help of the given subcommand(s)

            Options:
                  --trace [<TRACE>]  Trace log file
              -v, --verbose...       Increase logging verbosity
              -q, --quiet...         Decrease logging verbosity
              -h, --help             Print help
              -V, --version          Print version
        "};
        // when
        cmd
            // then
            .assert()
            .failure()
            .stderr(print("stderr").and(predicate::str::diff(expected_output)))
            .stdout(print("stdout"));
    }

    mod pcb_help {
        use assert_cmd::Command;
        use indoc::indoc;
        use predicates::prelude::{predicate, PredicateBooleanExt};
        use util::test::print;

        #[test]
        fn no_args() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                PCB mode

                Usage: planner_cli pcb [OPTIONS] --pcb-file <PCB_FILE> <COMMAND>

                Commands:
                  create                  Create a PCB file
                  configure-panel-sizing  Configure a PCB
                  help                    Print this message or the help of the given subcommand(s)

                Options:
                      --pcb-file <PCB_FILE>  Specify a PCB context
                  -v, --verbose...           Increase logging verbosity
                  -q, --quiet...             Decrease logging verbosity
                  -h, --help                 Print help
            "};

            // when
            cmd.args(&["pcb"])
                // then
                .assert()
                .failure()
                .stderr(print("stderr").and(predicate::str::diff(expected_output)))
                .stdout(print("stdout"));
        }

        #[test]
        fn help_for_create() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Create a PCB file

                Usage: planner_cli pcb --pcb-file <PCB_FILE> create [OPTIONS] --name <NAME> --units <UNITS> --design <DESIGN>...

                Options:
                      --name <NAME>         Name of the PCB, e.g. 'panel_1'
                      --units <UNITS>       The number of individual PCB units. 1 = single, >1 = panel
                      --design <DESIGN>...  The mapping of designs to units e.g. '1=design_a,2=design_b,3=design_a,4=design_b'. unit is 1-based
                  -v, --verbose...          Increase logging verbosity
                  -q, --quiet...            Decrease logging verbosity
                  -h, --help                Print help
            "};

            // when
            cmd.args(["pcb", "create", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_configure_panel_sizing() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Configure a PCB
                
                Usage: planner_cli pcb --pcb-file <PCB_FILE> configure-panel-sizing [OPTIONS]
                
                Options:
                      --edge-rails <DIMENSIONS>
                          Edge rails (left,right,top,bottom) e.g. 'left=5,right=5,top=10,bottom=10'
                      --size <VECTOR2>
                          PCB size (x,y) e.g. 'x=100,y=100'
                      --design-sizing <DESIGN_SIZING>
                          Design sizing (e.g. 'design_a:origin=x=15.25,y=15.25:offset=x=-10.0,y=-10.0:size=x=30.5,y=30.5')
                      --pcb-unit-position <PCB_UNIT_POSITIONING>
                          PCB unit positioning (e.g. '1:offset=x=10,y=10:rotation=90')
                  -v, --verbose...
                          Increase logging verbosity
                  -q, --quiet...
                          Decrease logging verbosity
                  -h, --help
                          Print help
            "};

            // when
            cmd.args(["pcb", "configure-panel-sizing", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }
    }

    mod project_help {
        use assert_cmd::Command;
        use indoc::indoc;
        use predicates::prelude::{predicate, PredicateBooleanExt};
        use util::test::print;

        #[test]
        fn no_args() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Project mode
                
                Usage: planner_cli project [OPTIONS] --project <PROJECT_NAME> <COMMAND>
                
                Commands:
                  create                          Create a new job
                  add-pcb                         Add a PCB file to the project
                  assign-variant-to-unit          Assign a design variant to a PCB unit
                  refresh-from-design-variants    Refresh from design variants
                  create-process-from-preset      Create a process from presets
                  delete-process                  Delete a process from the project
                  assign-process-to-parts         Assign a process to parts
                  create-phase                    Create a phase
                  assign-placements-to-phase      Assign placements to a phase
                  assign-feeder-to-load-out-item  Assign feeder to load-out item
                  set-placement-ordering          Set placement ordering for a phase
                  generate-artifacts              Generate artifacts
                  record-phase-operation          Record phase operation
                  record-placements-operation     Record placements operation
                  reset-operations                Reset operations
                  help                            Print this message or the help of the given subcommand(s)
                
                Options:
                      --path <PATH>             Path [default: .]
                      --project <PROJECT_NAME>  Project name
                  -v, --verbose...              Increase logging verbosity
                  -q, --quiet...                Decrease logging verbosity
                  -h, --help                    Print help
            "};

            // when
            cmd.args(&["project"])
                // then
                .assert()
                .failure()
                .stderr(print("stderr").and(predicate::str::diff(expected_output)))
                .stdout(print("stdout"));
        }

        #[test]
        fn help_for_create() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Create a new job

                Usage: planner_cli project --project <PROJECT_NAME> create [OPTIONS]

                Options:
                  -v, --verbose...  Increase logging verbosity
                  -q, --quiet...    Decrease logging verbosity
                  -h, --help        Print help
            "};

            // when
            cmd.args(["project", "create", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_add_pcb() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Add a PCB file to the project

                Usage: planner_cli project --project <PROJECT_NAME> add-pcb [OPTIONS] --file <FILE_REFERENCE>

                Options:
                      --file <FILE_REFERENCE>  The path of the PCB, e.g. 'relative:<some_relative_path>' or '<some_absolute_path>' paths can be prefixed with `relative:` to make them relative to the project path
                  -v, --verbose...             Increase logging verbosity
                  -q, --quiet...               Decrease logging verbosity
                  -h, --help                   Print help
            "};

            // when
            cmd.args(["project", "add-pcb", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_assign_variant_to_unit() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Assign a design variant to a PCB unit

                Usage: planner_cli project --project <PROJECT_NAME> assign-variant-to-unit [OPTIONS] --unit <OBJECT_PATH> --variant <VARIANT_NAME>

                Options:
                      --unit <OBJECT_PATH>      PCB unit path
                      --variant <VARIANT_NAME>  Variant of the design
                  -v, --verbose...              Increase logging verbosity
                  -q, --quiet...                Decrease logging verbosity
                  -h, --help                    Print help
            "};

            // when
            cmd.args(["project", "assign-variant-to-unit", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_refresh_from_design_variants() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Refresh from design variants

                Usage: planner_cli project --project <PROJECT_NAME> refresh-from-design-variants [OPTIONS]
                
                Options:
                  -v, --verbose...  Increase logging verbosity
                  -q, --quiet...    Decrease logging verbosity
                  -h, --help        Print help
            "};

            // when
            cmd.args(["project", "refresh-from-design-variants", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_create_process_from_preset() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Create a process from presets

                Usage: planner_cli project --project <PROJECT_NAME> create-process-from-preset [OPTIONS] --preset <PRESET>

                Options:
                      --preset <PRESET>  Process preset name
                  -v, --verbose...       Increase logging verbosity
                  -q, --quiet...         Decrease logging verbosity
                  -h, --help             Print help
            "};

            // when
            cmd.args(["project", "create-process-from-preset", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_delete_process() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Delete a process from the project
                
                Usage: planner_cli project --project <PROJECT_NAME> delete-process [OPTIONS] --process <PROCESS>
                
                Options:
                      --process <PROCESS>  Process preset name
                  -v, --verbose...         Increase logging verbosity
                  -q, --quiet...           Decrease logging verbosity
                  -h, --help               Print help
            "};

            // when
            cmd.args(["project", "delete-process", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_assign_process_to_parts() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Assign a process to parts

                Usage: planner_cli project --project <PROJECT_NAME> assign-process-to-parts [OPTIONS] --process <PROCESS> --operation <OPERATION> --manufacturer <MANUFACTURER> --mpn <MPN>

                Options:
                      --process <PROCESS>            Process name
                      --operation <OPERATION>        Operation [possible values: add, remove]
                      --manufacturer <MANUFACTURER>  Manufacturer pattern (regexp)
                      --mpn <MPN>                    Manufacturer part number (regexp)
                  -v, --verbose...                   Increase logging verbosity
                  -q, --quiet...                     Decrease logging verbosity
                  -h, --help                         Print help
            "};

            // when
            cmd.args(["project", "assign-process-to-parts", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_create_phase() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Create a phase

                Usage: planner_cli project --project <PROJECT_NAME> create-phase [OPTIONS] --process <PROCESS> --reference <REFERENCE> --load-out <LOAD_OUT> --pcb-side <PCB_SIDE>

                Options:
                      --process <PROCESS>      Process name
                      --reference <REFERENCE>  Phase reference (e.g. 'top_1')
                      --load-out <LOAD_OUT>    Load-out source (e.g. 'load_out_1')
                      --pcb-side <PCB_SIDE>    PCB side [possible values: top, bottom]
                  -v, --verbose...             Increase logging verbosity
                  -q, --quiet...               Decrease logging verbosity
                  -h, --help                   Print help
            "};

            // when
            cmd.args(["project", "create-phase", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_assign_placements_to_phase() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Assign placements to a phase

                Usage: planner_cli project --project <PROJECT_NAME> assign-placements-to-phase [OPTIONS] --phase <PHASE> --operation <OPERATION> --placements <PLACEMENTS>

                Options:
                      --phase <PHASE>            Phase reference (e.g. 'top_1')
                      --operation <OPERATION>    Operation [possible values: set, clear]
                      --placements <PLACEMENTS>  Placements object path pattern (regexp)
                  -v, --verbose...               Increase logging verbosity
                  -q, --quiet...                 Decrease logging verbosity
                  -h, --help                     Print help
            "};

            // when
            cmd.args(["project", "assign-placements-to-phase", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_assign_feeder_to_load_out_item() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Assign feeder to load-out item

                Usage: planner_cli project --project <PROJECT_NAME> assign-feeder-to-load-out-item [OPTIONS] --phase <PHASE> --feeder-reference <FEEDER_REFERENCE> --manufacturer <MANUFACTURER> --mpn <MPN>

                Options:
                      --phase <PHASE>                        Phase reference (e.g. 'top_1')
                      --feeder-reference <FEEDER_REFERENCE>  Feeder reference (e.g. 'FEEDER_1')
                      --manufacturer <MANUFACTURER>          Manufacturer pattern (regexp)
                      --mpn <MPN>                            Manufacturer part number (regexp)
                  -v, --verbose...                           Increase logging verbosity
                  -q, --quiet...                             Decrease logging verbosity
                  -h, --help                                 Print help
            "};

            // when
            cmd.args(["project", "assign-feeder-to-load-out-item", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_set_placement_ordering() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Set placement ordering for a phase

                Usage: planner_cli project --project <PROJECT_NAME> set-placement-ordering [OPTIONS] --phase <PHASE> --placement-orderings [<PLACEMENT_ORDERINGS>...]

                Options:
                      --phase <PHASE>
                          Phase reference (e.g. 'top_1')
                      --placement-orderings [<PLACEMENT_ORDERINGS>...]
                          Orderings (e.g. 'PCB_UNIT:ASC,FEEDER_REFERENCE:ASC,REF_DES:ASC')
                  -v, --verbose...
                          Increase logging verbosity
                  -q, --quiet...
                          Decrease logging verbosity
                  -h, --help
                          Print help
            "};

            // when
            cmd.args(["project", "set-placement-ordering", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_generate_artifacts() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Generate artifacts

                Usage: planner_cli project --project <PROJECT_NAME> generate-artifacts [OPTIONS]

                Options:
                  -v, --verbose...  Increase logging verbosity
                  -q, --quiet...    Decrease logging verbosity
                  -h, --help        Print help
            "};

            // when
            cmd.args(["project", "generate-artifacts", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_record_phase_operation() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Record phase operation

                Usage: planner_cli project --project <PROJECT_NAME> record-phase-operation [OPTIONS] --phase <PHASE> --operation <OPERATION> --task <TASK> --action <ACTION>

                Options:
                      --phase <PHASE>          Phase reference (e.g. 'top_1')
                      --operation <OPERATION>  Operation reference
                      --task <TASK>            The task to update
                      --action <ACTION>        The task action to apply [possible values: start, complete, abandon]
                  -v, --verbose...             Increase logging verbosity
                  -q, --quiet...               Decrease logging verbosity
                  -h, --help                   Print help
            "};

            // when
            cmd.args(["project", "record-phase-operation", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_record_placements_operation() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Record placements operation

                Usage: planner_cli project --project <PROJECT_NAME> record-placements-operation [OPTIONS] --object-path-patterns <OBJECT_PATH_PATTERNS>... --operation <OPERATION>

                Options:
                      --object-path-patterns <OBJECT_PATH_PATTERNS>...
                          List of reference designators to apply the operation to
                      --operation <OPERATION>
                          The completed operation to apply [possible values: placed, skipped, reset]
                  -v, --verbose...
                          Increase logging verbosity
                  -q, --quiet...
                          Decrease logging verbosity
                  -h, --help
                          Print help
            "};

            // when
            cmd.args(["project", "record-placements-operation", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_reset_operations() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Reset operations

                Usage: planner_cli project --project <PROJECT_NAME> reset-operations [OPTIONS]

                Options:
                  -v, --verbose...  Increase logging verbosity
                  -q, --quiet...    Decrease logging verbosity
                  -h, --help        Print help
            "};

            // when
            cmd.args(["project", "reset-operations", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }
    }
}

/// calculate the offset for the bottom left position of a unit.
///
/// assumes the routing gap fully surrounds each unit
/// i.e. does not assume routing goes into the rails
/// a routing gap of 0 would be common for panels that use v-scoring and no routing.
/// assumes the combination of unit map and design_sizings will result in a rectangular panel.
/// assumes designs are not rotated.
fn calculate_offset(
    unit_map: &BTreeMap<PcbUnitIndex, DesignIndex>,
    design_sizings: &Vec<TestDesignSizing>,
    edge_rails: &Dimensions<f64>,
    routing_gap: f64,
    x_count: usize,
    y_count: usize,
    x_index: usize,
    y_index: usize,
) -> Vector2<f64> {
    let units = x_count * y_count;
    assert_eq!(units, unit_map.len());

    let mut x_value = edge_rails.left + routing_gap;
    let mut y_value = edge_rails.bottom + routing_gap;

    for x in 0..x_index {
        let map_index = y_index * x_count + x;

        let design_sizing_index = unit_map
            .get(&(map_index as PcbUnitIndex))
            .unwrap();
        let design_sizing = &design_sizings[*design_sizing_index];

        x_value += design_sizing.size.x + routing_gap;
    }

    for y in 0..y_index {
        let map_index = y * x_count + x_index;

        let design_sizing_index = unit_map
            .get(&(map_index as PcbUnitIndex))
            .unwrap();
        let design_sizing = &design_sizings[*design_sizing_index];

        y_value += design_sizing.size.y + routing_gap;
    }

    Vector2::new(x_value, y_value)
}

/// calculate the size of a panel.
///
/// assumes the routing gap fully surrounds each unit
/// i.e. does not assume routing goes into the rails
/// a routing gap of 0 would be common for panels that use v-scoring and no routing.
///
/// uses the first row of designs and first column of designs to calculate the size.
/// assumes the combination of unit map and design_sizings will result in a rectangular panel.
/// assumes designs are not rotated.
fn calculate_size(
    unit_map: &BTreeMap<PcbUnitIndex, DesignIndex>,
    design_sizings: &Vec<TestDesignSizing>,
    edge_rails: &Dimensions<f64>,
    routing_gap: f64,
    x_count: usize,
    y_count: usize,
) -> Vector2<f64> {
    let units = x_count * y_count;
    assert_eq!(units, unit_map.len());

    let mut x_value = edge_rails.left;
    let mut y_value = edge_rails.bottom;

    for x in 0..x_count {
        let map_index = x;

        let design_sizing_index = unit_map
            .get(&(map_index as PcbUnitIndex))
            .unwrap();
        let design_sizing = &design_sizings[*design_sizing_index];
        x_value += routing_gap;
        x_value += design_sizing.size.x;
    }

    for y in 0..y_count {
        let map_index = y * x_count;

        let design_sizing_index = unit_map
            .get(&(map_index as PcbUnitIndex))
            .unwrap();
        let design_sizing = &design_sizings[*design_sizing_index];

        y_value += routing_gap;
        y_value += design_sizing.size.y;
    }

    Vector2::new(
        x_value + routing_gap + edge_rails.right,
        y_value + routing_gap + edge_rails.top,
    )
}

#[cfg(test)]
mod sizing_tests {
    use std::collections::BTreeMap;

    use nalgebra::Vector2;
    use pnp::panel::Dimensions;

    use crate::common::project_builder::TestDesignSizing;
    use crate::{calculate_offset, calculate_size};

    #[test]
    pub fn panel_size_and_unit_offsets() {
        // given
        let edge_rails = Dimensions {
            left: 10.0,
            right: 10.0,
            top: 5.0,
            bottom: 5.0,
        };
        let routing_gap = 2.0;

        let design_a_sizing = TestDesignSizing {
            size: Vector2::new(60.0, 50.0),
            origin: Vector2::new(30.0, 25.0),
            gerber_offset: Default::default(),
            placement_offset: Default::default(),
        };
        let design_b_sizing = TestDesignSizing {
            size: Vector2::new(40.0, 50.0),
            origin: Vector2::new(20.0, 25.0),
            gerber_offset: Default::default(),
            placement_offset: Default::default(),
        };
        let design_sizings = vec![design_a_sizing, design_b_sizing];

        // layout:         unit_map:
        //
        // Row 2  : A,B,A    6,7,8
        // Row 1  : A,B,A    3,4,5
        // Row 0  : A,B,A    0,1,2
        // Columns: 0 1 2
        //
        // A and B have equal heights, but different widths

        let unit_map = BTreeMap::from_iter([(0, 0), (1, 1), (2, 0), (3, 0), (4, 1), (5, 0), (6, 0), (7, 1), (8, 0)]);

        // when
        let size = calculate_size(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3);
        let offsets = vec![
            calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3, 0, 0),
            calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3, 1, 0),
            calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3, 2, 0),
            calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3, 0, 1),
            calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3, 1, 1),
            calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3, 2, 1),
            calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3, 0, 2),
            calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3, 1, 2),
            calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3, 2, 2),
        ];

        // then
        assert_eq!(size, Vector2::new(188.0, 168.0));
        assert_eq!(offsets, vec![
            Vector2::new(10.0 + 2.0, 5.0 + 2.0),
            Vector2::new(10.0 + 2.0 + 60.0 + 2.0, 5.0 + 2.0),
            Vector2::new(10.0 + 2.0 + 60.0 + 2.0 + 40.0 + 2.0, 5.0 + 2.0),
            Vector2::new(10.0 + 2.0, 5.0 + 2.0 + 50.0 + 2.0),
            Vector2::new(10.0 + 2.0 + 60.0 + 2.0, 5.0 + 2.0 + 50.0 + 2.0),
            Vector2::new(10.0 + 2.0 + 60.0 + 2.0 + 40.0 + 2.0, 5.0 + 2.0 + 50.0 + 2.0),
            Vector2::new(10.0 + 2.0, 5.0 + 2.0 + 50.0 + 2.0 + 50.0 + 2.0),
            Vector2::new(10.0 + 2.0 + 60.0 + 2.0, 5.0 + 2.0 + 50.0 + 2.0 + 50.0 + 2.0),
            Vector2::new(
                10.0 + 2.0 + 60.0 + 2.0 + 40.0 + 2.0,
                5.0 + 2.0 + 50.0 + 2.0 + 50.0 + 2.0
            ),
        ]);
    }
}
