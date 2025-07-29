use std::ops::{Deref, DerefMut};

use rust_decimal::Decimal;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};

use crate::eda_units::angle::angle_decimal::DecimalAngleExt;
// TODO maybe add custom serializer/deserializers
// TODO add various conversions, etc

mod angle_decimal;
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Radians {
    value: Decimal,
}

impl Radians {
    pub fn new_decimal(value: Decimal) -> Radians {
        Self {
            value,
        }
    }

    pub fn new_f64(value: f64) -> Radians {
        Self {
            value: Decimal::from_f64(value).unwrap_or_default(),
        }
    }
}

impl Deref for Radians {
    type Target = Decimal;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl DerefMut for Radians {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AngleUnit {
    radians: Radians,
    unit: AngleUnitSystem,
}

impl AngleUnit {
    pub fn new_degrees_f64(value: f64) -> AngleUnit {
        Self {
            radians: Radians::new_f64(value.to_radians()),
            unit: AngleUnitSystem::Degrees,
        }
    }

    pub fn new_degrees_decimal(value: Decimal) -> AngleUnit {
        Self {
            radians: Radians::new_decimal(value.to_radians()),
            unit: AngleUnitSystem::Degrees,
        }
    }

    pub fn to_radians_f64(&self) -> f64 {
        self.radians
            .to_f64()
            .unwrap_or_default()
    }

    pub fn to_radians_decimal(&self) -> Decimal {
        self.radians.value
    }

    pub fn to_degrees_f64(&self) -> f64 {
        self.radians
            .to_f64()
            .unwrap_or_default()
            .to_degrees()
    }

    pub fn to_degrees_decimal(&self) -> Decimal {
        self.radians.to_degrees()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AngleUnitSystem {
    Degrees,
    Radians,
}
