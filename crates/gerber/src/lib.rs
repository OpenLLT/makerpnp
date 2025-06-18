use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use gerber_types::{Command, ExtendedCode, ExtendedPosition, FileAttribute, FileFunction, Position};
use planning::gerber::GerberPurpose;
use pnp::pcb::PcbSide;
use thiserror::Error;
use tracing::{error, info, trace};

#[allow(dead_code)]
#[cfg(test)]
mod testing;

#[allow(dead_code)]
trait AsGerberPurpose {
    fn as_gerber_purpose(&self) -> (GerberPurpose, Option<PcbSide>);
}

impl AsGerberPurpose for FileFunction {
    fn as_gerber_purpose(&self) -> (GerberPurpose, Option<PcbSide>) {
        fn map_extended_position_to_pcb_side(pos: &ExtendedPosition) -> Option<PcbSide> {
            match pos {
                ExtendedPosition::Top => Some(PcbSide::Top),
                ExtendedPosition::Bottom => Some(PcbSide::Bottom),
                _ => None,
            }
        }

        fn map_position_to_pcb_side(pos: &Position) -> PcbSide {
            match pos {
                Position::Top => PcbSide::Top,
                Position::Bottom => PcbSide::Bottom,
            }
        }

        match self {
            FileFunction::AssemblyDrawing(pos) => (GerberPurpose::Assembly, Some(map_position_to_pcb_side(pos))),
            FileFunction::Component {
                pos, ..
            } => (GerberPurpose::Component, Some(map_position_to_pcb_side(pos))),
            FileFunction::Legend {
                pos, ..
            } => (GerberPurpose::Legend, Some(map_position_to_pcb_side(pos))),
            FileFunction::Copper {
                pos, ..
            } => (GerberPurpose::Copper, map_extended_position_to_pcb_side(pos)),
            FileFunction::Paste(pos) => (GerberPurpose::Paste, Some(map_position_to_pcb_side(pos))),
            FileFunction::SolderMask {
                pos, ..
            } => (GerberPurpose::Solder, Some(map_position_to_pcb_side(pos))),
            _ => (GerberPurpose::Other, None),
        }
    }
}

#[cfg(test)]
mod into_gerber_purpose_tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::component(FileFunction::Component{ layer:0, pos: Position::Top}, (GerberPurpose::Component, Some(PcbSide::Top)))]
    #[case::copper(FileFunction::Copper{ layer: 0, pos: ExtendedPosition::Top, copper_type: None}, (GerberPurpose::Copper, Some(PcbSide::Top)))]
    #[case::legend(FileFunction::Legend { pos: Position::Top, index: None }, (GerberPurpose::Legend, Some(PcbSide::Top)))]
    #[case::solder(FileFunction::SolderMask { pos: Position::Bottom, index: None }, (GerberPurpose::Solder, Some(PcbSide::Bottom)))]
    #[case::paste(FileFunction::Paste(Position::Top), (GerberPurpose::Paste, Some(PcbSide::Top)))]
    #[case::assembly(FileFunction::AssemblyDrawing(Position::Top), (GerberPurpose::Assembly, Some(PcbSide::Top)))]
    #[case::other_drawing(FileFunction::OtherDrawing("Other".to_string()), (GerberPurpose::Other, None))]
    #[case::other(FileFunction::Other("Other".to_string()), (GerberPurpose::Other, None))]
    fn test_into_gerber_purpose_file_function(
        #[case] file_function: FileFunction,
        #[case] expected: (GerberPurpose, Option<PcbSide>),
    ) {
        let (gerber_purpose, pcb_side) = file_function.as_gerber_purpose();
        assert_eq!((gerber_purpose, pcb_side), expected);
    }
}

#[derive(Error, Debug)]
pub enum DetectionError {
    #[error("Parse error")]
    ParseError,
    #[error("IO error")]
    IoError(#[from] std::io::Error),
    #[error("Unable to detect purpose")]
    UnknownPurpose,
}

/// Attempts to detect the purpose and optional pcb side of the gerber file.
///
/// Only looks at the first 20 lines of the gerber file.
/// Looks for 'TF' FileFunction attributes. e.g.
/// `%TF.FileFunction,AssemblyDrawing,Top*%`
#[allow(dead_code)]
pub fn detect_purpose(path: PathBuf) -> Result<(GerberPurpose, Option<PcbSide>), DetectionError> {
    let file = std::fs::File::open(&path).map_err(DetectionError::IoError)?;
    let reader = BufReader::new(file);

    // FUTURE it would be nice if the gerber_parser had a streaming API, so we could just just read as much of the file
    //        as we need.

    let mut headers: Vec<String> = Vec::with_capacity(20);
    let mut lines = reader.lines();
    while let Some(Ok(line)) = lines.next() {
        headers.push(line);

        if headers.len() >= 20 {
            break;
        }
    }

    let headers_content = headers.join("\n");
    trace!("headers: {0}", headers_content);
    let headers_reader = BufReader::new(headers_content.as_bytes());

    let doc = gerber_parser::parse(headers_reader).map_err(|(_partial_doc, e)| {
        error!("Unable to parse gerber file: {0}", e);

        DetectionError::ParseError
    })?;

    doc.commands
        .iter()
        .find_map(|command| match command {
            Ok(Command::ExtendedCode(ExtendedCode::FileAttribute(FileAttribute::FileFunction(file_function)))) => {
                Some(file_function.as_gerber_purpose())
            }
            _ => None,
        })
        .inspect(|(_purpose, pcb_side)| {
            info!(
                "Detected gerber purpose: {:?}, pcb side: {:?}, path: {:?}",
                _purpose, pcb_side, path
            );
        })
        .ok_or(DetectionError::UnknownPurpose)
}

#[cfg(test)]
mod detect_purpose_tests {
    use std::fs::File;
    use std::io::Write;

    use gerber_types::{Command, ExtendedCode, FileAttribute, GenerationSoftware, GerberCode};
    use tempfile::tempdir;

    use super::*;
    use crate::testing::logging_init;

    #[test]
    pub fn test_detect_purpose() {
        // given
        logging_init();

        let temp_dir = tempdir().unwrap();
        let temp_file_path = temp_dir.path().join("test.gbr");

        let mut file = File::create_new(&temp_file_path).expect("create");

        let generation_software = GenerationSoftware {
            vendor: "MakerPnP".to_string(),
            application: "tests".to_string(),
            version: None,
        };

        let commands = vec![
            Command::ExtendedCode(ExtendedCode::FileAttribute(FileAttribute::GenerationSoftware(
                generation_software,
            ))),
            Command::ExtendedCode(ExtendedCode::FileAttribute(FileAttribute::FileFunction(
                FileFunction::AssemblyDrawing(Position::Top),
            ))),
        ];

        commands
            .serialize(&mut file)
            .expect("written");
        file.flush().unwrap();
        drop(file);

        // when
        let result = detect_purpose(temp_file_path);

        // then
        println!("detection result: {:#?}", result);
        let Ok(result) = result else {
            panic!("Unable to detect purpose");
        };

        assert_eq!(result, (GerberPurpose::Assembly, Some(PcbSide::Top)));
    }
}
