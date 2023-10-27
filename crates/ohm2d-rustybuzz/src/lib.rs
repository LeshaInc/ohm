use std::collections::{hash_map, HashMap};
use std::sync::Arc;

use ohm2d_core::math::IVec2;
use ohm2d_core::text::{FontFace, FontId, ShapedGlyph, TextShaper};
use rustybuzz::{Direction, Face, UnicodeBuffer};

self_cell::self_cell! {
    struct CachedFace {
        owner: Arc<dyn AsRef<[u8]> + Send + Sync>,
        #[covariant]
        dependent: Face,
    }
}

#[derive(Default)]
pub struct RustybuzzShaper {
    buffer: UnicodeBuffer,
    faces: HashMap<FontId, CachedFace>,
}

impl RustybuzzShaper {
    pub fn new() -> RustybuzzShaper {
        RustybuzzShaper::default()
    }
}

impl TextShaper for RustybuzzShaper {
    fn shape(
        &mut self,
        font_face: &FontFace,
        text: &str,
        size: f32,
        is_rtl: bool,
        buf: &mut Vec<ShapedGlyph>,
    ) {
        let face = match self.faces.entry(font_face.id()) {
            hash_map::Entry::Occupied(v) => v.into_mut().borrow_dependent(),
            hash_map::Entry::Vacant(v) => {
                let index = font_face.face_index();
                let cached_face = match CachedFace::try_new(Arc::clone(font_face.data()), |data| {
                    rustybuzz::ttf_parser::Face::parse((**data).as_ref(), index)
                        .map(Face::from_face)
                }) {
                    Ok(v) => v,
                    Err(_) => {
                        return;
                    }
                };

                v.insert(cached_face).borrow_dependent()
            }
        };

        let scale = size / face.units_per_em() as f32;

        let mut buffer = std::mem::take(&mut self.buffer);
        buffer.clear();
        buffer.push_str(text);
        buffer.guess_segment_properties();

        buffer.set_direction(if is_rtl {
            Direction::RightToLeft
        } else {
            Direction::LeftToRight
        });

        let glyphs = rustybuzz::shape(&face, &[], buffer);

        let it = glyphs.glyph_infos().iter().zip(glyphs.glyph_positions());
        buf.extend(it.map(|(info, pos)| ShapedGlyph {
            glyph_id: info.glyph_id as u16,
            x_advance: (pos.x_advance as f32) * scale,
            offset: IVec2::new(pos.x_offset, -pos.y_offset).as_vec2() * scale,
            cluster: info.cluster as usize,
        }));

        let start = buf.len() - glyphs.len();

        if is_rtl {
            buf[start..].reverse();
        }

        self.buffer = glyphs.clear();
    }
}
