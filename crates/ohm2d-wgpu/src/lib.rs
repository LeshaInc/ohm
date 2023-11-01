use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use ohm2d_core::math::{URect, UVec2, Vec2, Vec4};
use ohm2d_core::{
    Batcher, DrawList, ImageData, ImageFormat, RectInstance, Renderer, Source, SurfaceId,
    TextureCache, TextureCommand, TextureId, Vertex,
};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use slotmap::SlotMap;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

const MAX_INSTANCES_PER_BUFFER: usize = 128;

#[derive(Debug)]
pub struct WgpuRenderer {
    instance: Instance,
    context: Option<RendererContext>,
}

impl WgpuRenderer {
    pub fn new() -> WgpuRenderer {
        let instance = Instance::new(Default::default());
        WgpuRenderer {
            instance,
            context: None,
        }
    }

    fn context(&self) -> &RendererContext {
        self.context
            .as_ref()
            .expect("context hasn't been initialized yet")
    }

    fn context_mut(&mut self) -> &mut RendererContext {
        self.context
            .as_mut()
            .expect("context hasn't been initialized yet")
    }

    pub unsafe fn create_surface<W: HasRawWindowHandle + HasRawDisplayHandle>(
        &mut self,
        handle: &W,
        size: UVec2,
    ) -> Result<SurfaceId> {
        let surface = unsafe { self.instance.create_surface(handle)? };

        if self.context.is_none() {
            let context = RendererContext::new(&self.instance, &surface)?;
            self.context = Some(context);
        }

        self.context_mut().create_surface(surface, size)
    }

    pub fn resize_surface(&mut self, id: SurfaceId, new_size: UVec2) {
        self.context_mut().resize_surface(id, new_size);
    }

    pub fn destroy_surface(&mut self, id: SurfaceId) {
        self.context_mut().destroy_surface(id);
    }
}

impl Renderer for WgpuRenderer {
    fn get_surface_size(&self, surface: SurfaceId) -> UVec2 {
        self.context().get_surface_size(surface)
    }

    fn update_textures(&mut self, commands: &[TextureCommand]) {
        self.context_mut().update_textures(commands);
    }

    fn render(&mut self, texture_cache: &TextureCache, draw_lists: &[DrawList<'_>]) {
        if draw_lists.is_empty() {
            return;
        }

        self.context_mut().render(texture_cache, draw_lists);
    }

    fn present(&mut self) {
        if let Some(context) = &mut self.context {
            context.present();
        }
    }
}

#[derive(Debug)]
struct RendererContext {
    batcher: Batcher,
    adapter: Adapter,
    device: Device,
    queue: Queue,
    uber_bind_group_layout: BindGroupLayout,
    uber_render_pipeline: RenderPipeline,
    blit_bind_group_layout: BindGroupLayout,
    blit_render_pipeline_layout: PipelineLayout,
    blit_render_pipeline_shader_module: ShaderModule,
    blit_render_pipelines: HashMap<TextureFormat, RenderPipeline>,
    textures: HashMap<TextureId, TextureEntry>,
    white_texture_view: TextureView,
    sampler: Sampler,
    surfaces: SlotMap<SurfaceId, SurfaceEntry>,
    to_present: Vec<SurfaceTexture>,
}

#[derive(Debug)]
struct TextureEntry {
    texture: Texture,
    view: TextureView,
    desc: TextureDescriptor<'static>,
    mipmaps_dirty: bool,
}

#[derive(Debug)]
struct SurfaceEntry {
    surface: Surface,
    config: SurfaceConfiguration,
    framebuffers: Vec<Framebuffer>,
}

#[derive(Debug)]
struct Framebuffer {
    texture_view: TextureView,
    texture_view_srgbless: TextureView,
}

impl RendererContext {
    fn new(instance: &Instance, main_surface: &Surface) -> Result<RendererContext> {
        let adapter = pollster::block_on(create_adapter(&instance, main_surface))?;
        let (device, queue) = pollster::block_on(create_device(&adapter))?;

        let uber_bind_group_layout = create_uber_bind_group_layout(&device);

        let pipeline_layout = create_pipeline_layout(&device, &uber_bind_group_layout);
        let shader_module = create_shader_module(&device, include_str!("uber.wgsl"));
        let uber_render_pipeline =
            create_uber_render_pipeline(&device, &pipeline_layout, &shader_module);

        let blit_bind_group_layout = create_blit_bind_group_layout(&device);
        let blit_render_pipeline_layout = create_pipeline_layout(&device, &blit_bind_group_layout);
        let blit_render_pipeline_shader_module =
            create_shader_module(&device, include_str!("blit.wgsl"));
        let blit_render_pipelines = HashMap::new();

        let white_texture_view = create_white_texture_view(&device, &queue);
        let sampler = create_sampler(&device);

        Ok(RendererContext {
            batcher: Batcher::new(MAX_INSTANCES_PER_BUFFER),
            adapter,
            device,
            queue,
            uber_bind_group_layout,
            uber_render_pipeline,
            blit_bind_group_layout,
            blit_render_pipeline_layout,
            blit_render_pipeline_shader_module,
            blit_render_pipelines,
            textures: HashMap::default(),
            white_texture_view,
            sampler,
            surfaces: SlotMap::default(),
            to_present: Vec::new(),
        })
    }

    fn create_surface(&mut self, surface: Surface, size: UVec2) -> Result<SurfaceId> {
        let caps = surface.get_capabilities(&self.adapter);

        let formats = caps.formats.iter().copied();
        let format = formats
            .max_by_key(|format| format.is_srgb() as u8 + format.components())
            .unwrap_or(TextureFormat::Bgra8Unorm);

        let alpha_mode = if caps
            .alpha_modes
            .contains(&CompositeAlphaMode::PreMultiplied)
        {
            CompositeAlphaMode::PreMultiplied
        } else {
            CompositeAlphaMode::Auto
        };

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.x,
            height: size.y,
            present_mode: PresentMode::AutoVsync,
            alpha_mode,
            view_formats: vec![],
        };
        surface.configure(&self.device, &config);

        let framebuffer = create_framebuffer(&self.device, size.x, size.y);

        let id = self.surfaces.insert(SurfaceEntry {
            surface,
            config,
            framebuffers: vec![framebuffer],
        });

        Ok(id)
    }

    fn resize_surface(&mut self, id: SurfaceId, size: UVec2) {
        let entry = &mut self.surfaces[id];
        entry.config.width = size.x;
        entry.config.height = size.y;
        entry.surface.configure(&self.device, &entry.config);

        let framebuffer = create_framebuffer(&self.device, size.x, size.y);
        entry.framebuffers = vec![framebuffer];
    }

    fn destroy_surface(&mut self, id: SurfaceId) {
        self.surfaces.remove(id);
    }

    fn texture_mark_mipmaps_dirty(&mut self, id: TextureId) {
        self.textures.get_mut(&id).unwrap().mipmaps_dirty = true;
    }

    fn texture_cmd_create_static(&mut self, id: TextureId, data: &ImageData) {
        let desc = TextureDescriptor {
            label: None,
            size: Extent3d {
                width: data.size.x,
                height: data.size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_count(data.size),
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: map_format(data.format),
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_SRC
                | TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let texture = self
            .device
            .create_texture_with_data(&self.queue, &desc, &data.data);
        let view = texture.create_view(&Default::default());

        self.textures.insert(
            id,
            TextureEntry {
                texture,
                view,
                desc,
                mipmaps_dirty: true,
            },
        );
    }

    fn texture_cmd_create_dynamic(&mut self, id: TextureId, format: ImageFormat, size: UVec2) {
        let desc = TextureDescriptor {
            label: None,
            size: Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: if format == ImageFormat::Srgba8 {
                mip_count(size)
            } else {
                1
            },
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: map_format(format),
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_SRC
                | TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let texture = self.device.create_texture(&desc);
        let view = texture.create_view(&Default::default());

        self.textures.insert(
            id,
            TextureEntry {
                texture,
                view,
                desc,
                mipmaps_dirty: true,
            },
        );
    }

    fn texture_cmd_copy(
        &mut self,
        src_id: TextureId,
        dst_id: TextureId,
        src_rect: URect,
        dst_rect: URect,
    ) {
        let size = src_rect.size();
        let src_texture = &self.textures[&src_id].texture;
        let dst_texture = &self.textures[&dst_id].texture;

        let mut encoder = self.device.create_command_encoder(&Default::default());

        encoder.copy_texture_to_texture(
            ImageCopyTexture {
                texture: src_texture,
                mip_level: 0,
                origin: Origin3d {
                    x: src_rect.min.x,
                    y: src_rect.min.y,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            ImageCopyTexture {
                texture: dst_texture,
                mip_level: 0,
                origin: Origin3d {
                    x: dst_rect.min.x,
                    y: dst_rect.min.y,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        self.texture_mark_mipmaps_dirty(dst_id);
    }

    fn texture_cmd_write(&mut self, dst_id: TextureId, dst_rect: URect, data: &ImageData) {
        let size = dst_rect.size();
        let texture = &self.textures[&dst_id].texture;

        self.queue.write_texture(
            ImageCopyTexture {
                texture,
                mip_level: 0,
                origin: Origin3d {
                    x: dst_rect.min.x,
                    y: dst_rect.min.y,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            &data.data,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some((data.data.len() / (size.y as usize)) as u32),
                rows_per_image: Some(size.y),
            },
            Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
        );

        self.texture_mark_mipmaps_dirty(dst_id);
    }

    fn texture_generate_mipmaps(&mut self, encoder: &mut CommandEncoder, id: TextureId) {
        let entry = &self.textures[&id];

        let mut desc = entry.desc.clone();
        desc.usage |= TextureUsages::RENDER_ATTACHMENT;
        let temp = self.device.create_texture(&desc);

        for mip_level in 1..desc.mip_level_count {
            let mip_size = desc.size.mip_level_size(mip_level, TextureDimension::D2);

            let src_view = entry.texture.create_view(&TextureViewDescriptor {
                aspect: TextureAspect::All,
                base_mip_level: mip_level - 1,
                mip_level_count: Some(1),
                ..Default::default()
            });

            let dst_view = temp.create_view(&TextureViewDescriptor {
                aspect: TextureAspect::All,
                base_mip_level: mip_level,
                mip_level_count: Some(1),
                ..Default::default()
            });

            let bind_group = create_blit_bind_group(
                &self.device,
                &self.blit_bind_group_layout,
                &src_view,
                &self.sampler,
            );

            let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &dst_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            let blit_render_pipeline = self
                .blit_render_pipelines
                .entry(entry.desc.format)
                .or_insert_with(|| {
                    create_blit_render_pipeline(
                        &self.device,
                        &self.blit_render_pipeline_layout,
                        &self.blit_render_pipeline_shader_module,
                        entry.desc.format,
                    )
                });

            rpass.set_pipeline(blit_render_pipeline);
            rpass.set_bind_group(0, &bind_group, &[]);
            rpass.draw(0..3, 0..1);

            drop(rpass);

            encoder.copy_texture_to_texture(
                ImageCopyTexture {
                    texture: &temp,
                    mip_level,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                ImageCopyTexture {
                    texture: &entry.texture,
                    mip_level,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                mip_size,
            );
        }
    }

    fn get_surface_size(&self, id: SurfaceId) -> UVec2 {
        let config = &self.surfaces[id].config;
        UVec2::new(config.width, config.height)
    }

    fn update_textures(&mut self, commands: &[TextureCommand]) {
        for command in commands {
            match command {
                TextureCommand::CreateStatic { id, data } => {
                    self.texture_cmd_create_static(*id, data);
                }

                &TextureCommand::CreateDynamic { id, format, size } => {
                    self.texture_cmd_create_dynamic(id, format, size);
                }

                &TextureCommand::Copy {
                    src_id,
                    dst_id,
                    src_rect,
                    dst_rect,
                } => {
                    self.texture_cmd_copy(src_id, dst_id, src_rect, dst_rect);
                }

                TextureCommand::Write {
                    dst_id,
                    dst_rect,
                    data,
                } => {
                    self.texture_cmd_write(*dst_id, *dst_rect, data);
                }

                TextureCommand::Free { id } => {
                    self.textures.remove(id);
                }
            }
        }

        let mut to_update = Vec::new();

        for (&id, entry) in &mut self.textures {
            if !entry.mipmaps_dirty {
                continue;
            }

            if entry.desc.format != TextureFormat::Rgba8UnormSrgb {
                entry.mipmaps_dirty = false;
                continue;
            }

            to_update.push(id);
            entry.mipmaps_dirty = false;
        }

        let mut encoder = None;

        for id in to_update {
            let encoder = encoder
                .get_or_insert_with(|| self.device.create_command_encoder(&Default::default()));
            self.texture_generate_mipmaps(encoder, id);
        }

        if let Some(encoder) = encoder {
            self.queue.submit(std::iter::once(encoder.finish()));
        }
    }

    fn render(&mut self, texture_cache: &TextureCache, draw_lists: &[DrawList<'_>]) {
        let surface_size_getter = |id| {
            let config = &self.surfaces[id].config;
            UVec2::new(config.width, config.height)
        };

        let data = self
            .batcher
            .prepare(texture_cache, &surface_size_getter, draw_lists);

        let vertex_buffer = create_vertex_buffer(&self.device, data.vertices);
        let index_buffer = create_index_buffer(&self.device, data.indices);

        let mut bind_groups = HashMap::new();

        for batch in data.batches {
            let surface = &mut self.surfaces[batch.surface];
            let framebuffers = &mut surface.framebuffers;

            while framebuffers.len() <= batch.framebuffer.0 as usize {
                framebuffers.push(create_framebuffer(
                    &self.device,
                    surface.config.width,
                    surface.config.height,
                ));
            }

            bind_groups
                .entry((batch.rect_instances_buffer, batch.source))
                .or_insert_with(|| {
                    let config = &self.surfaces[batch.surface].config;
                    let resolution = UVec2::new(config.width, config.height).as_vec2();
                    let globals = Globals { resolution };

                    let texture_view = match batch.source {
                        Source::White => &self.white_texture_view,
                        Source::Texture(id) => self
                            .textures
                            .get(&id)
                            .map(|t| &t.view)
                            .unwrap_or(&self.white_texture_view),
                        Source::Framebuffer(surface, framebuffer) => self
                            .surfaces
                            .get(surface)
                            .and_then(|surface| surface.framebuffers.get(framebuffer.0 as usize))
                            .map(|t| &t.texture_view)
                            .unwrap_or(&self.white_texture_view),
                    };

                    create_uber_bind_group(
                        &self.device,
                        &self.uber_bind_group_layout,
                        &globals,
                        data.rect_instances
                            .chunks(MAX_INSTANCES_PER_BUFFER)
                            .nth(batch.rect_instances_buffer)
                            .unwrap_or(&[]),
                        texture_view,
                        &self.sampler,
                    )
                });
        }

        let mut encoder = self.device.create_command_encoder(&Default::default());
        let mut batches = data.batches.iter().peekable();

        while let Some(batch) = batches.peek() {
            let surface_id = batch.surface;
            let framebuffer_id = batch.framebuffer;

            let surface = &self.surfaces[surface_id];
            let framebuffers = &surface.framebuffers;
            let framebuffer = &framebuffers[framebuffer_id.0 as usize];

            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &framebuffer.texture_view,
                    resolve_target: None,
                    ops: Operations {
                        load: batch
                            .clear_color
                            .map(|c| {
                                LoadOp::Clear(wgpu::Color {
                                    r: c.r as f64,
                                    g: c.g as f64,
                                    b: c.b as f64,
                                    a: c.a as f64,
                                })
                            })
                            .unwrap_or(LoadOp::Load),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            pass.set_pipeline(&self.uber_render_pipeline);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint32);

            while let Some(batch) =
                batches.next_if(|b| b.surface == surface_id && b.framebuffer == framebuffer_id)
            {
                if batch.index_range.is_empty() {
                    continue;
                }

                let bind_group = bind_groups
                    .get(&(batch.rect_instances_buffer, batch.source))
                    .unwrap();
                pass.set_bind_group(0, bind_group, &[]);
                pass.draw_indexed(batch.index_range.clone(), 0, 0..1);
            }
        }

        let mut touched_surfaces = data.batches.iter().map(|v| v.surface).collect::<Vec<_>>();
        touched_surfaces.sort();
        touched_surfaces.dedup();

        self.to_present.clear();

        for surface in touched_surfaces {
            let surface_entry = &self.surfaces[surface];
            let surface_format = surface_entry.config.format;
            let frame = surface_entry
                .surface
                .get_current_texture()
                .expect("Failed to acquire next swap chain texture");
            let surface_view = frame.texture.create_view(&TextureViewDescriptor::default());
            self.to_present.push(frame);

            let framebuffer = &surface_entry.framebuffers[0];

            let bind_group = create_blit_bind_group(
                &self.device,
                &self.blit_bind_group_layout,
                if surface_format.is_srgb() {
                    &framebuffer.texture_view
                } else {
                    &framebuffer.texture_view_srgbless
                },
                &self.sampler,
            );

            let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &surface_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            let blit_render_pipeline = self
                .blit_render_pipelines
                .entry(surface_format)
                .or_insert_with(|| {
                    create_blit_render_pipeline(
                        &self.device,
                        &self.blit_render_pipeline_layout,
                        &self.blit_render_pipeline_shader_module,
                        surface_format,
                    )
                });

            rpass.set_pipeline(&blit_render_pipeline);
            rpass.set_bind_group(0, &bind_group, &[]);
            rpass.draw(0..3, 0..1);

            drop(rpass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    fn present(&mut self) {
        for frame in self.to_present.drain(..) {
            frame.present();
        }
    }
}

#[repr(packed)]
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
struct OurVertex {
    pos: Vec2,
    tex: Vec2,
    color: Vec4,
    rect_id: u32,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default, encase::ShaderType)]
struct OurRectInstance {
    corner_radii: Vec4,
    border_color: Vec4,
    shadow_color: Vec4,
    shadow_offset: Vec2,
    pos: Vec2,
    size: Vec2,
    border_width: f32,
    shadow_blur_radius: f32,
    shadow_spread_radius: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, encase::ShaderType)]
struct Globals {
    resolution: Vec2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, encase::ShaderType)]
struct RectInstances {
    arr: [OurRectInstance; MAX_INSTANCES_PER_BUFFER],
}

async fn create_adapter(instance: &Instance, main_surface: &Surface) -> Result<Adapter> {
    let adapter = instance
        .request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(main_surface),
        })
        .await;

    let Some(adapter) = adapter else {
        bail!("No compatible video adapters");
    };

    Ok(adapter)
}

async fn create_device(adapter: &Adapter) -> Result<(Device, Queue)> {
    adapter
        .request_device(
            &DeviceDescriptor {
                label: None,
                features: Features::empty(),
                limits: Limits::downlevel_defaults().using_resolution(adapter.limits()),
            },
            None,
        )
        .await
        .context("Failed to create graphics device")
}

fn create_uber_bind_group_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX_FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::VERTEX_FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

fn create_blit_bind_group_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

fn create_vertex_buffer(device: &Device, vertices: &[Vertex]) -> Buffer {
    let vertices = vertices
        .iter()
        .map(|v| OurVertex {
            pos: v.pos,
            tex: v.tex,
            color: v.color,
            rect_id: v.rect_id,
        })
        .collect::<Vec<_>>();

    let contents = unsafe {
        std::slice::from_raw_parts(
            vertices.as_ptr() as *const u8,
            vertices.len() * std::mem::size_of::<OurVertex>(),
        )
    };

    device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents,
        usage: BufferUsages::VERTEX,
    })
}

fn create_index_buffer(device: &Device, data: &[u32]) -> Buffer {
    let contents = unsafe {
        std::slice::from_raw_parts(
            data.as_ptr() as *const u8,
            data.len() * std::mem::size_of::<u32>(),
        )
    };

    device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents,
        usage: BufferUsages::INDEX,
    })
}

fn create_uniform_buffer<T: encase::ShaderType + encase::internal::WriteInto>(
    device: &Device,
    data: &T,
) -> Buffer {
    let mut buffer = encase::UniformBuffer::new(Vec::new());
    buffer.write(data).unwrap();

    let encoded = buffer.into_inner();
    device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: &encoded,
        usage: BufferUsages::UNIFORM,
    })
}

fn create_uber_bind_group(
    device: &Device,
    layout: &BindGroupLayout,
    globals: &Globals,
    rect_instances: &[RectInstance],
    texture_view: &TextureView,
    sampler: &Sampler,
) -> BindGroup {
    let globals_buffer = create_uniform_buffer(device, globals);

    let mut rect_instances_arr = [OurRectInstance::default(); MAX_INSTANCES_PER_BUFFER];

    for (i, v) in rect_instances.iter().enumerate() {
        rect_instances_arr[i] = OurRectInstance {
            corner_radii: v.corner_radii,
            border_color: v.border_color,
            shadow_color: v.shadow_color,
            shadow_offset: v.shadow_offset,
            pos: v.pos,
            size: v.size,
            border_width: v.border_width,
            shadow_blur_radius: v.shadow_blur_radius,
            shadow_spread_radius: v.shadow_spread_radius,
        };
    }

    let rect_instances_buffer = create_uniform_buffer(
        device,
        &RectInstances {
            arr: rect_instances_arr,
        },
    );

    device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: rect_instances_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: BindingResource::TextureView(texture_view),
            },
            BindGroupEntry {
                binding: 3,
                resource: BindingResource::Sampler(sampler),
            },
        ],
    })
}

fn create_blit_bind_group(
    device: &Device,
    layout: &BindGroupLayout,
    texture_view: &TextureView,
    sampler: &Sampler,
) -> BindGroup {
    device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(texture_view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Sampler(sampler),
            },
        ],
    })
}

fn create_pipeline_layout(device: &Device, bind_group_layout: &BindGroupLayout) -> PipelineLayout {
    device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    })
}

fn create_shader_module(device: &Device, source: &str) -> ShaderModule {
    device.create_shader_module(ShaderModuleDescriptor {
        label: None,
        source: ShaderSource::Wgsl(source.into()),
    })
}

fn create_uber_render_pipeline(
    device: &Device,
    layout: &PipelineLayout,
    shader_module: &ShaderModule,
) -> RenderPipeline {
    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: None,
        layout: Some(layout),
        vertex: VertexState {
            module: shader_module,
            entry_point: "vs_main",
            buffers: &[VertexBufferLayout {
                array_stride: 36,
                step_mode: VertexStepMode::Vertex,
                attributes: &[
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 8,
                        shader_location: 1,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 16,
                        shader_location: 2,
                    },
                    VertexAttribute {
                        format: VertexFormat::Uint32,
                        offset: 32,
                        shader_location: 3,
                    },
                ],
            }],
        },
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState::default(),
        fragment: Some(FragmentState {
            module: shader_module,
            entry_point: "fs_main",
            targets: &[Some(ColorTargetState {
                format: TextureFormat::Rgba8UnormSrgb,
                blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: ColorWrites::all(),
            })],
        }),
        multiview: None,
    })
}

fn create_blit_render_pipeline(
    device: &Device,
    layout: &PipelineLayout,
    shader_module: &ShaderModule,
    format: TextureFormat,
) -> RenderPipeline {
    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: None,
        layout: Some(layout),
        vertex: VertexState {
            module: shader_module,
            entry_point: "vs_main",
            buffers: &[],
        },
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState::default(),
        fragment: Some(FragmentState {
            module: shader_module,
            entry_point: "fs_main",
            targets: &[Some(ColorTargetState {
                format,
                blend: None,
                write_mask: ColorWrites::all(),
            })],
        }),
        multiview: None,
    })
}

fn create_white_texture_view(device: &Device, queue: &Queue) -> TextureView {
    let texture = device.create_texture_with_data(
        queue,
        &TextureDescriptor {
            label: None,
            size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        &[255, 255, 255, 255],
    );

    texture.create_view(&Default::default())
}

fn create_sampler(device: &Device) -> Sampler {
    device.create_sampler(&SamplerDescriptor {
        label: None,
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        address_mode_w: AddressMode::ClampToEdge,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        mipmap_filter: FilterMode::Linear,
        lod_min_clamp: 0.0,
        lod_max_clamp: 32.0,
        compare: None,
        anisotropy_clamp: 1,
        border_color: None,
    })
}

fn create_framebuffer(device: &Device, width: u32, height: u32) -> Framebuffer {
    let texture = device.create_texture(&TextureDescriptor {
        label: None,
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[TextureFormat::Rgba8Unorm],
    });

    let texture_view = texture.create_view(&Default::default());

    let texture_view_srgbless = texture.create_view(&TextureViewDescriptor {
        format: Some(TextureFormat::Rgba8Unorm),
        ..Default::default()
    });

    Framebuffer {
        texture_view,
        texture_view_srgbless,
    }
}

fn map_format(format: ImageFormat) -> TextureFormat {
    match format {
        ImageFormat::Srgba8 => TextureFormat::Rgba8UnormSrgb,
        ImageFormat::Gray8 => TextureFormat::R8Unorm,
    }
}

fn mip_count(size: UVec2) -> u32 {
    size.max_element().ilog2() + 1
}
