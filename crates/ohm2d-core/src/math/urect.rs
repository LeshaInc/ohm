use crate::math::UVec2;

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
