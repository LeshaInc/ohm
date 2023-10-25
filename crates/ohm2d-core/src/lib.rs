pub mod math;

mod asset;
mod batcher;
mod color;
mod corner_radii;
mod image;
pub mod text;
mod texture;

pub use self::asset::*;
pub use self::batcher::*;
pub use self::color::*;
pub use self::corner_radii::*;
pub use self::image::*;
pub use self::texture::*;
use crate::math::{URect, UVec2, Vec2};
use crate::text::FontId;

slotmap::new_key_type! {
    pub struct SurfaceId;
}

pub trait Renderer {
    fn get_surface_size(&self, surface: SurfaceId) -> UVec2;

    fn update_textures(&mut self, commands: &[TextureCommand]);

    fn render(&mut self, texture_cache: &TextureCache, draw_lists: &[DrawList<'_>]);

    fn present(&mut self);
}

#[derive(Debug, Clone, Copy)]
pub struct DrawList<'a> {
    pub surface: SurfaceId,
    pub commands: &'a [Command],
}

#[derive(Debug, Clone, Copy)]
pub enum Command {
    Clear(Color),
    DrawRect(DrawRect),
    DrawGlyph(DrawGlyph),
    BeginAlpha(f32),
    EndAlpha,
}

#[derive(Debug, Clone, Copy)]
pub struct DrawRect {
    pub pos: Vec2,
    pub size: Vec2,
    pub fill: Fill,
    pub corner_radii: CornerRadii,
    pub border: Option<Border>,
    pub shadow: Option<Shadow>,
}

#[derive(Debug, Clone, Copy)]
pub struct DrawGlyph {
    pub pos: Vec2,
    pub size: f32,
    pub font: FontId,
    pub glyph: u16,
    pub color: Color,
}

#[derive(Debug, Clone, Copy)]
pub enum Fill {
    Solid(Color),
    Image(FillImage),
}

#[derive(Debug, Clone, Copy)]
pub struct FillImage {
    pub image: ImageId,
    pub tint: Color,
    pub clip_rect: Option<URect>, // TODO
}

#[derive(Debug, Clone, Copy)]
pub struct Border {
    pub color: Color,
    pub width: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Shadow {
    pub blur_radius: f32,
    pub spread_radius: f32,
    pub offset: Vec2,
    pub color: Color,
}
