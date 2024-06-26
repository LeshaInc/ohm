use crate::math::Vec2;
use crate::text::{FontFace, GlyphId};

#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    pub glyph_id: GlyphId,
    pub cluster: usize,
    pub x_advance: f32,
    pub offset: Vec2,
}

pub trait TextShaper: Send + Sync + 'static {
    fn shape(
        &mut self,
        font_face: &FontFace,
        text: &str,
        size: f32,
        is_rtl: bool,
        buf: &mut Vec<ShapedGlyph>,
    );
}

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
