use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::math::Vec2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathEvent {
    MoveTo { point: Vec2 },
    LineTo { point: Vec2 },
    QuadTo { control: Vec2, point: Vec2 },
    CubicTo { control: [Vec2; 2], point: Vec2 },
    Close,
}

#[derive(Default)]
pub struct PathBuilder {
    events: Vec<PathEvent>,
}

impl PathBuilder {
    pub fn new() -> PathBuilder {
        PathBuilder::default()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn move_to(&mut self, point: Vec2) {
        self.events.push(PathEvent::MoveTo { point });
    }

    pub fn line_to(&mut self, point: Vec2) {
        self.events.push(PathEvent::LineTo { point });
    }

    pub fn quad_to(&mut self, control: Vec2, point: Vec2) {
        self.events.push(PathEvent::QuadTo { control, point });
    }

    pub fn cubic_to(&mut self, control1: Vec2, control2: Vec2, point: Vec2) {
        self.events.push(PathEvent::CubicTo {
            control: [control1, control2],
            point,
        });
    }

    pub fn close(&mut self) {
        self.events.push(PathEvent::Close);
    }

    pub fn finish(&mut self) -> Path {
        let path = Path {
            events: self.events.clone().into(),
        };
        self.clear();
        path
    }
}

#[derive(Debug, Clone)]
pub struct Path {
    events: Arc<[PathEvent]>,
}

impl Path {
    pub fn events(&self) -> &[PathEvent] {
        &self.events
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum FillRule {
    #[default]
    EvenOdd,
    NonZero,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub struct FillOptions {
    pub fill_rule: FillRule,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum LineCap {
    #[default]
    Butt,
    Square,
    Round,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum LineJoin {
    #[default]
    Miter,
    MiterClip,
    Round,
    Bevel,
}

#[derive(Debug, Clone, Copy)]
pub struct StrokeOptions {
    pub line_cap: LineCap,
    pub line_join: LineJoin,
    pub line_width: f32,
    pub mitter_limit: f32,
}

impl Default for StrokeOptions {
    fn default() -> Self {
        Self {
            line_cap: LineCap::default(),
            line_join: LineJoin::default(),
            line_width: 1.0,
            mitter_limit: 4.0,
        }
    }
}

impl PartialEq for StrokeOptions {
    fn eq(&self, other: &Self) -> bool {
        self.line_cap == other.line_cap
            && self.line_join == other.line_join
            && self.line_width.to_bits() == other.line_width.to_bits()
            && self.mitter_limit.to_bits() == other.mitter_limit.to_bits()
    }
}

impl Eq for StrokeOptions {}

impl Hash for StrokeOptions {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.line_cap.hash(state);
        self.line_join.hash(state);
        self.line_width.to_bits().hash(state);
        self.mitter_limit.to_bits().hash(state);
    }
}
