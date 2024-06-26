use crate::math::{UVec2, Vec2};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct URect {
    pub min: UVec2,
    pub max: UVec2,
}

impl URect {
    pub const ZERO: URect = URect::new(UVec2::ZERO, UVec2::ZERO);

    pub const fn new(min: UVec2, max: UVec2) -> URect {
        URect { min, max }
    }

    pub fn size(&self) -> UVec2 {
        self.max - self.min
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    pub const ZERO: Rect = Rect::new(Vec2::ZERO, Vec2::ZERO);

    pub const fn new(min: Vec2, max: Vec2) -> Rect {
        Rect { min, max }
    }

    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }
}
