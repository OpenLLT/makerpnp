use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use lexical_sort::natural_lexical_cmp;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use thiserror::Error;

use crate::pcb::PcbKind;
use crate::placement::RefDes;

#[derive(Debug, Clone, PartialOrd, Ord, Eq, PartialEq, Hash)]
struct ObjectPathChunk {
    key: String,
    value: String,
}

impl FromStr for ObjectPathChunk {
    type Err = ObjectPathError;

    fn from_str(chunk: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = chunk.split('=').collect();
        if parts.len() == 2 {
            let (key, value) = (parts[0], parts[1]);

            match key {
                "pcb" => {
                    let pcb_kind = value.to_string();
                    let pcb_kind: PcbKind =
                        PcbKind::try_from(&pcb_kind).map_err(|_err| ObjectPathError::InvalidPcbKind(pcb_kind))?;
                    Ok(ObjectPathChunk {
                        key: key.to_string(),
                        value: pcb_kind.to_string(),
                    })
                }
                "instance" | "unit" => {
                    let index: usize = value
                        .parse()
                        .map_err(|_err| ObjectPathError::InvalidIndex(value.to_string()))?;
                    if index == 0 {
                        Err(ObjectPathError::IndexLessThanOne)
                    } else {
                        Ok(ObjectPathChunk {
                            key: key.to_string(),
                            value: index.to_string(),
                        })
                    }
                }
                "ref_des" => Ok(ObjectPathChunk {
                    key: key.to_string(),
                    value: value.to_string(),
                }),
                _ => Err(ObjectPathError::UnknownKey(key.to_string())),
            }
        } else {
            Err(ObjectPathError::InvalidChunk(chunk.to_string()))
        }
    }
}

impl Display for ObjectPathChunk {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// A path to an object
///
/// `<pcb=("panel"|"single")::instance=<instance>::"unit"=<unit>[::("ref_dex")=<ref_des>]`
///
/// <instance> = the instance of the pcb within the project, 1 based index.
/// <unit> = the design variant unit on instance, for panels this is >= 1, for single pcbs this is always 1.  1 based index.
///
///
/// valid examples:
///
/// `pcb=panel::instance=1` (points to a panel pcb instance)
/// `pcb=single::instance=1` (points to a single pcb instance)
/// `pcb=panel::instance=1::unit=1` (points to a unit of a panel pcb instance)
/// `pcb=panel::instance=1::unit=2::ref_des=R1` (points to a refdes on a unit of a panel pcb instance)
/// `pcb=single::instance=1::unit=1::ref_des=R1` (points to a refdes on the only unit of a single pcb instance)
///
/// invalid examples:
/// `pcb=panel::instance=1::ref_des=R1` (missing unit)
/// `pcb=single::instance=1::ref_des=R1` (missing unit)
/// `pcb=single::instance=1::unit=2::ref_des=R1` (invalid unit)
///
/// Currently, there are example where wildcards are used to search for objects, e.g. `pcb=panel::instance=.*::unit=.*::ref_des=R1`
/// however this object is not for STORING such patterns, but the string representation of a path
/// can be compared to such a pattern.
#[derive(Debug, Clone, DeserializeFromStr, SerializeDisplay, Eq, PartialEq, Default, Hash)]
pub struct ObjectPath {
    // FUTURE consider if it's better/simpler to use a HashMap here.
    chunks: Vec<ObjectPathChunk>,
}

impl ObjectPath {
    /// pcb_instance is a 1-based index.
    pub fn set_pcb_kind_and_instance(&mut self, pcb_kind: PcbKind, instance: u16) {
        self.set_chunk(ObjectPathChunk {
            key: "pcb".to_string(),
            value: pcb_kind.to_string(),
        });
        self.set_chunk(ObjectPathChunk {
            key: "instance".to_string(),
            value: instance.to_string(),
        });
        if self.validate_instance().is_err() {
            panic!("invalid pcb instance");
        }
    }

    /// only applicable to panels
    /// pcb_unit is a 1-based index.
    pub fn set_pcb_unit(&mut self, unit: u16) {
        self.set_chunk(ObjectPathChunk {
            key: "unit".to_string(),
            value: unit.to_string(),
        });
        if self.validate_pcb_and_unit().is_err() {
            panic!("invalid pcb unit");
        }
    }

    pub fn set_ref_des(&mut self, ref_des: RefDes) {
        self.set_chunk(ObjectPathChunk {
            key: "ref_des".to_string(),
            value: ref_des.to_string(),
        })
    }

    pub fn pcb_unit(&self) -> Result<ObjectPath, ObjectPathError> {
        const PCB_UNIT_KEYS: [&str; 3] = ["pcb", "instance", "unit"];

        let chunks = self
            .chunks
            .iter()
            .filter(|chunk| PCB_UNIT_KEYS.contains(&chunk.key.as_str()))
            .collect::<Vec<_>>();

        if chunks.len() != PCB_UNIT_KEYS.len() {
            return Err(ObjectPathError::MissingPcbUnit);
        }

        Ok(chunks
            .iter()
            .fold(ObjectPath::default(), |mut object_path, &chunk| {
                object_path.chunks.push(chunk.clone());

                object_path
            }))
    }

    pub fn pcb_kind_and_instance(&self) -> Option<(PcbKind, usize)> {
        self.find_chunk_by_key("pcb")
            .zip(self.find_chunk_by_key("instance"))
            .map(|(pcb_chunk, instance_chunk)| {
                (
                    PcbKind::try_from(&pcb_chunk.value).unwrap(),
                    instance_chunk.value.parse().unwrap(),
                )
            })
    }

    fn set_chunk(&mut self, chunk: ObjectPathChunk) {
        let existing_chunk = self.find_chunk_by_key_mut(&chunk.key);
        match existing_chunk {
            Some(existing_chunk) => existing_chunk.value = chunk.value,
            _ => self.chunks.push(chunk),
        }
    }

    fn find_chunk_by_key(&self, key: &str) -> Option<&ObjectPathChunk> {
        let existing_chunk = self
            .chunks
            .iter()
            .find(|existing| existing.key.eq(key));

        existing_chunk
    }

    fn find_chunk_by_key_mut(&mut self, key: &str) -> Option<&mut ObjectPathChunk> {
        let existing_chunk = self
            .chunks
            .iter_mut()
            .find(|existing| existing.key.eq(key));

        existing_chunk
    }

    fn validate_pcb_and_unit(&self) -> Result<&ObjectPath, ObjectPathError> {
        let pcb = self.find_chunk_by_key("pcb");
        let unit = self.find_chunk_by_key("unit");
        match (pcb, unit) {
            (Some(pcb), Some(unit)) if pcb.value == "single" && unit.value != "1" => {
                Err(ObjectPathError::InvalidUnitForPcbKind(unit.value.clone()))
            }
            (_, Some(unit)) if unit.value == "0" => Err(ObjectPathError::IndexLessThanOne),
            _ => Ok(self),
        }
    }

    fn validate_instance(&self) -> Result<&ObjectPath, ObjectPathError> {
        let instance = self.find_chunk_by_key("instance");
        match instance {
            Some(instance) if instance.value == "0" => Err(ObjectPathError::IndexLessThanOne),
            _ => Ok(self),
        }
    }

    fn validate_chunk_ordering(&self) -> Result<&Self, ObjectPathError> {
        /// Ensure all keys are in the correct order, it's ok to have fewer keys, but not missing or additional keys
        fn is_valid_ordering(keys: &[&String]) -> bool {
            const KEY_ORDERING: [&str; 4] = ["pcb", "instance", "unit", "ref_des"];
            let mut key_iter = KEY_ORDERING.iter();

            for &item in keys {
                if let Some(pos) = key_iter.position(|&key| key == item) {
                    if pos == 0 {
                        continue;
                    }
                }
                return false;
            }

            true
        }

        let keys = self
            .chunks
            .iter()
            .map(|chunk| &chunk.key)
            .collect::<Vec<_>>();

        if !is_valid_ordering(keys.as_slice()) {
            Err(ObjectPathError::InvalidChunkOrder)
        } else {
            Ok(self)
        }
    }
}

#[cfg(test)]
mod pcb_unit_tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("pcb=panel")]
    #[case("pcb=single")]
    #[case("pcb=panel::instance=1")]
    #[case("pcb=single::instance=1")]
    #[case("pcb=panel::instance=1::unit=1")]
    #[case("pcb=single::instance=1::unit=1")]
    #[case("pcb=panel::instance=1::unit=1::ref_des=R1")]
    #[case("pcb=single::instance=1::unit=1::ref_des=R1")]
    pub fn from_str(#[case] input: &str) {
        // expect
        ObjectPath::from_str(input).expect("ok");
    }

    #[rstest]
    #[case("bad", Err(ObjectPathError::InvalidChunk("bad".to_string())))]
    #[case("foo=bar", Err(ObjectPathError::UnknownKey("foo".to_string())))]
    #[case("instance=1::", Err(ObjectPathError::InvalidChunk("".to_string())))]
    /// the invalid trailing ':' becomes part of the index.
    #[case("instance=1:", Err(ObjectPathError::InvalidIndex("1:".to_string())))]
    #[case("instance=0", Err(ObjectPathError::IndexLessThanOne))]
    #[case("unit=0", Err(ObjectPathError::IndexLessThanOne))]
    #[case("pcb=bad", Err(ObjectPathError::InvalidPcbKind("bad".to_string())))]
    #[case("pcb=panel::instance=1::::ref_des=R1", Err(ObjectPathError::InvalidChunk("".to_string())))]
    #[case("pcb=single::instance=1::unit=2", Err(ObjectPathError::InvalidUnitForPcbKind("2".to_string())))]
    #[case("instance=1::pcb=single", Err(ObjectPathError::InvalidChunkOrder))]
    #[case("instance=1", Err(ObjectPathError::InvalidChunkOrder))]
    #[case("unit=1", Err(ObjectPathError::InvalidChunkOrder))]
    #[case("ref_des=1", Err(ObjectPathError::InvalidChunkOrder))]
    #[case("pcb=panel::instance=1::ref_des=R1", Err(ObjectPathError::InvalidChunkOrder))]
    pub fn from_str_errors(#[case] input: &str, #[case] expected_result: Result<ObjectPath, ObjectPathError>) {
        // expect
        assert_eq!(ObjectPath::from_str(input), expected_result);
    }

    #[test]
    pub fn pcb_unit() {
        // given
        let object_path = ObjectPath::from_str("pcb=panel::instance=1::unit=1::ref_des=R1").expect("always ok");

        // and
        let expected_result = Ok(ObjectPath::from_str("pcb=panel::instance=1::unit=1").expect("always ok"));

        // when
        let result = object_path.pcb_unit();

        // then
        assert_eq!(result, expected_result);
    }

    #[test]
    pub fn pcb_unit_with_no_unit() {
        // given
        let mut object_path = ObjectPath::default();
        object_path.set_pcb_kind_and_instance(PcbKind::Panel, 1);
        object_path.set_ref_des("R1".to_ascii_lowercase().into());

        // when
        let result = object_path.pcb_unit();

        // then
        assert_eq!(result, Err(ObjectPathError::MissingPcbUnit))
    }

    #[test]
    pub fn set_ref_des() {
        // given
        let mut object_path = ObjectPath::from_str("pcb=panel::instance=1::unit=1").expect("always ok");

        // and
        let expected_result = ObjectPath::from_str("pcb=panel::instance=1::unit=1::ref_des=R1").expect("always ok");

        // when
        object_path.set_ref_des("R1".to_string().into());

        // then
        assert_eq!(object_path, expected_result);
    }

    #[test]
    pub fn update_ref_des() {
        // given
        let mut object_path = ObjectPath::from_str("pcb=panel::instance=1::unit=1::ref_des=R2").expect("always ok");

        // and
        let expected_result = ObjectPath::from_str("pcb=panel::instance=1::unit=1::ref_des=R1").expect("always ok");

        // when
        object_path.set_ref_des("R1".to_string().into());

        // then
        assert_eq!(object_path, expected_result);
    }

    #[test]
    pub fn set_pcb_kind_and_instance() {
        // given
        let mut object_path = ObjectPath::default();

        // and
        let expected_result = ObjectPath::from_str("pcb=panel::instance=1").expect("always ok");

        // when
        object_path.set_pcb_kind_and_instance(PcbKind::Panel, 1);

        // then
        assert_eq!(object_path, expected_result);
    }

    #[test]
    pub fn set_pcb_unit() {
        // given
        let mut object_path = ObjectPath::from_str("pcb=panel::instance=1").expect("always ok");

        // and
        let expected_result = ObjectPath::from_str("pcb=panel::instance=1::unit=1").expect("always ok");

        // when
        object_path.set_pcb_unit(1);

        // then
        assert_eq!(object_path, expected_result);
    }

    #[test]
    pub fn append_chunks() {
        // given
        let mut object_path = ObjectPath::default();

        // and
        let expected_result = ObjectPath::from_str("pcb=panel::instance=1::unit=1::ref_des=R1").expect("always ok");

        // when
        object_path.set_pcb_kind_and_instance(PcbKind::Panel, 1);
        object_path.set_pcb_unit(1);
        object_path.set_ref_des("R1".to_string().into());

        // then
        assert_eq!(object_path, expected_result);
    }
}

impl Display for ObjectPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let formatted_chunks: Vec<String> = self
            .chunks
            .iter()
            .map(|chunk| format!("{}", chunk))
            .collect();

        write!(f, "{}", formatted_chunks.join("::"))
    }
}

impl FromStr for ObjectPath {
    type Err = ObjectPathError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let object_path =
            value
                .split("::")
                .try_fold(ObjectPath::default(), |mut object_path: ObjectPath, chunk_str| {
                    match ObjectPathChunk::from_str(chunk_str) {
                        Ok(chunk) => {
                            object_path.chunks.push(chunk);
                            Ok(object_path)
                        }
                        Err(err) => Err(err),
                    }
                });

        if let Ok(object_path) = &object_path {
            object_path.validate_chunk_ordering()?;
            object_path.validate_pcb_and_unit()?;
        }

        object_path
    }
}

#[derive(Error, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub enum ObjectPathError {
    #[error("Invalid object path. value: '{0:}'")]
    Invalid(String),
    #[error("Invalid index in path. index: '{0:}'")]
    InvalidIndex(String),
    #[error("Invalid chunk in path. chunk: '{0:}'")]
    InvalidChunk(String),
    #[error("Invalid chunk key in path. key: '{0:}'")]
    UnknownKey(String),
    #[error("Index must be greater than zero")]
    IndexLessThanOne,
    #[error("Invalid PCB kind. value: '{0:}'")]
    InvalidPcbKind(String),
    #[error("Missing PCB unit.")]
    MissingPcbUnit,
    #[error("Invalid unit for PCB kind. value: '{0:}'")]
    InvalidUnitForPcbKind(String),
    #[error("Invalid chunk order, required ordering is: pcb, instance, unit, ref_des.")]
    InvalidChunkOrder,
}

impl Ord for ObjectPath {
    fn cmp(&self, other: &Self) -> Ordering {
        //extract_numbers(&self.to_string()).cmp(&extract_numbers(&other.to_string()))
        natural_lexical_cmp(&self.to_string(), &other.to_string())
    }
}

impl PartialOrd for ObjectPath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod ordering_tests {
    use std::str::FromStr;

    use crate::object_path::ObjectPath;

    #[test]
    fn test_object_paths_are_sorted_lexicographically() {
        let mut values = [
            ObjectPath::from_str("pcb=panel::instance=10::unit=1").unwrap(),
            ObjectPath::from_str("pcb=panel::instance=10::unit=10").unwrap(),
            ObjectPath::from_str("pcb=panel::instance=1::unit=10").unwrap(),
            ObjectPath::from_str("pcb=panel::instance=1::unit=1").unwrap(),
        ];

        values.sort(); // Uses our custom Ord implementation

        let sorted_strings: Vec<String> = values
            .iter()
            .map(|obj| obj.to_string())
            .collect();
        let expected = vec![
            "pcb=panel::instance=1::unit=1",
            "pcb=panel::instance=1::unit=10",
            "pcb=panel::instance=10::unit=1",
            "pcb=panel::instance=10::unit=10",
        ];

        assert_eq!(sorted_strings, expected);
    }
}
