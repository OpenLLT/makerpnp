use num_traits::Float;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use crate::decimal;

/// Normalize to (-180, 180]
pub fn normalize_angle_deg_signed<T: Float>(angle: T) -> T {
    let full_circle = T::from(360.0).unwrap();
    let half_circle = T::from(180.0).unwrap();

    let mut normalized = angle % full_circle;
    if normalized < -half_circle {
        normalized = normalized + full_circle;
    } else if normalized > half_circle {
        normalized = normalized - full_circle;
    }
    normalized
}

/// Normalize to [0, 360)
pub fn normalize_angle_deg_unsigned<T: Float>(angle: T) -> T {
    let full_circle = T::from(360.0).unwrap();
    let normalized = angle % full_circle;
    if normalized < T::zero() {
        normalized + full_circle
    } else {
        normalized
    }
}

fn rem_euclid(a: Decimal, b: Decimal) -> Decimal {
    let r = a % b;
    if r < Decimal::ZERO { r + b } else { r }
}

/// Normalize Decimal to (-180, 180]
pub fn normalize_angle_deg_signed_decimal(angle: Decimal) -> Decimal {
    let full = dec!(360);
    let half = dec!(180);

    let mut norm = angle % full;
    if norm > half {
        norm -= full;
    } else if norm < -half {
        norm += full;
    }
    norm
}

/// Normalize Decimal to [0, <360)
pub fn normalize_angle_deg_unsigned_decimal(angle: Decimal) -> Decimal {
    let full = dec!(360);
    rem_euclid(angle, full)
}

#[cfg(test)]
mod angle_normalization_tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(0.0f32, 0.0)]
    #[case(180.0f32, 180.0)]
    #[case(-180.0f32, -180.0)]
    #[case(190.0f32, -170.0)]
    #[case(-190.0f32, 170.0)]
    #[case(360.0f32, 0.0)]
    #[case(-360.0f32, 0.0)]
    #[case(540.0f32, 180.0)]
    #[case(-540.0f32, -180.0)]
    #[case(720.0f32, 0.0)]
    #[case(-720.0f32, 0.0)]
    fn test_normalize_f32_signed(#[case] input: f32, #[case] expected: f32) {
        let result = normalize_angle_deg_signed(input);
        assert!(
            (result - expected).abs() < f32::EPSILON,
            "input: {input}, result: {result}, expected: {expected}"
        );
    }

    #[rstest]
    #[case(0.0f64, 0.0)]
    #[case(180.0f64, 180.0)]
    #[case(-180.0f64, -180.0)]
    #[case(190.0f64, -170.0)]
    #[case(-190.0f64, 170.0)]
    #[case(360.0f64, 0.0)]
    #[case(-360.0f64, 0.0)]
    #[case(540.0f64, 180.0)]
    #[case(-540.0f64,- 180.0)]
    #[case(720.0f64, 0.0)]
    #[case(-720.0f64, 0.0)]
    fn test_normalize_f64_signed(#[case] input: f64, #[case] expected: f64) {
        let result = normalize_angle_deg_signed(input);
        assert!(
            (result - expected).abs() < f64::EPSILON,
            "input: {input}, result: {result}, expected: {expected}"
        );
    }

    #[rstest]
    #[case(0.0f32, 0.0)]
    #[case(360.0f32, 0.0)]
    #[case(720.0f32, 0.0)]
    #[case(-90.0f32, 270.0)]
    #[case(-360.0f32, 0.0)]
    #[case(450.0f32, 90.0)]
    #[case(-450.0f32, 270.0)]
    fn test_normalize_f32_unsigned(#[case] input: f32, #[case] expected: f32) {
        let result = normalize_angle_deg_unsigned(input);
        assert!(
            (result - expected).abs() < f32::EPSILON,
            "input: {input}, result: {result}, expected: {expected}"
        );
    }

    #[rstest]
    #[case(0.0f64, 0.0)]
    #[case(360.0f64, 0.0)]
    #[case(720.0f64, 0.0)]
    #[case(-90.0f64, 270.0)]
    #[case(-360.0f64, 0.0)]
    #[case(450.0f64, 90.0)]
    #[case(-450.0f64, 270.0)]
    fn test_normalize_f64_unsigned(#[case] input: f64, #[case] expected: f64) {
        let result = normalize_angle_deg_unsigned(input);
        assert!(
            (result - expected).abs() < f64::EPSILON,
            "input: {input}, result: {result}, expected: {expected}"
        );
    }

    #[rstest]
    #[case(dec!(0), dec!(0))]
    #[case(dec!(180), dec!(180))]
    #[case(dec!(-180), dec!(-180))]
    #[case(dec!(190), dec!(-170))]
    #[case(dec!(-190), dec!(170))]
    #[case(dec!(360), dec!(0))]
    #[case(dec!(-360), dec!(0))]
    #[case(dec!(540), dec!(180))]
    #[case(dec!(-540), dec!(-180))]
    #[case(dec!(720), dec!(0))]
    #[case(dec!(-720), dec!(0))]
    fn test_normalize_decimal_signed(#[case] input: Decimal, #[case] expected: Decimal) {
        let result = normalize_angle_deg_signed_decimal(input);
        assert_eq!(
            result, expected,
            "input: {}, result: {}, expected: {}",
            input, result, expected
        );
    }

    #[rstest]
    #[case(dec!(0), dec!(0))]
    #[case(dec!(360), dec!(0))]
    #[case(dec!(720), dec!(0))]
    #[case(dec!(-90), dec!(270))]
    #[case(dec!(-360), dec!(0))]
    #[case(dec!(450), dec!(90))]
    #[case(dec!(-450), dec!(270))]
    fn test_normalize_decimal_unsigned(#[case] input: Decimal, #[case] expected: Decimal) {
        let result = normalize_angle_deg_unsigned_decimal(input);
        assert_eq!(
            result, expected,
            "input: {}, result: {}, expected: {}",
            input, result, expected
        );
    }
}

/// Trait from previous implementation
pub trait DecimalAngleExt {
    fn to_radians(&self) -> Decimal;
    fn to_degrees(&self) -> Decimal;
}

impl DecimalAngleExt for Decimal {
    /// Convert from degrees to radianns.
    fn to_radians(&self) -> Decimal {
        self * decimal::PI / dec!(180)
    }

    /// Convert from radiants to degrees.
    fn to_degrees(&self) -> Decimal {
        self * dec!(180) / decimal::PI
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use num_traits::ToPrimitive;
    use rust_decimal::Decimal;

    pub(crate) fn approx_eq_decimal_float(lhs: Decimal, rhs: f64, eps: f64) -> bool {
        let lhs_as_f64 = lhs.to_f64().unwrap();
        let difference = (lhs_as_f64 - rhs).abs();
        println!("lhs_as_f64: {}, rhs: {}, difference: {}", lhs_as_f64, rhs, difference);
        difference <= eps
    }

    pub(crate) fn approx_eq_decimal_decimal(lhs: Decimal, rhs: Decimal, eps: f64) -> bool {
        let lhs_as_f64 = lhs.to_f64().unwrap();
        let rhs_as_f64 = rhs.to_f64().unwrap();
        let difference = (lhs_as_f64 - rhs_as_f64).abs();
        println!(
            "lhs_as_f64: {}, rhs_as_f64: {}, difference: {}",
            lhs_as_f64, rhs_as_f64, difference
        );
        difference <= eps
    }
}

#[cfg(test)]
mod to_radians_tests {
    use num_traits::FromPrimitive;
    use rstest::rstest;

    use super::*;
    use crate::angle::test_helpers::approx_eq_decimal_float;

    // Helper: compare Decimal vs f64 with tolerance

    #[rstest]
    #[case(0.0)]
    #[case(45.0)]
    #[case(90.0)]
    #[case(135.0)]
    #[case(180.0)]
    #[case(225.0)]
    #[case(270.0)]
    #[case(315.0)]
    #[case(360.0)]
    fn test_to_radians(#[case] degrees: f64) {
        let decimal_degrees = Decimal::from_f64(degrees).unwrap();
        let expected = degrees.to_radians();

        let decimal_result = decimal_degrees.to_radians();

        assert!(
            approx_eq_decimal_float(decimal_result, expected, 0.000000000001),
            "Failed to_radians for {}: got {}, expected {}",
            degrees,
            decimal_result,
            expected
        );
    }
}

#[cfg(test)]
mod to_degrees_tests {
    use num_traits::FromPrimitive;
    use rstest::rstest;

    use super::*;
    use crate::angle::test_helpers::approx_eq_decimal_float;

    #[rstest]
    #[case(0.0)]
    #[case(45.0)]
    #[case(90.0)]
    #[case(135.0)]
    #[case(180.0)]
    #[case(225.0)]
    #[case(270.0)]
    #[case(315.0)]
    #[case(360.0)]
    fn test_to_degrees(#[case] input_degrees: f64) {
        // to make it easier to maintain and debug, the input is specified in degrees
        let input_radians = input_degrees.to_radians();

        let decimal_radians = Decimal::from_f64(input_radians).unwrap();

        let decimal_result = decimal_radians.to_degrees();

        println!(
            "input_degrees: {}, input_radians: {}, decimal_radians: {}, decimal_result: {}",
            input_degrees, input_radians, decimal_radians, decimal_result
        );

        assert!(
            approx_eq_decimal_float(decimal_result, input_degrees, 0.000000000001),
            "Failed to_degrees for {} rad: got {}, expected {}",
            input_degrees.to_radians(),
            decimal_result,
            input_degrees
        );
    }
}

#[cfg(test)]
mod to_degrees_radians_round_trip_tests {
    use num_traits::FromPrimitive;
    use rstest::rstest;

    use super::*;
    use crate::angle::test_helpers::approx_eq_decimal_decimal;

    #[rstest]
    #[case(0.0)]
    #[case(45.0)]
    #[case(90.0)]
    #[case(135.0)]
    #[case(180.0)]
    #[case(225.0)]
    #[case(270.0)]
    #[case(315.0)]
    #[case(360.0)]
    fn test_to_radians_to_degrees_and_back(#[case] input_degrees: f64) {
        // to make it easier to maintain and debug, the input is specified in degrees
        let input_radians = input_degrees.to_radians();
        let decimal_radians_input = Decimal::from_f64(input_radians).unwrap();
        let decimal_degrees = decimal_radians_input.to_degrees();
        let decimal_radians_output = decimal_degrees.to_radians();

        println!(
            "input_degrees: {}, radians: {}, decimal_radians_input: {}, decimal_degrees: {}, decimal_radians_output: {}",
            input_degrees, input_radians, decimal_radians_input, decimal_degrees, decimal_radians_output
        );

        assert!(
            approx_eq_decimal_decimal(decimal_radians_input, decimal_radians_output, 0.000000000001),
            "Failed to_degrees/to_radians round trip for {} radians",
            input_radians
        );
    }

    #[rstest]
    #[case(0.0)]
    #[case(45.0)]
    #[case(90.0)]
    #[case(135.0)]
    #[case(180.0)]
    #[case(225.0)]
    #[case(270.0)]
    #[case(315.0)]
    #[case(360.0)]
    fn test_to_degrees_to_radians_and_back(#[case] input_degrees: f64) {
        let decimal_degrees_input = Decimal::from_f64(input_degrees).unwrap();
        let decimal_radians = decimal_degrees_input.to_radians();
        let decimal_degrees_output = decimal_radians.to_degrees();

        println!(
            "input_degrees: {}, decimal_degrees_input: {}, decimal_degrees: {}, decimal_radians_output: {}",
            input_degrees, decimal_degrees_input, decimal_radians, decimal_degrees_output
        );

        assert!(
            approx_eq_decimal_decimal(decimal_degrees_input, decimal_degrees_output, 0.000000000001),
            "Failed to_radians/to_degrees round trip for {} degrees",
            input_degrees
        );
    }
}
