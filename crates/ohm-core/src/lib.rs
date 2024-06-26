pub mod math;
pub mod renderer;

pub mod asset;
mod color;
mod corner_radii;
mod error;
pub mod image;
pub mod text;
pub mod texture;

pub use self::color::*;
pub use self::corner_radii::*;
pub use self::error::*;
use self::image::ImageId;
use self::renderer::SurfaceId;
use crate::math::{Affine2, URect, Vec2};
use crate::text::FontId;

#[derive(Debug, Clone, Copy)]
pub struct DrawList<'a> {
    pub surface: SurfaceId,
    pub commands: &'a [Command<'a>],
}

#[derive(Debug, Clone, Copy)]
pub enum Command<'a> {
    DrawRect(DrawRect),
    DrawGlyph(DrawGlyph),
    DrawLayer(DrawLayer<'a>),
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
pub struct DrawLayer<'a> {
    pub commands: &'a [Command<'a>],
    pub tint: Color,
    pub scissor: Option<Scissor>,
    pub transform: Affine2,
}

#[derive(Debug, Clone, Copy)]
pub struct Scissor {
    pub pos: Vec2,
    pub size: Vec2,
    pub corner_radii: CornerRadii,
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
