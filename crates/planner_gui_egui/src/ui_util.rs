use std::fmt::{Display, Formatter};
use std::ops::Deref;
use num_rational::Ratio;

use egui::{Color32, Style};

pub fn green_orange_red_grey_from_style(style: &Style) -> (Color32, Color32, Color32, Color32) {
    let visual = &style.visuals;

    // Credit: following snippet from egui-data-tables
    // Following logic simply gets 'green' color from current background's brightness.
    let green = if visual.window_fill.g() > 128 {
        Color32::DARK_GREEN
    } else {
        Color32::GREEN
    };

    (green, Color32::ORANGE, Color32::RED, Color32::LIGHT_GRAY)
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, PartialEq, Eq, Hash)]
pub struct NavigationPath(String);

impl NavigationPath {
    pub fn new(path: String) -> Self {
        Self(path)
    }
}

impl Deref for NavigationPath {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for NavigationPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for NavigationPath {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl Default for NavigationPath {
    fn default() -> Self {
        Self::new("/".to_string())
    }
}

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
    fn test_ratio_of_f64(
        #[case] a: f64,
        #[case] b: f64,
        #[case] expected_num: i64,
        #[case] expected_denom: i64,
    ) {
        let ratio = ratio_of_f64(a, b).expect("Failed to convert to ratio");
        assert_eq!(ratio.numer(), &expected_num);
        assert_eq!(ratio.denom(), &expected_denom);
    }
}
