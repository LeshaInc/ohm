use crate::math::Vec2;
use crate::text::FontFace;

#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    pub glyph_id: u16,
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
