#[macro_use]
extern crate util;

pub mod common;

mod operation_sequence_1 {
    use std::collections::BTreeMap;
    use std::fs::{read_to_string, File};
    use std::io::Write;
    use std::path::PathBuf;
    use std::str::FromStr;

    use assert_cmd::Command;
    use indoc::indoc;
    use planning::design::DesignVariant;
    use planning::placement::{PlacementOperation, PlacementStatus, ProjectPlacementStatus};
    use planning::process::TaskStatus;
    use pnp::object_path::ObjectPath;
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
        TestAutomatedSolderingTaskState, TestLoadPcbsTaskState, TestManualSolderingTaskState, TestOperationState,
        TestPartState, TestPhase, TestPlacement, TestPlacementState, TestPlacementTaskState,
        TestProcessOperationStatus, TestProjectBuilder, TestSerializableTaskState,
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

    #[test]
    fn sequence_01_create_job() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(1);
        let ctx = ctx_guard.1.as_mut().unwrap();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_default_processes()
            .content();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "-vvv",
            "create",
        ]);
        println!("args: {:?}", args);

        // when
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

        assert_contains_inorder!(trace_content, ["Created project successfully",]);

        // and
        let project_content: String = read_to_string(ctx.test_project_path.clone())?;
        println!("{}", project_content);

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_02_add_pcb() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(2);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![
                project::TestProjectPcb { 
                    pcb: project::TestPcb {
                        name: "panel_a".to_string(),
                        units: 2,
                        design_names: vec![],
                        design_to_unit_mapping: BTreeMap::default(),
                    },
                    unit_assignments: BTreeMap::default(),
                }
            ])
            .content();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "add-pcb",
            "--name panel_a",
            "--units 2",
        ]);
        println!("args: {:?}", args);

        // when
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

        assert_contains_inorder!(trace_content, ["Added PCB. name: 'panel_a'\n",]);

        // and
        let project_content: String = read_to_string(ctx.test_project_path.clone())?;
        println!("{}", project_content);

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_03_assign_variant_to_unit() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(3);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let design_a_variant_a_placements_csv_content = indoc! {r#"
            "RefDes","Manufacturer","Mpn","Place","PcbSide","X","Y","Rotation"
            "R3","RES_MFR1","RES1","true","Top","5","105","90"
            "C1","CAP_MFR1","CAP1","true","Bottom","30","130","180"
            "J1","CONN_MFR1","CONN1","true","Bottom","40","140","-90"
            "R1","RES_MFR1","RES1","true","Top","10","110","0"
        "#};
        // two refdes on the same side should use the same part (R1, R3)

        let mut placements_path = ctx.temp_dir.path().to_path_buf();
        placements_path.push("design_a_variant_a_placements.csv");

        println!("creating placements, path: {:?}", &placements_path);

        let mut placements_file = File::create(placements_path)?;
        let _written = placements_file.write(design_a_variant_a_placements_csv_content.as_bytes())?;
        placements_file.flush()?;

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![
                project::TestProjectPcb {
                    pcb: project::TestPcb {
                        name: "panel_a".to_string(),
                        units: 2,
                        design_names: vec!["design_a".into()],
                        design_to_unit_mapping: BTreeMap::default(),
                    },
                    unit_assignments: BTreeMap::from_iter([
                        (0, (0, "variant_a".into())),
                    ]),
                }
            ])
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
                            dec!(30),
                            dec!(130),
                            dec!(180),
                        ),
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
                            dec!(40),
                            dec!(140),
                            dec!(-90),
                        ),
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
                            dec!(10),
                            dec!(110),
                            dec!(0),
                        ),
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
                            dec!(5),
                            dec!(105),
                            dec!(90),
                        ),
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
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "assign-variant-to-unit",
            "--design design_a",
            "--variant variant_a",
            "--unit pcb=1::unit=1",
        ]);

        // when
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

        assert_contains_inorder!(trace_content, [
            "Added design to PCB. design: design_a\n",
            "Unit assignment added. unit: 'pcb=1::unit=1', design_variant: design_a-variant_a\n",
            "New part. part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }\n",
            "New part. part: Part { manufacturer: \"CAP_MFR1\", mpn: \"CAP1\" }\n",
            "New part. part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }\n",
            "New placement. placement: Placement { ref_des: \"R3\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 5, y: 105, rotation: 90 }\n",
            "New placement. placement: Placement { ref_des: \"C1\", part: Part { manufacturer: \"CAP_MFR1\", mpn: \"CAP1\" }, place: true, pcb_side: Bottom, x: 30, y: 130, rotation: 180 }\n",
            "New placement. placement: Placement { ref_des: \"J1\", part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }, place: true, pcb_side: Bottom, x: 40, y: 140, rotation: -90 }\n",
            "New placement. placement: Placement { ref_des: \"R1\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 10, y: 110, rotation: 0 }\n",
        ]);

        // and
        let project_content: String = read_to_string(ctx.test_project_path.clone())?;
        println!("{}", project_content);

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_04_assign_process_to_parts() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(4);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // R2 should be before R1.
        let design_a_variant_a_placements_csv_content = indoc! {r#"
            "RefDes","Manufacturer","Mpn","Place","PcbSide","X","Y","Rotation"
            "R3","RES_MFR1","RES1","true","Top","105","1105","91"
            "R2","RES_MFR2","RES2","true","Top","120","1120","91"
            "J1","CONN_MFR1","CONN1","true","Bottom","130","1130","-179"
            "R1","RES_MFR1","RES1","true","Top","110","1110","1"
        "#};

        let mut placements_path = ctx.temp_dir.path().to_path_buf();
        placements_path.push("design_a_variant_a_placements.csv");

        let mut placments_file = File::create(placements_path)?;
        let _written = placments_file.write(design_a_variant_a_placements_csv_content.as_bytes())?;
        placments_file.flush()?;

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![
                project::TestProjectPcb {
                    pcb: project::TestPcb {
                        name: "panel_a".to_string(),
                        units: 2,
                        design_names: vec!["design_a".into()],
                        design_to_unit_mapping: BTreeMap::default(),
                    },
                    unit_assignments: BTreeMap::from_iter([
                        (0, (0, "variant_a".into())),
                    ]),
                }
            ])
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
                            dec!(30),
                            dec!(130),
                            dec!(180),
                        ),
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
                            dec!(130),
                            dec!(1130),
                            dec!(-179),
                        ),
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
                            dec!(110),
                            dec!(1110),
                            dec!(1),
                        ),
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
                            dec!(120),
                            dec!(1120),
                            dec!(91),
                        ),
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
                            dec!(105),
                            dec!(1105),
                            dec!(91),
                        ),
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
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "assign-process-to-parts",
            "--process manual",
            "--operation add",
            "--manufacturer CONN_MFR.*",
            "--mpn .*",
        ]);

        // when
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

        assert_contains_inorder!(trace_content, [
            "New part. part: Part { manufacturer: \"RES_MFR2\", mpn: \"RES2\" }\n",
            "Removing unused part. part: Part { manufacturer: \"CAP_MFR1\", mpn: \"CAP1\" }\n",
            "Updating placement. old: Placement { ref_des: \"R3\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 5, y: 105, rotation: 90 }, new: Placement { ref_des: \"R3\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 105, y: 1105, rotation: 91 }\n",
            "New placement. placement: Placement { ref_des: \"R2\", part: Part { manufacturer: \"RES_MFR2\", mpn: \"RES2\" }, place: true, pcb_side: Top, x: 120, y: 1120, rotation: 91 }\n",
            "Updating placement. old: Placement { ref_des: \"J1\", part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }, place: true, pcb_side: Bottom, x: 40, y: 140, rotation: -90 }, new: Placement { ref_des: \"J1\", part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }, place: true, pcb_side: Bottom, x: 130, y: 1130, rotation: -179 }\n",
            "Updating placement. old: Placement { ref_des: \"R1\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 10, y: 110, rotation: 0 }, new: Placement { ref_des: \"R1\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 110, y: 1110, rotation: 1 }\n",
            "Marking placement as unused. placement: Placement { ref_des: \"C1\", part: Part { manufacturer: \"CAP_MFR1\", mpn: \"CAP1\" }, place: true, pcb_side: Bottom, x: 30, y: 130, rotation: 180 }\n",
            "Added process. part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }, applicable_processes: [\"manual\"]",
        ]);

        // and
        let project_content: String = read_to_string(ctx.test_project_path.clone())?;
        println!("{}", project_content);

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_05_create_phase_top() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(5);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![
                project::TestProjectPcb {
                    pcb: project::TestPcb {
                        name: "panel_a".to_string(),
                        units: 2,
                        design_names: vec!["design_a".into()],
                        design_to_unit_mapping: BTreeMap::default(),
                    },
                    unit_assignments: BTreeMap::from_iter([
                        (0, (0, "variant_a".into())),
                    ]),
                }
            ])
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
                            dec!(30),
                            dec!(130),
                            dec!(180),
                        ),
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
                            dec!(130),
                            dec!(1130),
                            dec!(-179),
                        ),
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
                            dec!(110),
                            dec!(1110),
                            dec!(1),
                        ),
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
                            dec!(120),
                            dec!(1120),
                            dec!(91),
                        ),
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
                            dec!(105),
                            dec!(1105),
                            dec!(91),
                        ),
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
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "create-phase",
            "--reference top_1",
            "--process pnp",
            &phase_1_load_out_arg,
            "--pcb-side top",
        ]);

        // when
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

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
        let project_content: String = read_to_string(ctx.test_project_path.clone())?;
        println!("{}", project_content);

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_06_create_phase_bottom() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(6);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![
                project::TestProjectPcb {
                    pcb: project::TestPcb {
                        name: "panel_a".to_string(),
                        units: 2,
                        design_names: vec!["design_a".into()],
                        design_to_unit_mapping: BTreeMap::default(),
                    },
                    unit_assignments: BTreeMap::from_iter([
                        (0, (0, "variant_a".into())),
                    ]),
                }
            ])
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
                            dec!(30),
                            dec!(130),
                            dec!(180),
                        ),
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
                            dec!(130),
                            dec!(1130),
                            dec!(-179),
                        ),
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
                            dec!(110),
                            dec!(1110),
                            dec!(1),
                        ),
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
                            dec!(120),
                            dec!(1120),
                            dec!(91),
                        ),
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
                            dec!(105),
                            dec!(1105),
                            dec!(91),
                        ),
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
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "create-phase",
            "--reference bottom_1",
            "--process manual",
            &phase_2_load_out_arg,
            "--pcb-side bottom",
        ]);

        // when
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

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
        let project_content: String = read_to_string(ctx.test_project_path.clone())?;
        println!("{}", project_content);

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_07_assign_placements_to_phase() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(7);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![
                project::TestProjectPcb {
                    pcb: project::TestPcb {
                        name: "panel_a".to_string(),
                        units: 2,
                        design_names: vec!["design_a".into()],
                        design_to_unit_mapping: BTreeMap::default(),
                    },
                    unit_assignments: BTreeMap::from_iter([
                        (0, (0, "variant_a".into())),
                    ]),
                }
            ])
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
                            dec!(30),
                            dec!(130),
                            dec!(180),
                        ),
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
                            dec!(130),
                            dec!(1130),
                            dec!(-179),
                        ),
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
                            dec!(110),
                            dec!(1110),
                            dec!(1),
                        ),
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
                            dec!(120),
                            dec!(1120),
                            dec!(91),
                        ),
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
                            dec!(105),
                            dec!(1105),
                            dec!(91),
                        ),
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
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

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
        let project_content: String = read_to_string(ctx.test_project_path.clone())?;
        println!("{}", project_content);

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
    fn sequence_08_assign_feeder_to_load_out_item() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(8);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
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
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

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
    fn sequence_09_set_placement_ordering() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(9);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![
                project::TestProjectPcb {
                    pcb: project::TestPcb {
                        name: "panel_a".to_string(),
                        units: 2,
                        design_names: vec!["design_a".into()],
                        design_to_unit_mapping: BTreeMap::default(),
                    },
                    unit_assignments: BTreeMap::from_iter([
                        (0, (0, "variant_a".into())),
                    ]),
                }
            ])
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
                    &[("PcbUnit", "Asc"), ("FeederReference", "Asc"), ("RefDes", "Desc")],
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
                            dec!(30),
                            dec!(130),
                            dec!(180),
                        ),
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
                            dec!(130),
                            dec!(1130),
                            dec!(-179),
                        ),
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
                            dec!(110),
                            dec!(1110),
                            dec!(1),
                        ),
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
                            dec!(120),
                            dec!(1120),
                            dec!(91),
                        ),
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
                            dec!(105),
                            dec!(1105),
                            dec!(91),
                        ),
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
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "set-placement-ordering",
            "--phase top_1",
            "--placement-orderings PCB_UNIT:ASC,FEEDER_REFERENCE:ASC,REF_DES:DESC",
            // example for PnP machine placement
            //"--orderings PCB_UNIT:ASC,COST:ASC,AREA:ASC,HEIGHT;ASC,FEEDER_REFERENCE:ASC",
            // example for manual placement
            //"--orderings COST:ASC,AREA:ASC,HEIGHT;ASC,PART:ASC,PCB_UNIT:ASC",
        ]);

        // when
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

        assert_contains_inorder!(trace_content, [
            "Phase placement orderings set. phase: 'top_1', orderings: [PCB_UNIT:ASC, FEEDER_REFERENCE:ASC, REF_DES:DESC]",
        ]);

        // and
        let project_content: String = read_to_string(ctx.test_project_path.clone())?;
        println!("{}", project_content);

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_10_generate_artifacts() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(10);
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
                    x: dec!(120),
                    y: dec!(1120),
                    rotation: dec!(91),
                },
                TestPhasePlacementRecord {
                    object_path: "pcb=1::unit=1::ref_des=R3".to_string(),
                    feeder_reference: "FEEDER_1".to_string(),
                    manufacturer: "RES_MFR1".to_string(),
                    mpn: "RES1".to_string(),
                    x: dec!(105),
                    y: dec!(1105),
                    rotation: dec!(91),
                },
                TestPhasePlacementRecord {
                    object_path: "pcb=1::unit=1::ref_des=R1".to_string(),
                    feeder_reference: "FEEDER_1".to_string(),
                    manufacturer: "RES_MFR1".to_string(),
                    mpn: "RES1".to_string(),
                    x: dec!(110),
                    y: dec!(1110),
                    rotation: dec!(1),
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
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "generate-artifacts",
            "-vv",
        ]);
        // when
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

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
    fn sequence_11_record_phase_operations() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(11);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd_1 = Command::new(env!("CARGO_BIN_EXE_planner_cli"));
        let mut cmd_2 = Command::new(env!("CARGO_BIN_EXE_planner_cli"));
        let mut cmd_3 = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![
                project::TestProjectPcb {
                    pcb: project::TestPcb {
                        name: "panel_a".to_string(),
                        units: 2,
                        design_names: vec!["design_a".into()],
                        design_to_unit_mapping: BTreeMap::default(),
                    },
                    unit_assignments: BTreeMap::from_iter([
                        (0, (0, "variant_a".into())),
                    ]),
                }
            ])
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
                    &[("PcbUnit", "Asc"), ("FeederReference", "Asc"), ("RefDes", "Desc")],
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
                            dec!(30),
                            dec!(130),
                            dec!(180),
                        ),
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
                            dec!(130),
                            dec!(1130),
                            dec!(-179),
                        ),
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
                            dec!(110),
                            dec!(1110),
                            dec!(1),
                        ),
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
                            dec!(120),
                            dec!(1120),
                            dec!(91),
                        ),
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
                            dec!(105),
                            dec!(1105),
                            dec!(91),
                        ),
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
        let project_content: String = read_to_string(ctx.test_project_path.clone())?;
        println!("{}", project_content);

        assert_eq!(project_content, expected_project_content);

        // and
        let operation_history_file = File::open(ctx.phase_1_log_path.clone())?;
        let operation_history: Vec<TestOperationHistoryItem> = serde_json::from_reader(operation_history_file)?;

        assert_operation_history(operation_history, operation_expectations);

        Ok(())
    }

    #[test]
    fn sequence_12_record_placements_operation() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(12);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![
                project::TestProjectPcb {
                    pcb: project::TestPcb {
                        name: "panel_a".to_string(),
                        units: 2,
                        design_names: vec!["design_a".into()],
                        design_to_unit_mapping: BTreeMap::default(),
                    },
                    unit_assignments: BTreeMap::from_iter([
                        (0, (0, "variant_a".into())),
                    ]),
                }
            ])
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
                    &[("PcbUnit", "Asc"), ("FeederReference", "Asc"), ("RefDes", "Desc")],
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
                            dec!(30),
                            dec!(130),
                            dec!(180),
                        ),
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
                            dec!(130),
                            dec!(1130),
                            dec!(-179),
                        ),
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
                            dec!(110),
                            dec!(1110),
                            dec!(1),
                        ),
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
                            dec!(120),
                            dec!(1120),
                            dec!(91),
                        ),
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
                            dec!(105),
                            dec!(1105),
                            dec!(91),
                        ),
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
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "record-placements-operation",
            "--object-path-patterns pcb=1::unit=1::ref_des=R([1-3])?,pcb=1::unit=2::ref_des=.*",
            "--operation placed",
        ]);
        // when
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

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
        let project_content: String = read_to_string(ctx.test_project_path.clone())?;
        println!("{}", project_content);

        assert_eq!(project_content, expected_project_content);

        // and
        let operation_history_file = File::open(ctx.phase_1_log_path.clone())?;
        let operation_history: Vec<TestOperationHistoryItem> = serde_json::from_reader(operation_history_file)?;
        println!("{:?}", operation_history);

        assert_operation_history(operation_history, operation_expectations);

        Ok(())
    }

    #[test]
    fn sequence_13_reset_operations() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(13);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_default_processes()
            .with_pcbs(vec![
                project::TestProjectPcb {
                    pcb: project::TestPcb {
                        name: "panel_a".to_string(),
                        units: 2,
                        design_names: vec!["design_a".into()],
                        design_to_unit_mapping: BTreeMap::default(),
                    },
                    unit_assignments: BTreeMap::from_iter([
                        (0, (0, "variant_a".into())),
                    ]),
                }
            ])
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
                    &[("PcbUnit", "Asc"), ("FeederReference", "Asc"), ("RefDes", "Desc")],
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
                            dec!(30),
                            dec!(130),
                            dec!(180),
                        ),
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
                            dec!(130),
                            dec!(1130),
                            dec!(-179),
                        ),
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
                            dec!(110),
                            dec!(1110),
                            dec!(1),
                        ),
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
                            dec!(120),
                            dec!(1120),
                            dec!(91),
                        ),
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
                            dec!(105),
                            dec!(1105),
                            dec!(91),
                        ),
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
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "reset-operations",
        ]);
        // when
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

        assert_contains_inorder!(trace_content, [
            "Placement operations reset.\n",
            "Phase operations reset. phase: bottom_1\n",
            "Phase operations reset. phase: top_1\n",
        ]);

        // and
        let project_content: String = read_to_string(ctx.test_project_path.clone())?;
        println!("{}", project_content);

        assert_eq!(project_content, expected_project_content);

        Ok(())
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

    #[test]
    fn sequence_14_cleanup() {
        let mut ctx_guard = context::acquire(14);
        let ctx = ctx_guard.1.take().unwrap();
        drop(ctx);
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
            Usage: planner_cli [OPTIONS] <--project <PROJECT_NAME>> <COMMAND>

            Commands:
              create                          Create a new job
              add-pcb                         Add a PCB
              assign-variant-to-unit          Assign a design variant to a PCB unit
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
                  --trace [<TRACE>]         Trace log file
                  --path <PATH>             Path [default: .]
                  --project <PROJECT_NAME>  Project name
              -v, --verbose...              Increase logging verbosity
              -q, --quiet...                Decrease logging verbosity
              -h, --help                    Print help
              -V, --version                 Print version
        "};

        // when
        cmd
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

            Usage: planner_cli <--project <PROJECT_NAME>> create [OPTIONS]

            Options:
              -v, --verbose...  Increase logging verbosity
              -q, --quiet...    Decrease logging verbosity
              -h, --help        Print help
        "};

        // when
        cmd.args(["create", "--help"])
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
            Add a PCB

            Usage: planner_cli <--project <PROJECT_NAME>> add-pcb [OPTIONS] --name <NAME> --units <UNITS>

            Options:
                  --name <NAME>    Name of the PCB, e.g. 'panel_1'
                  --units <UNITS>  Units
              -v, --verbose...     Increase logging verbosity
              -q, --quiet...       Decrease logging verbosity
              -h, --help           Print help
        "};

        // when
        cmd.args(["add-pcb", "--help"])
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

            Usage: planner_cli <--project <PROJECT_NAME>> assign-variant-to-unit [OPTIONS] --design <DESIGN_NAME> --variant <VARIANT_NAME> --unit <OBJECT_PATH>

            Options:
                  --design <DESIGN_NAME>    Name of the design
                  --variant <VARIANT_NAME>  Variant of the design
                  --unit <OBJECT_PATH>      PCB unit path
              -v, --verbose...              Increase logging verbosity
              -q, --quiet...                Decrease logging verbosity
              -h, --help                    Print help
        "};

        // when
        cmd.args(["assign-variant-to-unit", "--help"])
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

            Usage: planner_cli <--project <PROJECT_NAME>> assign-process-to-parts [OPTIONS] --process <PROCESS> --operation <OPERATION> --manufacturer <MANUFACTURER> --mpn <MPN>

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
        cmd.args(["assign-process-to-parts", "--help"])
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

            Usage: planner_cli <--project <PROJECT_NAME>> create-phase [OPTIONS] --process <PROCESS> --reference <REFERENCE> --load-out <LOAD_OUT> --pcb-side <PCB_SIDE>

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
        cmd.args(["create-phase", "--help"])
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

            Usage: planner_cli <--project <PROJECT_NAME>> assign-placements-to-phase [OPTIONS] --phase <PHASE> --operation <OPERATION> --placements <PLACEMENTS>

            Options:
                  --phase <PHASE>            Phase reference (e.g. 'top_1')
                  --operation <OPERATION>    Operation [possible values: set, clear]
                  --placements <PLACEMENTS>  Placements object path pattern (regexp)
              -v, --verbose...               Increase logging verbosity
              -q, --quiet...                 Decrease logging verbosity
              -h, --help                     Print help
        "};

        // when
        cmd.args(["assign-placements-to-phase", "--help"])
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

            Usage: planner_cli <--project <PROJECT_NAME>> assign-feeder-to-load-out-item [OPTIONS] --phase <PHASE> --feeder-reference <FEEDER_REFERENCE> --manufacturer <MANUFACTURER> --mpn <MPN>

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
        cmd.args(["assign-feeder-to-load-out-item", "--help"])
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

            Usage: planner_cli <--project <PROJECT_NAME>> set-placement-ordering [OPTIONS] --phase <PHASE> --placement-orderings [<PLACEMENT_ORDERINGS>...]

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
        cmd.args(["set-placement-ordering", "--help"])
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

            Usage: planner_cli <--project <PROJECT_NAME>> generate-artifacts [OPTIONS]

            Options:
              -v, --verbose...  Increase logging verbosity
              -q, --quiet...    Decrease logging verbosity
              -h, --help        Print help
        "};

        // when
        cmd.args(["generate-artifacts", "--help"])
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

            Usage: planner_cli <--project <PROJECT_NAME>> record-phase-operation [OPTIONS] --phase <PHASE> --operation <OPERATION> --task <TASK> --action <ACTION>

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
        cmd.args(["record-phase-operation", "--help"])
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

            Usage: planner_cli <--project <PROJECT_NAME>> record-placements-operation [OPTIONS] --object-path-patterns <OBJECT_PATH_PATTERNS>... --operation <OPERATION>

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
        cmd.args(["record-placements-operation", "--help"])
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

            Usage: planner_cli <--project <PROJECT_NAME>> reset-operations [OPTIONS]

            Options:
              -v, --verbose...  Increase logging verbosity
              -q, --quiet...    Decrease logging verbosity
              -h, --help        Print help
        "};

        // when
        cmd.args(["reset-operations", "--help"])
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout").and(predicate::str::diff(expected_output)));
    }
}
