use std::collections::BTreeSet;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Error};
use csv::QuoteStyle;
use planning::process::{Process, ProcessReference, ProcessRuleReference};
use planning::reference::Reference;
use pnp::load_out::LoadOutItem;
use pnp::part::Part;
use regex::Regex;
use thiserror::Error;
use tracing::trace;
use tracing::{info, Level};

use crate::csv::LoadOutItemRecord;

#[tracing::instrument(level = Level::DEBUG)]
pub fn load_items(load_out_source: &LoadOutSource) -> Result<Vec<LoadOutItem>, Error> {
    info!("Loading load-out. source: '{}'", load_out_source);

    let load_out_path_buf = PathBuf::from(load_out_source.to_string());
    let load_out_path = load_out_path_buf.as_path();
    let mut csv_reader = csv::ReaderBuilder::new()
        .from_path(load_out_path)
        .with_context(|| format!("Error reading load-out. file: {}", load_out_path.to_str().unwrap()))?;

    let mut items: Vec<LoadOutItem> = vec![];

    for result in csv_reader.deserialize() {
        let record: LoadOutItemRecord = result.with_context(|| "Deserializing load-out record".to_string())?;

        trace!("{:?}", record);

        let load_out_item = record
            .build_load_out_item()
            .with_context(|| format!("Building load-out from record. record: {:?}", record))?;

        items.push(load_out_item);
    }
    Ok(items)
}

pub fn store_items(load_out_source: &LoadOutSource, items: &[LoadOutItem]) -> Result<(), Error> {
    info!("Storing load-out. source: '{}'", load_out_source);

    let output_path = PathBuf::from(load_out_source.to_string());

    let mut writer = csv::WriterBuilder::new()
        .quote_style(QuoteStyle::Always)
        .from_path(output_path)?;

    for item in items {
        writer.serialize(LoadOutItemRecord {
            reference: item.reference.to_string(),
            manufacturer: item.manufacturer.to_string(),
            mpn: item.mpn.to_string(),
        })?;
    }

    writer.flush()?;

    Ok(())
}

pub fn ensure_load_out(load_out_source: &LoadOutSource) -> anyhow::Result<()> {
    let load_out_path_buf = PathBuf::from(load_out_source.to_string());
    let load_out_path = load_out_path_buf.as_path();
    if !load_out_path.exists() {
        File::create(&load_out_path)?;
        info!("Created load-out. source: '{}'", load_out_source);
    }

    Ok(())
}

// FUTURE maybe this should be a url?
#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash
)]
pub struct LoadOutSource(String);

impl FromStr for LoadOutSource {
    type Err = LoadOutSourceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(LoadOutSource(s.to_string()))
    }
}

impl LoadOutSource {
    pub fn try_from_relative_path(path: PathBuf) -> Result<LoadOutSource, LoadOutSourceError> {
        if !path.is_relative() {
            panic!()
        }
        Self::try_from_path_inner(path)
    }

    pub fn try_from_path_inner(path: PathBuf) -> Result<LoadOutSource, LoadOutSourceError> {
        if !path.exists() {
            return Err(LoadOutSourceError::PathDoesNotExist(path));
        }
        if !path.is_file() {
            return Err(LoadOutSourceError::PathIsNotAFile(path));
        }
        Ok(LoadOutSource(path.to_str().unwrap().to_string()))
    }

    pub fn try_from_absolute_path(path: PathBuf) -> Result<LoadOutSource, LoadOutSourceError> {
        if !path.is_absolute() {
            panic!()
        }
        Self::try_from_path_inner(path)
    }

    pub fn try_from_path(project_path: PathBuf, path: PathBuf) -> Result<LoadOutSource, LoadOutSourceError> {
        match path.is_absolute() {
            true => Self::from_absolute_path(path.clone()),
            false => {
                let full_path = project_path.join(path);
                Self::try_from_path_inner(full_path)
            }
        }
    }

    pub fn from_absolute_path(path: PathBuf) -> Result<LoadOutSource, LoadOutSourceError> {
        assert!(path.is_absolute());
        Ok(LoadOutSource(path.to_str().unwrap().to_string()))
    }
}

impl Display for LoadOutSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

#[derive(Debug, Error)]
pub enum LoadOutSourceError {
    #[error("Path does not exist. path: {0}")]
    PathDoesNotExist(PathBuf),
    #[error("Path is not a file. path: {0}")]
    PathIsNotAFile(PathBuf),
}

#[derive(Error, Debug)]
pub enum LoadOutOperationError {
    #[error("Unable to load items. source: {load_out_source}, error: {reason}")]
    UnableToLoadItems {
        load_out_source: LoadOutSource,
        reason: anyhow::Error,
    },

    #[error("Unable to store items. source: {load_out_source}, error: {reason}")]
    UnableToStoreItems {
        load_out_source: LoadOutSource,
        reason: anyhow::Error,
    },

    #[error("Load-out operation error. source: {load_out_source}, error: {reason}")]
    OperationError {
        load_out_source: LoadOutSource,
        reason: anyhow::Error,
    },
}

pub fn perform_load_out_operation<F, R, E>(source: &LoadOutSource, mut f: F) -> Result<R, LoadOutOperationError>
where
    F: FnMut(&mut Vec<LoadOutItem>) -> Result<R, E>,
    E: std::error::Error + Send + Sync + 'static,
{
    let mut load_out_items = load_items(source).map_err(|err| LoadOutOperationError::UnableToLoadItems {
        load_out_source: source.clone(),
        reason: err,
    })?;

    let result = f(&mut load_out_items).map_err(|err| LoadOutOperationError::OperationError {
        load_out_source: source.clone(),
        reason: err.into(),
    })?;

    store_items(source, &load_out_items).map_err(|err| LoadOutOperationError::UnableToStoreItems {
        load_out_source: source.clone(),
        reason: err,
    })?;

    Ok(result)
}

pub fn add_parts_to_load_out(
    load_out_source: &LoadOutSource,
    parts: BTreeSet<Part>,
) -> Result<(), LoadOutOperationError> {
    perform_load_out_operation(load_out_source, |load_out_items| {
        for part in parts.iter() {
            trace!("Checking for part in load_out. part: {:?}", part);

            let matched = pnp::load_out::find_load_out_item_by_part(load_out_items, part);

            if matched.is_some() {
                continue;
            }

            let load_out_item = LoadOutItem {
                reference: "".to_string(),
                manufacturer: part.manufacturer.clone(),
                mpn: part.mpn.clone(),
            };

            info!("Adding part to load_out. part: {:?}", part);
            load_out_items.push(load_out_item)
        }

        Ok::<(), std::io::Error>(())
    })
}

#[derive(Error, Debug)]
pub enum FeederAssignmentError {
    #[error("No matching part; patterns must match exactly one part. manufacturer: {manufacturer}, mpn: {mpn}")]
    NoMatchingPart { manufacturer: Regex, mpn: Regex },

    #[error("Multiple matching parts; patterns must match exactly one part for the process. process: {process}, manufacturer: {manufacturer}, mpn: {mpn}")]
    MultipleMatchingParts {
        process: ProcessReference,
        manufacturer: Regex,
        mpn: Regex,
    },
}

pub fn assign_feeder_to_load_out_item(
    load_out_source: &LoadOutSource,
    process: &Process,
    feeder_reference: &Reference,
    manufacturer: Regex,
    mpn: Regex,
) -> anyhow::Result<Vec<Part>> {
    let mut parts: Vec<Part> = vec![];

    perform_load_out_operation(load_out_source, |load_out_items| {
        let mut items: Vec<_> = load_out_items
            .iter_mut()
            .filter(|item| manufacturer.is_match(&item.manufacturer) && mpn.is_match(&item.mpn))
            .collect();

        if items.is_empty() {
            return Err(FeederAssignmentError::NoMatchingPart {
                manufacturer: manufacturer.clone(),
                mpn: mpn.clone(),
            });
        }

        if process.has_rule(&ProcessRuleReference::from_raw_str("core::unique_feeder_references")) && items.len() > 1 {
            return Err(FeederAssignmentError::MultipleMatchingParts {
                process: process.reference.clone(),
                manufacturer: manufacturer.clone(),
                mpn: mpn.clone(),
            });
        }

        for item in items.iter_mut() {
            let part = Part {
                manufacturer: item.manufacturer.clone(),
                mpn: item.mpn.clone(),
            };

            item.reference = feeder_reference.to_string();

            parts.push(part);
        }

        Ok(())
    })?;

    for part in parts.iter() {
        info!(
            "Assigned feeder to load-out item. feeder: {}, part: {:?}",
            feeder_reference, part
        );
    }

    Ok(parts)
}
