use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

/// Trait for defining how to serialize/deserialize sequential fields
pub trait SequentialFields<T> {
    /// Get the field names for a specific index
    fn field_names(index: usize) -> Vec<String>;

    /// Extract values from a collection for serialization into a flat structure
    fn to_flat_record(items: &[T]) -> FlatRecord;

    /// Build items from a flat record during deserialization
    fn from_flat_record(record: &FlatRecord) -> Vec<T>;
}

/// A dynamic record that can hold any number of fields for CSV serialization
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FlatRecord {
    #[serde(flatten)]
    fields: std::collections::HashMap<String, String>,
}

impl FlatRecord {
    pub fn new() -> Self {
        Self {
            fields: std::collections::HashMap::new(),
        }
    }

    pub fn set_field(&mut self, name: String, value: String) {
        self.fields.insert(name, value);
    }

    pub fn get_field(&self, name: &str) -> Option<&String> {
        self.fields.get(name)
    }
}

/// Helper for serializing vector fields as sequentially numbered fields in CSV
pub struct SequentialFieldsHelper<T, S: SequentialFields<T>>(PhantomData<(T, S)>);

impl<T, S: SequentialFields<T>> SequentialFieldsHelper<T, S> {
    /// Convert a vector of items to a flat record for CSV serialization
    pub fn to_record(items: &[T]) -> FlatRecord {
        S::to_flat_record(items)
    }

    /// Convert a flat record back to a vector of items
    pub fn from_record(record: &FlatRecord) -> Vec<T> {
        S::from_flat_record(record)
    }
}
