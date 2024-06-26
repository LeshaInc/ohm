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

#[derive(Debug, Clone, PartialEq)]
pub struct TextAttrs {
    pub size: f32,
    pub align: TextAlign,
    pub fonts: FontFamilies,
    pub weight: FontWeight,
    pub width: FontWidth,
    pub style: FontStyle,
    pub line_height: LineHeight,
}

impl Default for TextAttrs {
    fn default() -> Self {
        Self {
            size: 16.0,
            align: Default::default(),
            fonts: Default::default(),
            weight: Default::default(),
            width: Default::default(),
            style: Default::default(),
            line_height: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum TextAlign {
    #[default]
    Start,
    End,
    Left,
    Right,
    Center,
    Justify,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineHeight {
    Px(f32),
    Relative(f32),
}

impl Default for LineHeight {
    fn default() -> LineHeight {
        LineHeight::Relative(1.2)
    }
}
