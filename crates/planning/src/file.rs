use std::fmt::Display;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub enum FileReference {
    Relative(PathBuf),
    Absolute(PathBuf),
}

impl FileReference {
    pub fn build_path(&self, root: &Path) -> PathBuf {
        match self {
            FileReference::Relative(relative_path) => {
                let mut path = PathBuf::from(root);
                path.push(relative_path);
                path
            }
            FileReference::Absolute(path) => path.clone(),
        }
    }
}

impl Display for FileReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileReference::Relative(path) => write!(f, "Relative: '{}'", path.display()),
            FileReference::Absolute(path) => write!(f, "Absolute: '{}'", path.display()),
        }
    }
}

pub fn load<'de, T: Deserialize<'de>>(file_path: &PathBuf) -> Result<T, std::io::Error> {
    let file = File::open(file_path.clone())?;
    let mut de = serde_json::Deserializer::from_reader(file);
    let t = T::deserialize(&mut de)?;
    Ok(t)
}

pub fn save<'se, T: Serialize>(t: &T, file_path: &PathBuf) -> Result<(), std::io::Error> {
    let file = File::create(file_path)?;
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(file, formatter);
    t.serialize(&mut ser)?;

    let mut file = ser.into_inner();
    let _written = file.write(b"\n")?;

    Ok(())
}
