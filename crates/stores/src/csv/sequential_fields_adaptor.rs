use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serializer};
use serde_with::{DeserializeAs, SerializeAs};

/// Trait for defining how to serialize/deserialize sequential fields
pub trait SequentialFields {
    /// Type of object this represents
    type Item: Clone;

    /// Get the prefixes for the field names in each group
    fn field_prefixes() -> &'static [&'static str];

    /// Create an item from the field values in a group
    fn from_values(values: Vec<String>) -> Option<Self::Item>;

    /// Extract values from an item for serialization
    fn to_values(item: &Self::Item) -> Vec<String>;
}

/// A type for serializing vectors of items as sequentially numbered fields
pub struct SequentialFieldsAdapter<S: SequentialFields>(PhantomData<S>);

impl<'de, S> DeserializeAs<'de, Vec<S::Item>> for SequentialFieldsAdapter<S>
where
    S: SequentialFields,
    S::Item: Deserialize<'de>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<S::Item>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SeqVisitor<S: SequentialFields> {
            marker: PhantomData<S>,
        }

        impl<'de, S: SequentialFields> Visitor<'de> for SeqVisitor<S> {
            type Value = Vec<S::Item>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map with sequentially numbered fields")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut result = Vec::new();
                let mut values = HashMap::new();

                // Collect all fields
                while let Some((key, value)) = map.next_entry::<String, String>()? {
                    values.insert(key, value);
                }

                // Get field prefixes
                let prefixes = S::field_prefixes();

                // Process groups in sequence
                for i in 0.. {
                    let num = i + 1; // 1-based indexing

                    // Collect values for all fields in this group
                    let mut group_values = Vec::with_capacity(prefixes.len());
                    let mut all_fields_present = true;

                    for &prefix in prefixes {
                        let field_key = format!("{}{}", prefix, num);

                        if let Some(field_value) = values.get(&field_key) {
                            group_values.push(field_value.clone());
                        } else {
                            all_fields_present = false;
                            break;
                        }
                    }

                    if all_fields_present {
                        // Try to create an item from this group of values
                        if let Some(item) = S::from_values(group_values) {
                            result.push(item);
                        }
                    } else {
                        // We've reached the end of our numbered sequence
                        break;
                    }
                }

                Ok(result)
            }
        }

        deserializer.deserialize_map(SeqVisitor {
            marker: PhantomData::<S>,
        })
    }
}

impl<S> SerializeAs<Vec<S::Item>> for SequentialFieldsAdapter<S>
where
    S: SequentialFields,
{
    fn serialize_as<SE>(source: &Vec<S::Item>, serializer: SE) -> Result<SE::Ok, SE::Error>
    where
        SE: Serializer,
    {
        let prefixes = S::field_prefixes();
        let field_count = source.len() * prefixes.len(); // Fields per item * number of items
        let mut map = serializer.serialize_map(Some(field_count))?;

        for (i, item) in source.iter().enumerate() {
            let num = i + 1; // 1-based indexing
            let values = S::to_values(item);

            // Make sure we have the right number of values
            assert_eq!(
                values.len(),
                prefixes.len(),
                "Number of values must match number of prefixes"
            );

            // Serialize each field in the group
            for (prefix_idx, &prefix) in prefixes.iter().enumerate() {
                let field_key = format!("{}{}", prefix, num);
                map.serialize_entry(&field_key, &values[prefix_idx])?;
            }
        }

        map.end()
    }
}
