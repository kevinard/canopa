use crate::{Point, Radians, Rectangle, Size, Vector};

use glam::{Mat4, Vec3, Vec4};
use std::ops::Mul;

/// A 2D transformation matrix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transformation(Mat4);

impl Transformation {
    /// A [`Transformation`] that preserves whatever is transformed.
    pub const IDENTITY: Self = Self(Mat4::IDENTITY);

    /// Creates an orthographic projection.
    #[rustfmt::skip]
    pub fn orthographic(width: u32, height: u32) -> Self{
        Self(Mat4::orthographic_rh_gl(
            0.0, width as f32,
            height as f32, 0.0,
            -1.0, 1.0
        ))
    }

    /// Creates a translate transformation.
    pub fn translate(x: f32, y: f32) -> Self {
        Self(Mat4::from_translation(Vec3::new(x, y, 0.0)))
    }

    /// Creates a uniform scaling transformation.
    pub fn scale(scaling: f32) -> Self {
        Self(Mat4::from_scale(Vec3::new(scaling, scaling, 1.0)))
    }

    /// Creates a rotation transformation.
    pub fn rotate(rotation: impl Into<Radians>) -> Self {
        Self(Mat4::from_rotation_z(f32::from(rotation.into())))
    }

    /// Returns the inverse of the [`Transformation`].
    pub fn inverse(self) -> Self {
        Self(self.0.inverse())
    }

    /// Returns the scale factor of the [`Transformation`].
    pub fn scale_factor(&self) -> f32 {
        self.0.x_axis.truncate().length()
    }

    /// Returns the rotation angle of the [`Transformation`].
    pub fn rotation(&self) -> Radians {
        Radians(self.0.x_axis.y.atan2(self.0.x_axis.x))
    }

    /// Returns the translation of the [`Transformation`].
    pub fn translation(&self) -> Vector {
        Vector::new(self.0.w_axis.x, self.0.w_axis.y)
    }
}

impl Default for Transformation {
    fn default() -> Self {
        Transformation::IDENTITY
    }
}

impl Mul for Transformation {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        Self(self.0 * rhs.0)
    }
}

impl Mul<Transformation> for Point {
    type Output = Self;

    fn mul(self, transformation: Transformation) -> Self {
        let point = transformation
            .0
            .mul_vec4(Vec4::new(self.x, self.y, 1.0, 1.0));

        Point::new(point.x, point.y)
    }
}

impl Mul<Transformation> for Vector {
    type Output = Self;

    fn mul(self, transformation: Transformation) -> Self {
        let new_vector = transformation
            .0
            .mul_vec4(Vec4::new(self.x, self.y, 1.0, 0.0));

        Vector::new(new_vector.x, new_vector.y)
    }
}

impl Mul<Transformation> for Size {
    type Output = Self;

    fn mul(self, transformation: Transformation) -> Self {
        let x_axis = Vector::new(self.width, 0.0) * transformation;
        let y_axis = Vector::new(0.0, self.height) * transformation;

        Size::new(
            x_axis.x.abs() + y_axis.x.abs(),
            x_axis.y.abs() + y_axis.y.abs(),
        )
    }
}

impl Mul<Transformation> for Rectangle {
    type Output = Self;

    fn mul(self, transformation: Transformation) -> Self {
        let top_left = self.position() * transformation;
        let top_right =
            Point::new(self.x + self.width, self.y) * transformation;
        let bottom_left =
            Point::new(self.x, self.y + self.height) * transformation;
        let bottom_right = Point::new(
            self.x + self.width,
            self.y + self.height,
        ) * transformation;

        let min_x = top_left
            .x
            .min(top_right.x)
            .min(bottom_left.x)
            .min(bottom_right.x);
        let max_x = top_left
            .x
            .max(top_right.x)
            .max(bottom_left.x)
            .max(bottom_right.x);
        let min_y = top_left
            .y
            .min(top_right.y)
            .min(bottom_left.y)
            .min(bottom_right.y);
        let max_y = top_left
            .y
            .max(top_right.y)
            .max(bottom_left.y)
            .max(bottom_right.y);

        Self::new(
            Point::new(min_x, min_y),
            Size::new(max_x - min_x, max_y - min_y),
        )
    }
}

impl AsRef<[f32; 16]> for Transformation {
    fn as_ref(&self) -> &[f32; 16] {
        self.0.as_ref()
    }
}

impl From<Transformation> for [f32; 16] {
    fn from(t: Transformation) -> [f32; 16] {
        *t.as_ref()
    }
}

impl From<Transformation> for Mat4 {
    fn from(transformation: Transformation) -> Self {
        transformation.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_preserves_scale_factor() {
        let rotation =
            Transformation::rotate(Radians(std::f32::consts::FRAC_PI_2));

        assert!((rotation.scale_factor() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn rotation_returns_angle() {
        let rotation =
            Transformation::rotate(Radians(std::f32::consts::FRAC_PI_2));

        assert!(
            (f32::from(rotation.rotation()) - std::f32::consts::FRAC_PI_2)
                .abs()
                < 0.001
        );
    }

    #[test]
    fn rotated_size_uses_positive_bounding_box() {
        let size = Size::new(20.0, 80.0)
            * Transformation::rotate(Radians(std::f32::consts::FRAC_PI_2));

        assert!((size.width - 80.0).abs() < 0.001);
        assert!((size.height - 20.0).abs() < 0.001);
    }

    #[test]
    fn rotated_rectangle_uses_positive_bounding_box() {
        let rectangle = Rectangle::new(
            Point::new(10.0, 20.0),
            Size::new(20.0, 80.0),
        ) * Transformation::rotate(Radians(std::f32::consts::FRAC_PI_2));

        assert!(rectangle.width > 0.0);
        assert!(rectangle.height > 0.0);
        assert!((rectangle.width - 80.0).abs() < 0.001);
        assert!((rectangle.height - 20.0).abs() < 0.001);
    }
}
