use num_rational::Ratio;

pub fn ratio_of_f64(a: f64, b: f64) -> Option<Ratio<i64>> {
    if b == 0.0 {
        return None;
    }
    let ra = Ratio::approximate_float(a)?;
    let rb = Ratio::approximate_float(b)?;

    // This automatically simplifies the result
    Some(ra / rb)
}

#[cfg(test)]
mod ratio_tests {
    use rstest::rstest;

    use super::ratio_of_f64;

    #[rstest]
    #[case(1.0, 2.0, 1, 2)]
    #[case(5.0, 10.0, 1, 2)]
    #[case(6.0, 9.0, 2, 3)]
    #[case(2.5, 5.0, 1, 2)]
    #[case(10.0, 4.0, 5, 2)]
    fn test_ratio_of_f64(#[case] a: f64, #[case] b: f64, #[case] expected_num: i64, #[case] expected_denom: i64) {
        let ratio = ratio_of_f64(a, b).expect("Failed to convert to ratio");
        assert_eq!(ratio.numer(), &expected_num);
        assert_eq!(ratio.denom(), &expected_denom);
    }
}
