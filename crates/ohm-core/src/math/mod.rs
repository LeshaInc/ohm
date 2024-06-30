//! Reexports [`glam`] and adds some additional types like [`Rect`] and [`URect`].

mod rect;

pub use glam::*;

pub use self::rect::*;
