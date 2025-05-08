use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use lexical_sort::natural_lexical_cmp;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use thiserror::Error;

use crate::object_path::leading_keys::leading_keys;
use crate::placement::RefDes;

// TODO consider if unit paths should use zero-based index
//      * there is a lot of +1 -1 in the codebase
//      * greater potential for error
//      * less potential for confusion
//      * making it 0 based would remove some error checking

#[derive(Debug, Clone, PartialOrd, Ord, Eq, PartialEq, Hash)]
pub struct ObjectPathChunk {
    key: String,
    value: String,
}

impl ObjectPathChunk {
    /// See [`ObjectPathChunk::from_str`] for a variant that does validation
    ///
    /// Safety: no validation is done
    pub const fn from_raw(key: String, value: String) -> Self {
        Self {
            key,
            value,
        }
    }

    /// See [`ObjectPathChunk::from_str`] for a variant that does validation
    ///
    /// Safety: no validation is done
    pub fn from_raw_str(key: &str, value: &str) -> Self {
        Self {
            key: key.to_string(),
            value: value.to_string(),
        }
    }
}

const KEY_ORDERING: [&str; 3] = ["pcb", "unit", "ref_des"];

impl FromStr for ObjectPathChunk {
    type Err = ObjectPathError;

    fn from_str(maybe_chunk: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = maybe_chunk.split('=').collect();
        if parts.len() == 2 {
            let (key, value) = (parts[0], parts[1]);

            let chunk = ObjectPathChunk {
                key: key.to_string(),
                value: value.to_string(),
            };

            if !KEY_ORDERING.contains(&key) {
                return Err(ObjectPathError::UnknownKey(chunk));
            }

            match key {
                "pcb" | "unit" => {
                    ObjectPath::validate_index_chunk_u16(&chunk)?;
                }
                _ => {}
            }
            Ok(chunk)
        } else {
            Err(ObjectPathError::InvalidChunk(maybe_chunk.to_string()))
        }
    }
}

impl Display for ObjectPathChunk {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// A path to an object of a pcb
///
/// `pcb=<instance>::"unit"=<unit>[::("ref_dex")=<ref_des>]`
///
/// <instance> = the instance of the pcb within the project, 1 based index.
/// <unit> = the design variant unit on instance, for panels this is >= 1, for single pcbs this is always 1.  1 based index.
///
///
/// valid examples:
///
/// `pcb=1` (points to a pcb instance)
/// `pcb=1::unit=1` (points to a unit of a panel pcb instance)
/// `pcb=2::unit=2::ref_des=R1` (points to a refdes on the second unit of a panel pcb instance)
///
/// invalid examples:
/// `pcb=1::ref_des=R1` (missing unit)
///
/// Currently, there are example where wildcards are used to search for objects, e.g. `pcb=.*::unit=.*::ref_des=R1`
/// however this object is not for STORING such patterns, but the string representation of a path
/// can be compared to such a pattern.
#[derive(Debug, Clone, DeserializeFromStr, SerializeDisplay, Eq, PartialEq, Default, Hash)]
pub struct ObjectPath {
    // FUTURE consider if it's better/simpler to use a HashMap here.
    chunks: Vec<ObjectPathChunk>,
}

impl ObjectPath {
    // TODO rename to set_pcb_number?
    /// pcb_instance is a 1-based index.
    pub fn set_pcb_instance(&mut self, instance: u16) {
        assert!(instance > 0);
        self.set_chunk(ObjectPathChunk {
            key: "pcb".to_string(),
            value: instance.to_string(),
        });
        if self.validate_pcb().is_err() {
            panic!("invalid pcb instance");
        }
    }

    // TODO rename to set_pcb_unit_number?
    /// pcb_unit is a 1-based index.
    pub fn set_pcb_unit(&mut self, unit: u16) {
        assert!(unit > 0);
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

    pub fn pcb_unit_path(&self) -> Result<ObjectPath, ObjectPathError> {
        const PCB_UNIT_KEYS: [&str; 2] = ["pcb", "unit"];

        let (count, chunks) = leading_keys(&PCB_UNIT_KEYS, &self.chunks, |required_key, chunk| {
            chunk.key.eq(required_key)
        });

        if count != PCB_UNIT_KEYS.len() {
            return Err(ObjectPathError::MissingOrderedChunks(
                PCB_UNIT_KEYS
                    .iter()
                    .map(|key| key.to_string())
                    .collect(),
            ));
        }

        Ok(chunks
            .iter()
            .fold(ObjectPath::default(), |mut object_path, chunk| {
                object_path.chunks.push(chunk.clone());

                object_path
            }))
    }

    pub fn pcb_instance(&self) -> Result<u16, ObjectPathError> {
        let chunk = self
            .find_chunk_by_key("pcb")
            .ok_or(ObjectPathError::MissingChunk("pcb".to_string()))?;
        Self::validate_index_chunk_u16(chunk)
    }

    pub fn pcb_unit(&self) -> Result<u16, ObjectPathError> {
        // this ensures all the required chunks are present
        let _pcb_unit_path = self.pcb_unit_path()?;

        let chunk = self.find_chunk_by_key("unit").unwrap();
        Self::validate_index_chunk_u16(chunk)
    }

    pub fn pcb_instance_and_unit(&self) -> Result<(u16, u16), ObjectPathError> {
        let pcb_instance = self.pcb_instance()?;
        let pcb_unit = self.pcb_unit()?;
        Ok((pcb_instance, pcb_unit))
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

    fn validate_pcb_and_unit(&self) -> Result<&Self, ObjectPathError> {
        let pcb_and_unit_path = self.pcb_unit_path()?;

        for chunk in pcb_and_unit_path.chunks.iter() {
            Self::validate_index_chunk_u16(chunk)?;
        }

        Ok(&self)
    }

    fn validate_index_chunk_u16(chunk: &ObjectPathChunk) -> Result<u16, ObjectPathError> {
        let value = chunk
            .value
            .parse::<u16>()
            .map_err(|_err| ObjectPathError::InvalidIndex(chunk.clone()))?;

        if value == 0 {
            return Err(ObjectPathError::IndexLessThanOne(chunk.clone()));
        }
        Ok(value)
    }

    fn validate_pcb(&self) -> Result<&Self, ObjectPathError> {
        let pcb_chunk = self
            .find_chunk_by_key("pcb")
            .ok_or(ObjectPathError::MissingChunk("pcb".to_string()))?;
        Self::validate_index_chunk_u16(pcb_chunk)?;
        Ok(&self)
    }

    fn validate_chunk_ordering(&self) -> Result<&Self, ObjectPathError> {
        /// Ensure all keys are in the correct order, it's ok to have fewer keys, but not missing or additional keys
        fn is_valid_ordering(keys: &[&String], ordered_keys: &[&str]) -> bool {
            let mut key_iter = ordered_keys.iter();

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

        if !is_valid_ordering(keys.as_slice(), &KEY_ORDERING) {
            Err(ObjectPathError::InvalidChunkOrdering(
                KEY_ORDERING
                    .iter()
                    .map(|key| key.to_string())
                    .collect(),
            ))
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
    #[case("pcb=1")]
    #[case("pcb=1::unit=1")]
    #[case("pcb=65535::unit=65535")]
    #[case("pcb=3::unit=3::ref_des=R1")]
    pub fn from_str(#[case] input: &str) {
        // expect
        ObjectPath::from_str(input).expect("ok");
    }

    #[rstest]
    #[case("bad", Err(ObjectPathError::InvalidChunk("bad".to_string())))]
    #[case(
        "foo=bar",
        Err(ObjectPathError::UnknownKey(ObjectPathChunk::from_raw_str("foo", "bar")))
    )]
    // trailing chunk separator is invalid.
    #[case("pcb=1::unit=1::", Err(ObjectPathError::InvalidChunk("".to_string())))]
    // the invalid trailing ':' becomes part of the index.
    #[case(
        "pcb=1:",
        Err(ObjectPathError::InvalidIndex(ObjectPathChunk::from_raw_str("pcb", "1:")))
    )]
    #[case(
        "pcb=0",
        Err(ObjectPathError::IndexLessThanOne(ObjectPathChunk::from_raw_str("pcb", "0")))
    )]
    #[case(
        "unit=0",
        Err(ObjectPathError::IndexLessThanOne(ObjectPathChunk::from_raw_str("unit", "0")))
    )]
    #[case(
        "pcb=bad",
        Err(ObjectPathError::InvalidIndex(ObjectPathChunk::from_raw_str("pcb", "bad")))
    )]
    #[case(
        "pcb=65536",
        Err(ObjectPathError::InvalidIndex(ObjectPathChunk::from_raw_str("pcb", "65536")))
    )]
    #[case(
        "pcb=1::unit=65536",
        Err(ObjectPathError::InvalidIndex(ObjectPathChunk::from_raw_str("unit", "65536")))
    )]
    #[case("pcb=1::::ref_des=R1", Err(ObjectPathError::InvalidChunk("".to_string())))]
    #[case("unit=1::pcb=1", Err(ObjectPathError::InvalidChunkOrdering(vec!["pcb".to_string(), "unit".to_string(), "ref_des".to_string()])))]
    #[case("unit=1", Err(ObjectPathError::InvalidChunkOrdering(vec!["pcb".to_string(), "unit".to_string(), "ref_des".to_string()])))]
    #[case("ref_des=1", Err(ObjectPathError::InvalidChunkOrdering(vec!["pcb".to_string(), "unit".to_string(), "ref_des".to_string()])))]
    #[case("pcb=1::ref_des=R1", Err(ObjectPathError::InvalidChunkOrdering(vec!["pcb".to_string(), "unit".to_string(), "ref_des".to_string()])))]
    pub fn from_str_errors_1(#[case] input: &str, #[case] expected_result: Result<ObjectPath, ObjectPathError>) {
        // expect
        assert_eq!(ObjectPath::from_str(input), expected_result);
    }

    #[test]
    pub fn pcb_unit() {
        // given
        let object_path = ObjectPath::from_str("pcb=1::unit=1::ref_des=R1").expect("always ok");

        // and
        let expected_result = Ok(ObjectPath::from_str("pcb=1::unit=1").expect("always ok"));

        // when
        let result = object_path.pcb_unit_path();

        // then
        assert_eq!(result, expected_result);
    }

    #[test]
    pub fn pcb_unit_with_no_unit() {
        // given
        let mut object_path = ObjectPath::default();
        object_path.set_pcb_instance(1);
        object_path.set_ref_des("R1".to_ascii_lowercase().into());

        // when
        let result = object_path.pcb_unit_path();

        // then
        assert_eq!(
            result,
            Err(ObjectPathError::MissingOrderedChunks(vec![
                "pcb".to_string(),
                "unit".to_string()
            ]))
        );
    }

    #[test]
    pub fn set_ref_des() {
        // given
        let mut object_path = ObjectPath::from_str("pcb=1::unit=1").expect("always ok");

        // and
        let expected_result = ObjectPath::from_str("pcb=1::unit=1::ref_des=R1").expect("always ok");

        // when
        object_path.set_ref_des("R1".to_string().into());

        // then
        assert_eq!(object_path, expected_result);
    }

    #[test]
    pub fn update_ref_des() {
        // given
        let mut object_path = ObjectPath::from_str("pcb=1::unit=1::ref_des=R2").expect("always ok");

        // and
        let expected_result = ObjectPath::from_str("pcb=1::unit=1::ref_des=R1").expect("always ok");

        // when
        object_path.set_ref_des("R1".to_string().into());

        // then
        assert_eq!(object_path, expected_result);
    }

    #[test]
    pub fn set_pcb_instance() {
        // given
        let mut object_path = ObjectPath::default();

        // and
        let expected_result = ObjectPath::from_str("pcb=1").expect("always ok");

        // when
        object_path.set_pcb_instance(1);

        // then
        assert_eq!(object_path, expected_result);
    }

    #[test]
    pub fn set_pcb_unit() {
        // given
        let mut object_path = ObjectPath::from_str("pcb=1").expect("always ok");

        // and
        let expected_result = ObjectPath::from_str("pcb=1::unit=1").expect("always ok");

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
        let expected_result = ObjectPath::from_str("pcb=1::unit=1::ref_des=R1").expect("always ok");

        // when
        object_path.set_pcb_instance(1);
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
            object_path.validate_pcb()?;
        }

        object_path
    }
}

#[derive(Error, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub enum ObjectPathError {
    #[error("Invalid object path. value: '{0}'")]
    Invalid(String),
    #[error("Invalid index in path. chunk: '{0}'")]
    InvalidIndex(ObjectPathChunk),
    #[error("Invalid chunk in path. chunk: '{0}'")]
    InvalidChunk(String),
    #[error("Invalid chunk key in path. chunk: '{0}'")]
    UnknownKey(ObjectPathChunk),
    #[error("Index must be greater than zero, chunk: '{0}'")]
    IndexLessThanOne(ObjectPathChunk),
    #[error("Missing chunk. required chunk key: {0}")]
    MissingChunk(String),
    #[error("Missing ordered chunks. required chunk keys (ordered): {0:?}")]
    MissingOrderedChunks(Vec<String>),
    #[error("Invalid chunk ordering. required chunk key ordering: {0:?}")]
    InvalidChunkOrdering(Vec<String>),
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
            ObjectPath::from_str("pcb=10::unit=1").unwrap(),
            ObjectPath::from_str("pcb=10::unit=10").unwrap(),
            ObjectPath::from_str("pcb=1::unit=10").unwrap(),
            ObjectPath::from_str("pcb=1::unit=1").unwrap(),
        ];

        values.sort(); // Uses our custom Ord implementation

        let sorted_strings: Vec<String> = values
            .iter()
            .map(|obj| obj.to_string())
            .collect();
        let expected = vec!["pcb=1::unit=1", "pcb=1::unit=10", "pcb=10::unit=1", "pcb=10::unit=10"];

        assert_eq!(sorted_strings, expected);
    }
}

mod leading_keys {
    pub fn leading_keys<'a, T, U, F>(required_keys: &[T], keys: &'a [U], pred: F) -> (usize, &'a [U])
    where
        F: Fn(&T, &U) -> bool,
    {
        let mut count = 0;

        for (req, key) in required_keys.iter().zip(keys.iter()) {
            if pred(req, key) {
                count += 1;
            } else {
                break;
            }
        }

        (count, &keys[..count])
    }

    #[cfg(test)]
    mod tests {
        use rstest::rstest;

        use super::*;

        #[rstest]
        #[case(vec!["a", "b", "c"], (2, vec!["a", "b"]))]
        #[case(vec!["b", "c"], (0, vec![]))]
        #[case(vec!["a"], (1, vec!["a"]))]
        #[case(vec!["a", "c"], (1, vec!["a"]))]
        #[case(vec!["a", "b", "c", "d"], (2, vec!["a", "b"]))]
        #[case(vec!["z", "a", "b"], (0, vec![]))]
        fn test_leading_keys(#[case] input: Vec<&str>, #[case] expected: (usize, Vec<&str>)) {
            let required = ["a", "b"];
            let eq = |a: &&str, b: &&str| a == b;

            let result = leading_keys(&required, &input, eq);
            assert_eq!(result, (expected.0, expected.1.as_slice()));
        }
    }
}
