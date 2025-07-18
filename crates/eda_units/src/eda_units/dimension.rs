use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

use nalgebra::{Point2, Vector2};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::eda_units::unit_system::UnitSystem;

/// Represents a dimension with nanometer precision
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Dimension {
    /// Value in nanometers (internal representation)
    nm_value: i32,
}

impl Dimension {
    /// Create a new dimension from a value in a specific unit system
    pub fn from_f64(value: f64, unit_system: UnitSystem) -> Self {
        Self {
            nm_value: unit_system.to_nm_f64(value),
        }
    }

    /// Get the value in a specific unit system
    pub fn value_f64_in(&self, unit_system: UnitSystem) -> f64 {
        unit_system.from_nm_f64(self.nm_value)
    }

    /// Create a new dimension from a Decimal value in a specific unit system
    pub fn from_decimal(value: Decimal, unit_system: UnitSystem) -> Self {
        Self {
            nm_value: unit_system.to_nm_decimal(value),
        }
    }

    /// Get the value as a Decimal in a specific unit system
    pub fn value_decimal_in(&self, unit_system: UnitSystem) -> Decimal {
        unit_system.from_nm_decimal(self.nm_value)
    }

    /// Get the raw value in nanometers (internal representation)
    pub fn as_nm(&self) -> i32 {
        self.nm_value
    }

    /// Create from raw nanometer value
    pub fn from_nm(nm: i32) -> Self {
        Self {
            nm_value: nm,
        }
    }

    /// Format the dimension with the appropriate unit and specified precision using Decimal
    pub fn display_decimal(&self, unit_system: UnitSystem, precision: Option<usize>) -> String {
        let precision_value = precision.unwrap_or_else(|| unit_system.default_precision()) as u32;
        let decimal_value = self.value_decimal_in(unit_system);

        if precision_value == 0 {
            // No decimal places
            format!("{} {}", decimal_value.round().to_string(), unit_system.display_name())
        } else {
            // Format with specified precision
            format!(
                "{} {}",
                decimal_value.round_dp(precision_value),
                unit_system.display_name()
            )
        }
    }

    /// Format the dimension with the appropriate unit and specified precision
    pub fn display_f64(&self, unit_system: UnitSystem, precision: Option<usize>) -> String {
        let precision_value = precision.unwrap_or_else(|| unit_system.default_precision());

        if precision_value == 0 {
            // No decimal places
            format!(
                "{} {}",
                self.value_f64_in(unit_system).round() as i64,
                unit_system.display_name()
            )
        } else {
            // Format with specified precision
            format!(
                "{:.*} {}",
                precision_value,
                self.value_f64_in(unit_system),
                unit_system.display_name()
            )
        }
    }

    /// Format the dimension with the default precision for the unit system
    pub fn display_default(&self, unit_system: UnitSystem) -> String {
        self.display_f64(unit_system, None)
    }

    /// Zero dimension
    pub fn zero() -> Self {
        Self {
            nm_value: 0,
        }
    }
}

// Implement Add for Dimension
impl Add for Dimension {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            nm_value: self
                .nm_value
                .saturating_add(rhs.nm_value),
        }
    }
}

// Implement Sub for Dimension
impl Sub for Dimension {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            nm_value: self
                .nm_value
                .saturating_sub(rhs.nm_value),
        }
    }
}

// Implement Neg for Dimension
impl Neg for Dimension {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            nm_value: self.nm_value.saturating_neg(),
        }
    }
}

// Implement Mul<f64> for Dimension
impl Mul<f64> for Dimension {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        let result = self.nm_value as f64 * rhs;
        Self {
            nm_value: if result > i32::MAX as f64 {
                i32::MAX
            } else if result < i32::MIN as f64 {
                i32::MIN
            } else {
                result.round() as i32
            },
        }
    }
}

// Implement Div<f64> for Dimension
impl Div<f64> for Dimension {
    type Output = Self;

    fn div(self, rhs: f64) -> Self::Output {
        if rhs == 0.0 {
            return self; // Prevent division by zero
        }

        let result = self.nm_value as f64 / rhs;
        Self {
            nm_value: if result > i32::MAX as f64 {
                i32::MAX
            } else if result < i32::MIN as f64 {
                i32::MIN
            } else {
                result.round() as i32
            },
        }
    }
}

impl Mul<Decimal> for Dimension {
    type Output = Self;

    fn mul(self, rhs: Decimal) -> Self::Output {
        let result = Decimal::from(self.nm_value) * rhs;

        if result > Decimal::from(i32::MAX) {
            Self {
                nm_value: i32::MAX,
            }
        } else if result < Decimal::from(i32::MIN) {
            Self {
                nm_value: i32::MIN,
            }
        } else {
            Self {
                nm_value: result.round().to_i32().unwrap_or(0),
            }
        }
    }
}

impl Div<Decimal> for Dimension {
    type Output = Self;

    fn div(self, rhs: Decimal) -> Self::Output {
        if rhs.is_zero() {
            return self; // Prevent division by zero
        }

        let result = Decimal::from(self.nm_value) / rhs;

        if result > Decimal::from(i32::MAX) {
            Self {
                nm_value: i32::MAX,
            }
        } else if result < Decimal::from(i32::MIN) {
            Self {
                nm_value: i32::MIN,
            }
        } else {
            Self {
                nm_value: result.round().to_i32().unwrap_or(0),
            }
        }
    }
}

// Implement AddAssign for Dimension
impl AddAssign for Dimension {
    fn add_assign(&mut self, rhs: Self) {
        self.nm_value = self
            .nm_value
            .saturating_add(rhs.nm_value);
    }
}

// Implement SubAssign for Dimension
impl SubAssign for Dimension {
    fn sub_assign(&mut self, rhs: Self) {
        self.nm_value = self
            .nm_value
            .saturating_sub(rhs.nm_value);
    }
}

// Allow conversion from f64 to Dimension (using millimeters as default)
impl From<f64> for Dimension {
    fn from(value: f64) -> Self {
        Self::from_f64(value, UnitSystem::Millimeters)
    }
}

// Allow conversion from Decimal to Dimension (using millimeters as default)
impl From<Decimal> for Dimension {
    fn from(value: Decimal) -> Self {
        Self::from_decimal(value, UnitSystem::Millimeters)
    }
}

// Implementation to display Dimension (defaults to mm)
impl fmt::Display for Dimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_default(UnitSystem::Millimeters))
    }
}

// Extension trait for Point2<Dimension>
pub trait DimensionPoint2Ext {
    /// Create a new Point2<Dimension> from x, y coordinates in the specified unit system
    fn new_dim_f64(x: f64, y: f64, unit_system: UnitSystem) -> Self;
    fn new_dim_decimal(x: Decimal, y: Decimal, unit_system: UnitSystem) -> Self;

    /// Convert the point to a different unit system
    fn to_unit_system_f64(&self, unit_system: UnitSystem) -> Point2<f64>;
    fn to_unit_system_decimal(&self, unit_system: UnitSystem) -> Point2<Decimal>;

    /// Get the x value in the specified unit system
    fn x_f64_in(&self, unit_system: UnitSystem) -> f64;
    fn x_decimal_in(&self, unit_system: UnitSystem) -> Decimal;

    /// Get the y value in the specified unit system
    fn y_f64_in(&self, unit_system: UnitSystem) -> f64;
    fn y_decimal_in(&self, unit_system: UnitSystem) -> Decimal;

    /// Get the point in millimeters
    fn as_mm(&self) -> Point2<f64>;

    /// Format the point with appropriate units and specified precision
    fn display_f64(&self, unit_system: UnitSystem, precision: Option<usize>) -> String;
    fn display_decimal(&self, unit_system: UnitSystem, precision: Option<usize>) -> String;

    /// Format the point with default precision for the unit system
    fn display_default(&self, unit_system: UnitSystem) -> String;
}

impl DimensionPoint2Ext for Point2<Dimension> {
    fn new_dim_f64(x: f64, y: f64, unit_system: UnitSystem) -> Self {
        Point2::new(Dimension::from_f64(x, unit_system), Dimension::from_f64(y, unit_system))
    }

    fn new_dim_decimal(x: Decimal, y: Decimal, unit_system: UnitSystem) -> Self {
        Point2::new(
            Dimension::from_decimal(x, unit_system),
            Dimension::from_decimal(y, unit_system),
        )
    }

    fn to_unit_system_f64(&self, unit_system: UnitSystem) -> Point2<f64> {
        Point2::new(self.x.value_f64_in(unit_system), self.y.value_f64_in(unit_system))
    }

    fn to_unit_system_decimal(&self, unit_system: UnitSystem) -> Point2<Decimal> {
        Point2::new(
            self.x.value_decimal_in(unit_system),
            self.y.value_decimal_in(unit_system),
        )
    }

    fn x_f64_in(&self, unit_system: UnitSystem) -> f64 {
        self.x.value_f64_in(unit_system)
    }

    fn x_decimal_in(&self, unit_system: UnitSystem) -> Decimal {
        self.x.value_decimal_in(unit_system)
    }

    fn y_f64_in(&self, unit_system: UnitSystem) -> f64 {
        self.y.value_f64_in(unit_system)
    }

    fn y_decimal_in(&self, unit_system: UnitSystem) -> Decimal {
        self.y.value_decimal_in(unit_system)
    }

    fn as_mm(&self) -> Point2<f64> {
        self.to_unit_system_f64(UnitSystem::Millimeters)
    }

    fn display_f64(&self, unit_system: UnitSystem, precision: Option<usize>) -> String {
        let precision_value = precision.unwrap_or_else(|| unit_system.default_precision());

        if precision_value == 0 {
            format!(
                "({}, {}) {}",
                self.x_f64_in(unit_system).round() as i64,
                self.y_f64_in(unit_system).round() as i64,
                unit_system.display_name()
            )
        } else {
            format!(
                "({:.*}, {:.*}) {}",
                precision_value,
                self.x_f64_in(unit_system),
                precision_value,
                self.y_f64_in(unit_system),
                unit_system.display_name()
            )
        }
    }

    fn display_decimal(&self, unit_system: UnitSystem, precision: Option<usize>) -> String {
        let precision_value = precision.unwrap_or_else(|| unit_system.default_precision()) as u32;
        let x = self.x.value_decimal_in(unit_system);
        let y = self.y.value_decimal_in(unit_system);

        format!(
            "({}, {}) {}",
            x.round_dp(precision_value),
            y.round_dp(precision_value),
            unit_system.display_name()
        )
    }

    fn display_default(&self, unit_system: UnitSystem) -> String {
        self.display_f64(unit_system, None)
    }
}

// Extension trait for Vector2<Dimension>
pub trait DimensionVector2Ext {
    /// Create a new Vector2<Dimension> from x, y coordinates in the specified unit system
    fn new_dim_f64(x: f64, y: f64, unit_system: UnitSystem) -> Self;
    fn new_dim_decimal(x: Decimal, y: Decimal, unit_system: UnitSystem) -> Self;

    /// Convert the vector to a different unit system
    fn to_unit_system(&self, unit_system: UnitSystem) -> Vector2<f64>;

    /// Get the x value in the specified unit system
    fn x_f64_in(&self, unit_system: UnitSystem) -> f64;
    fn x_decimal_in(&self, unit_system: UnitSystem) -> Decimal;

    /// Get the y value in the specified unit system
    fn y_f64_in(&self, unit_system: UnitSystem) -> f64;
    fn y_decimal_in(&self, unit_system: UnitSystem) -> Decimal;

    /// Get the vector in millimeters
    fn as_mm(&self) -> Vector2<f64>;

    /// Format the vector with appropriate units and specified precision
    fn display_f64(&self, unit_system: UnitSystem, precision: Option<usize>) -> String;
    fn display_decimal(&self, unit_system: UnitSystem, precision: Option<usize>) -> String;

    /// Format the vector with default precision for the unit system
    fn display_default(&self, unit_system: UnitSystem) -> String;
}

impl DimensionVector2Ext for Vector2<Dimension> {
    fn new_dim_f64(x: f64, y: f64, unit_system: UnitSystem) -> Self {
        Vector2::new(Dimension::from_f64(x, unit_system), Dimension::from_f64(y, unit_system))
    }

    fn new_dim_decimal(x: Decimal, y: Decimal, unit_system: UnitSystem) -> Self {
        Vector2::new(
            Dimension::from_decimal(x, unit_system),
            Dimension::from_decimal(y, unit_system),
        )
    }

    fn to_unit_system(&self, unit_system: UnitSystem) -> Vector2<f64> {
        Vector2::new(self.x.value_f64_in(unit_system), self.y.value_f64_in(unit_system))
    }

    fn x_f64_in(&self, unit_system: UnitSystem) -> f64 {
        self.x.value_f64_in(unit_system)
    }

    fn x_decimal_in(&self, unit_system: UnitSystem) -> Decimal {
        self.x.value_decimal_in(unit_system)
    }

    fn y_f64_in(&self, unit_system: UnitSystem) -> f64 {
        self.y.value_f64_in(unit_system)
    }

    fn y_decimal_in(&self, unit_system: UnitSystem) -> Decimal {
        self.y.value_decimal_in(unit_system)
    }

    fn as_mm(&self) -> Vector2<f64> {
        self.to_unit_system(UnitSystem::Millimeters)
    }

    fn display_f64(&self, unit_system: UnitSystem, precision: Option<usize>) -> String {
        let precision_value = precision.unwrap_or_else(|| unit_system.default_precision());

        if precision_value == 0 {
            format!(
                "[{}, {}] {}",
                self.x_f64_in(unit_system).round() as i64,
                self.y_f64_in(unit_system).round() as i64,
                unit_system.display_name()
            )
        } else {
            format!(
                "[{:.*}, {:.*}] {}",
                precision_value,
                self.x_f64_in(unit_system),
                precision_value,
                self.y_f64_in(unit_system),
                unit_system.display_name()
            )
        }
    }

    fn display_decimal(&self, unit_system: UnitSystem, precision: Option<usize>) -> String {
        let precision_value = precision.unwrap_or_else(|| unit_system.default_precision()) as u32;
        let x = self.x.value_decimal_in(unit_system);
        let y = self.y.value_decimal_in(unit_system);

        format!(
            "[{}, {}] {}",
            x.round_dp(precision_value),
            y.round_dp(precision_value),
            unit_system.display_name()
        )
    }

    fn display_default(&self, unit_system: UnitSystem) -> String {
        self.display_f64(unit_system, None)
    }
}

// Extension for converting between Point2<f64> and Point2<Dimension>
pub trait PointConversion {
    fn to_dim_point(&self, unit_system: UnitSystem) -> Point2<Dimension>;
}

impl PointConversion for Point2<f64> {
    fn to_dim_point(&self, unit_system: UnitSystem) -> Point2<Dimension> {
        Point2::new_dim_f64(self.x, self.y, unit_system)
    }
}

// Extension for converting between Vector2<f64> and Vector2<Dimension>
pub trait VectorConversion {
    fn to_dim_vector(&self, unit_system: UnitSystem) -> Vector2<Dimension>;
}

impl VectorConversion for Vector2<f64> {
    fn to_dim_vector(&self, unit_system: UnitSystem) -> Vector2<Dimension> {
        Vector2::new_dim_f64(self.x, self.y, unit_system)
    }
}

#[cfg(test)]
mod tests {
    use nalgebra::{Point2, Vector2};
    use rstest::rstest;

    use super::*;

    // Helper function to check if two f64 values are approximately equal
    fn approx_eq(a: f64, b: f64, epsilon: f64) -> bool {
        (a - b).abs() < epsilon
    }

    #[rstest]
    #[case(1.0, UnitSystem::Inches, 25_400_000)]
    #[case(25.4, UnitSystem::Millimeters, 25_400_000)]
    #[case(1000.0, UnitSystem::Mils, 25_400_000)]
    #[case(2540.0, UnitSystem::Si, 25_400_000)]
    fn test_dimension_new(#[case] value: f64, #[case] unit: UnitSystem, #[case] expected_nm: i32) {
        let dim = Dimension::from_f64(value, unit);
        assert_eq!(dim.as_nm(), expected_nm);
    }

    #[rstest]
    #[case(25_400_000, UnitSystem::Inches, 1.0)]
    #[case(25_400_000, UnitSystem::Millimeters, 25.4)]
    #[case(25_400_000, UnitSystem::Mils, 1000.0)]
    #[case(25_400_000, UnitSystem::Si, 2540.0)]
    fn test_dimension_value_in(#[case] nm_value: i32, #[case] unit: UnitSystem, #[case] expected: f64) {
        let dim = Dimension::from_nm(nm_value);
        let result = dim.value_f64_in(unit);
        assert!(approx_eq(result, expected, 0.001));
    }

    #[test]
    fn test_dimension_as_nm() {
        let dim = Dimension::from_nm(12345);
        assert_eq!(dim.as_nm(), 12345);
    }

    #[test]
    fn test_dimension_from_nm() {
        let dim = Dimension::from_nm(54321);
        assert_eq!(dim.as_nm(), 54321);
    }

    #[rstest]
    #[case(1.0, UnitSystem::Inches, Some(2), "1.00 in")]
    #[case(25.4, UnitSystem::Millimeters, Some(1), "25.4 mm")]
    #[case(1000.0, UnitSystem::Mils, Some(0), "1000 mil")]
    #[case(2540.0, UnitSystem::Si, Some(1), "2540.0 丝")]
    fn test_dimension_display(
        #[case] value: f64,
        #[case] unit: UnitSystem,
        #[case] precision: Option<usize>,
        #[case] expected: &str,
    ) {
        let dim = Dimension::from_f64(value, unit);
        assert_eq!(dim.display_f64(unit, precision), expected);
    }

    #[rstest]
    #[case(1.0, UnitSystem::Inches, "1.000000 in")]
    #[case(25.4, UnitSystem::Millimeters, "25.4000 mm")]
    #[case(1000.0, UnitSystem::Mils, "1000.000 mil")]
    #[case(2540.0, UnitSystem::Si, "2540.00 丝")]
    fn test_dimension_display_default(#[case] value: f64, #[case] unit: UnitSystem, #[case] expected: &str) {
        let dim = Dimension::from_f64(value, unit);
        assert_eq!(dim.display_default(unit), expected);
    }

    #[test]
    fn test_dimension_zero() {
        let dim = Dimension::zero();
        assert_eq!(dim.as_nm(), 0);
    }

    #[test]
    fn test_dimension_add() {
        let a = Dimension::from_nm(1_000_000); // 1mm
        let b = Dimension::from_nm(500_000); // 0.5mm
        let result = a + b;
        assert_eq!(result.as_nm(), 1_500_000); // 1.5mm
    }

    #[test]
    fn test_dimension_sub() {
        let a = Dimension::from_nm(1_000_000); // 1mm
        let b = Dimension::from_nm(300_000); // 0.3mm
        let result = a - b;
        assert_eq!(result.as_nm(), 700_000); // 0.7mm
    }

    #[test]
    fn test_dimension_neg() {
        let a = Dimension::from_nm(1_000_000); // 1mm
        let result = -a;
        assert_eq!(result.as_nm(), -1_000_000); // -1mm
    }

    #[test]
    fn test_dimension_add_assign() {
        let mut dim = Dimension::from_nm(1_000_000); // 1mm
        let other = Dimension::from_nm(500_000); // 0.5mm
        dim += other;
        assert_eq!(dim.as_nm(), 1_500_000); // Should be 1.5mm
    }

    #[test]
    fn test_dimension_sub_assign() {
        let mut dim = Dimension::from_nm(1_000_000); // 1mm
        let other = Dimension::from_nm(300_000); // 0.3mm
        dim -= other;
        assert_eq!(dim.as_nm(), 700_000); // Should be 0.7mm
    }

    #[test]
    fn test_dimension_add_assign_saturating() {
        let mut dim = Dimension::from_nm(i32::MAX - 100);
        let other = Dimension::from_nm(200);
        dim += other;
        assert_eq!(dim.as_nm(), i32::MAX); // Should saturate at MAX
    }

    #[test]
    fn test_dimension_sub_assign_saturating() {
        let mut dim = Dimension::from_nm(i32::MIN + 100);
        let other = Dimension::from_nm(200);
        dim -= other;
        assert_eq!(dim.as_nm(), i32::MIN); // Should saturate at MIN
    }

    #[rstest]
    #[case(1_000_000, 2.5, 2_500_000)] // 1mm * 2.5 = 2.5mm
    #[case(1_000_000, 0.5, 500_000)] // 1mm * 0.5 = 0.5mm
    #[case(1_000_000, 0.0, 0)] // 1mm * 0 = 0mm
    #[case(1_000_000, -1.0, -1_000_000)] // 1mm * -1 = -1mm
    fn test_dimension_mul_f64(#[case] nm_value: i32, #[case] factor: f64, #[case] expected: i32) {
        let dim = Dimension::from_nm(nm_value);
        let result = dim * factor;
        assert_eq!(result.as_nm(), expected);
    }

    #[rstest]
    #[case(1_000_000, 2.0, 500_000)] // 1mm / 2 = 0.5mm
    #[case(1_000_000, 0.5, 2_000_000)] // 1mm / 0.5 = 2mm
    #[case(1_000_000, -1.0, -1_000_000)] // 1mm / -1 = -1mm
    fn test_dimension_div_f64(#[case] nm_value: i32, #[case] divisor: f64, #[case] expected: i32) {
        let dim = Dimension::from_nm(nm_value);
        let result = dim / divisor;
        assert_eq!(result.as_nm(), expected);
    }

    #[test]
    fn test_dimension_div_by_zero() {
        let dim = Dimension::from_nm(1_000_000);
        let result = dim / 0.0;
        // Division by zero should return the original value
        assert_eq!(result.as_nm(), dim.as_nm());
    }

    #[test]
    fn test_dimension_from_f64() {
        let dim: Dimension = 1.5.into();
        // Default unit system is millimeters
        assert_eq!(dim.as_nm(), 1_500_000);
    }

    #[test]
    fn test_dimension_display_format() {
        let dim = Dimension::from_nm(1_500_000); // 1.5mm
        let formatted = format!("{}", dim);
        assert_eq!(formatted, "1.5000 mm");
    }

    #[test]
    fn test_point2_new_dim() {
        let point = Point2::new_dim_f64(1.0, 2.0, UnitSystem::Millimeters);
        assert_eq!(point.x.as_nm(), 1_000_000);
        assert_eq!(point.y.as_nm(), 2_000_000);
    }

    #[rstest]
    #[case(UnitSystem::Inches, 0.03937, 0.07874)] // 1mm = 0.03937in, 2mm = 0.07874in
    #[case(UnitSystem::Millimeters, 1.0, 2.0)] // 1mm = 1mm, 2mm = 2mm
    #[case(UnitSystem::Mils, 39.37, 78.74)] // 1mm = 39.37mil, 2mm = 78.74mil
    #[case(UnitSystem::Si, 100.0, 200.0)] // 1mm = 100丝, 2mm = 200丝
    fn test_point2_to_unit_system(#[case] unit: UnitSystem, #[case] expected_x: f64, #[case] expected_y: f64) {
        let point = Point2::new_dim_f64(1.0, 2.0, UnitSystem::Millimeters);
        let converted = point.to_unit_system_f64(unit);
        assert!(approx_eq(converted.x, expected_x, 0.001));
        assert!(approx_eq(converted.y, expected_y, 0.001));
    }

    #[rstest]
    #[case(UnitSystem::Inches, 0.03937)] // 1mm = 0.03937in
    #[case(UnitSystem::Millimeters, 1.0)] // 1mm = 1mm
    #[case(UnitSystem::Mils, 39.37)] // 1mm = 39.37mil
    #[case(UnitSystem::Si, 100.0)] // 1mm = 100丝
    fn test_point2_x_in(#[case] unit: UnitSystem, #[case] expected: f64) {
        let point = Point2::new_dim_f64(1.0, 2.0, UnitSystem::Millimeters);
        assert!(approx_eq(point.x_f64_in(unit), expected, 0.001));
    }

    #[rstest]
    #[case(UnitSystem::Inches, 0.07874)] // 2mm = 0.07874in
    #[case(UnitSystem::Millimeters, 2.0)] // 2mm = 2mm
    #[case(UnitSystem::Mils, 78.74)] // 2mm = 78.74mil
    #[case(UnitSystem::Si, 200.0)] // 2mm = 200丝
    fn test_point2_y_in(#[case] unit: UnitSystem, #[case] expected: f64) {
        let point = Point2::new_dim_f64(1.0, 2.0, UnitSystem::Millimeters);
        assert!(approx_eq(point.y_f64_in(unit), expected, 0.001));
    }

    #[test]
    fn test_point2_as_mm() {
        let point = Point2::new_dim_f64(1.0, 2.0, UnitSystem::Inches);
        let mm_point = point.as_mm();
        assert!(approx_eq(mm_point.x, 25.4, 0.001));
        assert!(approx_eq(mm_point.y, 50.8, 0.001));
    }

    #[rstest]
    #[case(UnitSystem::Inches, Some(2), "(1.00, 2.00) in")]
    #[case(UnitSystem::Millimeters, Some(1), "(1.0, 2.0) mm")]
    #[case(UnitSystem::Mils, Some(0), "(1, 2) mil")]
    #[case(UnitSystem::Si, Some(0), "(1, 2) 丝")]
    fn test_point2_display(#[case] unit: UnitSystem, #[case] precision: Option<usize>, #[case] expected: &str) {
        let point = Point2::new_dim_f64(1.0, 2.0, unit);
        assert_eq!(point.display_f64(unit, precision), expected);
    }

    #[rstest]
    #[case(UnitSystem::Inches, "(1.000000, 2.000000) in")]
    #[case(UnitSystem::Millimeters, "(1.0000, 2.0000) mm")]
    #[case(UnitSystem::Mils, "(1.000, 2.000) mil")]
    #[case(UnitSystem::Si, "(1.00, 2.00) 丝")]
    fn test_point2_display_default(#[case] unit: UnitSystem, #[case] expected: &str) {
        let point = Point2::new_dim_f64(1.0, 2.0, unit);
        assert_eq!(point.display_default(unit), expected);
    }

    #[test]
    fn test_vector2_new_dim() {
        let vector = Vector2::new_dim_f64(1.0, 2.0, UnitSystem::Millimeters);
        assert_eq!(vector.x.as_nm(), 1_000_000);
        assert_eq!(vector.y.as_nm(), 2_000_000);
    }

    #[rstest]
    #[case(UnitSystem::Inches, 0.03937, 0.07874)] // 1mm = 0.03937in, 2mm = 0.07874in
    #[case(UnitSystem::Millimeters, 1.0, 2.0)] // 1mm = 1mm, 2mm = 2mm
    #[case(UnitSystem::Mils, 39.37, 78.74)] // 1mm = 39.37mil, 2mm = 78.74mil
    #[case(UnitSystem::Si, 100.0, 200.0)] // 1mm = 100丝, 2mm = 200丝
    fn test_vector2_to_unit_system(#[case] unit: UnitSystem, #[case] expected_x: f64, #[case] expected_y: f64) {
        let vector = Vector2::new_dim_f64(1.0, 2.0, UnitSystem::Millimeters);
        let converted = vector.to_unit_system(unit);
        assert!(approx_eq(converted.x, expected_x, 0.001));
        assert!(approx_eq(converted.y, expected_y, 0.001));
    }

    #[rstest]
    #[case(UnitSystem::Inches, 0.03937)] // 1mm = 0.03937in
    #[case(UnitSystem::Millimeters, 1.0)] // 1mm = 1mm
    #[case(UnitSystem::Mils, 39.37)] // 1mm = 39.37mil
    #[case(UnitSystem::Si, 100.0)] // 1mm = 100丝
    fn test_vector2_x_in(#[case] unit: UnitSystem, #[case] expected: f64) {
        let vector = Vector2::new_dim_f64(1.0, 2.0, UnitSystem::Millimeters);
        assert!(approx_eq(vector.x_f64_in(unit), expected, 0.001));
    }

    #[rstest]
    #[case(UnitSystem::Inches, 0.07874)] // 2mm = 0.07874in
    #[case(UnitSystem::Millimeters, 2.0)] // 2mm = 2mm
    #[case(UnitSystem::Mils, 78.74)] // 2mm = 78.74mil
    #[case(UnitSystem::Si, 200.0)] // 2mm = 200丝
    fn test_vector2_y_in(#[case] unit: UnitSystem, #[case] expected: f64) {
        let vector = Vector2::new_dim_f64(1.0, 2.0, UnitSystem::Millimeters);
        assert!(approx_eq(vector.y_f64_in(unit), expected, 0.001));
    }

    #[test]
    fn test_vector2_as_mm() {
        let vector = Vector2::new_dim_f64(1.0, 2.0, UnitSystem::Inches);
        let mm_vector = vector.as_mm();
        assert!(approx_eq(mm_vector.x, 25.4, 0.001));
        assert!(approx_eq(mm_vector.y, 50.8, 0.001));
    }

    #[rstest]
    #[case(UnitSystem::Inches, Some(2), "[1.00, 2.00] in")]
    #[case(UnitSystem::Millimeters, Some(1), "[1.0, 2.0] mm")]
    #[case(UnitSystem::Mils, Some(0), "[1, 2] mil")]
    #[case(UnitSystem::Si, Some(0), "[1, 2] 丝")]
    fn test_vector2_display(#[case] unit: UnitSystem, #[case] precision: Option<usize>, #[case] expected: &str) {
        let vector = Vector2::new_dim_f64(1.0, 2.0, unit);
        assert_eq!(vector.display_f64(unit, precision), expected);
    }

    #[rstest]
    #[case(UnitSystem::Inches, "[1.000000, 2.000000] in")]
    #[case(UnitSystem::Millimeters, "[1.0000, 2.0000] mm")]
    #[case(UnitSystem::Mils, "[1.000, 2.000] mil")]
    #[case(UnitSystem::Si, "[1.00, 2.00] 丝")]
    fn test_vector2_display_default(#[case] unit: UnitSystem, #[case] expected: &str) {
        let vector = Vector2::new_dim_f64(1.0, 2.0, unit);
        assert_eq!(vector.display_default(unit), expected);
    }

    #[test]
    fn test_point_conversion() {
        let orig_point = Point2::new(1.0, 2.0);
        let dim_point = orig_point.to_dim_point(UnitSystem::Millimeters);
        assert_eq!(dim_point.x.as_nm(), 1_000_000);
        assert_eq!(dim_point.y.as_nm(), 2_000_000);
    }

    #[test]
    fn test_vector_conversion() {
        let orig_vector = Vector2::new(1.0, 2.0);
        let dim_vector = orig_vector.to_dim_vector(UnitSystem::Millimeters);
        assert_eq!(dim_vector.x.as_nm(), 1_000_000);
        assert_eq!(dim_vector.y.as_nm(), 2_000_000);
    }

    #[test]
    fn test_nalgebra_operations() {
        // Test vector addition
        let v1 = Vector2::new_dim_f64(1.0, 2.0, UnitSystem::Millimeters);
        let v2 = Vector2::new_dim_f64(3.0, 4.0, UnitSystem::Millimeters);
        let v_sum = v1 + v2;
        assert_eq!(v_sum.x.as_nm(), 4_000_000);
        assert_eq!(v_sum.y.as_nm(), 6_000_000);

        // Test point + vector
        let p1 = Point2::new_dim_f64(1.0, 2.0, UnitSystem::Millimeters);
        let p_new = p1 + v1;
        assert_eq!(p_new.x.as_nm(), 2_000_000);
        assert_eq!(p_new.y.as_nm(), 4_000_000);

        // Test point - point = vector
        let p2 = Point2::new_dim_f64(5.0, 7.0, UnitSystem::Millimeters);
        let v_diff = p2 - p1;
        assert_eq!(v_diff.x.as_nm(), 4_000_000);
        assert_eq!(v_diff.y.as_nm(), 5_000_000);
    }

    #[test]
    fn test_cross_unit_display() {
        // Create a point in millimeters
        let point_mm = Point2::new_dim_f64(1.0, 2.0, UnitSystem::Millimeters);

        // Display using default precision for each unit system
        assert_eq!(point_mm.display_default(UnitSystem::Inches), "(0.039370, 0.078740) in");
        assert_eq!(point_mm.display_default(UnitSystem::Millimeters), "(1.0000, 2.0000) mm");
        assert_eq!(point_mm.display_default(UnitSystem::Mils), "(39.370, 78.740) mil");
        assert_eq!(point_mm.display_default(UnitSystem::Si), "(100.00, 200.00) 丝");
    }

    #[test]
    fn test_i32_overflow_handling() {
        // Test values at the edge of i32
        let large_value = (i32::MAX as f64) + 100.0;
        let dim = Dimension::from_f64(large_value / 1_000_000.0, UnitSystem::Millimeters);
        assert_eq!(dim.as_nm(), i32::MAX); // Should clamp to i32::MAX

        let negative_large = (i32::MIN as f64) - 100.0;
        let dim_neg = Dimension::from_f64(negative_large / 1_000_000.0, UnitSystem::Millimeters);
        assert_eq!(dim_neg.as_nm(), i32::MIN); // Should clamp to i32::MIN
    }

    #[test]
    fn test_saturating_operations() {
        // Test saturating addition
        let max_dim = Dimension::from_nm(i32::MAX);
        let positive_dim = Dimension::from_nm(1);
        let result = max_dim + positive_dim;
        assert_eq!(result.as_nm(), i32::MAX); // Should saturate at MAX

        // Test saturating subtraction
        let min_dim = Dimension::from_nm(i32::MIN);
        let negative_dim = Dimension::from_nm(-1);
        let result = min_dim + negative_dim;
        assert_eq!(result.as_nm(), i32::MIN); // Should saturate at MIN

        // Test saturating negation
        let min_dim = Dimension::from_nm(i32::MIN);
        let result = -min_dim;
        assert_eq!(result.as_nm(), i32::MAX); // Should saturate at MAX
    }

    #[test]
    fn test_unit_conversions_chain() {
        // Create a point in inches
        let point_inches = Point2::new_dim_f64(1.0, 2.0, UnitSystem::Inches);

        // Convert to millimeters
        let mm_val = point_inches.x_f64_in(UnitSystem::Millimeters);
        assert!(approx_eq(mm_val, 25.4, 0.001));

        // Convert to Si
        let si_val = point_inches.x_f64_in(UnitSystem::Si);
        assert!(approx_eq(si_val, 2540.0, 0.001));

        // Convert to mils
        let mil_val = point_inches.x_f64_in(UnitSystem::Mils);
        assert!(approx_eq(mil_val, 1000.0, 0.001));

        // Full circle back to inches
        let inch_val = point_inches.x_f64_in(UnitSystem::Inches);
        assert!(approx_eq(inch_val, 1.0, 0.001));
    }
}
