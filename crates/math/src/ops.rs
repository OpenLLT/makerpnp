use nalgebra::Point2;

pub trait Ops2D<T> {
    fn add_x(self, value: T) -> Self;
    fn add_y(self, value: T) -> Self;
}

impl Ops2D<f64> for Point2<f64> {
    fn add_x(self, value: f64) -> Self {
        Self::new(self.x + value, self.y)
    }

    fn add_y(self, value: f64) -> Self {
        Self::new(self.x, self.y + value)
    }
}
