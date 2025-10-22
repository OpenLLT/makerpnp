use pnp::package::Package;

use crate::criteria::PartMappingCriteria;

#[cfg_attr(any(test, feature = "testing"), derive(PartialEq))]
#[derive(Debug)]
pub struct PackageMapping<'package> {
    pub package: &'package Package,
    pub criteria: Vec<Box<dyn PartMappingCriteria>>,
}

impl<'package> PackageMapping<'package> {
    pub fn new(part: &'package Package, criteria: Vec<Box<dyn PartMappingCriteria>>) -> Self {
        Self {
            package: part,
            criteria,
        }
    }
}
