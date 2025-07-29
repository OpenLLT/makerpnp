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
