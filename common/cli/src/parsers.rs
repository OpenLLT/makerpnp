use std::collections::HashMap;
use std::ffi::{OsStr, OsString};

use clap::builder::TypedValueParser;
use clap::error::ErrorKind;
use clap::{value_parser, Arg, Command, Error};
use nalgebra::Vector2;
use planning::file::FileReference;
use planning::placement::PlacementSortingItem;
use pnp::panel::Dimensions;
use rust_decimal::Decimal;

use crate::args::{PlacementSortingModeArg, SortOrderArg};

#[derive(Clone, Default)]
pub struct PlacementSortingItemParser {}

impl TypedValueParser for PlacementSortingItemParser {
    type Value = PlacementSortingItem;

    /// Parses a value in the format '<MODE>:<SORT_ORDER>' with values in SCREAMING_SNAKE_CASE, e.g. 'FEEDER_REFERENCE:ASC'
    fn parse_ref(&self, cmd: &Command, _arg: Option<&Arg>, value: &OsStr) -> Result<Self::Value, Error> {
        let chunks_str = match value.to_str() {
            Some(str) => Ok(str),
            // TODO create a test for this edge case, how to invoke this code path, is the message helpful to the user, how is it displayed by clap?
            None => Err(Error::raw(ErrorKind::InvalidValue, "Invalid argument encoding")),
        }?;

        let mut chunks: Vec<_> = chunks_str.split(':').collect();
        if chunks.len() != 2 {
            return Err(Error::raw(
                ErrorKind::InvalidValue,
                format!(
                    "Invalid argument. Required format: '<MODE>:<SORT_ORDER>', found: '{}'",
                    chunks_str
                ),
            ));
        }

        let sort_order_str = chunks.pop().unwrap();
        let mode_str = chunks.pop().unwrap();

        let mode_parser = value_parser!(PlacementSortingModeArg);
        let mode_os_str = OsString::from(mode_str);
        let mode_arg = mode_parser.parse_ref(cmd, None, &mode_os_str)?;

        let sort_order_parser = value_parser!(SortOrderArg);
        let sort_order_os_str = OsString::from(sort_order_str);
        let sort_order_arg = sort_order_parser.parse_ref(cmd, None, &sort_order_os_str)?;

        Ok(PlacementSortingItem {
            mode: mode_arg.to_placement_sorting_mode(),
            sort_order: sort_order_arg.to_sort_order(),
        })
    }
}

#[derive(Clone, Default)]
pub struct FileReferenceParser {}

impl TypedValueParser for FileReferenceParser {
    type Value = FileReference;

    fn parse_ref(&self, _cmd: &Command, _arg: Option<&Arg>, value: &OsStr) -> Result<Self::Value, Error> {
        let value = value
            .to_str()
            .ok_or_else(|| Error::raw(ErrorKind::InvalidValue, "Invalid argument encoding"))?;

        FileReference::try_from(value).map_err(|error| Error::raw(ErrorKind::InvalidValue, error.to_string()))
    }
}

pub fn dimensions_decimal_parser(s: &str) -> Result<Dimensions<Decimal>, String> {
    let mut values = HashMap::new();
    let mut errors = Vec::new();
    let required_keys = ["left", "right", "top", "bottom"];

    for chunk in s.split(',') {
        let chunk_chunks: Vec<_> = chunk.split('=').collect();
        if chunk_chunks.len() != 2 {
            errors.push(format!(
                "Expected exactly 1 equal sign in '{}', found {}",
                chunk,
                chunk_chunks.len() - 1
            ));
            continue;
        }

        let key = chunk_chunks[0].trim();
        let value_str = chunk_chunks[1].trim();

        if !required_keys.contains(&key) {
            errors.push(format!("Invalid key: '{}'", key));
            continue;
        }

        match value_str.parse::<Decimal>() {
            Ok(value) => {
                values.insert(key, value);
            }
            Err(e) => {
                errors.push(format!("Failed to parse decimal value for key '{}': {}", key, e));
            }
        }
    }

    // Make sure all fields are present
    let mut missing = vec![];
    for key in &required_keys {
        if !values.contains_key(*key) {
            missing.push(*key);
        }
    }

    if !missing.is_empty() {
        errors.push(format!(
            "Missing/invalid keys: {}, expected keys: {}",
            missing.join(", "),
            required_keys.join(", ")
        ));
    }

    if !errors.is_empty() {
        return Err(errors.join("; "));
    }

    Ok(Dimensions {
        left: values["left"],
        right: values["right"],
        top: values["top"],
        bottom: values["bottom"],
    })
}
#[cfg(test)]
mod dimensions_parser_tests {
    use rust_decimal_macros::dec;

    use super::*;

    #[test]
    fn test_dimensions_parser() {
        // when
        let result = dimensions_decimal_parser("left=1,right=2.2,top=3.33,bottom=44.4");

        // then
        assert_eq!(
            result,
            Ok(Dimensions {
                left: dec!(1.0),
                right: dec!(2.2),
                top: dec!(3.33),
                bottom: dec!(44.4),
            })
        )
    }

    #[test]
    fn test_dimensions_parser_errors() {
        // when
        let result = dimensions_decimal_parser("foo=bar,bar=foo,meh=blah=blah,right=XXX,left=1");

        // then
        assert_eq!(result, Err("Invalid key: 'foo'; Invalid key: 'bar'; Expected exactly 1 equal sign in 'meh=blah=blah', found 2; Failed to parse decimal value for key 'right': Invalid decimal: unknown character; Missing/invalid keys: right, top, bottom, expected keys: left, right, top, bottom".to_string()))
    }
}

pub fn vector2_decimal_parser(s: &str) -> Result<Vector2<Decimal>, String> {
    let mut values = HashMap::new();
    let mut errors = Vec::new();
    let required_keys = ["x", "y"];

    for chunk in s.split(',') {
        let chunk_chunks: Vec<_> = chunk.split('=').collect();
        if chunk_chunks.len() != 2 {
            errors.push(format!(
                "Expected exactly 1 equal sign in '{}', found {}",
                chunk,
                chunk_chunks.len() - 1
            ));
            continue;
        }

        let key = chunk_chunks[0].trim();
        let value_str = chunk_chunks[1].trim();

        if !required_keys.contains(&key) {
            errors.push(format!("Invalid key: '{}'", key));
            continue;
        }

        match value_str.parse::<Decimal>() {
            Ok(value) => {
                values.insert(key, value);
            }
            Err(e) => {
                errors.push(format!("Failed to parse decimal value for key '{}': {}", key, e));
            }
        }
    }

    // Make sure all fields are present
    let mut missing = vec![];
    for key in &required_keys {
        if !values.contains_key(*key) {
            missing.push(*key);
        }
    }

    if !missing.is_empty() {
        errors.push(format!(
            "Missing/invalid keys: {}, expected keys: {}",
            missing.join(", "),
            required_keys.join(", ")
        ));
    }

    if !errors.is_empty() {
        return Err(errors.join("; "));
    }

    Ok(Vector2::new(values["x"], values["y"]))
}

#[cfg(test)]
mod vector2_parser_tests {
    use rust_decimal_macros::dec;

    use super::*;

    #[test]
    fn test_vector2_parser() {
        // when
        let result = vector2_decimal_parser("x=1,y=22.222");

        // then
        assert_eq!(result, Ok(Vector2::new(dec!(1.0), dec!(22.222))))
    }

    #[test]
    fn test_vector2_parser_errors() {
        // when
        let result = vector2_decimal_parser("foo=bar,bar=foo,meh=blah=blah,x=XXX,y=1");

        // then
        assert_eq!(result, Err("Invalid key: 'foo'; Invalid key: 'bar'; Expected exactly 1 equal sign in 'meh=blah=blah', found 2; Failed to parse decimal value for key 'x': Invalid decimal: unknown character; Missing/invalid keys: x, expected keys: x, y".to_string()))
    }
}
