use std::cmp::{Ordering, PartialEq, PartialOrd};
use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

use nalgebra::{Point2, Vector2};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::eda_units::dimension::Dimension;
use crate::eda_units::unit_system::UnitSystem;

/// A dimension value with associated unit system and precision
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DimensionUnit {
    /// The underlying dimension value
    dimension: Dimension,
    /// The unit system this dimension is expressed in
    unit_system: UnitSystem,
    /// Optional custom precision (if None, uses unit system default)
    precision: Option<usize>,
}

impl DimensionUnit {
    /// Create a new dimension with unit and default precision
    pub fn from_f64(value: f64, unit_system: UnitSystem) -> Self {
        Self {
            dimension: Dimension::from_f64(value, unit_system),
            unit_system,
            precision: None,
        }
    }

    /// Create a new dimension with unit and custom precision
    pub fn from_f64_with_precision(value: f64, unit_system: UnitSystem, precision: usize) -> Self {
        Self {
            dimension: Dimension::from_f64(value, unit_system),
            unit_system,
            precision: Some(precision),
        }
    }

    pub fn from_decimal(value: Decimal, unit_system: UnitSystem) -> Self {
        Self {
            dimension: Dimension::from_decimal(value, unit_system),
            unit_system,
            precision: None,
        }
    }

    pub fn from_decimal_with_precision(value: Decimal, unit_system: UnitSystem, precision: usize) -> Self {
        Self {
            dimension: Dimension::from_decimal(value, unit_system),
            unit_system,
            precision: Some(precision),
        }
    }

    /// Create from an existing Dimension with unit system and default precision
    pub fn from_dimension(dimension: Dimension, unit_system: UnitSystem) -> Self {
        Self {
            dimension,
            unit_system,
            precision: None,
        }
    }

    /// Create a zero dimension with the specified unit system
    pub fn zero(unit_system: UnitSystem) -> Self {
        Self {
            dimension: Dimension::zero(),
            unit_system,
            precision: None,
        }
    }

    /// Get the raw Dimension value
    pub fn dimension(&self) -> Dimension {
        self.dimension
    }

    /// Get the unit system
    pub fn unit_system(&self) -> UnitSystem {
        self.unit_system
    }

    /// Get the precision (if custom) or the unit system default
    pub fn precision(&self) -> usize {
        self.precision
            .unwrap_or_else(|| self.unit_system.default_precision())
    }

    /// Set a custom precision
    pub fn with_precision(mut self, precision: usize) -> Self {
        self.precision = Some(precision);
        self
    }

    /// Reset to use default precision for the unit system
    pub fn with_default_precision(mut self) -> Self {
        self.precision = None;
        self
    }

    /// Change the unit system, preserving the actual value
    pub fn in_unit_system(&self, unit_system: UnitSystem) -> Self {
        // Get the value in the target unit system, but keep the same physical measurement
        let value = self.dimension.value_f64_in(unit_system);
        Self {
            dimension: Dimension::from_f64(value, unit_system),
            unit_system,
            precision: None, // Reset to default precision for the new unit system
        }
    }

    /// Get the value in a specific unit system
    pub fn value_f64_in(&self, unit_system: UnitSystem) -> f64 {
        self.dimension.value_f64_in(unit_system)
    }

    /// Get the value in the current unit system
    pub fn value_f64(&self) -> f64 {
        self.dimension
            .value_f64_in(self.unit_system)
    }

    pub fn value_decimal_in(&self, unit_system: UnitSystem) -> Decimal {
        self.dimension
            .value_decimal_in(unit_system)
    }

    pub fn value_decimal(&self) -> Decimal {
        self.dimension
            .value_decimal_in(self.unit_system)
    }
}

impl fmt::Display for DimensionUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = self
            .dimension
            .value_f64_in(self.unit_system);
        let precision = self.precision();

        if precision == 0 {
            write!(f, "{} {}", value.round() as i64, self.unit_system.display_name())
        } else {
            write!(f, "{:.*} {}", precision, value, self.unit_system.display_name())
        }
    }
}

impl Add for DimensionUnit {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        // Convert rhs to the same unit system as self
        let rhs_value = rhs.value_f64_in(self.unit_system);
        let rhs_dimension = Dimension::from_f64(rhs_value, self.unit_system);

        Self {
            dimension: self.dimension + rhs_dimension,
            unit_system: self.unit_system,
            precision: self.precision,
        }
    }
}

impl Sub for DimensionUnit {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        // Convert rhs to the same unit system as self
        let rhs_value = rhs.value_f64_in(self.unit_system);
        let rhs_dimension = Dimension::from_f64(rhs_value, self.unit_system);

        Self {
            dimension: self.dimension - rhs_dimension,
            unit_system: self.unit_system,
            precision: self.precision,
        }
    }
}

impl Neg for DimensionUnit {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            dimension: -self.dimension,
            unit_system: self.unit_system,
            precision: self.precision,
        }
    }
}

impl Mul<f64> for DimensionUnit {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        Self {
            dimension: self.dimension * rhs,
            unit_system: self.unit_system,
            precision: self.precision,
        }
    }
}

impl Div<f64> for DimensionUnit {
    type Output = Self;

    fn div(self, rhs: f64) -> Self::Output {
        if rhs == 0.0 {
            return self; // Prevent division by zero
        }

        Self {
            dimension: self.dimension / rhs,
            unit_system: self.unit_system,
            precision: self.precision,
        }
    }
}

impl Mul<Decimal> for DimensionUnit {
    type Output = Self;

    fn mul(self, rhs: Decimal) -> Self::Output {
        Self {
            dimension: self.dimension * rhs,
            unit_system: self.unit_system,
            precision: self.precision,
        }
    }
}

impl Div<Decimal> for DimensionUnit {
    type Output = Self;

    fn div(self, rhs: Decimal) -> Self::Output {
        if rhs == dec!(0.0) {
            return self; // Prevent division by zero
        }

        Self {
            dimension: self.dimension / rhs,
            unit_system: self.unit_system,
            precision: self.precision,
        }
    }
}

impl AddAssign for DimensionUnit {
    fn add_assign(&mut self, rhs: Self) {
        // Convert rhs to the same unit system as self
        let rhs_value = rhs.value_decimal_in(self.unit_system);
        let rhs_dimension = Dimension::from_decimal(rhs_value, self.unit_system);

        self.dimension += rhs_dimension;
    }
}

impl SubAssign for DimensionUnit {
    fn sub_assign(&mut self, rhs: Self) {
        // Convert rhs to the same unit system as self
        let rhs_value = rhs.value_decimal_in(self.unit_system);
        let rhs_dimension = Dimension::from_decimal(rhs_value, self.unit_system);

        self.dimension -= rhs_dimension;
    }
}

impl MulAssign<f64> for DimensionUnit {
    fn mul_assign(&mut self, rhs: f64) {
        self.dimension = self.dimension * rhs;
    }
}

impl DivAssign<f64> for DimensionUnit {
    fn div_assign(&mut self, rhs: f64) {
        if rhs != 0.0 {
            self.dimension = self.dimension / rhs;
        }
    }
}

impl MulAssign<Decimal> for DimensionUnit {
    fn mul_assign(&mut self, rhs: Decimal) {
        self.dimension = self.dimension * rhs;
    }
}

impl DivAssign<Decimal> for DimensionUnit {
    fn div_assign(&mut self, rhs: Decimal) {
        if rhs != dec!(0.0) {
            self.dimension = self.dimension / rhs;
        }
    }
}

impl PartialEq for DimensionUnit {
    fn eq(&self, other: &Self) -> bool {
        // Compare in nanometers to ensure accurate comparison regardless of unit system
        self.dimension.as_nm() == other.dimension.as_nm()
    }
}

impl PartialOrd for DimensionUnit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Compare in nanometers to ensure accurate comparison regardless of unit system
        self.dimension
            .as_nm()
            .partial_cmp(&other.dimension.as_nm())
    }
}

/// Conversion traits from f64 to DimensionUnit
impl From<(f64, UnitSystem)> for DimensionUnit {
    fn from((value, unit_system): (f64, UnitSystem)) -> Self {
        Self::from_f64(value, unit_system)
    }
}

/// Conversion trait from DimensionUnit to f64 (in specified unit system)
impl DimensionUnit {
    pub fn to_f64_in(&self, unit_system: UnitSystem) -> f64 {
        self.value_f64_in(unit_system)
    }
}

/// Point2 extension for DimensionUnit
pub type DimensionUnitPoint2 = Point2<DimensionUnit>;

/// Vector2 extension for DimensionUnit
pub type DimensionUnitVector2 = Vector2<DimensionUnit>;

/// Extension trait for Point2<DimensionUnit>
pub trait DimensionUnitPoint2Ext {
    /// Create a new Point2<DimensionUnit> from x, y coordinates
    fn new_dim_f64(x: f64, y: f64, unit_system: UnitSystem) -> Self;
    fn new_dim_decimal(x: Decimal, y: Decimal, unit_system: UnitSystem) -> Self;

    /// Convert to Point2<f64> in the specified unit system
    fn to_point2(&self, unit_system: UnitSystem) -> Point2<f64>;

    /// Change the unit system for both coordinates
    fn in_unit_system(&self, unit_system: UnitSystem) -> Self;

    /// Format the point with appropriate units
    fn display(&self) -> String;
}

impl DimensionUnitPoint2Ext for Point2<DimensionUnit> {
    fn new_dim_f64(x: f64, y: f64, unit_system: UnitSystem) -> Self {
        Point2::new(
            DimensionUnit::from_f64(x, unit_system),
            DimensionUnit::from_f64(y, unit_system),
        )
    }

    fn new_dim_decimal(x: Decimal, y: Decimal, unit_system: UnitSystem) -> Self {
        Point2::new(
            DimensionUnit::from_decimal(x, unit_system),
            DimensionUnit::from_decimal(y, unit_system),
        )
    }

    fn to_point2(&self, unit_system: UnitSystem) -> Point2<f64> {
        Point2::new(self.x.value_f64_in(unit_system), self.y.value_f64_in(unit_system))
    }

    fn in_unit_system(&self, unit_system: UnitSystem) -> Self {
        Point2::new(self.x.in_unit_system(unit_system), self.y.in_unit_system(unit_system))
    }

    fn display(&self) -> String {
        let unit_system = self.x.unit_system();
        let precision = self
            .x
            .precision()
            .max(self.y.precision());

        if precision == 0 {
            format!(
                "({}, {}) {}",
                self.x.value_decimal().round(),
                self.y.value_decimal().round(),
                unit_system.display_name()
            )
        } else {
            format!(
                "({:.*}, {:.*}) {}",
                precision,
                self.x
                    .value_decimal()
                    .round_dp(precision as u32),
                precision,
                self.y
                    .value_decimal()
                    .round_dp(precision as u32),
                unit_system.display_name()
            )
        }
    }
}

/// Extension trait for Vector2<DimensionUnit>
pub trait DimensionUnitVector2Ext {
    /// Create a new Vector2<DimensionUnit> from x, y coordinates
    fn new_dim_f64(x: f64, y: f64, unit_system: UnitSystem) -> Self;
    fn new_dim_decimal(x: Decimal, y: Decimal, unit_system: UnitSystem) -> Self;

    /// Convert to Vector2<f64> in the specified unit system
    fn to_vector2(&self, unit_system: UnitSystem) -> Vector2<f64>;

    /// Change the unit system for both coordinates
    fn in_unit_system(&self, unit_system: UnitSystem) -> Self;

    /// Format the vector with appropriate units
    fn display(&self) -> String;
}

impl DimensionUnitVector2Ext for Vector2<DimensionUnit> {
    fn new_dim_f64(x: f64, y: f64, unit_system: UnitSystem) -> Self {
        Vector2::new(
            DimensionUnit::from_f64(x, unit_system),
            DimensionUnit::from_f64(y, unit_system),
        )
    }

    fn new_dim_decimal(x: Decimal, y: Decimal, unit_system: UnitSystem) -> Self {
        Vector2::new(
            DimensionUnit::from_decimal(x, unit_system),
            DimensionUnit::from_decimal(y, unit_system),
        )
    }

    fn to_vector2(&self, unit_system: UnitSystem) -> Vector2<f64> {
        Vector2::new(self.x.value_f64_in(unit_system), self.y.value_f64_in(unit_system))
    }

    fn in_unit_system(&self, unit_system: UnitSystem) -> Self {
        Vector2::new(self.x.in_unit_system(unit_system), self.y.in_unit_system(unit_system))
    }

    fn display(&self) -> String {
        let unit_system = self.x.unit_system();
        let precision = self
            .x
            .precision()
            .max(self.y.precision());

        if precision == 0 {
            format!(
                "[{}, {}] {}",
                self.x.value_decimal().round(),
                self.y.value_decimal().round(),
                unit_system.display_name()
            )
        } else {
            format!(
                "[{:.*}, {:.*}] {}",
                precision,
                self.x
                    .value_decimal()
                    .round_dp(precision as u32),
                precision,
                self.y
                    .value_decimal()
                    .round_dp(precision as u32),
                unit_system.display_name()
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f64::EPSILON;

    use super::*;

    // Helper function to check if two f64 values are approximately equal
    fn approx_eq(a: f64, b: f64, epsilon: f64) -> bool {
        (a - b).abs() < epsilon
    }

    #[test]
    fn test_dimension_unit_new() {
        let dim_unit = DimensionUnit::from_f64(1.0, UnitSystem::Millimeters);
        assert_eq!(dim_unit.unit_system(), UnitSystem::Millimeters);
        assert_eq!(dim_unit.precision(), 4); // Default for mm
        assert!(approx_eq(dim_unit.value_f64(), 1.0, EPSILON));
    }

    #[test]
    fn test_dimension_unit_new_with_precision() {
        let dim_unit = DimensionUnit::from_f64_with_precision(1.0, UnitSystem::Millimeters, 3);
        assert_eq!(dim_unit.unit_system(), UnitSystem::Millimeters);
        assert_eq!(dim_unit.precision(), 3); // Custom precision
        assert!(approx_eq(dim_unit.value_f64(), 1.0, EPSILON));
    }

    #[test]
    fn test_dimension_unit_from_dimension() {
        let dimension = Dimension::from_f64(25.4, UnitSystem::Millimeters);
        let dim_unit = DimensionUnit::from_dimension(dimension, UnitSystem::Inches);
        assert_eq!(dim_unit.unit_system(), UnitSystem::Inches);
        assert!(approx_eq(dim_unit.value_f64(), 1.0, EPSILON));
    }

    #[test]
    fn test_dimension_unit_with_precision() {
        let dim_unit = DimensionUnit::from_f64(1.0, UnitSystem::Millimeters);
        let dim_unit_precise = dim_unit.with_precision(4);
        assert_eq!(dim_unit_precise.precision(), 4);
    }

    #[test]
    fn test_dimension_unit_with_default_precision() {
        let dim_unit = DimensionUnit::from_f64_with_precision(1.0, UnitSystem::Inches, 5);
        assert_eq!(dim_unit.precision(), 5);

        let reset_dim_unit = dim_unit.with_default_precision();
        assert_eq!(reset_dim_unit.precision(), 6); // Default for inches
    }

    #[test]
    fn test_dimension_unit_in_unit_system() {
        let mm_unit = DimensionUnit::from_f64(25.4, UnitSystem::Millimeters);
        let in_unit = mm_unit.in_unit_system(UnitSystem::Inches);

        assert_eq!(in_unit.unit_system(), UnitSystem::Inches);
        assert!(approx_eq(in_unit.value_f64(), 1.0, EPSILON));
        assert_eq!(in_unit.precision(), 6); // Default for inches
    }

    #[test]
    fn test_dimension_unit_value_in() {
        let in_unit = DimensionUnit::from_f64(1.0, UnitSystem::Inches);

        assert!(approx_eq(in_unit.value_f64_in(UnitSystem::Inches), 1.0, EPSILON));
        assert!(approx_eq(in_unit.value_f64_in(UnitSystem::Millimeters), 25.4, EPSILON));
        assert!(approx_eq(in_unit.value_f64_in(UnitSystem::Mils), 1000.0, EPSILON));
        assert!(approx_eq(in_unit.value_f64_in(UnitSystem::Si), 2540.0, EPSILON));
    }

    #[test]
    fn test_dimension_unit_display() {
        let mm_unit = DimensionUnit::from_f64(1.0005, UnitSystem::Millimeters);
        assert_eq!(format!("{}", mm_unit), "1.0005 mm");

        let in_unit = DimensionUnit::from_f64(1.000005, UnitSystem::Inches);
        assert_eq!(format!("{}", in_unit), "1.000005 in");

        let mil_unit = DimensionUnit::from_f64(1.005, UnitSystem::Mils);
        assert_eq!(format!("{}", mil_unit), "1.005 mil");

        let si_unit = DimensionUnit::from_f64(1.05, UnitSystem::Si);
        assert_eq!(format!("{}", si_unit), "1.05 丝");

        let custom_unit = DimensionUnit::from_f64_with_precision(1.005, UnitSystem::Millimeters, 3);
        assert_eq!(format!("{}", custom_unit), "1.005 mm");
    }
}

#[cfg(test)]
mod dimension_unit_arithmetic_tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn test_addition() {
        let a = DimensionUnit::from_f64(1.0, UnitSystem::Inches);
        let b = DimensionUnit::from_f64(25.4, UnitSystem::Millimeters); // 1 inch in mm

        let sum = a + b;
        assert_eq!(sum.unit_system(), UnitSystem::Inches);
        assert!(approx_eq(sum.value_f64(), 2.0, 0.001));
    }

    #[test]
    fn test_subtraction() {
        let a = DimensionUnit::from_f64(2.0, UnitSystem::Inches);
        let b = DimensionUnit::from_f64(25.4, UnitSystem::Millimeters); // 1 inch in mm

        let diff = a - b;
        assert_eq!(diff.unit_system(), UnitSystem::Inches);
        assert!(approx_eq(diff.value_f64(), 1.0, 0.001));
    }

    #[test]
    fn test_negation() {
        let a = DimensionUnit::from_f64(1.5, UnitSystem::Inches);
        let neg = -a;

        assert_eq!(neg.unit_system(), UnitSystem::Inches);
        assert!(approx_eq(neg.value_f64(), -1.5, 0.001));
    }

    #[rstest]
    #[case(1.0, 2.5, 2.5)]
    #[case(2.0, 0.5, 1.0)]
    #[case(1.0, -1.0, -1.0)]
    fn test_multiplication(#[case] value: f64, #[case] factor: f64, #[case] expected: f64) {
        let dim = DimensionUnit::from_f64(value, UnitSystem::Millimeters);
        let result = dim * factor;

        assert_eq!(result.unit_system(), UnitSystem::Millimeters);
        assert!(approx_eq(result.value_f64(), expected, 0.001));
    }

    #[rstest]
    #[case(2.0, 2.0, 1.0)]
    #[case(2.0, 0.5, 4.0)]
    #[case(2.0, -1.0, -2.0)]
    fn test_division(#[case] value: f64, #[case] divisor: f64, #[case] expected: f64) {
        let dim = DimensionUnit::from_f64(value, UnitSystem::Millimeters);
        let result = dim / divisor;

        assert_eq!(result.unit_system(), UnitSystem::Millimeters);
        assert!(approx_eq(result.value_f64(), expected, 0.001));
    }

    #[test]
    fn test_div_by_zero() {
        let dim = DimensionUnit::from_f64(2.0, UnitSystem::Millimeters);
        let result = dim / 0.0;

        // Should return the original value
        assert_eq!(result.unit_system(), UnitSystem::Millimeters);
        assert!(approx_eq(result.value_f64(), 2.0, 0.001));
    }

    #[test]
    fn test_add_assign() {
        let mut a = DimensionUnit::from_f64(1.0, UnitSystem::Inches);
        let b = DimensionUnit::from_f64(25.4, UnitSystem::Millimeters); // 1 inch in mm

        a += b;
        assert_eq!(a.unit_system(), UnitSystem::Inches);
        assert!(approx_eq(a.value_f64(), 2.0, 0.001));
    }

    #[test]
    fn test_sub_assign() {
        let mut a = DimensionUnit::from_f64(2.0, UnitSystem::Inches);
        let b = DimensionUnit::from_f64(25.4, UnitSystem::Millimeters); // 1 inch in mm

        a -= b;
        assert_eq!(a.unit_system(), UnitSystem::Inches);
        assert!(approx_eq(a.value_f64(), 1.0, 0.001));
    }

    #[test]
    fn test_mul_assign() {
        let mut a = DimensionUnit::from_f64(2.0, UnitSystem::Inches);

        a *= 1.5;
        assert_eq!(a.unit_system(), UnitSystem::Inches);
        assert!(approx_eq(a.value_f64(), 3.0, 0.001));
    }

    #[test]
    fn test_div_assign() {
        let mut a = DimensionUnit::from_f64(2.0, UnitSystem::Inches);

        a /= 2.0;
        assert_eq!(a.unit_system(), UnitSystem::Inches);
        assert!(approx_eq(a.value_f64(), 1.0, 0.001));
    }

    // Helper function for approximately equal comparisons
    fn approx_eq(a: f64, b: f64, epsilon: f64) -> bool {
        (a - b).abs() < epsilon
    }
}

#[cfg(test)]
mod dimension_unit_comparison_tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn test_equality() {
        let a = DimensionUnit::from_f64(1.0, UnitSystem::Inches);
        let b = DimensionUnit::from_f64(25.4, UnitSystem::Millimeters); // 1 inch in mm
        let c = DimensionUnit::from_f64(1.1, UnitSystem::Inches);

        assert_eq!(a, b); // Same physical measurement
        assert_ne!(a, c); // Different measurements
    }

    #[rstest]
    #[case(1.0, UnitSystem::Inches, 25.4, UnitSystem::Millimeters, true)] // 1 in == 25.4 mm
    #[case(1.0, UnitSystem::Inches, 1000.0, UnitSystem::Mils, true)] // 1 in == 1000 mil
    #[case(1.0, UnitSystem::Millimeters, 100.0, UnitSystem::Si, true)] // 1 mm == 100 丝
    #[case(1.0, UnitSystem::Inches, 26.0, UnitSystem::Millimeters, false)] // 1 in != 26 mm
    fn test_equality_different_units(
        #[case] value1: f64,
        #[case] unit1: UnitSystem,
        #[case] value2: f64,
        #[case] unit2: UnitSystem,
        #[case] should_be_equal: bool,
    ) {
        let a = DimensionUnit::from_f64(value1, unit1);
        let b = DimensionUnit::from_f64(value2, unit2);

        if should_be_equal {
            assert_eq!(a, b);
        } else {
            assert_ne!(a, b);
        }
    }

    #[test]
    fn test_ordering() {
        let small = DimensionUnit::from_f64(1.0, UnitSystem::Inches);
        let medium = DimensionUnit::from_f64(2.0, UnitSystem::Inches);
        let large = DimensionUnit::from_f64(3.0, UnitSystem::Inches);

        assert!(small < medium);
        assert!(medium < large);
        assert!(small < large);

        assert!(large > medium);
        assert!(medium > small);
        assert!(large > small);
    }

    #[rstest]
    #[case(1.0, UnitSystem::Inches, 20.0, UnitSystem::Millimeters, true)] // 1 in > 20 mm
    #[case(10.0, UnitSystem::Millimeters, 1.0, UnitSystem::Inches, false)] // 10 mm < 1 in
    #[case(1.0, UnitSystem::Inches, 1000.0, UnitSystem::Mils, false)] // 1 in == 1000 mil
    fn test_ordering_different_units(
        #[case] value1: f64,
        #[case] unit1: UnitSystem,
        #[case] value2: f64,
        #[case] unit2: UnitSystem,
        #[case] first_greater: bool,
    ) {
        let a = DimensionUnit::from_f64(value1, unit1);
        let b = DimensionUnit::from_f64(value2, unit2);

        if first_greater {
            assert!(a > b);
        } else if a == b {
            assert!(!(a < b) && !(a > b));
        } else {
            assert!(a < b);
        }
    }
}

#[cfg(test)]
mod dimension_unit_conversion_tests {
    use super::*;

    #[test]
    fn test_from_tuple() {
        let value_unit_pair = (2.5, UnitSystem::Millimeters);
        let dim: DimensionUnit = value_unit_pair.into();

        assert_eq!(dim.unit_system(), UnitSystem::Millimeters);
        assert!(approx_eq(dim.value_f64(), 2.5, 0.001));
    }

    #[test]
    fn test_to_f64() {
        let dim = DimensionUnit::from_f64(1.0, UnitSystem::Inches);

        assert!(approx_eq(dim.to_f64_in(UnitSystem::Inches), 1.0, 0.001));
        assert!(approx_eq(dim.to_f64_in(UnitSystem::Millimeters), 25.4, 0.001));
        assert!(approx_eq(dim.to_f64_in(UnitSystem::Mils), 1000.0, 0.001));
    }

    // Helper function for approximately equal comparisons
    fn approx_eq(a: f64, b: f64, epsilon: f64) -> bool {
        (a - b).abs() < epsilon
    }
}

#[cfg(all(test, feature = "serde"))]
mod dimension_unit_serialization_tests {
    use serde_json;

    use super::*;

    #[test]
    fn test_serialize_deserialize() {
        let original = DimensionUnit::from_f64(1.5, UnitSystem::Inches);

        // Serialize to JSON string
        let serialized = serde_json::to_string(&original).unwrap();

        // Deserialize back to DimensionUnit
        let deserialized: DimensionUnit = serde_json::from_str(&serialized).unwrap();

        // Check if they are equal
        assert_eq!(original, deserialized);
        assert_eq!(original.unit_system(), deserialized.unit_system());
        assert_eq!(original.precision(), deserialized.precision());
    }

    #[test]
    fn test_serialize_deserialize_with_custom_precision() {
        let original = DimensionUnit::from_f64_with_precision(1.5, UnitSystem::Inches, 4);

        // Serialize to JSON string
        let serialized = serde_json::to_string(&original).unwrap();

        // Deserialize back to DimensionUnit
        let deserialized: DimensionUnit = serde_json::from_str(&serialized).unwrap();

        // Check if they are equal
        assert_eq!(original, deserialized);
        assert_eq!(original.unit_system(), deserialized.unit_system());
        assert_eq!(original.precision(), deserialized.precision());
    }
}

#[cfg(test)]
mod dimension_point_vector_tests {
    use super::*;

    #[test]
    fn test_dimension_point_creation() {
        let point = Point2::new_dim_f64(1.0, 2.0, UnitSystem::Millimeters);

        assert_eq!(point.x.unit_system(), UnitSystem::Millimeters);
        assert_eq!(point.y.unit_system(), UnitSystem::Millimeters);
        assert!(approx_eq(point.x.value_f64(), 1.0, 0.001));
        assert!(approx_eq(point.y.value_f64(), 2.0, 0.001));
    }

    #[test]
    fn test_dimension_point_conversion() {
        let point = Point2::new_dim_f64(1.0, 2.0, UnitSystem::Inches);
        let point_mm = point.to_point2(UnitSystem::Millimeters);

        assert!(approx_eq(point_mm.x, 25.4, 0.001));
        assert!(approx_eq(point_mm.y, 50.8, 0.001));
    }

    #[test]
    fn test_dimension_point_unit_system_change() {
        let point = Point2::new_dim_f64(1.0, 2.0, UnitSystem::Inches);
        let point_mm = point.in_unit_system(UnitSystem::Millimeters);

        assert_eq!(point_mm.x.unit_system(), UnitSystem::Millimeters);
        assert_eq!(point_mm.y.unit_system(), UnitSystem::Millimeters);
        assert!(approx_eq(point_mm.x.value_f64(), 25.4, 0.001));
        assert!(approx_eq(point_mm.y.value_f64(), 50.8, 0.001));
    }

    #[test]
    fn test_dimension_point_display() {
        let point = Point2::new_dim_f64(1.0, 2.0, UnitSystem::Millimeters);
        assert_eq!(point.display(), "(1.0000, 2.0000) mm");

        let point_inch = Point2::new_dim_f64(1.0, 2.0, UnitSystem::Inches);
        assert_eq!(point_inch.display(), "(1.000000, 2.000000) in");

        let point = Point2::new(
            DimensionUnit::from_decimal_with_precision(dec!(1), UnitSystem::Millimeters, 0),
            DimensionUnit::from_decimal_with_precision(dec!(2), UnitSystem::Millimeters, 0),
        );
        assert_eq!(point.display(), "(1, 2) mm");

        let point = Point2::new(
            DimensionUnit::from_decimal_with_precision(dec!(1.1), UnitSystem::Millimeters, 1),
            DimensionUnit::from_decimal_with_precision(dec!(2.22), UnitSystem::Millimeters, 2),
        );
        assert_eq!(point.display(), "(1.10, 2.22) mm");
    }

    #[test]
    fn test_dimension_vector_creation() {
        let vector = Vector2::new_dim_f64(1.0, 2.0, UnitSystem::Millimeters);

        assert_eq!(vector.x.unit_system(), UnitSystem::Millimeters);
        assert_eq!(vector.y.unit_system(), UnitSystem::Millimeters);
        assert!(approx_eq(vector.x.value_f64(), 1.0, 0.001));
        assert!(approx_eq(vector.y.value_f64(), 2.0, 0.001));
    }

    #[test]
    fn test_dimension_vector_conversion() {
        let vector = Vector2::new_dim_f64(1.0, 2.0, UnitSystem::Inches);
        let vector_mm = vector.to_vector2(UnitSystem::Millimeters);

        assert!(approx_eq(vector_mm.x, 25.4, 0.001));
        assert!(approx_eq(vector_mm.y, 50.8, 0.001));
    }

    #[test]
    fn test_dimension_vector_unit_system_change() {
        let vector = Vector2::new_dim_f64(1.0, 2.0, UnitSystem::Inches);
        let vector_mm = vector.in_unit_system(UnitSystem::Millimeters);

        assert_eq!(vector_mm.x.unit_system(), UnitSystem::Millimeters);
        assert_eq!(vector_mm.y.unit_system(), UnitSystem::Millimeters);
        assert!(approx_eq(vector_mm.x.value_f64(), 25.4, 0.001));
        assert!(approx_eq(vector_mm.y.value_f64(), 50.8, 0.001));
    }

    #[test]
    fn test_dimension_vector_display() {
        let vector = Vector2::new_dim_f64(1.0, 2.0, UnitSystem::Millimeters);
        assert_eq!(vector.display(), "[1.0000, 2.0000] mm");

        let vector_inch = Vector2::new_dim_f64(1.0, 2.0, UnitSystem::Inches);
        assert_eq!(vector_inch.display(), "[1.000000, 2.000000] in");

        let vector = Vector2::new(
            DimensionUnit::from_decimal_with_precision(dec!(1), UnitSystem::Millimeters, 0),
            DimensionUnit::from_decimal_with_precision(dec!(2), UnitSystem::Millimeters, 0),
        );
        assert_eq!(vector.display(), "[1, 2] mm");

        let vector = Vector2::new(
            DimensionUnit::from_decimal_with_precision(dec!(1.1), UnitSystem::Millimeters, 1),
            DimensionUnit::from_decimal_with_precision(dec!(2.22), UnitSystem::Millimeters, 2),
        );
        assert_eq!(vector.display(), "[1.10, 2.22] mm");
    }

    #[test]
    fn test_point_vector_operations() {
        let point = Point2::new_dim_f64(1.0, 2.0, UnitSystem::Millimeters);
        let vector = Vector2::new_dim_f64(3.0, 4.0, UnitSystem::Millimeters);

        // Point + Vector = Point
        let new_point = point + vector;
        assert!(approx_eq(new_point.x.value_f64(), 4.0, 0.001));
        assert!(approx_eq(new_point.y.value_f64(), 6.0, 0.001));

        // Point - Point = Vector
        let point2 = Point2::new_dim_f64(5.0, 7.0, UnitSystem::Millimeters);
        let diff = point2 - point;
        assert!(approx_eq(diff.x.value_f64(), 4.0, 0.001));
        assert!(approx_eq(diff.y.value_f64(), 5.0, 0.001));
    }

    #[test]
    fn test_mixed_unit_operations() {
        let point_mm = Point2::new_dim_f64(25.4, 50.8, UnitSystem::Millimeters);
        let vector_inch = Vector2::new_dim_f64(1.0, 2.0, UnitSystem::Inches);

        // Adding vector in inches to point in mm
        let result = point_mm + vector_inch;

        // Result should be in mm (the point's unit system)
        assert_eq!(result.x.unit_system(), UnitSystem::Millimeters);
        assert!(approx_eq(result.x.value_f64(), 50.8, 0.001)); // 25.4 mm + 1 inch = 50.8 mm
        assert!(approx_eq(result.y.value_f64(), 101.6, 0.001)); // 50.8 mm + 2 inch = 101.6 mm
    }

    // Helper function for approximately equal comparisons
    fn approx_eq(a: f64, b: f64, epsilon: f64) -> bool {
        (a - b).abs() < epsilon
    }
}
