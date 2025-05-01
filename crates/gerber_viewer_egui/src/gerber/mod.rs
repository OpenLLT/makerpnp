pub mod layer;
pub mod position;
pub mod expressions;

use egui::Color32;
pub use position::*;

pub enum Winding {
    /// Aka 'Positive' in Geometry
    Clockwise,
    /// Aka 'Negative' in Geometry
    CounterClockwise,
}

pub fn calculate_winding(vertices: &[Position]) -> Winding {
    let mut sum = 0.0;
    for i in 0..vertices.len() {
        let j = (i + 1) % vertices.len();
        sum += vertices[i].x * vertices[j].y - vertices[j].x * vertices[i].y;
    }
    if sum > 0.0 {
        Winding::Clockwise
    } else {
        Winding::CounterClockwise
    }
}

#[derive(Debug, Clone)]
pub enum Exposure {
    CutOut,
    Add,
}

impl From<bool> for Exposure {
    fn from(value: bool) -> Self {
        match value {
            true => Exposure::Add,
            false => Exposure::CutOut,
        }
    }
}

impl Exposure {
    pub fn to_color(&self, color: &Color32) -> Color32 {
        match self {
            Exposure::CutOut => Color32::BLACK,
            Exposure::Add => *color,
        }
    }
}

