use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::math::Vec2;

/// A path builder operation.
///
/// All coordinates are absolute.
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(missing_docs)]
pub enum PathEvent {
    /// Move to the point without drawing anything.
    MoveTo { point: Vec2 },
    /// Move to the point drawing a line.
    LineTo { point: Vec2 },
    /// Draw a quadratic bezier curve.
    QuadTo { control: Vec2, point: Vec2 },
    /// Draw a cubic bezier curve.
    CubicTo { control: [Vec2; 2], point: Vec2 },
    /// Close the path.
    Close,
}

/// A [`Path`] builder.
///
/// All coordinates are absolute.
#[derive(Default)]
pub struct PathBuilder {
    events: Vec<PathEvent>,
}

impl PathBuilder {
    /// Create a new [`Path`] builder.
    pub fn new() -> PathBuilder {
        PathBuilder::default()
    }

    /// Clears the internal buffers, keeping the allocation.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Moves to the point without drawing anything.
    pub fn move_to(&mut self, point: Vec2) {
        self.events.push(PathEvent::MoveTo { point });
    }

    /// Draws a line.
    pub fn line_to(&mut self, point: Vec2) {
        self.events.push(PathEvent::LineTo { point });
    }

    /// Draws a quadratic bezier curve.
    pub fn quad_to(&mut self, control: Vec2, point: Vec2) {
        self.events.push(PathEvent::QuadTo { control, point });
    }

    /// Draws a cubic bezier curve.
    pub fn cubic_to(&mut self, control1: Vec2, control2: Vec2, point: Vec2) {
        self.events.push(PathEvent::CubicTo {
            control: [control1, control2],
            point,
        });
    }

    /// Closes the path.
    pub fn close(&mut self) {
        self.events.push(PathEvent::Close);
    }

    /// Returns a new [`Path`] from all previously recorded events.
    pub fn finish(&mut self) -> Path {
        let path = Path {
            events: self.events.clone().into(),
        };
        self.clear();
        path
    }
}

/// A 2D vector path.
#[derive(Debug, Clone)]
pub struct Path {
    events: Arc<[PathEvent]>,
}

impl Path {
    /// Returns a list of events, making up this path.
    pub fn events(&self) -> &[PathEvent] {
        &self.events
    }
}

/// Fill rule, used to determine the inside part of a shape.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum FillRule {
    /// Draw a line from a point to infinity, counting the number of
    /// intersections. If the number is odd, the point is inside the shape.
    #[default]
    EvenOdd,
    /// Draw a line from a point to infinity, counting the number of
    /// intersections. If the number if nonzero, the point is inside the shape.
    NonZero,
}

/// Options for filling a shape.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub struct FillOptions {
    /// Fill rule, specifying the inside part of the shape.
    pub fill_rule: FillRule,
}

/// Defines the shape of the ends of open stroked paths.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum LineCap {
    /// The ends don't extend beyound the endpoint.
    #[default]
    Butt,
    /// The ends are extended by a rectangle with width equal to half stroke
    /// width.
    Square,
    /// The ends are extended by a half circle.
    Round,
}

/// Defines the shape used to join two line segments.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum LineJoin {
    /// The segments create a sharp corner, regardless of the angle.
    #[default]
    Miter,
    /// The segments create a sharp corner, unless the `mitter_limit` is
    /// exceeded, in which case the corner will be clipped.
    MiterClip,
    /// The segments are connected by a circular arc, creating a round corner.
    Round,
    /// The segments create a bevelled corner.
    Bevel,
}

/// Options for stroking a shape.
#[derive(Debug, Clone, Copy)]
pub struct StrokeOptions {
    /// Defines the shape of the ends of open paths.
    pub line_cap: LineCap,
    /// Defines the shape used to join two line segments.
    pub line_join: LineJoin,
    /// Defines the width (thickness) of the stroke.
    pub line_width: f32,
    /// Defines the maximum miter length, used by [`LineJoin::MiterClip`].
    pub miter_limit: f32,
}

impl Default for StrokeOptions {
    fn default() -> Self {
        Self {
            line_cap: LineCap::default(),
            line_join: LineJoin::default(),
            line_width: 1.0,
            miter_limit: 4.0,
        }
    }
}

impl PartialEq for StrokeOptions {
    fn eq(&self, other: &Self) -> bool {
        self.line_cap == other.line_cap
            && self.line_join == other.line_join
            && self.line_width.to_bits() == other.line_width.to_bits()
            && self.miter_limit.to_bits() == other.miter_limit.to_bits()
    }
}

impl Eq for StrokeOptions {}

impl Hash for StrokeOptions {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.line_cap.hash(state);
        self.line_join.hash(state);
        self.line_width.to_bits().hash(state);
        self.miter_limit.to_bits().hash(state);
    }
}
