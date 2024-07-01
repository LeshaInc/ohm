use crate::math::Vec4;

/// Defines the radii of four rectangle corners.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CornerRadii {
    /// Radius of the top-left corner.
    pub top_left: f32,
    /// Radius of the top-right corner.
    pub top_right: f32,
    /// Radius of the bottom-right corner.
    pub bottom_right: f32,
    /// Radius of the bottom-left corner.
    pub bottom_left: f32,
}

impl CornerRadii {
    /// Creates [`CornerRadii`] with all four specified corners, starting from
    /// top-left in clockwise order.
    #[inline]
    pub fn new(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> CornerRadii {
        CornerRadii {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    /// Creates [`CornerRadii`] with all corners' radii equal to the specified
    /// value.
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
