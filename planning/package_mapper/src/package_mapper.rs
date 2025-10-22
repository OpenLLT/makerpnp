use std::collections::BTreeSet;

use pnp::package::Package;
use pnp::part::Part;

use crate::package_mapping::PackageMapping;

pub struct PackageMapper {}

impl PackageMapper {
    /// Maps parts to packages using mappings.
    ///
    /// The first matching mapping wins.
    pub fn process<'parts, 'part, 'mappings, 'packages>(
        parts: &'parts BTreeSet<&'part Part>,
        package_mappings: &'mappings Vec<PackageMapping<'packages>>,
    ) -> Result<Vec<PartPackageMappingResult<'part, 'mappings>>, PackageMapperError> {
        let mapping_results = parts
            .iter()
            .map(|part| {
                let mut mapping_results = vec![];

                for mapping in package_mappings.iter() {
                    for criteria in mapping.criteria.iter() {
                        if criteria.matches(part) {
                            mapping_results.push(PackageMappingResult {
                                mapping,
                            });
                        }
                    }
                }

                // use the first matching mapping
                let package = mapping_results
                    .first()
                    .map(|result| result.mapping.package);

                PartPackageMappingResult {
                    part,
                    mapping_results,
                    package,
                }
            })
            .collect::<Vec<_>>();

        Ok(mapping_results)
    }
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug)]
pub enum PackageMapperError {
    None,
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug)]
pub struct PartPackageMappingResult<'part, 'packages> {
    pub part: &'part Part,
    pub mapping_results: Vec<PackageMappingResult<'packages>>,
    pub package: Option<&'packages Package>,
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug)]
pub struct PackageMappingResult<'mappings> {
    pub mapping: &'mappings PackageMapping<'mappings>,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use criteria::{ExactMatchCriterion, GenericCriteria};
    use pnp::package::Package;
    use pnp::part::Part;

    use crate::package_mapper::PackageMapper;
    use crate::package_mapping::PackageMapping;
    use crate::{PackageMappingResult, PartPackageMappingResult};

    #[test]
    fn map_parts_to_packages() {
        let part1 = Part::new("MFR1".into(), "MPN1".into());
        let part2 = Part::new("MFR2".into(), "MPN2".into());
        let part3 = Part::new("MFR3".into(), "MPN3".into());

        let parts = BTreeSet::from_iter(vec![&part1, &part2, &part3]);

        let packages = vec![
            Package::new("PACKAGE1".into()),
            Package::new("PACKAGE2".into()),
            Package::new("PACKAGE3".into()),
        ];

        let criteria1 = GenericCriteria {
            criteria: vec![
                Box::new(ExactMatchCriterion::new("manufacturer".to_string(), "MFR1".to_string())),
                Box::new(ExactMatchCriterion::new("mpn".to_string(), "MPN1".to_string())),
            ],
        };
        let package_mapping1 = PackageMapping::new(&packages[1 - 1], vec![Box::new(criteria1)]);

        let criteria2 = GenericCriteria {
            criteria: vec![
                Box::new(ExactMatchCriterion::new("manufacturer".to_string(), "MFR2".to_string())),
                Box::new(ExactMatchCriterion::new("mpn".to_string(), "MPN2".to_string())),
            ],
        };
        let package_mapping2 = PackageMapping::new(&packages[2 - 1], vec![Box::new(criteria2)]);

        let criteria3 = GenericCriteria {
            criteria: vec![
                Box::new(ExactMatchCriterion::new("manufacturer".to_string(), "MFR3".to_string())),
                Box::new(ExactMatchCriterion::new("mpn".to_string(), "MPN3".to_string())),
            ],
        };
        let package_mapping3 = PackageMapping::new(&packages[3 - 1], vec![Box::new(criteria3)]);

        let package_mappings = vec![package_mapping1, package_mapping2, package_mapping3];

        // and

        let expected_result = Ok(vec![
            PartPackageMappingResult {
                part: &part1,
                mapping_results: vec![PackageMappingResult {
                    mapping: &package_mappings[1 - 1],
                }],
                package: Some(&packages[1 - 1]),
            },
            PartPackageMappingResult {
                part: &part2,
                mapping_results: vec![PackageMappingResult {
                    mapping: &package_mappings[2 - 1],
                }],
                package: Some(&packages[2 - 1]),
            },
            PartPackageMappingResult {
                part: &part3,
                mapping_results: vec![PackageMappingResult {
                    mapping: &package_mappings[3 - 1],
                }],
                package: Some(&packages[3 - 1]),
            },
        ]);

        // when
        let result = PackageMapper::process(&parts, &package_mappings);

        // then
        assert_eq!(result, expected_result);
    }
}
