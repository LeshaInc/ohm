use std::borrow::Borrow;
use std::collections::{hash_map, HashMap};
use std::sync::Arc;

use freetype::bitmap::PixelMode;
use freetype::face::LoadFlag;
use freetype::{Face, Library, Matrix, Vector};
use ohm_core::image::{ImageData, ImageFormat};
use ohm_core::math::{UVec2, Vec2};
use ohm_core::text::{FontFace, FontId, GlyphId, RasterizedGlyph, Rasterizer, SubpixelBin};

struct FaceBuffer(Arc<dyn AsRef<[u8]> + Send + Sync + 'static>);

impl Borrow<[u8]> for FaceBuffer {
    fn borrow(&self) -> &[u8] {
        (*self.0).as_ref()
    }
}

pub struct FreetypeRasterizer {
    faces: HashMap<FontId, Face<FaceBuffer>>,
    library: Option<Library>,
}

impl FreetypeRasterizer {
    pub fn new() -> FreetypeRasterizer {
        FreetypeRasterizer {
            faces: HashMap::default(),
            library: Library::init().ok(),
        }
    }
}

impl Rasterizer for FreetypeRasterizer {
    fn rasterize(
        &mut self,
        font_face: &FontFace,
        glyph_id: GlyphId,
        size: f32,
        subpixel_bin: SubpixelBin,
    ) -> Option<RasterizedGlyph> {
        let face = match self.faces.entry(font_face.id()) {
            hash_map::Entry::Occupied(entry) => entry.into_mut(),
            hash_map::Entry::Vacant(entry) => {
                let face = self
                    .library
                    .as_mut()?
                    .new_memory_face2(
                        FaceBuffer(Arc::clone(font_face.data())),
                        font_face.face_index() as isize,
                    )
                    .ok()?;
                entry.insert(face)
            }
        };

        let size = (size * 64.0) as isize;
        face.set_char_size(size, size, 72, 72).ok()?;

        let mut matrix = Matrix {
            xx: 1 << 16,
            xy: 0,
            yx: 0,
            yy: 1 << 16,
        };

        let offset = subpixel_bin.offset() * 64.0;
        let mut delta = Vector {
            x: offset.x as _,
            y: offset.y as _,
        };

        face.set_transform(&mut matrix, &mut delta);
        face.load_glyph(glyph_id.0 as u32, LoadFlag::RENDER).ok()?;

        let glyph = face.glyph();
        let bitmap = glyph.bitmap();

        let offset = Vec2::new(glyph.bitmap_left() as f32, -glyph.bitmap_top() as f32);

        let (format, pixel_size) = match bitmap.pixel_mode().ok()? {
            PixelMode::Gray => (ImageFormat::Gray8, 1),
            _ => return None,
        };

        let pitch = bitmap.pitch().unsigned_abs() as usize;
        let width = bitmap.width() as usize;
        let height = bitmap.rows() as usize;

        if pitch == 0 || width == 0 || height == 0 {
            return None;
        }

        let data = bitmap
            .buffer()
            .chunks(pitch)
            .map(|chunk| chunk[..width * pixel_size].iter().copied());

        let reverse = bitmap.pitch() < 0;

        let data = if reverse {
            data.rev().flatten().collect::<Vec<_>>()
        } else {
            data.flatten().collect::<Vec<_>>()
        };

        let image = ImageData {
            format,
            size: UVec2::new(width as u32, height as u32),
            data,
        };

        Some(RasterizedGlyph { image, offset })
    }
}

impl Default for FreetypeRasterizer {
    fn default() -> Self {
        FreetypeRasterizer::new()
    }
}
