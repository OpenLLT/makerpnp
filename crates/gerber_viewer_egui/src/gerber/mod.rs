pub mod position;

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