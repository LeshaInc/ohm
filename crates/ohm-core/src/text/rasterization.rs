use std::fmt;

use crate::image::ImageData;
use crate::math::Vec2;
use crate::text::{FontFace, FontId, GlyphId};

/// A key which represents a specific glyph rendered at a specific size and
/// subpixel offset
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct GlyphKey {
    /// ID of the font face.
    pub font: FontId,
    /// ID of the glyph in the font face.
    pub glyph: GlyphId,
    /// Size of the glyph, represented as an encoded `f32`.
    ///
    /// To convert to/from `f32`, use [`f32::to_bits`] and [`f32::from_bits`].
    pub size: u32,
    /// Quantized subpixel offset.
    pub subpixel_bin: SubpixelBin,
}

/// Quantized subpixel offset.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct SubpixelBin {
    x: u8,
    y: u8,
}

impl SubpixelBin {
    /// Returns subpixel offset, quantized to 4 positions per axis from the
    /// given position.
    pub fn new(pos: Vec2) -> SubpixelBin {
        let v = (pos.fract() * 4.0).floor();
        SubpixelBin {
            x: v.x as u8,
            y: v.y as u8,
        }
    }

    /// Returns the subpixel offset as a 2D vector.
    pub fn offset(self) -> Vec2 {
        Vec2::new((self.x as f32) / 4.0, (self.y as f32) / 4.0)
    }
}

/// A rasterized glyph's data.
#[derive(Debug, Clone)]
pub struct RasterizedGlyph {
    /// The image.
    pub image: ImageData,
    /// Offset from origin, at which the image should be rendered.
    pub offset: Vec2,
}

/// A glyph rasterizer. Takes information from a font face and returns a raster
/// image for a given glyph at a given size and subpixel offset.
pub trait Rasterizer {
    /// Rasterizes a given glyph, returning `None` if there is no such glyph, or
    /// if the glyph is whitespace.
    fn rasterize(
        &mut self,
        font_face: &FontFace,
        glyph_id: GlyphId,
        size: f32,
        subpixel_bin: SubpixelBin,
    ) -> Option<RasterizedGlyph>;
}

/// A list of [`FontRasterizer`]'s, that tries to rasterize the given glyph
/// using one of the underlying rasterizers, trying them in order.
#[derive(Default)]
pub struct FontRasterizers {
    rasterizers: Vec<Box<dyn Rasterizer>>,
}

impl FontRasterizers {
    /// Creates an empty list of [`FontRasterizer`]'s.
    pub fn new() -> FontRasterizers {
        FontRasterizers::default()
    }

    /// Adds a [`FontRasterier`] to the list.
    pub fn add_rasterizer<R: Rasterizer + 'static>(&mut self, rasterizer: R) {
        self.rasterizers.push(Box::new(rasterizer));
    }
}

impl Rasterizer for FontRasterizers {
    fn rasterize(
        &mut self,
        font_face: &FontFace,
        glyph_id: GlyphId,
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
