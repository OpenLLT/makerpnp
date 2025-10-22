use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use planning::design::DesignVariant;
use pnp::part::Part;
use pnp::pcb::PcbSide;
use pnp::placement::Placement;
use rust_decimal::Decimal;
use tracing::{info, trace};
use util::source::Source;

/// See `EdaPlacement` for details of co-ordinate system
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlacementRecord {
    pub ref_des: String,
    pub manufacturer: String,
    pub mpn: String,
    pub place: bool,
    pub pcb_side: PlacementRecordPcbSide,
    pub x: Decimal,
    pub y: Decimal,
    pub rotation: Decimal,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub enum PlacementRecordPcbSide {
    Top,
    Bottom,
}

impl From<&PlacementRecordPcbSide> for PcbSide {
    fn from(value: &PlacementRecordPcbSide) -> Self {
        match value {
            PlacementRecordPcbSide::Top => PcbSide::Top,
            PlacementRecordPcbSide::Bottom => PcbSide::Bottom,
        }
    }
}

impl From<&PcbSide> for PlacementRecordPcbSide {
    fn from(value: &PcbSide) -> Self {
        match value {
            PcbSide::Top => PlacementRecordPcbSide::Top,
            PcbSide::Bottom => PlacementRecordPcbSide::Bottom,
        }
    }
}

impl PlacementRecord {
    pub fn as_placement(&self) -> Placement {
        Placement {
            ref_des: self.ref_des.clone().into(),
            part: Part {
                manufacturer: self.manufacturer.clone(),
                mpn: self.mpn.clone(),
            },
            place: self.place,
            pcb_side: PcbSide::from(&self.pcb_side),
            x: self.x,
            y: self.y,
            rotation: self.rotation,
        }
    }
}

pub type PlacementsSource = Source;

pub fn load_placements(source: &PlacementsSource) -> Result<Vec<Placement>, anyhow::Error> {
    info!("Loading placements. source: {}", source);

    let path = source
        .path()
        .map_err(|error| anyhow!("Unsupported source type. cause: {:?}", error))?;

    let mut csv_reader = csv::ReaderBuilder::new()
        .from_path(path.clone())
        .with_context(|| format!("Error placements. file: {}", path.display()))?;

    let records = csv_reader
        .deserialize()
        .inspect(|record| {
            trace!("{:?}", record);
        })
        .filter_map(|record: Result<PlacementRecord, csv::Error>| {
            // TODO report errors
            match record {
                Ok(record) => Some(record.as_placement()),
                _ => None,
            }
        })
        .collect();

    Ok(records)
}

pub fn load_all_placements(
    unique_design_variants: HashSet<DesignVariant>,
    directory: &Path,
) -> anyhow::Result<BTreeMap<DesignVariant, Vec<Placement>>> {
    let mut all_placements: BTreeMap<DesignVariant, Vec<Placement>> = Default::default();

    for design_variant in unique_design_variants {
        let DesignVariant {
            design_name: design,
            variant_name: variant,
        } = &design_variant;

        let mut placements_path = PathBuf::from(directory);
        placements_path.push(format!("{}_{}_placements.csv", design, variant));
        let source = PlacementsSource::File(placements_path);

        let placements = load_placements(&source)?;
        let _ = all_placements.insert(design_variant, placements);
    }
    Ok(all_placements)
}
