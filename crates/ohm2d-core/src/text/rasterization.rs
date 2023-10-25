use std::fmt;

use ttf_parser::GlyphId;

use crate::math::{UVec2, Vec2};
use crate::text::{FontFace, FontId};
use crate::{ImageData, ImageFormat};

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

pub trait Rasterizer: Send + Sync + 'static {
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

    pub fn add<R: Rasterizer>(&mut self, rasterizer: R) {
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

pub struct EmbeddedImageRasterizer;

impl Rasterizer for EmbeddedImageRasterizer {
    fn rasterize(
        &mut self,
        font_face: &FontFace,
        glyph_id: u16,
        size: f32,
        _subpixel_bin: SubpixelBin,
    ) -> Option<RasterizedGlyph> {
        let face = font_face.ttfp_face();

        let raster =
            match face.glyph_raster_image(GlyphId(glyph_id), size.min(u16::MAX.into()) as u16) {
                Some(v) => v,
                None => return None,
            };

        let scale = size / (raster.pixels_per_em as f32);

        let mut image = image::load_from_memory(raster.data).ok()?.into_rgba8();

        let old_size = UVec2::new(image.width(), image.height());
        let size = (old_size.as_vec2() * scale).as_uvec2();

        if size.cmplt(old_size).any() {
            image = image::imageops::resize(
                &image,
                size.x,
                size.y,
                image::imageops::FilterType::Lanczos3,
            );
        }

        let data = image.into_raw();
        Some(RasterizedGlyph {
            image: ImageData {
                format: ImageFormat::Srgba8,
                size,
                data,
            },
            offset: Vec2::new(raster.x as f32, -(raster.height as f32) - (raster.y as f32)) * scale,
        })
    }
}
