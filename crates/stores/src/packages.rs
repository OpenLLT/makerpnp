use std::path::PathBuf;

use anyhow::{Context, Error};
use pnp::package::Package;
use tracing::{trace, Level};

use crate::csv::PackageRecord;

#[tracing::instrument(level = Level::DEBUG)]
pub fn load_packages(packages_source: &String) -> Result<Vec<Package>, Error> {
    let packages_path_buf = PathBuf::from(packages_source);
    let packages_path = packages_path_buf.as_path();
    let mut csv_reader = csv::ReaderBuilder::new()
        .from_path(packages_path)
        .with_context(|| format!("Error reading packages. file: {}", packages_path.to_str().unwrap()))?;

    let mut packages: Vec<Package> = vec![];

    for result in csv_reader.deserialize() {
        let record: PackageRecord = result.with_context(|| "Deserializing package record".to_string())?;

        trace!("{:?}", record);

        let package = record
            .build_package()
            .with_context(|| format!("Building package from record. record: {:?}", record))?;

        packages.push(package);
    }
    Ok(packages)
}

#[cfg(test)]
mod tests {
    use csv::QuoteStyle;
    use tempfile::tempdir;
    use util::test::{build_temp_csv_file, dump_file};

    use super::*;

    #[test]
    fn test_load_packages() -> Result<(), anyhow::Error> {
        // given
        let temp_dir = tempdir()?;

        // and packages
        let (test_packages_path, test_packages_file_name) = build_temp_csv_file(&temp_dir, "packages");

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_path(test_packages_path.clone())?;

        writer.serialize(PackageRecord {
            name: "NAME1".to_string(),
            ..PackageRecord::default()
        })?;

        writer.flush()?;

        dump_file("packages", test_packages_path.clone())?;

        // and
        let expected_packages = vec![Package::new("NAME1".into())];

        // when
        let packages = load_packages(
            &test_packages_path
                .to_string_lossy()
                .to_string(),
        )?;

        assert_eq!(packages, expected_packages);

        Ok(())
    }
}
