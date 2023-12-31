use std::ops::Range;

use crate::math::{Rect, UVec2, Vec2, Vec4};
use crate::text::{GlyphKey, SubpixelBin};
use crate::{
    Color, Command, CornerRadii, DrawGlyph, DrawLayer, DrawList, DrawRect, Fill, ImageFormat,
    TextureCache, TextureCommand, TextureId,
};

slotmap::new_key_type! {
    pub struct SurfaceId;
}

pub trait Renderer {
    fn get_surface_size(&self, surface: SurfaceId) -> UVec2;

    fn update_textures(&mut self, commands: &[TextureCommand]);

    fn render(&mut self, texture_cache: &TextureCache, draw_lists: &[DrawList<'_>]);

    fn present(&mut self);
}

pub const INSTANCE_FILL: u32 = 4294967295;
pub const INSTANCE_FILL_GRAY: u32 = 4294967294;

#[repr(packed)]
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub pos: Vec2,
    pub tex: Vec2,
    pub color: Vec4,
    pub instance_id: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Instance {
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

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub struct FramebufferId(pub u64);

#[derive(Debug, Default)]
pub struct BatcherScratch {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    instances: Vec<Instance>,
    batches: Vec<Batch>,
}

impl BatcherScratch {
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.instances.clear();
        self.batches.clear();
    }
}

#[derive(Debug)]
pub struct Batcher<'a> {
    texture_cache: &'a TextureCache,
    vertices: &'a mut Vec<Vertex>,
    indices: &'a mut Vec<u32>,
    instances: &'a mut Vec<Instance>,
    batches: &'a mut Vec<Batch>,
    cur_surface: SurfaceId,
    cur_surface_size: UVec2,
    cur_framebuffer: FramebufferId,
    cur_source: Source,
    max_instances_per_buffer: usize,
    cur_instance_buffer_id: usize,
    last_index: u32,
}

#[derive(Debug)]
pub struct Batch {
    pub surface: SurfaceId,
    pub framebuffer: FramebufferId,
    pub source: Source,
    pub index_range: Range<u32>,
    pub instance_buffer_id: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Source {
    White,
    Texture(TextureId),
    Framebuffer(SurfaceId, FramebufferId),
}

impl Batcher<'_> {
    pub fn new<'a>(
        scratch: &'a mut BatcherScratch,
        texture_cache: &'a TextureCache,
        max_instances_per_buffer: usize,
    ) -> Batcher<'a> {
        scratch.clear();
        Batcher {
            texture_cache,
            vertices: &mut scratch.vertices,
            indices: &mut scratch.indices,
            instances: &mut scratch.instances,
            batches: &mut scratch.batches,
            cur_surface: SurfaceId::default(),
            cur_surface_size: UVec2::ZERO,
            cur_framebuffer: FramebufferId(0),
            cur_source: Source::White,
            max_instances_per_buffer,
            cur_instance_buffer_id: 0,
            last_index: 0,
        }
    }

    pub fn prepare(&mut self, surface_size: UVec2, draw_list: &DrawList) {
        if draw_list.commands.is_empty() {
            return;
        }

        self.cur_surface = draw_list.surface;
        self.cur_surface_size = surface_size;
        self.dispatch_commands(draw_list.commands);
    }

    pub fn batches(&self) -> &[Batch] {
        &self.batches
    }

    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    pub fn instances(&self) -> &[Instance] {
        &self.instances
    }

    fn compute_bouding_rect(&self, batch_range: Range<usize>) -> Option<Rect> {
        let mut rect: Option<Rect> = None;

        for batch_idx in batch_range {
            for idx in self.batches[batch_idx as usize].index_range.clone() {
                let idx = self.indices[idx as usize];
                let pos = self.vertices[idx as usize].pos;
                if let Some(rect) = &mut rect {
                    rect.min = rect.min.min(pos);
                    rect.max = rect.max.max(pos);
                } else {
                    rect = Some(Rect::new(pos, pos));
                }
            }
        }

        rect
    }

    fn dispatch_commands(&mut self, commands: &[Command<'_>]) -> Range<usize> {
        let first_batch = self.batches.len();

        for &command in commands {
            match command {
                Command::DrawRect(rect) => self.cmd_draw_rect(rect),
                Command::DrawGlyph(glyph) => self.cmd_draw_glyph(glyph),
                Command::DrawLayer(layer) => self.cmd_draw_layer(layer),
            }
        }

        self.flush();

        first_batch..self.batches.len()
    }

    fn cmd_draw_rect(&mut self, rect: DrawRect) {
        let color = match rect.fill {
            Fill::Solid(color) => color,
            Fill::Image(fill) => fill.tint,
        };

        let (source, mut tex_min, mut tex_max) = match rect.fill {
            Fill::Image(fill) => self
                .texture_cache
                .get_image(fill.image)
                .map(|image| {
                    let tex_min = image.rect.min.as_vec2() / image.texture_size.as_vec2();
                    let tex_max = image.rect.max.as_vec2() / image.texture_size.as_vec2();
                    (Source::Texture(image.texture), tex_min, tex_max)
                })
                .unwrap_or((Source::White, Vec2::ZERO, Vec2::ZERO)),
            _ => (Source::White, Vec2::ZERO, Vec2::ZERO),
        };

        self.set_source(source);

        if rect.border.is_none()
            && rect.shadow.is_none()
            && rect.corner_radii == CornerRadii::default()
        {
            self.add_quad(
                rect.pos,
                rect.pos + rect.size,
                tex_min,
                tex_max,
                color,
                INSTANCE_FILL,
            );

            return;
        }

        let shadow_offset = rect.shadow.map(|s| s.offset).unwrap_or(Vec2::ZERO);
        let shadow_blur_radius = rect.shadow.map(|s| s.blur_radius).unwrap_or(0.0);
        let shadow_spread_radius = rect.shadow.map(|s| s.spread_radius).unwrap_or(0.0);

        let instance_id = self.add_instance(Instance {
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

        self.add_quad(min, max, tex_min, tex_max, color, instance_id);
    }

    fn cmd_draw_glyph(&mut self, glyph: DrawGlyph) {
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

        self.set_source(Source::Texture(glyph.texture));

        let tex_min = glyph.rect.min.as_vec2() / glyph.texture_size.as_vec2();
        let tex_max = glyph.rect.max.as_vec2() / glyph.texture_size.as_vec2();

        let pos = pos.trunc() + glyph.offset;
        let size = glyph.rect.size().as_vec2();

        let (color, instance_id) = if glyph.format == ImageFormat::Gray8 {
            (color, INSTANCE_FILL_GRAY)
        } else {
            (Color::WHITE, INSTANCE_FILL)
        };

        self.add_quad(pos, pos + size, tex_min, tex_max, color, instance_id);
    }

    fn cmd_draw_layer(&mut self, layer: DrawLayer<'_>) {
        let is_no_tint = layer.tint == Color::WHITE;
        let is_compatible_scissor = layer.scissor.is_none();

        let is_fast_path = is_no_tint && is_compatible_scissor;

        if is_fast_path {
            self.dispatch_commands(layer.commands);
            return;
        }

        self.flush();

        let old_framebuffer = self.cur_framebuffer;
        self.cur_framebuffer.0 += 1;

        let batch_range = self.dispatch_commands(layer.commands);

        let Some(rect) = self.compute_bouding_rect(batch_range) else {
            return;
        };

        self.cur_source = Source::Framebuffer(self.cur_surface, self.cur_framebuffer);
        self.cur_framebuffer = old_framebuffer;

        let tex_min = rect.min / self.cur_surface_size.as_vec2();
        let tex_max = rect.max / self.cur_surface_size.as_vec2();

        self.add_quad(
            rect.min,
            rect.max,
            tex_min,
            tex_max,
            layer.tint,
            INSTANCE_FILL,
        );
    }

    fn flush(&mut self) {
        let index_range = self.last_index..self.indices.len() as u32;
        if index_range.is_empty() {
            return;
        }

        self.last_index = index_range.end;
        self.batches.push(Batch {
            surface: self.cur_surface,
            framebuffer: self.cur_framebuffer,
            source: self.cur_source,
            index_range,
            instance_buffer_id: self.cur_instance_buffer_id,
        });
    }

    fn set_source(&mut self, source: Source) {
        if self.cur_source != source {
            self.flush();
        }

        self.cur_source = source;
    }

    fn add_vertex(&mut self, pos: Vec2, tex: Vec2, color: Color, instance_id: u32) -> u32 {
        let idx = self.vertices.len() as u32;

        self.vertices.push(Vertex {
            pos,
            tex,
            color: color.into(),
            instance_id,
        });

        idx
    }

    fn add_instance(&mut self, instance: Instance) -> u32 {
        if !self.instances.is_empty() && self.instances.len() % self.max_instances_per_buffer == 0 {
            self.flush();
            self.cur_instance_buffer_id += 1;
        }

        let idx = self.instances.len() as u32;
        self.instances.push(instance);
        idx
    }

    fn add_quad(
        &mut self,
        min: Vec2,
        max: Vec2,
        tex_min: Vec2,
        tex_max: Vec2,
        color: Color,
        instance_id: u32,
    ) {
        let a = self.add_vertex(
            Vec2::new(min.x, min.y),
            Vec2::new(tex_min.x, tex_min.y),
            color,
            instance_id,
        );
        let b = self.add_vertex(
            Vec2::new(max.x, min.y),
            Vec2::new(tex_max.x, tex_min.y),
            color,
            instance_id,
        );
        let c = self.add_vertex(
            Vec2::new(max.x, max.y),
            Vec2::new(tex_max.x, tex_max.y),
            color,
            instance_id,
        );
        let d = self.add_vertex(
            Vec2::new(min.x, max.y),
            Vec2::new(tex_min.x, tex_max.y),
            color,
            instance_id,
        );

        self.indices.extend_from_slice(&[a, b, c, c, d, a]);
    }
}
