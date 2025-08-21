use std::fmt::Debug;

use criteria::GenericCriteria;
use pnp::part::Part;
use util::dynamic::as_any::AsAny;
use util::dynamic::dynamic_eq::DynamicEq;

pub trait PackageMappingCriteria: Debug + AsAny + DynamicEq {
    fn matches(&self, part: &Part) -> bool;
}

impl PartialEq for dyn PackageMappingCriteria {
    fn eq(&self, other: &Self) -> bool {
        self.dynamic_eq(other.as_any())
    }
}

impl PackageMappingCriteria for GenericCriteria {
    fn matches(&self, part: &Part) -> bool {
        let result: Option<bool> = self
            .criteria
            .iter()
            .fold(None, |mut matched, criterion| {
                let fields = ["manufacturer", "mpn"];
                let matched_field = fields.into_iter().find(|&field| {
                    let value = match field {
                        "manufacturer" => part.manufacturer.as_str(),
                        "mpn" => part.mpn.as_str(),
                        _ => panic!("Unknown field"),
                    };
                    criterion.matches(field, value)
                });

                match (&mut matched, matched_field) {
                    // matched, previous fields checked
                    (Some(accumulated_result), Some(_field)) => *accumulated_result &= true,
                    // matched, first field
                    (None, Some(_field)) => matched = Some(true),
                    // not matched, previous fields checked
                    (Some(accumulated_result), None) => *accumulated_result = false,
                    // not matched, first field
                    (None, None) => matched = Some(false),
                }

                matched
            });

        result.unwrap_or(false)
    }
}

#[cfg(test)]
mod generic_criteria_tests {
    use criteria::{ExactMatchCriterion, GenericCriteria, RegexMatchCriterion};
    use pnp::part::Part;
    use regex::Regex;

    use crate::criteria::PackageMappingCriteria;

    #[test]
    fn matches() {
        // given
        let criteria = GenericCriteria {
            criteria: vec![
                Box::new(ExactMatchCriterion {
                    field_name: "manufacturer".to_string(),
                    field_pattern: "MFR1".to_string(),
                }),
                Box::new(RegexMatchCriterion {
                    field_name: "mpn".to_string(),
                    field_pattern: Regex::new(".*").unwrap(),
                }),
            ],
        };
        let part = Part {
            manufacturer: "MFR1".to_string(),
            mpn: "MPN1".to_string(),
        };

        // when
        assert!(criteria.matches(&part));
    }

    #[test]
    fn does_not_match_due_to_manufacturer() {
        // given
        let criteria = GenericCriteria {
            criteria: vec![
                Box::new(ExactMatchCriterion {
                    field_name: "manufacturer".to_string(),
                    field_pattern: "MFR1".to_string(),
                }),
                Box::new(RegexMatchCriterion {
                    field_name: "mpn".to_string(),
                    field_pattern: Regex::new(".*").unwrap(),
                }),
            ],
        };
        let part = Part {
            manufacturer: "MFR2".to_string(),
            mpn: "MPN1".to_string(),
        };

        // when
        assert!(!criteria.matches(&part));
    }

    #[test]
    fn does_not_match_due_to_mpn() {
        // given
        let criteria = GenericCriteria {
            criteria: vec![
                Box::new(ExactMatchCriterion {
                    field_name: "manufacturer".to_string(),
                    field_pattern: "MFR1".to_string(),
                }),
                Box::new(RegexMatchCriterion {
                    field_name: "mpn".to_string(),
                    field_pattern: Regex::new("(MPN1)").unwrap(),
                }),
            ],
        };
        let part = Part {
            manufacturer: "MFR1".to_string(),
            mpn: "MPN2".to_string(),
        };

        // when
        assert!(!criteria.matches(&part));
    }
}
