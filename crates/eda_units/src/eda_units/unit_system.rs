use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Represents different unit systems used in gerber files and UI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum UnitSystem {
    /// Inches (1 inch = 25.4 mm = 25,400,000 nm)
    Inches,
    /// Millimeters (1 mm = 1,000,000 nm)
    Millimeters,
    /// Mils (1/1000 of an inch, commonly used in PCB design)
    /// (1 mil = 0.001 inch = 25,400 nm)
    Mils,
    /// Chinese Si (丝) unit (1 Si = 0.01 mm = 10,000 nm)
    Si,
}

impl UnitSystem {
    /// Convert nanometers (as i32) to a Decimal value in this unit system
    pub fn from_nm_decimal(&self, nm_value: i32) -> Decimal {
        let nm_decimal = Decimal::from(nm_value);

        match self {
            UnitSystem::Inches => nm_decimal / Decimal::from(25_400_000),
            UnitSystem::Millimeters => nm_decimal / Decimal::from(1_000_000),
            UnitSystem::Mils => nm_decimal / Decimal::from(25_400),
            UnitSystem::Si => nm_decimal / Decimal::from(10_000),
        }
    }

    /// Convert a Decimal value from this unit system to nanometers (as i32)
    pub fn to_nm_decimal(&self, value: Decimal) -> i32 {
        let nm_decimal = match self {
            UnitSystem::Inches => value * Decimal::from(25_400_000),
            UnitSystem::Millimeters => value * Decimal::from(1_000_000),
            UnitSystem::Mils => value * Decimal::from(25_400),
            UnitSystem::Si => value * Decimal::from(10_000),
        };

        // Round to nearest nanometer and convert to i32
        if nm_decimal > Decimal::from(i32::MAX) {
            i32::MAX
        } else if nm_decimal < Decimal::from(i32::MIN) {
            i32::MIN
        } else {
            nm_decimal.round().to_i32().unwrap_or(0)
        }
    }

    /// Convert a value from this unit system to nanometers (internal representation)
    pub fn to_nm_f64(&self, value: f64) -> i32 {
        let result = match self {
            UnitSystem::Inches => value * 25_400_000.0,
            UnitSystem::Millimeters => value * 1_000_000.0,
            UnitSystem::Mils => value * 25_400.0,
            UnitSystem::Si => value * 10_000.0, // 1 Si (丝) = 0.01 mm = 10,000 nm
        };

        // Clamp to i32 range to prevent overflow
        if result > i32::MAX as f64 {
            i32::MAX
        } else if result < i32::MIN as f64 {
            i32::MIN
        } else {
            result.round() as i32
        }
    }

    /// Convert a value from nanometers to this unit system
    pub fn from_nm_f64(&self, value: i32) -> f64 {
        match self {
            UnitSystem::Inches => value as f64 / 25_400_000.0,
            UnitSystem::Millimeters => value as f64 / 1_000_000.0,
            UnitSystem::Mils => value as f64 / 25_400.0,
            UnitSystem::Si => value as f64 / 10_000.0, // 1 Si (丝) = 0.01 mm = 10,000 nm
        }
    }

    /// Get a display string for this unit system
    pub fn display_name(&self) -> &'static str {
        match self {
            UnitSystem::Inches => "in",
            UnitSystem::Millimeters => "mm",
            UnitSystem::Mils => "mil",
            UnitSystem::Si => "丝", // Chinese Si unit
        }
    }

    /// Get the default precision for this unit system
    pub fn default_precision(&self) -> usize {
        match self {
            UnitSystem::Inches => 6,      // e.g., 1.000000 in
            UnitSystem::Millimeters => 4, // e.g., 1.0000 mm
            UnitSystem::Mils => 3,        // e.g., 1.000 mil
            UnitSystem::Si => 2,          // e.g., 1.00 丝
        }
    }

    #[cfg(feature = "gerber")]
    /// Get the unit system from a gerber file unit
    pub fn from_gerber_unit(unit: Option<gerber_types::Unit>) -> Self {
        match unit {
            Some(gerber_types::Unit::Inches) => UnitSystem::Inches,
            Some(gerber_types::Unit::Millimeters) => UnitSystem::Millimeters,

            // Default to mm if not specified, the gerber spec (2024.05) says 'use metric, imperial will be deprecated'
            None => UnitSystem::Millimeters,
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use rust_decimal_macros::dec;

    use super::*;

    // Helper function to check if two f64 values are approximately equal
    fn approx_eq(a: f64, b: f64, epsilon: f64) -> bool {
        (a - b).abs() < epsilon
    }

    #[test]
    fn test_unit_system_default_precision() {
        assert_eq!(UnitSystem::Inches.default_precision(), 6);
        assert_eq!(UnitSystem::Millimeters.default_precision(), 4);
        assert_eq!(UnitSystem::Mils.default_precision(), 3);
        assert_eq!(UnitSystem::Si.default_precision(), 2);
    }

    #[rstest]
    #[case(1.0, UnitSystem::Inches, 25_400_000)]
    #[case(1.0, UnitSystem::Millimeters, 1_000_000)]
    #[case(1.0, UnitSystem::Mils, 25_400)]
    #[case(1.0, UnitSystem::Si, 10_000)]
    fn test_unit_system_to_nm_f64(#[case] value: f64, #[case] unit: UnitSystem, #[case] expected_nm: i32) {
        assert_eq!(unit.to_nm_f64(value), expected_nm);
    }

    #[rstest]
    #[case(25_400_000, UnitSystem::Inches, 1.0)]
    #[case(1_000_000, UnitSystem::Millimeters, 1.0)]
    #[case(25_400, UnitSystem::Mils, 1.0)]
    #[case(10_000, UnitSystem::Si, 1.0)]
    fn test_unit_system_from_nm(#[case] nm_value: i32, #[case] unit: UnitSystem, #[case] expected: f64) {
        let result = unit.from_nm_f64(nm_value);
        assert!(approx_eq(result, expected, f64::EPSILON));
    }

    #[rstest]
    #[case(dec!(1.0), UnitSystem::Inches, 25_400_000)]
    #[case(dec!(1.0), UnitSystem::Millimeters, 1_000_000)]
    #[case(dec!(1.0), UnitSystem::Mils, 25_400)]
    #[case(dec!(1.0), UnitSystem::Si, 10_000)]
    fn test_unit_system_to_nm_decimal(#[case] value: Decimal, #[case] unit: UnitSystem, #[case] expected_nm: i32) {
        assert_eq!(unit.to_nm_decimal(value), expected_nm);
    }

    #[rstest]
    #[case(25_400_000, UnitSystem::Inches, dec!(1.0))]
    #[case(1_000_000, UnitSystem::Millimeters, dec!(1.0))]
    #[case(25_400, UnitSystem::Mils, dec!(1.0))]
    #[case(10_000, UnitSystem::Si, dec!(1.0))]
    fn test_unit_system_from_nm_decimal(#[case] nm_value: i32, #[case] unit: UnitSystem, #[case] expected: Decimal) {
        let result = unit.from_nm_decimal(nm_value);
        assert_eq!(result, expected);
    }

    #[rstest]
    #[case(UnitSystem::Inches, "in")]
    #[case(UnitSystem::Millimeters, "mm")]
    #[case(UnitSystem::Mils, "mil")]
    #[case(UnitSystem::Si, "丝")]
    fn test_unit_system_display_name(#[case] unit: UnitSystem, #[case] expected: &str) {
        assert_eq!(unit.display_name(), expected);
    }

    #[cfg(feature = "gerber")]
    #[rstest]
    #[case(Some(gerber_types::Unit::Inches), UnitSystem::Inches)]
    #[case(Some(gerber_types::Unit::Millimeters), UnitSystem::Millimeters)]
    #[case(None, UnitSystem::Millimeters)]
    fn test_from_gerber_unit(#[case] gerber_unit: Option<gerber_types::Unit>, #[case] expected: UnitSystem) {
        assert_eq!(UnitSystem::from_gerber_unit(gerber_unit), expected);
    }
}
