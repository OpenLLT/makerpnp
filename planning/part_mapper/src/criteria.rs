use std::fmt::Debug;

use criteria::GenericCriteria;
use eda::placement::EdaPlacement;
use util::dynamic::as_any::AsAny;
use util::dynamic::dynamic_eq::DynamicEq;

pub trait PlacementMappingCriteria: Debug + AsAny + DynamicEq {
    fn matches(&self, placement: &EdaPlacement) -> bool;
}

impl PartialEq for dyn PlacementMappingCriteria {
    fn eq(&self, other: &Self) -> bool {
        self.dynamic_eq(other.as_any())
    }
}

impl PlacementMappingCriteria for GenericCriteria {
    fn matches(&self, eda_placement: &EdaPlacement) -> bool {
        self.criteria.iter().all(|criterion| {
            eda_placement
                .fields
                .iter()
                .any(|field| criterion.matches(field.name.as_str(), field.value.as_str()))
        })
    }
}

#[cfg(test)]
mod generic_criteria_tests {
    use criteria::{ExactMatchCriterion, GenericCriteria, RegexMatchCriterion};
    use eda::placement::{EdaPlacement, EdaPlacementField};
    use regex::Regex;

    use crate::criteria::PlacementMappingCriteria;

    #[test]
    fn matches() {
        // given
        let criteria = GenericCriteria {
            criteria: vec![
                Box::new(ExactMatchCriterion {
                    field_name: "name".to_string(),
                    field_pattern: "NAME1".to_string(),
                }),
                Box::new(RegexMatchCriterion {
                    field_name: "value".to_string(),
                    field_pattern: Regex::new(".*").unwrap(),
                }),
            ],
        };
        let placement = EdaPlacement {
            ref_des: "R1".to_string(),
            fields: vec![
                EdaPlacementField {
                    name: "name".to_string(),
                    value: "NAME1".to_string(),
                },
                EdaPlacementField {
                    name: "value".to_string(),
                    value: "VALUE1".to_string(),
                },
            ],
            ..EdaPlacement::default()
        };

        // when
        assert!(criteria.matches(&placement));
    }

    #[test]
    fn does_not_match_due_to_name() {
        // given
        let criteria = GenericCriteria {
            criteria: vec![
                Box::new(ExactMatchCriterion {
                    field_name: "name".to_string(),
                    field_pattern: "NAME1".to_string(),
                }),
                Box::new(RegexMatchCriterion {
                    field_name: "value".to_string(),
                    field_pattern: Regex::new(".*").unwrap(),
                }),
            ],
        };
        let placement = EdaPlacement {
            ref_des: "R1".to_string(),
            fields: vec![
                EdaPlacementField {
                    name: "name".to_string(),
                    value: "NAME2".to_string(),
                },
                EdaPlacementField {
                    name: "value".to_string(),
                    value: "VALUE1".to_string(),
                },
            ],
            ..EdaPlacement::default()
        };

        // when
        assert!(!criteria.matches(&placement));
    }

    #[test]
    fn does_not_match_due_to_value() {
        // given
        let criteria = GenericCriteria {
            criteria: vec![
                Box::new(ExactMatchCriterion {
                    field_name: "name".to_string(),
                    field_pattern: "NAME1".to_string(),
                }),
                Box::new(RegexMatchCriterion {
                    field_name: "value".to_string(),
                    field_pattern: Regex::new("(VALUE1)").unwrap(),
                }),
            ],
        };
        let placement = EdaPlacement {
            ref_des: "R1".to_string(),
            fields: vec![
                EdaPlacementField {
                    name: "name".to_string(),
                    value: "NAME2".to_string(),
                },
                EdaPlacementField {
                    name: "value".to_string(),
                    value: "VALUE2".to_string(),
                },
            ],
            ..EdaPlacement::default()
        };

        // when
        assert!(!criteria.matches(&placement));
    }
}
