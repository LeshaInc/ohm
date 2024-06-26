use std::fmt;

use crate::image::ImageData;
use crate::math::Vec2;
use crate::text::{FontFace, FontId};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct GlyphKey {
    pub font: FontId,
    pub glyph: u16,
    pub size: u32,
    pub subpixel_bin: SubpixelBin,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct SubpixelBin {
    x: u8,
    y: u8,
}

impl SubpixelBin {
    pub fn new(pos: Vec2) -> SubpixelBin {
        let v = (pos.fract() * 4.0).floor();
        SubpixelBin {
            x: v.x as u8,
            y: v.y as u8,
        }
    }

    pub fn offset(self) -> Vec2 {
        Vec2::new((self.x as f32) / 4.0, (self.y as f32) / 4.0)
    }
}

#[derive(Debug, Clone)]
pub struct RasterizedGlyph {
    pub image: ImageData,
    pub offset: Vec2,
}

pub trait Rasterizer {
    fn rasterize(
        &mut self,
        font_face: &FontFace,
        glyph_id: u16,
        size: f32,
        subpixel_bin: SubpixelBin,
    ) -> Option<RasterizedGlyph>;
}

#[derive(Default)]
pub struct FontRasterizers {
    rasterizers: Vec<Box<dyn Rasterizer>>,
}

impl FontRasterizers {
    pub fn new() -> FontRasterizers {
        FontRasterizers::default()
    }

    pub fn add_rasterizer<R: Rasterizer + 'static>(&mut self, rasterizer: R) {
        self.rasterizers.push(Box::new(rasterizer));
    }
}

impl Rasterizer for FontRasterizers {
    fn rasterize(
        &mut self,
        font_face: &FontFace,
        glyph_id: u16,
        size: f32,
        subpixel_bin: SubpixelBin,
    ) -> Option<RasterizedGlyph> {
        for rasterizer in &mut self.rasterizers {
            if let Some(res) = rasterizer.rasterize(font_face, glyph_id, size, subpixel_bin) {
                return Some(res);
            }
        }

        None
    }
}

impl fmt::Debug for FontRasterizers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontRasterizers").finish_non_exhaustive()
    }
}
