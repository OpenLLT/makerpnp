use pnp::package::Package;

use crate::criteria::PackageMappingCriteria;

#[cfg_attr(any(test, feature = "testing"), derive(PartialEq))]
#[derive(Debug)]
pub struct PackageMapping<'package> {
    pub part: &'package Package,
    pub criteria: Vec<Box<dyn PackageMappingCriteria>>,
}

impl<'package> PackageMapping<'package> {
    pub fn new(part: &'package Package, criteria: Vec<Box<dyn PackageMappingCriteria>>) -> Self {
        Self {
            part,
            criteria,
        }
    }
}
