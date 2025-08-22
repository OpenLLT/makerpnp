use anyhow::Error;
use package_mapper::package_mapping::PackageMapping;
use pnp::package::Package;
use tracing::Level;

#[tracing::instrument(level = Level::DEBUG)]
pub fn load_package_mappings<'packages>(
    packages: &'packages Vec<Package>,
    source: &String,
) -> Result<Vec<PackageMapping<'packages>>, Error> {
    todo!()
}
