use crate::math::Vec4;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CornerRadii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl CornerRadii {
    #[inline]
    pub fn new(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> CornerRadii {
        CornerRadii {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    #[inline]
    pub fn new_equal(v: f32) -> CornerRadii
    where
        f32: Copy,
    {
        CornerRadii {
            top_left: v,
            top_right: v,
            bottom_right: v,
            bottom_left: v,
        }
    }
}

impl From<[f32; 4]> for CornerRadii {
    #[inline]
    fn from([l, r, b, t]: [f32; 4]) -> Self {
        Self::new(l, r, b, t)
    }
}

impl From<f32> for CornerRadii {
    #[inline]
    fn from(v: f32) -> Self {
        Self::new_equal(v)
    }
}

impl From<CornerRadii> for Vec4 {
    fn from(v: CornerRadii) -> Self {
        Vec4::new(v.top_left, v.top_right, v.bottom_right, v.bottom_left)
    }
}
