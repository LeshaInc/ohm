use crate::math::{Affine2, UVec2, Vec2};

/// An axis-aligned rectangle, represented by two corners (uses `u32` for coordinates).
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct URect {
    /// A `min` corner (usually top-left).
    pub min: UVec2,
    /// A `max` corner (usually bottom-right).
    pub max: UVec2,
}

impl URect {
    /// All zeros ([0,0] - [0,0] rectangle).
    pub const ZERO: URect = URect::new(UVec2::ZERO, UVec2::ZERO);

    /// Creates a new rectangle
    pub const fn new(min: UVec2, max: UVec2) -> URect {
        URect { min, max }
    }

    /// Returns the size of the rectangle. Equivalent to `max - min`.
    pub fn size(&self) -> UVec2 {
        self.max - self.min
    }
}

/// An axis-aligned rectangle, represented by two corners (uses `f32` for coordinates).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// A `min` corner (usually top-left).
    pub min: Vec2,
    /// A `max` corner (usually bottom-right).
    pub max: Vec2,
}

impl Rect {
    /// All zeros ([0,0] - [0,0] rectangle).
    pub const ZERO: Rect = Rect::new(Vec2::ZERO, Vec2::ZERO);

    /// Creates a new rectangle.
    pub const fn new(min: Vec2, max: Vec2) -> Rect {
        Rect { min, max }
    }

    /// Returns the size of the rectangle. Equivalent to `max - min`.
    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    /// Computes the axis-aligned bounding rectangle of `self` transformed by `transform`.
    pub fn transform(self, transform: &Affine2) -> Rect {
        let mut vertices = [
            self.min,
            Vec2::new(self.min.x, self.max.y),
            Vec2::new(self.max.x, self.min.y),
            self.max,
        ];

        for vertex in &mut vertices {
            *vertex = transform.transform_point2(*vertex);
        }

        let min = vertices.into_iter().reduce(Vec2::min).unwrap_or(self.min);
        let max = vertices.into_iter().reduce(Vec2::max).unwrap_or(self.max);

        Rect::new(min, max)
    }

    /// Computes the union of `self` and `other` rectangles.
    pub fn union(self, other: Rect) -> Rect {
        Rect::new(self.min.min(other.min), self.max.max(other.max))
    }
}
