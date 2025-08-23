use anyhow::{anyhow, Context, Error};
use pnp::part::Part;
use tracing::Level;
use tracing::{info, trace};
use util::source::Source;

use crate::csv::PartRecord;

pub type PartsSource = Source;

#[tracing::instrument(level = Level::DEBUG)]
pub fn load_parts(source: &PartsSource) -> Result<Vec<Part>, Error> {
    info!("Loading parts. source: {}", source);

    let path = source
        .path()
        .map_err(|error| anyhow!("Unsupported source type. cause: {:?}", error))?;

    let mut csv_reader = csv::ReaderBuilder::new()
        .from_path(path.clone())
        .with_context(|| format!("Error reading parts. file: {}", path.display()))?;

    let mut parts: Vec<Part> = vec![];

    for result in csv_reader.deserialize() {
        let record: PartRecord = result.with_context(|| "Deserializing part record".to_string())?;

        trace!("{:?}", record);

        let part = record
            .build_part()
            .with_context(|| format!("Building part from record. record: {:?}", record))?;

        parts.push(part);
    }
    Ok(parts)
}
