use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crossbeam_queue::SegQueue;
use guillotiere::{AllocId, AtlasAllocator};
use slotmap::SlotMap;

use crate::asset::{AssetPath, AssetSource};
use crate::image::{ImageData, ImageDecoder, ImageFormat, ImageHandle};
use crate::math::{Affine2, URect, UVec2, Vec2};
use crate::renderer::PathCache;
use crate::text::{FontDatabase, GlyphKey, Rasterizer, SubpixelBin};
use crate::{
    Command, DrawList, DrawRect, Error, ErrorKind, Fill, FillPath, ImageId, Result, StrokePath,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub struct TextureId(pub u64);

#[derive(Debug)]
pub enum TextureCommand {
    CreateStatic {
        id: TextureId,
        data: ImageData,
        mipmap_mode: MipmapMode,
    },
    CreateDynamic {
        id: TextureId,
        format: ImageFormat,
        size: UVec2,
        mipmap_mode: MipmapMode,
    },
    Copy {
        src_id: TextureId,
        dst_id: TextureId,
        src_rect: URect,
        dst_rect: URect,
    },
    Write {
        dst_id: TextureId,
        dst_rect: URect,
        data: ImageData,
    },
    Free {
        id: TextureId,
    },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum MipmapMode {
    Disabled,
    Enabled,
}

slotmap::new_key_type! {
    struct AtlasId;
}

#[derive(Default)]
pub struct TextureCache {
    images: SlotMap<ImageId, ImageEntry>,
    images_by_path: HashMap<AssetPath<'static>, ImageId>,
    glyphs: HashMap<GlyphKey, GlyphEntry>,
    atlases: TextureAtlasPool,
    id_allocator: TextureIdAllocator,
    image_cleanup_queue: Arc<SegQueue<ImageId>>,
}

#[derive(Debug, Clone)]
struct ImageEntry {
    path: Option<AssetPath<'static>>,
    data: Option<ImageData>,
    mipmap_mode: MipmapMode,
    texture: Option<TextureId>,
    rect: URect,
    alloc_id: Option<(AtlasId, AllocId)>,
    requested_size: UVec2,
    max_size: UVec2,
}

#[derive(Debug, Clone)]
pub struct AllocatedImage {
    pub texture: TextureId,
    pub texture_size: UVec2,
    pub rect: URect,
}

#[derive(Debug, Clone, Copy)]
struct GlyphEntry {
    used: bool,
    rect: URect,
    alloc_id: Option<(AtlasId, AllocId)>,
    offset: Vec2,
    is_empty: bool,
}

#[derive(Debug, Clone)]
pub struct AllocatedGlyph {
    pub texture: TextureId,
    pub format: ImageFormat,
    pub texture_size: UVec2,
    pub rect: URect,
    pub offset: Vec2,
}

impl TextureCache {
    const MIN_STANDALONE_SIZE: UVec2 = UVec2::new(1024, 1024);

    pub fn new() -> TextureCache {
        TextureCache::default()
    }

    pub fn add_image(&mut self, data: ImageData, mipmap_mode: MipmapMode) -> ImageHandle {
        let id = self.images.insert(ImageEntry {
            path: None,
            mipmap_mode,
            texture: None,
            rect: URect::ZERO,
            alloc_id: None,
            requested_size: data.size,
            max_size: data.size,
            data: Some(data),
        });

        ImageHandle::new(id, self.image_cleanup_queue.clone())
    }

    pub fn add_image_from_path<'a>(
        &mut self,
        path: impl Into<AssetPath<'a>>,
        mipmap_mode: MipmapMode,
    ) -> ImageHandle {
        self.add_image_from_path_inner(path.into().into_owned(), mipmap_mode)
    }

    fn add_image_from_path_inner(
        &mut self,
        path: AssetPath<'static>,
        mipmap_mode: MipmapMode,
    ) -> ImageHandle {
        if let Some(&id) = self.images_by_path.get(&path) {
            return ImageHandle::new(id, self.image_cleanup_queue.clone());
        }

        let path = path.into_owned();

        let id = self.images.insert(ImageEntry {
            path: Some(path.clone()),
            data: None,
            mipmap_mode,
            texture: None,
            rect: URect::ZERO,
            alloc_id: None,
            requested_size: UVec2::ZERO,
            max_size: UVec2::ZERO,
        });

        self.images_by_path.insert(path, id);

        ImageHandle::new(id, self.image_cleanup_queue.clone())
    }

    pub fn add_glyph(&mut self, key: GlyphKey) {
        self.glyphs.entry(key).or_insert(GlyphEntry {
            used: true,
            rect: URect::ZERO,
            alloc_id: None,
            is_empty: false,
            offset: Vec2::ZERO,
        });
    }

    pub fn add_glyphs_from_lists(&mut self, lists: &[DrawList]) {
        for list in lists {
            self.add_glyphs_from_commands(list.commands);
        }
    }

    pub fn add_glyphs_from_commands(&mut self, commands: &[Command]) {
        for command in commands {
            match command {
                Command::DrawLayer(layer) => self.add_glyphs_from_commands(layer.commands),
                Command::DrawGlyph(glyph) => {
                    self.add_glyph(GlyphKey {
                        font: glyph.font,
                        glyph: glyph.glyph,
                        size: glyph.size.to_bits(),
                        subpixel_bin: SubpixelBin::new(glyph.pos),
                    });
                }
                _ => (),
            }
        }
    }

    pub fn set_image_sizes_from_lists(&mut self, path_cache: &mut PathCache, lists: &[DrawList]) {
        for list in lists {
            self.set_image_sizes_from_commands(path_cache, list.commands, Affine2::IDENTITY);
        }
    }

    pub fn set_image_sizes_from_commands(
        &mut self,
        path_cache: &mut PathCache,
        commands: &[Command],
        transform: Affine2,
    ) {
        let scale_x = transform.matrix2.col(0).length();
        let scale_y = transform.matrix2.col(1).length();
        let scale = scale_x.max(scale_y);

        for command in commands {
            let (size, fill) = match command {
                Command::DrawLayer(layer) => {
                    self.set_image_sizes_from_commands(
                        path_cache,
                        layer.commands,
                        layer.transform * transform,
                    );
                    continue;
                }
                Command::DrawRect(DrawRect {
                    size,
                    fill: Fill::Image(fill),
                    ..
                }) => (*size, fill),
                Command::FillPath(FillPath {
                    path,
                    options,
                    fill: Fill::Image(fill),
                    ..
                }) => {
                    let mesh = path_cache.fill(path, options);
                    let Some(rect) = mesh.bounding_rect else {
                        continue;
                    };
                    (rect.size(), fill)
                }
                Command::StrokePath(StrokePath {
                    path,
                    options,
                    fill: Fill::Image(fill),
                    ..
                }) => {
                    let mesh = path_cache.stroke(path, options);
                    let Some(rect) = mesh.bounding_rect else {
                        continue;
                    };
                    (rect.size(), fill)
                }
                _ => continue,
            };

            let Some(image) = self.images.get_mut(fill.image) else {
                continue;
            };

            if image.max_size != UVec2::ZERO && image.max_size.cmpge(image.requested_size).any() {
                continue;
            }

            let size = match fill.clip_rect {
                Some(clip) => size / clip.size(),
                None => size,
            };

            let size = (size * scale).as_uvec2();
            image.requested_size = image.requested_size.max(size);
        }
    }

    pub fn load_images(
        &mut self,
        source: &dyn AssetSource,
        decoder: &dyn ImageDecoder,
        commands: &mut Vec<TextureCommand>,
    ) -> Result<()> {
        for image in self.images.values_mut() {
            if image.texture.is_some() || image.alloc_id.is_some() {
                continue;
            }

            let requested_size = Some(image.requested_size).filter(|v| v.x != 0 && v.y != 0);

            let data = if let Some(data) = image.data.take() {
                data
            } else if let Some(path) = image.path.as_ref() {
                let raw_data = source
                    .load(path.as_borrowed())
                    .map_err(|e| e.with_context(format!("failed to load image from {path}")))?;

                decoder
                    .decode(path.extension(), &raw_data, requested_size)
                    .map_err(|e| e.with_context(format!("failed to decode image from {path}")))?
            } else {
                continue;
            };

            if requested_size.is_some_and(|v| v.cmpgt(data.size).any()) {
                image.max_size = data.size;
            }

            if data.size.cmpge(Self::MIN_STANDALONE_SIZE).any() {
                let texture_id = self.id_allocator.alloc();

                image.texture = Some(texture_id);
                image.rect = URect::new(UVec2::ZERO, data.size);

                commands.push(TextureCommand::CreateStatic {
                    id: texture_id,
                    data,
                    mipmap_mode: image.mipmap_mode,
                });

                continue;
            }

            let (alloc_id, rect) = self
                .atlases
                .alloc(&mut self.id_allocator, commands, data, image.mipmap_mode)
                .ok_or_else(|| {
                    Error::new(ErrorKind::AtlasAlloc, "failed to allocate image in atlas")
                })?;

            image.alloc_id = Some(alloc_id);
            image.rect = rect;
        }

        Ok(())
    }

    pub fn load_glyphs(
        &mut self,
        font_db: &dyn FontDatabase,
        rasterizer: &mut dyn Rasterizer,
        commands: &mut Vec<TextureCommand>,
    ) -> Result<()> {
        for (glyph_key, glyph) in &mut self.glyphs {
            if glyph.is_empty || glyph.alloc_id.is_some() {
                continue;
            }

            let Some(font) = font_db.get(glyph_key.font) else {
                continue;
            };

            let Some(result) = rasterizer.rasterize(
                font,
                glyph_key.glyph,
                f32::from_bits(glyph_key.size),
                glyph_key.subpixel_bin,
            ) else {
                glyph.is_empty = true;
                continue;
            };

            let (alloc_id, rect) = self
                .atlases
                .alloc(
                    &mut self.id_allocator,
                    commands,
                    result.image,
                    MipmapMode::Disabled,
                )
                .ok_or_else(|| {
                    Error::new(ErrorKind::AtlasAlloc, "failed to allocate glyph in atlas")
                })?;

            glyph.alloc_id = Some(alloc_id);
            glyph.rect = rect;
            glyph.offset = result.offset;
        }

        Ok(())
    }

    pub fn get_image(&self, id: ImageId) -> Option<AllocatedImage> {
        self.images.get(id).and_then(|entry| {
            let (texture, texture_size) =
                entry.texture.map(|tex| (tex, entry.rect.max)).or_else(|| {
                    let atlas_id = entry.alloc_id?.0;
                    let atlas = &self.atlases.atlases[atlas_id];
                    Some((atlas.texture, atlas.size))
                })?;

            Some(AllocatedImage {
                texture,
                texture_size,
                rect: entry.rect,
            })
        })
    }

    pub fn get_glyph(&self, key: &GlyphKey) -> Option<AllocatedGlyph> {
        self.glyphs.get(key).and_then(|entry| {
            let atlas_id = entry.alloc_id?.0;
            let atlas = &self.atlases.atlases[atlas_id];
            Some(AllocatedGlyph {
                texture: atlas.texture,
                texture_size: atlas.size,
                rect: entry.rect,
                format: atlas.format,
                offset: entry.offset,
            })
        })
    }

    pub fn cleanup(&mut self, commands: &mut Vec<TextureCommand>) {
        while let Some(image_id) = self.image_cleanup_queue.pop() {
            let Some(image) = self.images.remove(image_id) else {
                continue;
            };

            if let Some(alloc_id) = image.alloc_id {
                self.atlases.free(alloc_id);
            } else if let Some(id) = image.texture {
                commands.push(TextureCommand::Free { id });
            }

            if let Some(path) = &image.path {
                self.images_by_path.remove(path);
            }
        }

        self.glyphs.retain(|_, glyph| {
            if glyph.used {
                return true;
            }

            if let Some(alloc_id) = glyph.alloc_id {
                self.atlases.free(alloc_id);
            }

            false
        });

        self.atlases.cleanup(commands);
    }
}

impl fmt::Debug for TextureCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextureCache")
            .field("images", &self.images)
            .field("images_by_path", &self.images_by_path)
            .field("glyphs", &self.glyphs)
            .field("atlases", &self.atlases)
            .field("id_allocator", &self.id_allocator)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
struct TextureAtlasPool {
    atlases: SlotMap<AtlasId, TextureAtlas>,
}

impl TextureAtlasPool {
    fn alloc(
        &mut self,
        id_allocator: &mut TextureIdAllocator,
        commands: &mut Vec<TextureCommand>,
        data: ImageData,
        mipmap_mode: MipmapMode,
    ) -> Option<((AtlasId, AllocId), URect)> {
        let alloc_size = data.size;
        let alloc_format = data.format;
        let mut data = Some(data);

        for (atlas_id, atlas) in &mut self.atlases {
            if atlas.format != alloc_format {
                continue;
            }

            if atlas.mipmap_mode != mipmap_mode {
                continue;
            }

            if let Some((alloc_id, rect)) =
                atlas.alloc(id_allocator, commands, alloc_size, &mut data)
            {
                return Some(((atlas_id, alloc_id), rect));
            }
        }

        let new_atlas_size =
            TextureAtlas::MIN_SIZE.max(alloc_size.max_element().next_power_of_two());

        let mut new_atlas = TextureAtlas::new(
            id_allocator,
            commands,
            alloc_format,
            UVec2::splat(new_atlas_size),
            mipmap_mode,
        );

        let res = new_atlas.alloc(id_allocator, commands, alloc_size, &mut data);
        let atlas_id = self.atlases.insert(new_atlas);

        res.map(|(alloc_id, rect)| ((atlas_id, alloc_id), rect))
    }

    fn free(&mut self, (atlas_id, alloc_id): (AtlasId, AllocId)) {
        self.atlases[atlas_id].free(alloc_id);
    }

    fn cleanup(&mut self, commands: &mut Vec<TextureCommand>) {
        self.atlases.retain(|_, atlas| {
            if !atlas.is_empty() {
                return true;
            }

            commands.push(TextureCommand::Free { id: atlas.texture });

            false
        });
    }
}

struct TextureAtlas {
    texture: TextureId,
    format: ImageFormat,
    size: UVec2,
    allocator: AtlasAllocator,
    mipmap_mode: MipmapMode,
}

impl TextureAtlas {
    const MIN_SIZE: u32 = 512;
    const MAX_SIZE: u32 = 4096;

    fn new(
        id_allocator: &mut TextureIdAllocator,
        commands: &mut Vec<TextureCommand>,
        format: ImageFormat,
        size: UVec2,
        mipmap_mode: MipmapMode,
    ) -> TextureAtlas {
        let texture = id_allocator.alloc();

        commands.push(TextureCommand::CreateDynamic {
            id: texture,
            format,
            size,
            mipmap_mode,
        });

        TextureAtlas {
            texture,
            format,
            size,
            allocator: AtlasAllocator::new(size2d(size)),
            mipmap_mode,
        }
    }

    fn alloc(
        &mut self,
        id_allocator: &mut TextureIdAllocator,
        commands: &mut Vec<TextureCommand>,
        alloc_size: UVec2,
        data: &mut Option<ImageData>,
    ) -> Option<(AllocId, URect)> {
        if let Some(res) = self.try_alloc(commands, alloc_size, data) {
            return Some(res);
        }

        let mut new_size = self.size * 2;
        while new_size.cmplt(alloc_size).any() {
            new_size *= 2;
        }

        if new_size.x >= Self::MAX_SIZE || new_size.y >= Self::MAX_SIZE {
            return None;
        }

        let src_id = self.texture;
        let dst_id = id_allocator.alloc();

        commands.push(TextureCommand::CreateDynamic {
            id: dst_id,
            format: self.format,
            size: new_size,
            mipmap_mode: self.mipmap_mode,
        });

        commands.push(TextureCommand::Copy {
            src_id,
            dst_id,
            src_rect: URect::new(UVec2::ZERO, self.size),
            dst_rect: URect::new(UVec2::ZERO, self.size),
        });

        self.texture = dst_id;
        self.size = new_size;
        self.allocator.grow(size2d(new_size));

        self.try_alloc(commands, alloc_size, data)
    }

    fn try_alloc(
        &mut self,
        commands: &mut Vec<TextureCommand>,
        size: UVec2,
        data: &mut Option<ImageData>,
    ) -> Option<(AllocId, URect)> {
        let alloc = self.allocator.allocate(size2d(size))?;
        let rect = URect::new(
            UVec2::new(alloc.rectangle.min.x as u32, alloc.rectangle.min.y as u32),
            UVec2::new(alloc.rectangle.max.x as u32, alloc.rectangle.max.y as u32),
        );

        commands.push(TextureCommand::Write {
            dst_id: self.texture,
            dst_rect: rect,
            data: data.take().unwrap(),
        });

        Some((alloc.id, rect))
    }

    fn free(&mut self, id: AllocId) {
        self.allocator.deallocate(id);
    }

    fn is_empty(&self) -> bool {
        self.allocator.is_empty()
    }
}

impl fmt::Debug for TextureAtlas {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextureAtlas")
            .field("texture", &self.texture)
            .field("format", &self.format)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

fn size2d(size: UVec2) -> guillotiere::Size {
    guillotiere::Size::new(size.x as i32, size.y as i32)
}

#[derive(Debug, Default)]
struct TextureIdAllocator {
    next_id: TextureId,
}

impl TextureIdAllocator {
    fn alloc(&mut self) -> TextureId {
        let id = self.next_id;
        self.next_id.0 += 1;
        id
    }
}
