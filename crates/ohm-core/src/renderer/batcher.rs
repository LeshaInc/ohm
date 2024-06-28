use std::fmt;
use std::ops::Range;

use super::path_cache::{Mesh, PathCache};
use super::SurfaceId;
use crate::image::ImageFormat;
use crate::math::{Affine2, Rect, UVec2, Vec2, Vec4};
use crate::text::{GlyphKey, SubpixelBin};
use crate::texture::{TextureCache, TextureId};
use crate::{
    ClearRect, Color, Command, CornerRadii, DrawGlyph, DrawLayer, DrawList, DrawRect, Fill,
    FillPath, StrokePath,
};

pub const INSTANCE_FILL: u32 = 4294967295;
pub const INSTANCE_FILL_GRAY: u32 = 4294967294;

#[repr(packed)]
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub pos: Vec2,
    pub local_pos: Vec2,
    pub tex: Vec2,
    pub color: Vec4,
    pub instance_id: u32,
}

#[derive(Debug, Clone, Copy, Default)]
struct Quad {
    min: Vec2,
    max: Vec2,
    local_min: Vec2,
    local_max: Vec2,
    tex_min: Vec2,
    tex_max: Vec2,
    color: Vec4,
    instance_id: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Instance {
    pub corner_radii: Vec4,
    pub border_color: Vec4,
    pub shadow_color: Vec4,
    pub shadow_offset: Vec2,
    pub size: Vec2,
    pub border_width: f32,
    pub shadow_blur_radius: f32,
    pub shadow_spread_radius: f32,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub struct FramebufferId(pub u64);

#[derive(Debug)]
pub struct Batch {
    pub clear: bool,
    pub msaa_resolve: bool,
    pub target: Target,
    pub source: Source,
    pub index_range: Range<u32>,
    pub vertex_range: Range<u32>,
    pub instance_buffer_id: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Intermediate {
    pub size: UVec2,
    pub msaa: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct IntermediateId(pub usize);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Target {
    Surface(SurfaceId),
    Intermediate(IntermediateId),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Source {
    White,
    Texture(TextureId),
    Intermediate(IntermediateId),
}

pub struct BatcherScratch {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    instances: Vec<Instance>,
    batches: Vec<Batch>,
    transform_stack: Vec<Affine2>,
    intermediates: Vec<Intermediate>,
    path_cache: PathCache,
}

impl BatcherScratch {
    pub fn new() -> BatcherScratch {
        BatcherScratch::default()
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.instances.clear();
        self.batches.clear();
        self.transform_stack.clear();
        self.intermediates.clear();
    }
}

impl Default for BatcherScratch {
    fn default() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            instances: Vec::new(),
            batches: Vec::new(),
            transform_stack: Vec::new(),
            intermediates: Vec::new(),
            path_cache: PathCache::new(),
        }
    }
}

impl fmt::Debug for BatcherScratch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BatcherScratch").finish_non_exhaustive()
    }
}

pub struct Batcher<'a> {
    texture_cache: &'a TextureCache,
    vertices: &'a mut Vec<Vertex>,
    indices: &'a mut Vec<u32>,
    instances: &'a mut Vec<Instance>,
    batches: &'a mut Vec<Batch>,
    transform_stack: &'a mut Vec<Affine2>,
    intermediates: &'a mut Vec<Intermediate>,
    path_cache: &'a mut PathCache,
    cur_clear: bool,
    cur_target: Target,
    cur_source: Source,
    max_instances_per_buffer: usize,
    cur_instance_buffer_id: usize,
    last_index: u32,
    last_vertex: u32,
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
            transform_stack: &mut scratch.transform_stack,
            intermediates: &mut scratch.intermediates,
            path_cache: &mut scratch.path_cache,
            cur_clear: false,
            cur_target: Target::Intermediate(IntermediateId(0)),
            cur_source: Source::White,
            max_instances_per_buffer,
            cur_instance_buffer_id: 0,
            last_index: 0,
            last_vertex: 0,
        }
    }

    pub fn prepare(&mut self, draw_list: &DrawList) {
        if draw_list.commands.is_empty() {
            return;
        }

        self.set_target(Target::Surface(draw_list.surface));

        if self.should_enable_msaa(draw_list.commands) {
            self.draw_intermediate_layer(draw_list.commands, Color::WHITE, Affine2::IDENTITY, true);
        } else {
            self.dispatch_commands(draw_list.commands);
        }

        self.flush();
    }

    pub fn batches(&self) -> &[Batch] {
        self.batches
    }

    pub fn vertices(&self) -> &[Vertex] {
        self.vertices
    }

    pub fn indices(&self) -> &[u32] {
        self.indices
    }

    pub fn instances(&self) -> &[Instance] {
        self.instances
    }

    pub fn intermediates(&self) -> &[Intermediate] {
        &self.intermediates
    }

    fn compute_bouding_rect(&mut self, commands: &[Command]) -> Option<Rect> {
        let mut bounding_rect: Option<Rect> = None;

        for command in commands {
            let rect = match command {
                Command::ClearRect(rect) => Rect::new(rect.pos, rect.pos + rect.size),

                Command::DrawRect(rect) => {
                    let shadow_offset = rect.shadow.map(|s| s.offset).unwrap_or(Vec2::ZERO);
                    let shadow_blur_radius = rect.shadow.map(|s| s.blur_radius).unwrap_or(0.0);
                    let shadow_spread_radius = rect.shadow.map(|s| s.spread_radius).unwrap_or(0.0);
                    let shadow_radius = Vec2::splat(shadow_blur_radius + shadow_spread_radius);

                    let rect_min = rect.pos;
                    let rect_max = rect.pos + rect.size;

                    let shadow_min = rect_min - shadow_radius + shadow_offset;
                    let shadow_max = rect_max + shadow_radius + shadow_offset;

                    Rect::new(shadow_min.min(rect_min), shadow_max.max(rect_max))
                }

                Command::DrawGlyph(glyph) => {
                    let pos = glyph.pos;
                    let glyph_key = GlyphKey {
                        font: glyph.font,
                        glyph: glyph.glyph,
                        size: glyph.size.to_bits(),
                        subpixel_bin: SubpixelBin::new(pos),
                    };

                    let Some(glyph) = self.texture_cache.get_glyph(&glyph_key) else {
                        continue;
                    };

                    let pos = pos.trunc() + glyph.offset;
                    let size = glyph.rect.size().as_vec2();
                    Rect::new(pos, pos + size)
                }

                Command::DrawLayer(layer) => {
                    let Some(rect) = self.compute_bouding_rect(layer.commands) else {
                        continue;
                    };

                    rect.transform(&layer.transform)
                }

                Command::FillPath(path) => {
                    let mesh = self.path_cache.fill(path.path, &path.options);
                    let Some(rect) = mesh.bounding_rect else {
                        continue;
                    };
                    rect
                }

                Command::StrokePath(path) => {
                    let mesh = self.path_cache.stroke(path.path, &path.options);
                    let Some(rect) = mesh.bounding_rect else {
                        continue;
                    };
                    rect
                }
            };

            bounding_rect = bounding_rect.map(|v| v.union(rect)).or(Some(rect));
        }

        bounding_rect
    }

    fn should_enable_msaa(&self, commands: &[Command]) -> bool {
        for command in commands {
            match command {
                Command::FillPath(_) | Command::StrokePath(_) => return true,
                Command::DrawLayer(layer) => {
                    let is_no_tint = layer.tint == Color::WHITE;
                    let is_compatible_scissor = layer.scissor.is_none();
                    let is_fast_path = is_no_tint && is_compatible_scissor;

                    if is_fast_path && self.should_enable_msaa(layer.commands) {
                        return true;
                    }
                }
                _ => continue,
            }
        }

        false
    }

    fn push_transform(&mut self, transform: Affine2) {
        let transform = self
            .transform_stack
            .last()
            .map(|v| *v * transform)
            .unwrap_or(transform);
        self.transform_stack.push(transform);
    }

    fn pop_transform(&mut self) {
        self.transform_stack.pop();
    }

    fn alloc_intermediate(&mut self, size: UVec2, msaa: bool) -> IntermediateId {
        self.intermediates.push(Intermediate { size, msaa });
        IntermediateId(self.intermediates.len() - 1)
    }

    fn dispatch_commands(&mut self, commands: &[Command<'_>]) -> Range<usize> {
        let first_batch = self.batches.len();

        for &command in commands {
            match command {
                Command::ClearRect(rect) => self.cmd_clear_rect(rect),
                Command::DrawRect(rect) => self.cmd_draw_rect(rect),
                Command::DrawGlyph(glyph) => self.cmd_draw_glyph(glyph),
                Command::DrawLayer(layer) => self.cmd_draw_layer(layer),
                Command::FillPath(path) => self.cmd_fill_path(&path),
                Command::StrokePath(path) => self.cmd_stroke_path(&path),
            }
        }

        self.flush();

        first_batch..self.batches.len()
    }

    fn cmd_clear_rect(&mut self, rect: ClearRect) {
        self.set_clear(true);
        self.set_source(Source::White);

        self.add_quad(Quad {
            min: rect.pos,
            max: rect.pos + rect.size,
            color: rect.color.into(),
            instance_id: INSTANCE_FILL,
            ..Quad::default()
        });
    }

    fn cmd_draw_rect(&mut self, rect: DrawRect) {
        self.set_clear(false);

        let (color, source, mut tex_min, mut tex_max) = self.get_fill(&rect.fill);

        self.set_source(source);

        if rect.border.is_none()
            && rect.shadow.is_none()
            && rect.corner_radii == CornerRadii::default()
        {
            self.add_quad(Quad {
                min: rect.pos,
                max: rect.pos + rect.size,
                local_min: Vec2::ZERO,
                local_max: Vec2::ZERO,
                tex_min,
                tex_max,
                color: color.into(),
                instance_id: INSTANCE_FILL,
            });

            return;
        }

        let shadow_offset = rect.shadow.map(|s| s.offset).unwrap_or(Vec2::ZERO);
        let shadow_blur_radius = rect.shadow.map(|s| s.blur_radius).unwrap_or(0.0);
        let shadow_spread_radius = rect.shadow.map(|s| s.spread_radius).unwrap_or(0.0);

        let instance_id = self.add_instance(Instance {
            corner_radii: rect.corner_radii.into(),
            border_color: rect
                .border
                .map(|b| b.color)
                .unwrap_or(Color::TRANSPAENT)
                .into(),
            shadow_color: rect
                .shadow
                .map(|s| s.color)
                .unwrap_or(Color::TRANSPAENT)
                .into(),
            shadow_offset,
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

        self.add_quad(Quad {
            min,
            max,
            local_min: min - rect.pos,
            local_max: max - rect.pos,
            tex_min,
            tex_max,
            color: color.into(),
            instance_id,
        });
    }

    fn cmd_draw_glyph(&mut self, glyph: DrawGlyph) {
        self.set_clear(false);

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

        self.add_quad(Quad {
            min: pos,
            max: pos + size,
            local_min: Vec2::ZERO,
            local_max: Vec2::ZERO,
            tex_min,
            tex_max,
            color: color.into(),
            instance_id,
        });
    }

    fn cmd_draw_layer(&mut self, layer: DrawLayer<'_>) {
        self.set_clear(false);

        let is_no_tint = layer.tint == Color::WHITE;
        let is_compatible_scissor = layer.scissor.is_none();
        let is_fast_path = is_no_tint && is_compatible_scissor;

        if is_fast_path {
            if layer.transform != Affine2::IDENTITY {
                self.push_transform(layer.transform);
            }

            self.dispatch_commands(layer.commands);

            if layer.transform != Affine2::IDENTITY {
                self.pop_transform();
            }

            return;
        }

        let enable_msaa = self.should_enable_msaa(layer.commands);
        self.draw_intermediate_layer(layer.commands, layer.tint, layer.transform, enable_msaa);
    }

    fn draw_intermediate_layer(
        &mut self,
        commands: &[Command],
        tint: Color,
        transform: Affine2,
        enable_msaa: bool,
    ) {
        let Some(local_rect) = self.compute_bouding_rect(commands) else {
            return;
        };

        let layer_transform = self
            .transform_stack
            .last()
            .map(|v| *v * transform)
            .unwrap_or(transform);

        let mut rect = local_rect.transform(&layer_transform);
        rect.min = rect.min.floor() - 1.0;
        rect.max = rect.max.ceil() + 1.0;

        self.flush();

        let intermediate = self.alloc_intermediate(rect.size().as_uvec2(), enable_msaa);

        let old_target = self.cur_target;
        self.set_target(Target::Intermediate(intermediate));

        self.transform_stack.push(Affine2::IDENTITY);
        self.cmd_clear_rect(ClearRect {
            pos: Vec2::ZERO,
            size: rect.size(),
            color: Color::TRANSPAENT,
        });
        self.transform_stack.pop();

        self.transform_stack
            .push(Affine2::from_translation(-rect.min) * layer_transform);
        self.dispatch_commands(commands);
        self.transform_stack.pop();

        self.flush();

        if enable_msaa {
            for batch in self.batches.iter_mut().rev() {
                if batch.target == Target::Intermediate(intermediate) {
                    batch.msaa_resolve = true;
                    break;
                }
            }
        }

        self.set_target(old_target);
        self.set_source(Source::Intermediate(intermediate));

        self.transform_stack.push(Affine2::IDENTITY);
        self.add_quad(Quad {
            min: rect.min,
            max: rect.max,
            local_min: Vec2::ZERO,
            local_max: Vec2::ZERO,
            tex_min: Vec2::ZERO,
            tex_max: Vec2::ONE,
            color: tint.into(),
            instance_id: INSTANCE_FILL,
        });
        self.transform_stack.pop();
    }

    fn cmd_fill_path(&mut self, path: &FillPath<'_>) {
        let (color, source, tex_min, tex_max) = self.get_fill(&path.fill);

        self.set_source(source);

        let mesh = self.path_cache.fill(path.path, &path.options);
        Self::draw_mesh(
            &mut self.vertices,
            &mut self.indices,
            &self.transform_stack,
            path.pos,
            mesh,
            color,
            tex_min,
            tex_max,
        );
    }

    fn cmd_stroke_path(&mut self, path: &StrokePath<'_>) {
        let (color, source, tex_min, tex_max) = self.get_fill(&path.fill);

        self.set_source(source);

        let mesh = self.path_cache.stroke(path.path, &path.options);
        Self::draw_mesh(
            &mut self.vertices,
            &mut self.indices,
            &self.transform_stack,
            path.pos,
            mesh,
            color,
            tex_min,
            tex_max,
        );
    }

    fn draw_mesh(
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u32>,
        transform_stack: &[Affine2],
        origin: Vec2,
        mesh: &Mesh,
        color: Color,
        tex_min: Vec2,
        tex_max: Vec2,
    ) {
        let Some(rect) = mesh.bounding_rect else {
            return;
        };

        let tex_scale = (tex_max - tex_min) / rect.size();
        let first_idx = vertices.len() as u32;

        for &vertex in &mesh.vertices {
            let pos = match transform_stack.last() {
                Some(transform) => origin + transform.transform_point2(vertex.pos),
                None => origin + vertex.pos,
            };

            vertices.push(Vertex {
                pos,
                tex: tex_min + (vertex.pos - rect.min) * tex_scale,
                color: color.into(),
                ..vertex
            });
        }

        for &index in &mesh.indices {
            indices.push(index + first_idx);
        }
    }

    fn get_fill(&self, fill: &Fill) -> (Color, Source, Vec2, Vec2) {
        match fill {
            Fill::Image(fill) => self
                .texture_cache
                .get_image(fill.image)
                .map(|image| {
                    let (tex_min, tex_max) = match fill.clip_rect {
                        Some(clip) => (
                            (image.rect.min + clip.min).min(image.rect.max),
                            (image.rect.min + clip.max).min(image.rect.max),
                        ),
                        None => (image.rect.min, image.rect.max),
                    };

                    let tex_min = tex_min.as_vec2() / image.texture_size.as_vec2();
                    let tex_max = tex_max.as_vec2() / image.texture_size.as_vec2();

                    (fill.tint, Source::Texture(image.texture), tex_min, tex_max)
                })
                .unwrap_or((fill.tint, Source::White, Vec2::ZERO, Vec2::ZERO)),
            Fill::Solid(color) => (*color, Source::White, Vec2::ZERO, Vec2::ZERO),
        }
    }

    fn flush(&mut self) {
        let index_range = self.last_index..self.indices.len() as u32;
        if index_range.is_empty() {
            return;
        }

        let vertex_range = self.last_vertex..self.vertices.len() as u32;

        self.last_index = index_range.end;
        self.last_vertex = vertex_range.end;

        self.batches.push(Batch {
            clear: self.cur_clear,
            msaa_resolve: false,
            target: self.cur_target,
            source: self.cur_source,
            index_range,
            vertex_range,
            instance_buffer_id: self.cur_instance_buffer_id,
        });
    }

    fn set_source(&mut self, source: Source) {
        if self.cur_source != source {
            self.flush();
        }

        self.cur_source = source;
    }

    fn set_target(&mut self, target: Target) {
        if self.cur_target != target {
            self.flush();
        }

        self.cur_target = target;
    }

    fn set_clear(&mut self, clear: bool) {
        if self.cur_clear != clear {
            self.flush();
        }

        self.cur_clear = clear;
    }

    fn add_vertex(&mut self, mut vertex: Vertex) -> u32 {
        let idx = self.vertices.len() as u32;

        if let Some(transform) = self.transform_stack.last() {
            vertex.pos = transform.transform_point2(vertex.pos);
        }

        self.vertices.push(vertex);

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

    fn add_quad(&mut self, quad: Quad) {
        let a = self.add_vertex(Vertex {
            pos: Vec2::new(quad.min.x, quad.min.y),
            local_pos: Vec2::new(quad.local_min.x, quad.local_min.y),
            tex: Vec2::new(quad.tex_min.x, quad.tex_min.y),
            color: quad.color,
            instance_id: quad.instance_id,
        });
        let b = self.add_vertex(Vertex {
            pos: Vec2::new(quad.max.x, quad.min.y),
            local_pos: Vec2::new(quad.local_max.x, quad.local_min.y),
            tex: Vec2::new(quad.tex_max.x, quad.tex_min.y),
            color: quad.color,
            instance_id: quad.instance_id,
        });
        let c = self.add_vertex(Vertex {
            pos: Vec2::new(quad.max.x, quad.max.y),
            local_pos: Vec2::new(quad.local_max.x, quad.local_max.y),
            tex: Vec2::new(quad.tex_max.x, quad.tex_max.y),
            color: quad.color,
            instance_id: quad.instance_id,
        });
        let d = self.add_vertex(Vertex {
            pos: Vec2::new(quad.min.x, quad.max.y),
            local_pos: Vec2::new(quad.local_min.x, quad.local_max.y),
            tex: Vec2::new(quad.tex_min.x, quad.tex_max.y),
            color: quad.color,
            instance_id: quad.instance_id,
        });
        self.indices.extend_from_slice(&[a, b, c, c, d, a]);
    }
}

impl fmt::Debug for Batcher<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Batcher").finish_non_exhaustive()
    }
}
