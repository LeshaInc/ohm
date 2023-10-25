use std::ops::Range;

use crate::math::{vec2, Vec2, Vec4};
use crate::text::{GlyphKey, SubpixelBin};
use crate::{
    Color, Command, CornerRadii, DrawGlyph, DrawList, DrawRect, Fill, ImageFormat, SurfaceId,
    TextureCache, TextureId,
};

pub struct BatchList<'a> {
    pub batches: &'a [Batch],
    pub vertices: &'a [Vertex],
    pub indices: &'a [u32],
    pub rect_instances: &'a [RectInstance],
}

#[derive(Debug, Clone)]
pub struct Batch {
    pub surface: SurfaceId,
    pub texture: Option<TextureId>,
    pub clear_color: Option<Color>,
    pub shader_kind: ShaderKind,
    pub index_range: Range<u32>,
    pub rect_instances_buffer: usize,
}

#[derive(Debug, Clone, Copy)]
pub enum TextureBinding {
    White,
    Offscreen(u8),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ShaderKind {
    Uber,
}

#[repr(packed)]
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub pos: Vec2,
    pub tex: Vec2,
    pub color: Vec4,
    pub rect_id: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, encase::ShaderType)]
pub struct RectInstance {
    pub corner_radii: Vec4,
    pub border_color: Vec4,
    pub shadow_color: Vec4,
    pub shadow_offset: Vec2,
    pub pos: Vec2,
    pub size: Vec2,
    pub border_width: f32,
    pub shadow_blur_radius: f32,
    pub shadow_spread_radius: f32,
}

#[derive(Debug)]
pub struct Batcher {
    batches: Vec<Batch>,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    rect_instances: Vec<RectInstance>,
    max_instances_per_buffer: usize,
}

impl Batcher {
    pub fn new(max_instances_per_buffer: usize) -> Batcher {
        Batcher {
            batches: Vec::new(),
            vertices: Vec::new(),
            indices: Vec::new(),
            rect_instances: Vec::new(),
            max_instances_per_buffer,
        }
    }

    pub fn prepare(&mut self, texture_cache: &TextureCache, draw_lists: &[DrawList]) -> BatchList {
        self.batches.clear();
        self.vertices.clear();
        self.indices.clear();
        self.rect_instances.clear();

        for draw_list in draw_lists {
            let mut context = BatcherContext {
                texture_cache,
                max_instances_per_buffer: self.max_instances_per_buffer,
                last_index: 0,
                cur_clear_color: None,
                cur_rect_instance_buffer: 0,
                cur_texture: None,
                cur_surface: draw_list.surface,
                cur_shader_kind: ShaderKind::Uber,
                batches: &mut self.batches,
                vertices: &mut self.vertices,
                indices: &mut self.indices,
                rect_instances: &mut self.rect_instances,
            };

            context.prepare(draw_list.commands);
        }

        BatchList {
            batches: &self.batches,
            vertices: &self.vertices,
            indices: &self.indices,
            rect_instances: &self.rect_instances,
        }
    }
}

struct BatcherContext<'a> {
    texture_cache: &'a TextureCache,
    max_instances_per_buffer: usize,
    last_index: u32,
    cur_clear_color: Option<Color>,
    cur_rect_instance_buffer: usize,
    cur_texture: Option<TextureId>,
    cur_surface: SurfaceId,
    cur_shader_kind: ShaderKind,
    batches: &'a mut Vec<Batch>,
    vertices: &'a mut Vec<Vertex>,
    indices: &'a mut Vec<u32>,
    rect_instances: &'a mut Vec<RectInstance>,
}

impl BatcherContext<'_> {
    fn prepare(&mut self, commands: &[Command]) {
        let (skip, clear_color) = commands
            .iter()
            .enumerate()
            .flat_map(|(i, &command)| match command {
                Command::Clear(color) => Some((i + 1, Some(color))),
                _ => None,
            })
            .last()
            .unwrap_or((0, None));

        self.cur_clear_color = clear_color;

        for command in &commands[skip..] {
            match command {
                Command::DrawRect(rect) => self.cmd_draw_rect(rect),
                Command::DrawGlyph(glyph) => self.cmd_draw_glyph(glyph),
                Command::BeginAlpha(alpha) => self.cmd_begin_alpha(*alpha),
                Command::EndAlpha => self.cmd_end_alpha(),
                _ => {}
            }
        }

        self.flush();
    }

    fn cmd_draw_rect(&mut self, rect: &DrawRect) {
        let color = match rect.fill {
            Fill::Solid(color) => color,
            Fill::Image(fill) => fill.tint,
        };

        let (texture, mut tex_min, mut tex_max) = match rect.fill {
            Fill::Image(fill) => self
                .texture_cache
                .get_image(fill.image)
                .map(|image| {
                    let tex_min = image.rect.min.as_vec2() / image.texture_size.as_vec2();
                    let tex_max = image.rect.max.as_vec2() / image.texture_size.as_vec2();
                    (Some(image.texture), tex_min, tex_max)
                })
                .unwrap_or((None, Vec2::ZERO, Vec2::ZERO)),
            _ => (None, Vec2::ZERO, Vec2::ZERO),
        };

        self.set_texture(texture);

        if rect.border.is_none()
            && rect.shadow.is_none()
            && rect.corner_radii == CornerRadii::default()
        {
            self.rect(rect.pos, rect.size, color, tex_min, tex_max, u32::MAX);
            return;
        }

        let shadow_offset = rect.shadow.map(|s| s.offset).unwrap_or(vec2(0.0, 0.0));
        let shadow_blur_radius = rect.shadow.map(|s| s.blur_radius).unwrap_or(0.0);
        let shadow_spread_radius = rect.shadow.map(|s| s.spread_radius).unwrap_or(0.0);

        let rect_id = self.rect_instance(RectInstance {
            corner_radii: rect.corner_radii.into(),
            border_color: rect.border.map(|b| b.color).unwrap_or(color).into(),
            shadow_color: rect.shadow.map(|s| s.color).unwrap_or(color).into(),
            shadow_offset,
            pos: rect.pos,
            size: rect.size,
            border_width: rect.border.map(|b| b.width).unwrap_or(0.0),
            shadow_blur_radius,
            shadow_spread_radius,
        });

        let rect_min = rect.pos;
        let rect_max = rect.pos + rect.size;

        let shadow_radius = Vec2::splat(shadow_blur_radius + shadow_spread_radius);

        let shadow_min = rect_min - shadow_radius + shadow_offset;
        let shadow_max = rect_max + shadow_radius + shadow_offset;

        let min = shadow_min.min(rect_min);
        let max = shadow_max.max(rect_max);

        let tex_size = tex_max - tex_min;
        tex_min -= (rect_min - min) * tex_size / rect.size;
        tex_max += (max - rect_max) * tex_size / rect.size;

        self.rect(min, max - min, color, tex_min, tex_max, rect_id);
    }

    fn cmd_draw_glyph(&mut self, glyph: &DrawGlyph) {
        let color = glyph.color;
        let pos = glyph.pos;

        let glyph_key = GlyphKey {
            font: glyph.font,
            glyph: glyph.glyph,
            size: glyph.size.to_bits(),
            subpixel_bin: SubpixelBin::new(pos),
        };

        let Some(glyph) = self.texture_cache.get_glyph(&glyph_key) else {
            return;
        };

        self.set_texture(Some(glyph.texture));

        let tex_min = glyph.rect.min.as_vec2() / glyph.texture_size.as_vec2();
        let tex_max = glyph.rect.max.as_vec2() / glyph.texture_size.as_vec2();

        let pos = pos.trunc() + glyph.offset;
        let size = glyph.rect.size().as_vec2();

        let (color, rect_id) = if glyph.format == ImageFormat::Gray8 {
            (color, u32::MAX - 1)
        } else {
            (Color::WHITE, u32::MAX)
        };

        self.rect(pos, size, color, tex_min, tex_max, rect_id);
    }

    fn cmd_begin_alpha(&mut self, _alpha: f32) {
        todo!()
    }

    fn cmd_end_alpha(&mut self) {
        todo!()
    }

    fn flush(&mut self) {
        let index_range = self.last_index..self.indices.len() as u32;
        if index_range.is_empty() {
            return;
        }

        self.last_index = index_range.end;

        self.batches.push(Batch {
            surface: self.cur_surface,
            texture: self.cur_texture,
            clear_color: self.cur_clear_color,
            shader_kind: self.cur_shader_kind,
            index_range,
            rect_instances_buffer: self.cur_rect_instance_buffer,
        });

        self.cur_clear_color = None;
    }

    fn set_texture(&mut self, texture: Option<TextureId>) {
        if self.cur_texture != texture {
            self.flush();
        }

        self.cur_texture = texture;
    }

    // fn set_shader_kind(&mut self, shader_kind: ShaderKind) {
    //     if self.cur_shader_kind != shader_kind {
    //         self.flush();
    //     }

    //     self.cur_shader_kind = shader_kind;
    // }

    fn rect(
        &mut self,
        pos: Vec2,
        size: Vec2,
        color: Color,
        tex_min: Vec2,
        tex_max: Vec2,
        rect_id: u32,
    ) {
        let color = color.into();

        let a = self.vertex(pos, tex_min, color, rect_id);

        let b = self.vertex(
            pos + vec2(size.x, 0.0),
            vec2(tex_max.x, tex_min.y),
            color,
            rect_id,
        );

        let c = self.vertex(pos + size, tex_max, color, rect_id);

        let d = self.vertex(
            pos + vec2(0.0, size.y),
            vec2(tex_min.x, tex_max.y),
            color,
            rect_id,
        );

        self.quad_indices(a, b, c, d)
    }

    fn vertex(&mut self, pos: Vec2, tex: Vec2, color: Vec4, rect_id: u32) -> u32 {
        let idx = self.vertices.len() as u32;

        self.vertices.push(Vertex {
            pos,
            tex,
            color,
            rect_id,
        });

        idx
    }

    fn rect_instance(&mut self, instance: RectInstance) -> u32 {
        if !self.rect_instances.is_empty()
            && self.rect_instances.len() % self.max_instances_per_buffer == 0
        {
            self.flush();
            self.cur_rect_instance_buffer += 1;
        }

        let idx = (self.rect_instances.len() % self.max_instances_per_buffer) as u32;
        self.rect_instances.push(instance);
        idx
    }

    fn quad_indices(&mut self, a: u32, b: u32, c: u32, d: u32) {
        self.indices.extend_from_slice(&[a, b, c, c, d, a]);
    }
}
