pub mod math;
pub mod renderer;

mod asset;
mod color;
mod corner_radii;
mod image;
pub mod text;
mod texture;

pub use self::asset::*;
pub use self::color::*;
pub use self::corner_radii::*;
pub use self::image::*;
pub use self::renderer::Renderer;
use self::renderer::SurfaceId;
pub use self::texture::*;
use crate::math::{URect, Vec2};
use crate::text::FontId;

#[derive(Debug, Clone, Copy)]
pub struct DrawList<'a> {
    pub surface: SurfaceId,
    pub layers: &'a [Layer<'a>],
}

#[derive(Debug, Clone, Copy)]
pub struct Layer<'a> {
    pub commands: &'a [Command],
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct LayerId(pub usize);

#[derive(Debug, Clone, Copy)]
pub struct Scissor {
    pub pos: Vec2,
    pub size: Vec2,
    pub corner_radii: CornerRadii,
}

#[derive(Debug, Clone, Copy)]
pub enum Backdrop {
    Transparent,
    Fill(Color),
    Copy,
}

#[derive(Debug, Clone, Copy)]
pub enum Effect {
    Blur(BlurEffect),
}

#[derive(Debug, Clone, Copy)]
pub struct BlurEffect {
    pub radius: Vec2,
}

#[derive(Debug, Clone, Copy)]
pub enum Command {
    DrawRect(DrawRect),
    DrawGlyph(DrawGlyph),
    DrawLayer(DrawLayer),
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
pub struct DrawLayer {
    pub id: LayerId,
    pub tint: Color,
    pub scissor: Option<Scissor>,
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
