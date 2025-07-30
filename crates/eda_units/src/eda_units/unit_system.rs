use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Represents different unit systems used in gerber files and UI
/// Ordered largest first
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
    /// Returns the number of nanometers in one unit of this unit system
    fn nm_per_unit(&self) -> u32 {
        match self {
            UnitSystem::Inches => 25_400_000,
            UnitSystem::Millimeters => 1_000_000,
            UnitSystem::Mils => 25_400,
            UnitSystem::Si => 10_000,
        }
    }

    /// Convert nanometers (as i32) to a Decimal value in this unit system
    pub fn from_nm_decimal(&self, nm_value: i32) -> Decimal {
        let nm_decimal = Decimal::from(nm_value);
        nm_decimal / Decimal::from(self.nm_per_unit())
    }

    /// Convert a Decimal value from this unit system to nanometers (as i32)
    pub fn to_nm_decimal(&self, value: Decimal) -> i32 {
        let nm_decimal = value * Decimal::from(self.nm_per_unit());

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
        let result = value * self.nm_per_unit() as f64;

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
        value as f64 / self.nm_per_unit() as f64
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

    /// Calculate the scale factor to convert from this unit system to another unit system using f64
    pub fn scale_f64_for(&self, to: UnitSystem) -> f64 {
        self.nm_per_unit() as f64 / to.nm_per_unit() as f64
    }

    /// Calculate the scale factor to convert from this unit system to another unit system using Decimal
    pub fn scale_decimal_for(&self, to: UnitSystem) -> Decimal {
        Decimal::from(self.nm_per_unit()) / Decimal::from(to.nm_per_unit())
    }

    #[cfg(feature = "gerber")]
    /// Get the unit system from a gerber file unit
    pub fn from_gerber_unit(unit: &Option<gerber_types::Unit>) -> Self {
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

    #[test]
    fn ordering() {
        // given
        let mut unit_systems = vec![
            UnitSystem::Si,
            UnitSystem::Millimeters,
            UnitSystem::Inches,
            UnitSystem::Mils,
        ];

        // when
        unit_systems.sort();

        // then
        assert_eq!(unit_systems, vec![
            UnitSystem::Inches,
            UnitSystem::Millimeters,
            UnitSystem::Mils,
            UnitSystem::Si
        ]);
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
        assert_eq!(UnitSystem::from_gerber_unit(&gerber_unit), expected);
    }
}

#[cfg(test)]
mod conversion_tests {
    use rstest::rstest;
    use rust_decimal_macros::dec;

    use super::*;

    #[rstest]
    #[case(UnitSystem::Inches, UnitSystem::Millimeters, 25.4)]
    #[case(UnitSystem::Millimeters, UnitSystem::Inches, 1.0/25.4)]
    #[case(UnitSystem::Inches, UnitSystem::Mils, 1000.0)]
    #[case(UnitSystem::Mils, UnitSystem::Inches, 0.001)]
    #[case(UnitSystem::Millimeters, UnitSystem::Si, 100.0)]
    #[case(UnitSystem::Si, UnitSystem::Millimeters, 0.01)]
    #[case(UnitSystem::Mils, UnitSystem::Millimeters, 0.0254)]
    #[case(UnitSystem::Si, UnitSystem::Mils, 0.01 * 1000.0 / 25.4)]
    fn test_scale_f64_for(#[case] from: UnitSystem, #[case] to: UnitSystem, #[case] expected: f64) {
        let result = from.scale_f64_for(to);
        assert!(
            approx_eq(result, expected, 1e-10),
            "Expected {:.10} but got {:.10}",
            expected,
            result
        );
    }

    #[rstest]
    #[case(UnitSystem::Inches, UnitSystem::Millimeters, dec!(25.4))]
    #[case(UnitSystem::Millimeters, UnitSystem::Inches, dec!(1) / dec!(25.4))]
    #[case(UnitSystem::Inches, UnitSystem::Mils, dec!(1000))]
    #[case(UnitSystem::Mils, UnitSystem::Inches, dec!(0.001))]
    #[case(UnitSystem::Millimeters, UnitSystem::Si, dec!(100))]
    #[case(UnitSystem::Si, UnitSystem::Millimeters, dec!(0.01))]
    #[case(UnitSystem::Mils, UnitSystem::Millimeters, dec!(0.0254))]
    fn test_scale_decimal_for(#[case] from: UnitSystem, #[case] to: UnitSystem, #[case] expected: Decimal) {
        let result = from.scale_decimal_for(to);
        // For Decimal, we can use exact comparison
        assert_eq!(result, expected);
    }

    #[test]
    fn test_self_conversion() {
        // Converting to the same unit system should always be 1.0
        for unit in [
            UnitSystem::Inches,
            UnitSystem::Millimeters,
            UnitSystem::Mils,
            UnitSystem::Si,
        ] {
            assert_eq!(unit.scale_f64_for(unit), 1.0);
            assert_eq!(unit.scale_decimal_for(unit), dec!(1));
        }
    }

    #[test]
    fn test_roundtrip_conversions() {
        // Converting from A to B and back to A should equal 1.0
        let units = [
            UnitSystem::Inches,
            UnitSystem::Millimeters,
            UnitSystem::Mils,
            UnitSystem::Si,
        ];

        for &from in &units {
            for &to in &units {
                if from == to {
                    continue;
                }

                let scale_there = from.scale_f64_for(to);
                let scale_back = to.scale_f64_for(from);

                assert!(
                    approx_eq(scale_there * scale_back, 1.0, 1e-10),
                    "Failed roundtrip from {:?} to {:?}",
                    from,
                    to
                );

                let decimal_there = from.scale_decimal_for(to);
                let decimal_back = to.scale_decimal_for(from);

                // Allow a small margin due to potential decimal rounding errors
                let product = decimal_there * decimal_back;
                assert!(
                    product > dec!(0.9999) && product < dec!(1.0001),
                    "Failed decimal roundtrip from {:?} to {:?}: {} * {} = {}",
                    from,
                    to,
                    decimal_there,
                    decimal_back,
                    product
                );
            }
        }
    }
}

// Helper function to check if two f64 values are approximately equal
#[cfg(test)]
fn approx_eq(a: f64, b: f64, epsilon: f64) -> bool {
    (a - b).abs() < epsilon
}
