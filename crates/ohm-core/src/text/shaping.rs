use crate::math::Vec2;
use crate::text::{FontFace, GlyphId};

/// A shaped glyph, retrieved after text shaping.
#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    /// ID of the glyph in the provided font face.
    pub glyph_id: GlyphId,
    /// Unicode grapheme cluster (index of the first byte) in the provided text.
    pub cluster: usize,
    /// Horizontal advance from this origin to the next origin.
    pub x_advance: f32,
    /// Offset to draw the glyph at relative to the origin.
    pub offset: Vec2,
}

/// A text shaper responsible for choosing the right glyphs and placing them in
/// the right places.
///
/// Does things like replacing the letters "fl" with a corresponding ligature in
/// european scripts.
///
/// In complex scripts, such as arabic, performs context sensitive shaping,
/// where a single character can be one of many glyphs, depending on the
/// surrounding context.
///
/// In some cases, a single character may also turn into muliple glyphs. For
/// example, the letter "ü" could turn into two glyphs: "u" and a diacritical
/// mark "¨".
pub trait TextShaper: Send + Sync + 'static {
    /// Performs the text shaping.
    ///
    /// Parameters:
    /// - `font_face` - the font face to glyphs from
    /// - `text` - input text
    /// - `size` - font size in pixels
    /// - `is_rtl` - whether the input text should be written right-to-left.
    ///  - `buf` - the output buffer
    fn shape(
        &mut self,
        font_face: &FontFace,
        text: &str,
        size: f32,
        is_rtl: bool,
        buf: &mut Vec<ShapedGlyph>,
    );
}

/// A dummy text shaper, which just panics when calling [`TextShaper::shape`].
#[derive(Debug, Copy, Clone, Default)]
pub struct DummyTextShaper;

impl TextShaper for DummyTextShaper {
    fn shape(
        &mut self,
        _font_face: &FontFace,
        _text: &str,
        _size: f32,
        _is_rtl: bool,
        _buf: &mut Vec<ShapedGlyph>,
    ) {
        unimplemented!()
    }
}
