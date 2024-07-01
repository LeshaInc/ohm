use crate::math::Vec4;

/// Color in linear sRGB color space with premultiplied alpha.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    /// Red channel. Range: `0..=1`.
    pub r: f32,
    /// Green channel. Range: `0..=1`.
    pub g: f32,
    /// Blue channel. Range: `0..=1`.
    pub b: f32,
    /// Alpha channel. Range: `0..=1`.
    pub a: f32,
}

impl Color {
    /// Pure white color.
    pub const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);
    /// Pure black color.
    pub const BLACK: Color = Color::rgb(0.0, 0.0, 0.0);
    /// Fully transparent color.
    pub const TRANSPAENT: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);

    /// Creates a color given an RGBA fourtuplet in linear sRGB color space.
    ///
    /// Color components (`RGB`) are assumed to be premultiplied by alpha.
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color { r, g, b, a }
    }

    /// Creates a color given an RGB triplet in linear sRGB color space.
    ///
    /// Alpha is set to `1`.
    pub const fn rgb(r: f32, g: f32, b: f32) -> Color {
        Color::rgba(r, g, b, 1.0)
    }
}

impl From<Color> for Vec4 {
    fn from(c: Color) -> Vec4 {
        Vec4::new(c.r, c.g, c.b, c.a)
    }
}
