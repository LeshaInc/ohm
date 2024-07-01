//! Types and traits related font loading, text shaping and layout and glyph
//! rasterization

mod buffer;
mod font;
mod font_db;
mod rasterization;
mod shaping;

pub use self::buffer::*;
pub use self::font::*;
pub use self::font_db::*;
pub use self::rasterization::*;
pub use self::shaping::*;
use crate::Color;

/// Attributes of a text section.
#[derive(Debug, Clone, PartialEq)]
pub struct TextAttrs {
    /// Font size in physical pixels.
    ///
    /// Note that Ohm doesn't do DPI scaling, so all coordinates and dimensions
    /// are in physical pixels (unaffected by DPI scaling factor).
    ///
    /// Default: `16.0`.
    pub size: f32,

    /// Text color.
    ///
    /// Default: [`Color::BLACK`].
    pub color: Color,

    /// Horizontal text alignment.
    ///
    /// Default: [`TextAlign::Start`].
    pub align: TextAlign,

    /// List of font families in fallback order.
    ///
    /// Default: sans-serif.
    pub fonts: FontFamilies,

    /// Font weight (in other words, how bold it is).
    ///
    /// Default: [`FontWeight::NORMAL`].
    pub weight: FontWeight,

    /// Font width (in other words, how wide it is).
    ///
    /// Default: [`FontWidth::Normal`].
    pub width: FontWidth,

    /// Font style (normal, italic, oblique).
    ///
    /// Default: [`FontStyle::Normal`].
    pub style: FontStyle,

    /// Line height. Similar to the CSS property, can be relative or absolute.
    /// Commonly used to set the distance between lines.
    ///
    /// Default: `1.2` (relative).
    pub line_height: LineHeight,
}

impl Default for TextAttrs {
    fn default() -> Self {
        Self {
            size: 16.0,
            color: Color::BLACK,
            align: TextAlign::Start,
            fonts: FontFamilies::new(FontFamily::sans_serif()),
            weight: FontWeight::NORMAL,
            width: FontWidth::Normal,
            style: FontStyle::Normal,
            line_height: LineHeight::Relative(1.2),
        }
    }
}

/// Horizontal text alignment.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum TextAlign {
    /// Start of the line (left in LTR, right in RTL).
    #[default]
    Start,
    /// End of the line (right in LTR, left in RTL).
    End,
    /// Left side of the line (regardless of language direction).
    Left,
    /// Right side of the line (regardless of language direction).
    Right,
    /// Centered.
    Center,
    /// Justified. Spaces between words are stretched to fill the entire width,
    /// unless there is a forced newline.
    Justify,
}

/// Height of a line box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineHeight {
    /// Fixed line height regardless of font size.
    Px(f32),
    /// Relative to font size. Final height is measured by multiplying the
    /// factor by font size.
    Relative(f32),
}

impl Default for LineHeight {
    fn default() -> LineHeight {
        LineHeight::Relative(1.2)
    }
}
