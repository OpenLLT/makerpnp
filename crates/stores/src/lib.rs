pub mod eda_placements;
pub mod part_mappings;
/// Stores are for loading/storing different kinds of data.
///
/// Currently, all stores are just simple files, mostly CSV.
///
/// Example store backends:
/// * Files (e.g. CSV).
/// * Remote (e.g. REST).
/// * Databases.
/// * Etc.
pub mod parts;
pub mod placements;

pub mod assembly_rules;
pub mod csv;
pub mod load_out;
pub mod substitutions;

pub mod test;
