//! Contains the [`Renderer`] trait and some utilities for implementing custom
//! renderers.

use std::sync::Arc;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::math::UVec2;
use crate::texture::{TextureCache, TextureCommand};
use crate::{DrawList, Result};

mod batcher;
mod path_cache;
pub use self::batcher::*;
pub use self::path_cache::*;

slotmap::new_key_type! {
    /// ID of a surface (usually a window).
    pub struct SurfaceId;
}

/// A trait implemented by all renderers.
///
/// A renderer should manage resources like surfaces (windows) and textures, and
/// implement the actual rendering of [`DrawList`] onto the surface.
pub trait Renderer: Send + Sync + 'static {
    /// Creates a surface from the specified [`WindowHandle`] with a provided
    /// initial size.
    fn create_surface(&mut self, window: Arc<dyn WindowHandle>, size: UVec2) -> Result<SurfaceId>;

    /// Resizes a surface.
    ///
    /// # Panics
    ///
    /// This method is allowed to panic if the provided [`SurfaceId`] is
    /// invalid.
    fn resize_surface(&mut self, id: SurfaceId, new_size: UVec2) -> Result<()>;

    /// Returns the size of the provided surface.
    ///
    /// # Panics
    ///
    /// This method is allowed to panic if the provided [`SurfaceId`] is
    /// invalid.
    fn get_surface_size(&self, surface: SurfaceId) -> UVec2;

    /// Destroys a surface, releasing all of its associated memory.
    ///
    /// # Panics
    ///
    /// This method is allowed to panic if the provided [`SurfaceId`] is
    /// invalid.
    fn destroy_surface(&mut self, id: SurfaceId);

    /// Updates internally managed textures, applying each [`TextureCommand`] in
    /// sequence.
    ///
    /// This includes: creating new textures, copying data between textures,
    /// writing data to textures, and destroying textures.
    ///
    /// The provided vector is expected to be empty after a successful return.
    fn update_textures(&mut self, commands: &mut Vec<TextureCommand>) -> Result<()>;

    /// Renders each [`DrawList`] into its associated surface.
    fn render(
        &mut self,
        texture_cache: &TextureCache,
        path_cache: &mut PathCache,
        draw_lists: &[DrawList<'_>],
    ) -> Result<()>;

    /// Presents all touched surfaces to the screen.
    fn present(&mut self) -> Result<()>;
}

/// A trait for window handles. In most cases, this will be a `Window` from
/// `winit`.
pub trait WindowHandle: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static {}

impl<T: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static> WindowHandle for T {}
