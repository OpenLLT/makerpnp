use serde::{Deserialize, Deserializer, Serializer};
use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use std::fmt;
use std::marker::PhantomData;
use serde_with::{DeserializeAs, SerializeAs};
use pnp::package::ManufacturerPackageCode;

/// A custom serialization adapter for serializing vectors as sequentially numbered fields
pub struct NumericSequence<T, F = ManufacturerCodePairNamer>(PhantomData<(T, F)>);

/// A trait for naming fields in the sequence
pub trait FieldNamer {
    fn field_name_prefix() -> (&'static str, &'static str);
    fn make_field_names(index: usize) -> (String, String);
}

/// Implementation for manufacturer codes (Mfr1, MfrCode1, etc.)
pub struct ManufacturerCodePairNamer;

impl FieldNamer for ManufacturerCodePairNamer {
    fn field_name_prefix() -> (&'static str, &'static str) {
        ("Mfr", "MfrCode")
    }

    fn make_field_names(index: usize) -> (String, String) {
        let (mfr_prefix, code_prefix) = Self::field_name_prefix();
        let num = index + 1; // 1-based indexing
        (
            format!("{}{}", mfr_prefix, num),
            format!("{}{}", code_prefix, num)
        )
    }
}

impl<'de, T, F> DeserializeAs<'de, Vec<T>> for NumericSequence<T, F>
where
    T: Deserialize<'de> + ManufacturerCodePair,
    F: FieldNamer,
{
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SeqVisitor<T, F> {
            marker: PhantomData<(T, F)>,
        }

        impl<'de, T, F> Visitor<'de> for SeqVisitor<T, F>
        where
            T: Deserialize<'de> + ManufacturerCodePair,
            F: FieldNamer,
        {
            type Value = Vec<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map with sequentially numbered fields")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut result = Vec::new();
                let mut values = std::collections::HashMap::new();

                // Collect all fields
                while let Some((key, value)) = map.next_entry::<String, String>()? {
                    values.insert(key, value);
                }

                // Process pairs in sequence
                for i in 0.. {
                    let (mfr_key, code_key) = F::make_field_names(i);

                    match (values.get(&mfr_key), values.get(&code_key)) {
                        (Some(mfr), Some(code)) if !mfr.is_empty() && !code.is_empty() => {
                            result.push(T::from_pair(mfr.clone(), code.clone()));
                        }
                        _ => break, // Stop when we don't find a complete pair
                    }
                }

                Ok(result)
            }
        }

        deserializer.deserialize_map(SeqVisitor {
            marker: PhantomData,
        })
    }
}

/// Trait for creating manufacturer code pairs
pub trait ManufacturerCodePair {
    fn from_pair(manufacturer: String, code: String) -> Self;
    fn manufacturer(&self) -> &str;
    fn code(&self) -> &str;
}

impl<T, F> SerializeAs<Vec<T>> for NumericSequence<T, F>
where
    T: ManufacturerCodePair,
    F: FieldNamer,
{
    fn serialize_as<S>(source: &Vec<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let field_count = source.len() * 2; // Two fields per item
        let mut map = serializer.serialize_map(Some(field_count))?;

        for (i, item) in source.iter().enumerate() {
            let (mfr_key, code_key) = F::make_field_names(i);
            map.serialize_entry(&mfr_key, item.manufacturer())?;
            map.serialize_entry(&code_key, item.code())?;
        }

        map.end()
    }
}

// Implement the ManufacturerCodePair trait for ManufacturerPackageCode
impl ManufacturerCodePair for ManufacturerPackageCode {
    fn from_pair(manufacturer: String, code: String) -> Self {
        ManufacturerPackageCode { manufacturer, code }
    }

    fn manufacturer(&self) -> &str {
        &self.manufacturer
    }

    fn code(&self) -> &str {
        &self.code
    }
}