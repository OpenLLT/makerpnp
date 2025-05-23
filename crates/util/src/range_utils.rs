use std::cmp::{max, min};
use std::convert::TryInto;
use std::ops::RangeInclusive;

pub fn clamp_inclusive_range<T>(
    old: &RangeInclusive<T>,
    new: &RangeInclusive<T>,
    value: &RangeInclusive<T>,
) -> RangeInclusive<T>
where
    T: Ord + Clone,
{
    let old_start = old.start();
    let old_end = old.end();
    let new_start = new.start();
    let new_end = new.end();
    let value_start = value.start();
    let value_end = value.end();

    let start_using_old_bound = value_start == old_start;
    let end_using_old_bound = value_end == old_end;

    let result_start = if start_using_old_bound {
        new_start.clone()
    } else {
        max(value_start, new_start).clone()
    };

    let result_end = if end_using_old_bound {
        new_end.clone()
    } else {
        min(value_end, new_end).clone()
    };

    result_start..=result_end
}

#[cfg(test)]
mod clamp_range_tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(10..=20, 10..=21, 10..=20, 10..=21, "old range same as old limits, new limit end higher - use new limits")]
    #[case(10..=20, 9..=20, 10..=20, 9..=20, "old range same as old limits, new limit start lower - use new limits")]
    #[case(10..=20, 9..=21, 10..=20, 9..=21, "old range same as old limits, new limit wider on both sides - use new limits")]
    #[case(10..=20, 11..=19, 10..=20, 11..=19, "old range same as old limits, new narrower on both sides - use new limits")]
    #[case(10..=20, 11..=19, 12..=18, 12..=18, "old range fits inside new range, but wasn't using either limit - use old range")]
    #[case(10..=20, 5..=20, 12..=20, 12..=20, "old range start different from old limit start, new range start lower - use old range start")]
    #[case(10..=20, 10..=25, 10..=18, 10..=18, "old range end different from old limit end, new range end higher - use old range start")]
    #[case(1..=10, 1..=11, 2..=10, 2..=11, "expanding end, use new end")]
    #[case(10..=20, 9..=20, 10..=19, 9..=19, "expanding start, use new start")]
    fn test_clamp_inclusive_range(
        #[case] old_limits: RangeInclusive<usize>,
        #[case] new_limits: RangeInclusive<usize>,
        #[case] range: RangeInclusive<usize>,
        #[case] expected_result: RangeInclusive<usize>,
        #[case] scenario: &str,
    ) {
        let result = clamp_inclusive_range(&old_limits, &new_limits, &range);
        assert_eq!(
            result, expected_result,
            "clamp({}: {:?}, {:?}) = {:?}, expected_range {:?}",
            scenario, old_limits, new_limits, result, expected_result
        );
    }
}

/// Extension trait for converting `RangeInclusive<T>` into `RangeInclusive<usize>`
/// for types that safely implement `Into<usize>`, like `u8` or `u16`.
pub trait RangeIntoUsize {
    /// Converts a `RangeInclusive<T>` into `RangeInclusive<usize>`.
    ///
    /// # Panics
    ///
    /// Panics if `T` does not implement `Into<usize>` safely or if the values cannot be represented as `usize`.
    fn to_usize_range(&self) -> RangeInclusive<usize>;
}

impl<T> RangeIntoUsize for RangeInclusive<T>
where
    T: Clone + Into<usize>,
{
    fn to_usize_range(&self) -> RangeInclusive<usize> {
        self.start().clone().into()..=self.end().clone().into()
    }
}

/// Extension trait for converting `RangeInclusive<T>` into `RangeInclusive<usize>`
/// fallibly, using `TryInto<usize>`.
pub trait RangeTryIntoUsize {
    /// Attempts to convert a `RangeInclusive<T>` into `RangeInclusive<usize>`.
    ///
    /// Returns `None` if either endpoint cannot be converted into `usize`.
    fn try_to_usize_range(&self) -> Option<RangeInclusive<usize>>;
}

impl<T> RangeTryIntoUsize for RangeInclusive<T>
where
    T: Clone + TryInto<usize>,
{
    fn try_to_usize_range(&self) -> Option<RangeInclusive<usize>> {
        let start = self.start().clone().try_into().ok()?;
        let end = self.end().clone().try_into().ok()?;
        Some(start..=end)
    }
}

#[cfg(test)]
mod range_into_usize {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(0u8..=5, 0..=5)]
    #[case(1u16..=3, 1..=3)]
    fn test_range_into_usize(
        #[case] input: RangeInclusive<impl Clone + Into<usize>>,
        #[case] expected: RangeInclusive<usize>,
    ) {
        assert_eq!(input.to_usize_range(), expected);
    }
}

#[cfg(test)]
mod range_try_into_usize {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(0u8..=5, Some(0..=5))]
    #[case(2u16..=4, Some(2..=4))]
    #[case(10u32..=15, Some(10..=15))]
    #[case(0i8..=3, Some(0..=3))]
    #[case(1i16..=5, Some(1..=5))]
    #[case(7i32..=9, Some(7..=9))]
    #[case(-2i8..=3, None)]
    #[case(-10i32..=1, None)]
    fn test_range_try_into_usize<T>(#[case] input: RangeInclusive<T>, #[case] expected: Option<RangeInclusive<usize>>)
    where
        T: Clone + TryInto<usize>,
    {
        assert_eq!(input.try_to_usize_range(), expected);
    }
}
