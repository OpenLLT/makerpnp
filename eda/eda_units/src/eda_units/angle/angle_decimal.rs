use rust_decimal::Decimal;
use rust_decimal_macros::dec;

pub const PI: Decimal = dec!(3.1415926535897932384626433832);

/// Trait from previous implementation
pub trait DecimalAngleExt {
    fn to_radians(&self) -> Decimal;
    fn to_degrees(&self) -> Decimal;
}

impl DecimalAngleExt for Decimal {
    /// Convert from degrees to radianns.
    fn to_radians(&self) -> Decimal {
        self * PI / dec!(180)
    }

    /// Convert from radiants to degrees.
    fn to_degrees(&self) -> Decimal {
        self * dec!(180) / PI
    }
}

#[cfg(test)]
mod to_radians_tests {
    use num_traits::FromPrimitive;
    use rstest::rstest;

    use super::*;
    use crate::test::approx_eq_decimal_float;
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
    use crate::test::approx_eq_decimal_float;

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
    use crate::test::approx_eq_decimal_decimal;

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
