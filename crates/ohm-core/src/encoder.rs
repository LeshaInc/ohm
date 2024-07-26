use std::ops::{Deref, DerefMut};

use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;

use crate::asset::AssetPath;
use crate::image::{ImageHandle, ImageId};
use crate::math::{Affine2, Rect, Vec2};
use crate::renderer::SurfaceId;
use crate::text::{FontDatabase, TextBuffer, TextShaper};
use crate::texture::{MipmapMode, TextureCache};
use crate::{
    Border, ClearRect, Color, Command, CornerRadii, DrawGlyph, DrawLayer, DrawList, DrawRect, Fill,
    FillImage, FillOptions, FillPath, Path, Scissor, Shadow, StrokeOptions, StrokePath,
};

/// Scratch allocations, used by [`Encoder`].
#[derive(Default)]
pub struct EncoderScratch {
    bump: Bump,
}

impl EncoderScratch {
    /// Creates an empty scratch space.
    pub fn new() -> EncoderScratch {
        Default::default()
    }
}

/// Encoder for constructing a [`DrawList`].
pub struct Encoder<'g, 's> {
    /// Font database
    pub font_db: &'g mut dyn FontDatabase,
    /// Text shaper
    pub text_shaper: &'g mut dyn TextShaper,
    /// Texture cache
    pub texture_cache: &'g mut TextureCache,

    bump: &'s Bump,
    surface: SurfaceId,
    commands: BumpVec<'s, Command<'s>>,
}

impl<'g, 's> Encoder<'g, 's> {
    /// Creates a command encoder.
    pub fn new(
        scratch: &'s EncoderScratch,
        font_db: &'g mut dyn FontDatabase,
        text_shaper: &'g mut dyn TextShaper,
        texture_cache: &'g mut TextureCache,
        surface: SurfaceId,
    ) -> Encoder<'g, 's> {
        Encoder {
            bump: &scratch.bump,
            font_db,
            text_shaper,
            texture_cache,
            surface,
            commands: BumpVec::new_in(&scratch.bump),
        }
    }

    /// Finishes recording the commands, returning a [`DrawList`], ready for
    /// rendering.
    pub fn finish(self) -> DrawList<'s> {
        DrawList {
            surface: self.surface,
            commands: self.commands.into_bump_slice(),
        }
    }

    fn command(&mut self, command: Command<'s>) {
        self.commands.push(command);
    }

    /// Clears a rectangle (fills it with the specified color without blending).
    pub fn clear_rect(
        &mut self,
        pos: impl Into<Vec2>,
        size: impl Into<Vec2>,
        color: impl Into<Color>,
    ) {
        self.commands.push(Command::ClearRect(ClearRect {
            pos: pos.into(),
            size: size.into(),
            color: color.into(),
        }))
    }

    /// Draws a rectangle.
    pub fn rect(&mut self, pos: impl Into<Vec2>, size: impl Into<Vec2>) -> RectBuilder<'_, 'g, 's> {
        RectBuilder {
            encoder: self,
            pos: pos.into(),
            size: size.into(),
            corner_radii: CornerRadii::new_equal(0.0),
            fill: Fill::Solid(Color::BLACK),
            border: None,
            shadow: None,
        }
    }

    /// Draws a [`TextBuffer`].
    pub fn text(&mut self, pos: impl Into<Vec2>, buffer: &TextBuffer) {
        self.text_inner(pos.into(), buffer);
    }

    fn text_inner(&mut self, pos: Vec2, buffer: &TextBuffer) {
        for run in buffer.runs() {
            let mut pos = pos + run.pos;
            for glyph in &buffer.glyphs()[run.glyph_range.clone()] {
                self.command(Command::DrawGlyph(DrawGlyph {
                    pos: pos + glyph.offset,
                    size: run.font_size,
                    font: run.font,
                    glyph: glyph.glyph_id,
                    color: run.color,
                }));
                pos.x += glyph.x_advance;
            }
        }
    }

    /// Fills a path.
    pub fn fill_path(&mut self, pos: impl Into<Vec2>, path: &Path) -> FillPathBuilder<'_, 'g, 's> {
        FillPathBuilder {
            encoder: self,
            pos: pos.into(),
            path: Some(path.clone()),
            options: FillOptions::default(),
            fill: Fill::Solid(Color::BLACK),
        }
    }

    /// Strokes a path.
    pub fn stroke_path(
        &mut self,
        pos: impl Into<Vec2>,
        path: &Path,
    ) -> StrokePathBuilder<'_, 'g, 's> {
        let path = self.bump.alloc(path.clone());
        StrokePathBuilder {
            encoder: self,
            pos: pos.into(),
            path: Some(path.clone()),
            options: StrokeOptions::default(),
            fill: Fill::Solid(Color::BLACK),
        }
    }

    /// Draws a layer, returning the child encoder for the new layer. It should
    /// be dropped before you can use the parent (this) encoder again.
    pub fn layer(&mut self) -> LayerEncoder<'_, 'g, 's> {
        let parent_commands = std::mem::replace(&mut self.commands, BumpVec::new_in(self.bump));
        LayerEncoder {
            encoder: self,
            parent_commands: Some(parent_commands),
            tint: Color::WHITE,
            scissor: None,
            transform: Affine2::IDENTITY,
        }
    }
}

/// Builder for specifying rectangle parameters.
pub struct RectBuilder<'e, 'g, 's> {
    encoder: &'e mut Encoder<'g, 's>,
    pos: Vec2,
    size: Vec2,
    corner_radii: CornerRadii,
    fill: Fill,
    border: Option<Border>,
    shadow: Option<Shadow>,
}

impl RectBuilder<'_, '_, '_> {
    /// Specifies a solid fill color.
    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.fill = Fill::Solid(color.into());
        self
    }

    /// Specifies an image fill, by handle
    pub fn image(self, image: &ImageHandle) -> Self {
        self.image_id(image.id())
    }

    /// Specifies an image fill, by path
    pub fn image_path<'a>(self, image: impl Into<AssetPath<'a>>) -> Self {
        let image = self
            .encoder
            .texture_cache
            .add_image_from_path(image, MipmapMode::Enabled);
        self.image_id(image.id())
    }

    /// Specifies an image fill, by ID
    pub fn image_id(mut self, image: ImageId) -> Self {
        self.fill = Fill::Image(FillImage {
            image,
            tint: Color::WHITE,
            clip_rect: None,
        });

        self
    }

    /// Specifies tint of the image fill.
    pub fn image_tint(mut self, color: impl Into<Color>) -> Self {
        if let Fill::Image(image) = &mut self.fill {
            image.tint = color.into();
        }

        self
    }

    /// Specifies clip rect of the image fill.
    pub fn image_clip_rect(mut self, clip_rect: impl Into<Rect>) -> Self {
        if let Fill::Image(image) = &mut self.fill {
            image.clip_rect = Some(clip_rect.into());
        }

        self
    }

    /// Specifies the corner radii for a rounded rectangle.
    pub fn corner_radii(mut self, corner_radii: impl Into<CornerRadii>) -> Self {
        self.corner_radii = corner_radii.into();
        self
    }

    /// Specifies border color and width (thickness).
    pub fn border(mut self, color: impl Into<Color>, width: f32) -> Self {
        self.border = Some(Border {
            color: color.into(),
            width,
        });

        self
    }

    /// Spesifies a shadow.
    pub fn shadow(mut self, shadow: impl Into<Shadow>) -> Self {
        self.shadow = Some(shadow.into());
        self
    }
}

impl Drop for RectBuilder<'_, '_, '_> {
    fn drop(&mut self) {
        self.encoder.command(Command::DrawRect(DrawRect {
            pos: self.pos,
            size: self.size,
            fill: self.fill,
            corner_radii: self.corner_radii,
            border: self.border,
            shadow: self.shadow,
        }));
    }
}

pub struct FillPathBuilder<'e, 'g, 's> {
    encoder: &'e mut Encoder<'g, 's>,
    pos: Vec2,
    path: Option<Path>,
    options: FillOptions,
    fill: Fill,
}

impl FillPathBuilder<'_, '_, '_> {
    /// Specifies a solid fill color.
    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.fill = Fill::Solid(color.into());
        self
    }

    /// Specifies an image fill, by handle
    pub fn image(self, image: &ImageHandle) -> Self {
        self.image_id(image.id())
    }

    /// Specifies an image fill, by path
    pub fn image_path<'a>(self, image: impl Into<AssetPath<'a>>) -> Self {
        let image = self
            .encoder
            .texture_cache
            .add_image_from_path(image, MipmapMode::Enabled);
        self.image_id(image.id())
    }

    /// Specifies an image fill, by ID
    pub fn image_id(mut self, image: ImageId) -> Self {
        self.fill = Fill::Image(FillImage {
            image,
            tint: Color::WHITE,
            clip_rect: None,
        });

        self
    }

    /// Specifies tint of the image fill.
    pub fn image_tint(mut self, color: impl Into<Color>) -> Self {
        if let Fill::Image(image) = &mut self.fill {
            image.tint = color.into();
        }

        self
    }

    /// Specifies clip rect of the image fill.
    pub fn image_clip_rect(mut self, clip_rect: impl Into<Rect>) -> Self {
        if let Fill::Image(image) = &mut self.fill {
            image.clip_rect = Some(clip_rect.into());
        }

        self
    }
}

impl Drop for FillPathBuilder<'_, '_, '_> {
    fn drop(&mut self) {
        self.encoder.command(Command::FillPath(FillPath {
            pos: self.pos,
            path: self.path.take().unwrap(),
            options: self.options,
            fill: self.fill,
        }))
    }
}

pub struct StrokePathBuilder<'e, 'g, 's> {
    encoder: &'e mut Encoder<'g, 's>,
    pos: Vec2,
    path: Option<Path>,
    options: StrokeOptions,
    fill: Fill,
}

impl StrokePathBuilder<'_, '_, '_> {
    /// Specifies a solid fill color.
    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.fill = Fill::Solid(color.into());
        self
    }
}

impl Drop for StrokePathBuilder<'_, '_, '_> {
    fn drop(&mut self) {
        self.encoder.command(Command::StrokePath(StrokePath {
            pos: self.pos,
            path: self.path.take().unwrap(),
            options: self.options,
            fill: self.fill,
        }))
    }
}

/// Layer encoder.
pub struct LayerEncoder<'e, 'g, 's> {
    encoder: &'e mut Encoder<'g, 's>,
    parent_commands: Option<BumpVec<'s, Command<'s>>>,
    tint: Color,
    scissor: Option<Scissor>,
    transform: Affine2,
}

impl LayerEncoder<'_, '_, '_> {
    /// Specifies the tint of this layer.
    pub fn tint(mut self, color: impl Into<Color>) -> Self {
        self.tint = color.into();
        self
    }

    /// Specifies the transform of this layer.
    pub fn transform(mut self, transform: impl Into<Affine2>) -> Self {
        if self.transform == Affine2::IDENTITY {
            self.transform = transform.into();
        } else {
            self.transform *= transform.into();
        }

        self
    }

    /// Specifies the scissor of this layer.
    pub fn scissor(mut self, scissor: impl Into<Scissor>) -> Self {
        self.scissor = Some(scissor.into());
        self
    }
}

impl<'g, 's> Deref for LayerEncoder<'_, 'g, 's> {
    type Target = Encoder<'g, 's>;

    fn deref(&self) -> &Self::Target {
        self.encoder
    }
}

impl DerefMut for LayerEncoder<'_, '_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.encoder
    }
}

impl Drop for LayerEncoder<'_, '_, '_> {
    fn drop(&mut self) {
        let parent_commands = self.parent_commands.take().unwrap();
        let child_commands = std::mem::replace(&mut self.encoder.commands, parent_commands);
        self.encoder.command(Command::DrawLayer(DrawLayer {
            commands: child_commands.into_bump_slice(),
            tint: self.tint,
            scissor: self.scissor,
            transform: self.transform,
        }));
    }
}
