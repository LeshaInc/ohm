//! Core types and traits for Ohm (2D rendering library).

mod encoder;
pub mod math;
pub mod renderer;

pub mod asset;
mod color;
mod corner_radii;
mod error;
pub mod image;
mod path;
pub mod text;
pub mod texture;

pub use self::color::*;
pub use self::corner_radii::*;
pub use self::encoder::*;
pub use self::error::*;
use self::image::ImageId;
pub use self::path::*;
use self::renderer::SurfaceId;
use crate::math::{Affine2, Rect, Vec2};
use crate::text::{FontId, GlyphId};

/// A list of draw commands, tied to a single surface.
#[derive(Debug, Clone, Copy)]
pub struct DrawList<'a> {
    /// ID of the target surface.
    pub surface: SurfaceId,
    /// Commands to perform.
    pub commands: &'a [Command<'a>],
}

/// A graphical operation, like drawing or clearing a shape.
#[derive(Debug, Clone)]
pub enum Command<'a> {
    /// Clear a rectangular area.
    ClearRect(ClearRect),
    /// Draw a rectangle with optional borders, rounded corners, and a shadow.
    DrawRect(DrawRect),
    /// Draw a glyph
    DrawGlyph(DrawGlyph),
    /// Draw a sub-layer. Can be used to implement, e.g. scissoring, group tint,
    /// group transparency.
    DrawLayer(DrawLayer<'a>),
    /// Fill a vector path.
    FillPath(FillPath),
    /// Stroke a vector path.
    StrokePath(StrokePath),
}

/// Clear a rectangular area.
#[derive(Debug, Clone, Copy)]
pub struct ClearRect {
    /// Position of the top-left corner.
    pub pos: Vec2,
    /// Size of the rectangle.
    pub size: Vec2,
    /// Color to set without blending. Usually will be [`Color::TRANSPARENT`]
    /// or [`Color::WHITE`].
    pub color: Color,
}

/// Draw a rectangle with optional borders, rounded corners, and a shadow.
#[derive(Debug, Clone, Copy)]
pub struct DrawRect {
    /// Position of the top-left corner
    pub pos: Vec2,
    /// Size of the rectangle (including the borders).
    pub size: Vec2,
    /// Fill (color or an image).
    pub fill: Fill,
    /// Corner radii for rounding.
    pub corner_radii: CornerRadii,
    /// An optional border.
    pub border: Option<Border>,
    /// An optional shadow.
    pub shadow: Option<Shadow>,
}

/// Draw a glyph
#[derive(Debug, Clone, Copy)]
pub struct DrawGlyph {
    /// Position of the origin.
    pub pos: Vec2,
    /// Font size
    pub size: f32,
    /// Font ID.
    pub font: FontId,
    /// Glyph ID.
    pub glyph: GlyphId,
    /// Text color. Does not affect color emoji.
    pub color: Color,
}

/// Draw a sub-layer. Can be used to implement, e.g. scissoring, group tint,
/// group transparency.
#[derive(Debug, Clone, Copy)]
pub struct DrawLayer<'a> {
    /// List of commands to draw in the layer.
    pub commands: &'a [Command<'a>],
    /// Tint of the layer.
    pub tint: Color,
    /// An optional scissor (clip rect).
    pub scissor: Option<Scissor>,
    /// Transform to apply.
    pub transform: Affine2,
}

/// Fill a vector path.
#[derive(Debug, Clone)]
pub struct FillPath {
    /// Origin (i.e. offset applied to all path points)
    pub pos: Vec2,
    /// Path itself.
    pub path: Path,
    /// Fill options (e.g. fill rule).
    pub options: FillOptions,
    /// Fill (color or image).
    pub fill: Fill,
}

/// Stroke a vector path.
#[derive(Debug, Clone)]
pub struct StrokePath {
    /// Origin (i.e. offset applied to all path points)
    pub pos: Vec2,
    /// Path itself.
    pub path: Path,
    /// Stroke options (e.g. width, line cap).
    pub options: StrokeOptions,
    /// Fill (color or an image).
    pub fill: Fill,
}

/// A scissor (clip rect) for restricting draw commands to a specific area.
/// Intended for scroll views in UI.
// TODO: implement this
#[derive(Debug, Clone, Copy)]
pub struct Scissor {
    /// Position of the top-left corner.
    pub pos: Vec2,
    /// Size of the rectangle.
    pub size: Vec2,
    /// Corner radii for a rounded clip rect.
    pub corner_radii: CornerRadii,
}

/// Fill of a shape. Similar to the `background` property in CSS.
#[derive(Debug, Clone, Copy)]
pub enum Fill {
    /// A solid color.
    Solid(Color),
    /// An image.
    Image(FillImage),
}

/// An image used as a fill (background) of a shape.
#[derive(Debug, Clone, Copy)]
pub struct FillImage {
    /// ID of the image.
    pub image: ImageId,
    /// Tint to apply. Set to [`Color::WHITE`] for no tint.
    pub tint: Color,
    /// A rectangle in normalized image coordinates to select a specific portion
    /// of an image.
    ///
    /// For example, the top-left quadrant will be `[0,0] - [0.5,0.5]`.
    pub clip_rect: Option<Rect>,
}

/// Borders of a rectangle.
#[derive(Debug, Clone, Copy)]
pub struct Border {
    /// Color of the borders.
    pub color: Color,
    /// Border width.
    pub width: f32,
}

/// Box shadow. Similar to `box-shadow` property in CSS.
#[derive(Debug, Clone, Copy)]
pub struct Shadow {
    /// Blur radius in pixels.
    pub blur_radius: f32,
    /// Spread radius in pixels.
    pub spread_radius: f32,
    /// Offset of the shadow.
    pub offset: Vec2,
    /// Color of the shadow.
    pub color: Color,
}
