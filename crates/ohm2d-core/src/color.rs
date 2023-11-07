use crate::math::Vec4;

/// Color in linear sRGB color space with premultiplied alpha
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);
    pub const BLACK: Color = Color::rgb(0.0, 0.0, 0.0);
    pub const TRANSPAENT: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color { r, g, b, a }
    }

    pub const fn rgb(r: f32, g: f32, b: f32) -> Color {
        Color::rgba(r, g, b, 1.0)
    }
}

impl From<Color> for Vec4 {
    fn from(c: Color) -> Vec4 {
        Vec4::new(c.r, c.g, c.b, c.a)
    }
}
