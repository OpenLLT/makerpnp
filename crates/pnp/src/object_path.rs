use std::fmt::{Display, Formatter};
use std::str::FromStr;

use serde_with::{DeserializeFromStr, SerializeDisplay};
use thiserror::Error;

use crate::pcb::PcbKind;

#[derive(Debug, Clone, PartialOrd, Ord, Eq, PartialEq)]
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
                    let pcb_kind: PcbKind = PcbKind::try_from(&pcb_kind)
                        .map_err(|_err| ObjectPathError::InvalidPcbKind(pcb_kind))?;
                    Ok(ObjectPathChunk {
                        key: key.to_string(),
                        value: pcb_kind.to_string(),
                    })
                },
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
#[derive(
    Debug,
    Clone,
    DeserializeFromStr,
    SerializeDisplay,
    PartialOrd,
    Ord,
    Eq,
    PartialEq,
    Default
)]
pub struct ObjectPath {
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
    }

    /// only applicable to panels
    /// pcb_unit is a 1-based index.
     pub fn set_pcb_unit(&mut self, unit: u16) {
        self.set_chunk(ObjectPathChunk {
            key: "unit".to_string(),
            value: unit.to_string(),
        })
    }

    pub fn set_ref_des(&mut self, ref_des: String) {
        self.set_chunk(ObjectPathChunk {
            key: "ref_des".to_string(),
            value: ref_des,
        })
    }

    pub fn pcb_unit(&self) -> Result<ObjectPath, ObjectPathError> {
        const PCB_UNIT_KEYS: [&str; 3] = ["pcb", "instance", "unit"];

        let chunks = self.chunks
            .iter()
            .filter(|chunk|PCB_UNIT_KEYS.contains(&chunk.key.as_str()))
            .collect::<Vec<_>>();
        
        if chunks.len() != PCB_UNIT_KEYS.len() {
            return Err(ObjectPathError::MissingPcbUnit)
        }
            
        Ok(chunks
            .iter()
            .fold(ObjectPath::default(), |mut object_path, &chunk| {
                object_path.chunks.push(chunk.clone());

                object_path
            }))
    }

    pub fn pcb_kind_and_instance(&self) -> Option<(PcbKind, usize)> {
        self.find_chunk_by_key("pcb").zip(self.find_chunk_by_key("instance"))
            .map(|(pcb_chunk, instance_chunk)| (PcbKind::try_from(&pcb_chunk.value).unwrap(), instance_chunk.value.parse().unwrap()))
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
}

#[cfg(test)]
mod pcb_unit_tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
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
        let object_path = ObjectPath::from_str("pcb=single::instance=1::ref_des=R1").expect("always ok");

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
        object_path.set_ref_des("R1".to_string());

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
        object_path.set_ref_des("R1".to_string());

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
        let mut object_path = ObjectPath::default();

        // and
        let expected_result = ObjectPath::from_str("unit=1").expect("always ok");

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
        object_path.set_ref_des("R1".to_string());

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
        value
            .split("::")
            .try_fold(
                ObjectPath::default(),
                |mut object_path: ObjectPath, chunk_str| match ObjectPathChunk::from_str(chunk_str) {
                    Ok(chunk) => {
                        object_path.chunks.push(chunk);
                        Ok(object_path)
                    }
                    Err(err) => Err(err),
                },
            )

        // TODO validate the the order of the chunks is correct
        // TODO validate that the unit for a single pcbs is always 1
        // TODO validate that if a refdes is present, a unit must also be present
        // NOTE Since it's not /fully/ defined what a object path will be used for, this above are not
        //      implemented yet, but probably should be soon.
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
}
