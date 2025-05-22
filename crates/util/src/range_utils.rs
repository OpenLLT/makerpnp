use std::cmp::{max, min};
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
mod tests {
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
