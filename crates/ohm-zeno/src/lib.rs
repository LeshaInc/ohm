use ohm_core::image::{ImageData, ImageFormat};
use ohm_core::math::{IVec2, UVec2};
use ohm_core::text::{FontFace, GlyphId, RasterizedGlyph, Rasterizer, SubpixelBin};
use zeno::{Command, Format, Mask, PathBuilder, Scratch, Transform};

#[derive(Default)]
pub struct ZenoRasterizer {
    scratch: Scratch,
    command_buffer: Vec<Command>,
}

impl ZenoRasterizer {
    pub fn new() -> ZenoRasterizer {
        ZenoRasterizer::default()
    }
}

impl Rasterizer for ZenoRasterizer {
    fn rasterize(
        &mut self,
        font_face: &FontFace,
        glyph_id: GlyphId,
        size: f32,
        subpixel_bin: SubpixelBin,
    ) -> Option<RasterizedGlyph> {
        self.command_buffer.clear();

        font_face.ttfp_face().outline_glyph(
            glyph_id,
            &mut Outliner {
                buf: &mut self.command_buffer,
            },
        );

        let scale = size / (font_face.metrics().units_per_em as f32);
        let offset = subpixel_bin.offset();

        let (data, placement) = Mask::with_scratch(&self.command_buffer[..], &mut self.scratch)
            .transform(Some(
                Transform::scale(scale, scale).then_translate(offset.x, offset.y),
            ))
            .format(Format::Alpha)
            .render();

        if data.is_empty() {
            return None;
        }

        let data = data
            .chunks(placement.width as usize)
            .rev()
            .flatten()
            .map(|&v| ((v as f32 / 255.0).powf(0.5) * 255.0) as u8)
            // .copied()
            .collect::<Vec<_>>();

        let offset =
            IVec2::new(placement.left, -(placement.height as i32) - placement.top).as_vec2();
        let image = ImageData {
            size: UVec2::new(placement.width, placement.height),
            format: ImageFormat::Gray8,
            data,
        };

        Some(RasterizedGlyph { image, offset })
    }
}

struct Outliner<'a> {
    buf: &'a mut Vec<Command>,
}

impl ttf_parser::OutlineBuilder for Outliner<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.buf.move_to([x, y]);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.buf.line_to([x, y]);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.buf.quad_to([x1, y1], [x, y]);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.buf.curve_to([x1, y1], [x2, y2], [x, y]);
    }

    fn close(&mut self) {
        self.buf.close();
    }
}
