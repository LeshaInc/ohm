use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use ohm_core::image::{ImageData, ImageFormat};
use ohm_core::math::{URect, UVec2, Vec2, Vec4};
use ohm_core::renderer::{
    Batcher, BatcherScratch, Instance as BatcherInstance, Renderer, Source, SurfaceId, Vertex,
    WindowHandle,
};
use ohm_core::texture::{MipmapMode, TextureCache, TextureCommand, TextureId};
use ohm_core::{DrawList, Error, ErrorKind, Result};
use self_cell::self_cell;
use slotmap::SlotMap;
use wgpu::util::{BufferInitDescriptor, DeviceExt, TextureDataOrder};
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
}

impl Renderer for WgpuRenderer {
    fn create_surface(&mut self, window: Arc<dyn WindowHandle>, size: UVec2) -> Result<SurfaceId> {
        let surface =
            OwnedSurface::try_new(window, |window| self.instance.create_surface(&**window))
                .map_err(|e| Error::wrap(ErrorKind::Gpu, e))?;

        if self.context.is_none() {
            let context = RendererContext::new(&self.instance, &surface)?;
            self.context = Some(context);
        }

        self.context_mut().create_surface(surface, size)
    }

    fn resize_surface(&mut self, id: SurfaceId, new_size: UVec2) -> Result<()> {
        self.context_mut().resize_surface(id, new_size);
        Ok(())
    }

    fn get_surface_size(&self, surface: SurfaceId) -> UVec2 {
        self.context().get_surface_size(surface)
    }

    fn destroy_surface(&mut self, id: SurfaceId) {
        self.context_mut().destroy_surface(id);
    }

    fn update_textures(&mut self, commands: &mut Vec<TextureCommand>) -> Result<()> {
        self.context_mut().update_textures(commands);
        Ok(())
    }

    fn render(&mut self, texture_cache: &TextureCache, draw_lists: &[DrawList<'_>]) -> Result<()> {
        if !draw_lists.is_empty() {
            self.context_mut().render(texture_cache, draw_lists);
        }
        Ok(())
    }

    fn present(&mut self) -> Result<()> {
        if let Some(context) = &mut self.context {
            context.present();
        }
        Ok(())
    }
}

impl Default for WgpuRenderer {
    fn default() -> Self {
        WgpuRenderer::new()
    }
}

#[derive(Debug)]
struct RendererContext {
    batcher_scratch: BatcherScratch,
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

struct SurfaceEntry {
    surface: OwnedSurface,
    config: SurfaceConfiguration,
    framebuffers: Vec<Framebuffer>,
}

impl fmt::Debug for SurfaceEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SurfaceEntry")
            .field("config", &self.config)
            .field("framebuffers", &self.framebuffers)
            .finish_non_exhaustive()
    }
}

self_cell! {
    struct OwnedSurface {
        owner: Arc<dyn WindowHandle>,
        #[covariant]
        dependent: Surface,
    }
}

#[derive(Debug)]
struct Framebuffer {
    texture_view: TextureView,
    texture_view_srgbless: TextureView,
}

impl RendererContext {
    fn new(instance: &Instance, main_surface: &OwnedSurface) -> Result<RendererContext> {
        let adapter =
            pollster::block_on(create_adapter(instance, main_surface.borrow_dependent()))?;
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
            batcher_scratch: BatcherScratch::default(),
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

    fn create_surface(&mut self, surface: OwnedSurface, size: UVec2) -> Result<SurfaceId> {
        let caps = surface.borrow_dependent().get_capabilities(&self.adapter);

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
            desired_maximum_frame_latency: 2,
        };

        surface.borrow_dependent().configure(&self.device, &config);

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
        entry
            .surface
            .borrow_dependent()
            .configure(&self.device, &entry.config);

        let framebuffer = create_framebuffer(&self.device, size.x, size.y);
        entry.framebuffers = vec![framebuffer];
    }

    fn destroy_surface(&mut self, id: SurfaceId) {
        self.surfaces.remove(id);
    }

    fn texture_mark_mipmaps_dirty(&mut self, id: TextureId) {
        self.textures.get_mut(&id).unwrap().mipmaps_dirty = true;
    }

    fn texture_cmd_create_static(
        &mut self,
        id: TextureId,
        mut data: ImageData,
        mipmap_mode: MipmapMode,
    ) {
        let desc = TextureDescriptor {
            label: None,
            size: Extent3d {
                width: data.size.x,
                height: data.size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: if mipmap_mode == MipmapMode::Enabled {
                mip_count(data.size)
            } else {
                1
            },
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: map_format(data.format),
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_SRC
                | TextureUsages::COPY_DST,
            view_formats: &[],
        };

        if mipmap_mode == MipmapMode::Enabled {
            data.data.resize(data.data.len() * 4 / 3 + 1, 0);
        }

        let texture = self.device.create_texture_with_data(
            &self.queue,
            &desc,
            TextureDataOrder::LayerMajor,
            &data.data,
        );
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

    fn texture_cmd_create_dynamic(
        &mut self,
        id: TextureId,
        format: ImageFormat,
        size: UVec2,
        mipmap_mode: MipmapMode,
    ) {
        let desc = TextureDescriptor {
            label: None,
            size: Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: if mipmap_mode == MipmapMode::Enabled {
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
        encoder: &mut CommandEncoder,
        src_id: TextureId,
        dst_id: TextureId,
        src_rect: URect,
        dst_rect: URect,
    ) {
        let size = src_rect.size();
        let src_texture = &self.textures[&src_id].texture;
        let dst_texture = &self.textures[&dst_id].texture;

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

        self.texture_mark_mipmaps_dirty(dst_id);
    }

    fn texture_cmd_write(&mut self, dst_id: TextureId, dst_rect: URect, data: ImageData) {
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
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
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

    fn update_textures(&mut self, commands: &mut Vec<TextureCommand>) {
        if commands.is_empty() {
            return;
        }

        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.push_debug_group("ohm-textures");

        for command in commands.drain(..) {
            match command {
                TextureCommand::CreateStatic {
                    id,
                    data,
                    mipmap_mode,
                } => {
                    self.texture_cmd_create_static(id, data, mipmap_mode);
                }

                TextureCommand::CreateDynamic {
                    id,
                    format,
                    size,
                    mipmap_mode,
                } => {
                    self.texture_cmd_create_dynamic(id, format, size, mipmap_mode);
                }

                TextureCommand::Copy {
                    src_id,
                    dst_id,
                    src_rect,
                    dst_rect,
                } => {
                    self.texture_cmd_copy(&mut encoder, src_id, dst_id, src_rect, dst_rect);
                }

                TextureCommand::Write {
                    dst_id,
                    dst_rect,
                    data,
                } => {
                    self.texture_cmd_write(dst_id, dst_rect, data);
                }

                TextureCommand::Free { id } => {
                    self.textures.remove(&id);
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

        if !to_update.is_empty() {
            encoder.push_debug_group("mipmaps");
            for id in to_update {
                self.texture_generate_mipmaps(&mut encoder, id);
            }
            encoder.pop_debug_group();
        }

        encoder.pop_debug_group();

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    fn render(&mut self, texture_cache: &TextureCache, draw_lists: &[DrawList<'_>]) {
        let mut batcher = Batcher::new(
            &mut self.batcher_scratch,
            texture_cache,
            MAX_INSTANCES_PER_BUFFER,
        );

        for list in draw_lists {
            let surface_config = &self.surfaces[list.surface].config;
            let surface_size = UVec2::new(surface_config.width, surface_config.height);
            batcher.prepare(surface_size, list);
        }

        let vertex_buffer = create_vertex_buffer(&self.device, batcher.vertices());
        let index_buffer = create_index_buffer(&self.device, batcher.indices());

        let mut bind_groups = HashMap::new();

        for batch in batcher.batches() {
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
                .entry((batch.instance_buffer_id, batch.source))
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
                        batcher
                            .instances()
                            .chunks(MAX_INSTANCES_PER_BUFFER)
                            .nth(batch.instance_buffer_id)
                            .unwrap_or(&[]),
                        texture_view,
                        &self.sampler,
                    )
                });
        }

        let mut encoder = self.device.create_command_encoder(&Default::default());
        let mut batches = batcher.batches().iter().peekable();

        let mut touched_surfaces = HashSet::new();
        let mut touched_framebuffers = HashSet::new();

        encoder.push_debug_group("ohm");

        while let Some(batch) = batches.peek() {
            let surface_id = batch.surface;
            let framebuffer_id = batch.framebuffer;

            touched_surfaces.insert(surface_id);

            let load_op = if touched_framebuffers.contains(&(surface_id, framebuffer_id)) {
                LoadOp::Load
            } else {
                touched_framebuffers.insert((surface_id, framebuffer_id));
                LoadOp::Clear(Color::TRANSPARENT)
            };

            let surface = &self.surfaces[surface_id];
            let framebuffers = &surface.framebuffers;
            let framebuffer = &framebuffers[framebuffer_id.0 as usize];

            encoder.push_debug_group("pass");

            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &framebuffer.texture_view,
                    resolve_target: None,
                    ops: Operations {
                        load: load_op,
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
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
                    .get(&(batch.instance_buffer_id, batch.source))
                    .unwrap();
                pass.set_bind_group(0, bind_group, &[]);
                pass.draw_indexed(batch.index_range.clone(), 0, 0..1);
            }

            drop(pass);
            encoder.pop_debug_group(); // pass
        }

        self.to_present.clear();

        encoder.push_debug_group("present");

        for surface in touched_surfaces {
            let surface_entry = &self.surfaces[surface];
            let surface_format = surface_entry.config.format;
            let frame = surface_entry
                .surface
                .borrow_dependent()
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
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
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

            rpass.set_pipeline(blit_render_pipeline);
            rpass.set_bind_group(0, &bind_group, &[]);
            rpass.draw(0..3, 0..1);

            drop(rpass);
        }

        encoder.pop_debug_group(); // present
        encoder.pop_debug_group(); // ohm

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
    local_pos: Vec2,
    tex: Vec2,
    color: Vec4,
    instance_id: u32,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default, encase::ShaderType)]
struct OurInstance {
    corner_radii: Vec4,
    border_color: Vec4,
    shadow_color: Vec4,
    shadow_offset: Vec2,
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
    arr: [OurInstance; MAX_INSTANCES_PER_BUFFER],
}

async fn create_adapter(instance: &Instance, main_surface: &Surface<'_>) -> Result<Adapter> {
    let adapter = instance
        .request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(main_surface),
        })
        .await;

    let Some(adapter) = adapter else {
        return Err(Error::new(ErrorKind::Gpu, "no compatible video adapters"));
    };

    Ok(adapter)
}

async fn create_device(adapter: &Adapter) -> Result<(Device, Queue)> {
    adapter
        .request_device(
            &DeviceDescriptor {
                label: None,
                required_features: Features::empty(),
                required_limits: Limits::downlevel_defaults().using_resolution(adapter.limits()),
            },
            None,
        )
        .await
        .map_err(|e| Error::new(ErrorKind::Gpu, "failed to create graphics device").with_source(e))
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
            local_pos: v.local_pos,
            tex: v.tex,
            color: v.color,
            instance_id: v.instance_id,
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
        std::slice::from_raw_parts(data.as_ptr() as *const u8, std::mem::size_of_val(data))
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
    instances: &[BatcherInstance],
    texture_view: &TextureView,
    sampler: &Sampler,
) -> BindGroup {
    let globals_buffer = create_uniform_buffer(device, globals);

    let mut rect_instances_arr = [OurInstance::default(); MAX_INSTANCES_PER_BUFFER];

    for (i, v) in instances.iter().enumerate() {
        rect_instances_arr[i] = OurInstance {
            corner_radii: v.corner_radii,
            border_color: v.border_color,
            shadow_color: v.shadow_color,
            shadow_offset: v.shadow_offset,
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
                array_stride: 44,
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
                        format: VertexFormat::Float32x2,
                        offset: 16,
                        shader_location: 2,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 24,
                        shader_location: 3,
                    },
                    VertexAttribute {
                        format: VertexFormat::Uint32,
                        offset: 40,
                        shader_location: 4,
                    },
                ],
            }],
            compilation_options: Default::default(),
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
            compilation_options: Default::default(),
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
            compilation_options: Default::default(),
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
            compilation_options: Default::default(),
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
        TextureDataOrder::LayerMajor,
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
