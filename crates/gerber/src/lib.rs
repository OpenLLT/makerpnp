use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use gerber_types::{Command, ExtendedCode, ExtendedPosition, FileAttribute, FileFunction, Position};
use pnp::pcb::PcbSide;
use thiserror::Error;
use tracing::{error, info, trace};

#[allow(dead_code)]
#[cfg(test)]
mod testing;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct GerberFile {
    pub file: PathBuf,

    /// until the purpose is determined or set, this is `None`
    pub function: Option<GerberFileFunction>,
}

/// For Pick-and-Place planning, we only care about a subset of the gerber files.
/// See `gerber-types::FileFunction` for the full list.
#[derive(Debug, serde::Serialize, serde::Deserialize, Copy, Clone, PartialEq, Eq, Hash)]
pub enum GerberFileFunction {
    Assembly(PcbSide),
    Component(PcbSide),
    Copper(PcbSide),
    Legend(PcbSide),
    Paste(PcbSide),
    /// Aka 'outline', defines the profile / edge / outline of the PCB, hence no side is applicable.
    Profile,
    Solder(PcbSide),

    Other(Option<PcbSide>),
}

impl GerberFileFunction {
    pub fn pcb_side(&self) -> Option<PcbSide> {
        match self {
            GerberFileFunction::Assembly(pcb_side) => Some(*pcb_side),
            GerberFileFunction::Component(pcb_side) => Some(*pcb_side),
            GerberFileFunction::Copper(pcb_side) => Some(*pcb_side),
            GerberFileFunction::Legend(pcb_side) => Some(*pcb_side),
            GerberFileFunction::Paste(pcb_side) => Some(*pcb_side),
            GerberFileFunction::Profile => None,
            GerberFileFunction::Solder(pcb_side) => Some(*pcb_side),
            GerberFileFunction::Other(_) => None,
        }
    }
}

#[allow(dead_code)]
trait AsGerberFunction {
    fn as_gerber_file_function(&self) -> GerberFileFunction;
}

impl AsGerberFunction for FileFunction {
    // FUTURE it seems we need to detect the EDA tool and version in order do determine the file function properly.
    //        * diptrace 4.3 uses 'Drawing' not 'AssemblyDrawing' for the 'BottomAssembly.gbr', 'TopAssembly.gbr'
    //        so we need the 'GenerationSoftware' or list of file names to detect the EDA tool, then when we know the EDA
    //        tool and version we could use a more specialized mapping system.
    fn as_gerber_file_function(&self) -> GerberFileFunction {
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
            FileFunction::AssemblyDrawing(pos) => GerberFileFunction::Assembly(map_position_to_pcb_side(pos)),
            FileFunction::Component {
                pos, ..
            } => GerberFileFunction::Component(map_position_to_pcb_side(pos)),
            FileFunction::Legend {
                pos, ..
            } => GerberFileFunction::Legend(map_position_to_pcb_side(pos)),
            FileFunction::Copper {
                pos, ..
            } if *pos != ExtendedPosition::Inner => {
                GerberFileFunction::Copper(map_extended_position_to_pcb_side(pos).unwrap())
            }
            FileFunction::Paste(pos) => GerberFileFunction::Paste(map_position_to_pcb_side(pos)),
            FileFunction::Profile(_) => GerberFileFunction::Profile,
            FileFunction::SolderMask {
                pos, ..
            } => GerberFileFunction::Solder(map_position_to_pcb_side(pos)),
            _ => GerberFileFunction::Other(None),
        }
    }
}

#[cfg(test)]
mod into_gerber_purpose_tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::component(FileFunction::Component{ layer:0, pos: Position::Top}, GerberFileFunction::Component(PcbSide::Top))]
    #[case::copper(FileFunction::Copper{ layer: 0, pos: ExtendedPosition::Top, copper_type: None}, GerberFileFunction::Copper(PcbSide::Top))]
    #[case::legend(FileFunction::Legend { pos: Position::Top, index: None }, GerberFileFunction::Legend(PcbSide::Top))]
    #[case::solder(FileFunction::SolderMask { pos: Position::Bottom, index: None }, GerberFileFunction::Solder(PcbSide::Bottom))]
    #[case::paste(FileFunction::Paste(Position::Top), GerberFileFunction::Paste(PcbSide::Top))]
    #[case::paste(FileFunction::Profile(None), GerberFileFunction::Profile)]
    #[case::assembly(
        FileFunction::AssemblyDrawing(Position::Top),
        GerberFileFunction::Assembly(PcbSide::Top)
    )]
    #[case::other_drawing(FileFunction::OtherDrawing("Other".to_string()), GerberFileFunction::Other(None))]
    #[case::other(FileFunction::Other("Other".to_string()), GerberFileFunction::Other(None))]
    fn test_into_gerber_purpose_file_function(
        #[case] file_function: FileFunction,
        #[case] expected: GerberFileFunction,
    ) {
        let gerber_function = file_function.as_gerber_file_function();
        assert_eq!(gerber_function, expected);
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
pub fn detect_purpose(path: PathBuf) -> Result<GerberFileFunction, DetectionError> {
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
                Some(file_function.as_gerber_file_function())
            }
            _ => None,
        })
        .inspect(|gerber_file_function| {
            info!("Detected gerber function: {:?}, path: {:?}", gerber_file_function, path);
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

        assert_eq!(result, GerberFileFunction::Assembly(PcbSide::Top));
    }
}
