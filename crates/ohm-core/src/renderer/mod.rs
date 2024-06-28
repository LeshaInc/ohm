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
    pub struct SurfaceId;
}

pub trait Renderer: Send + Sync + 'static {
    fn create_surface(&mut self, window: Arc<dyn WindowHandle>, size: UVec2) -> Result<SurfaceId>;

    fn resize_surface(&mut self, id: SurfaceId, new_size: UVec2) -> Result<()>;

    fn get_surface_size(&self, surface: SurfaceId) -> UVec2;

    fn destroy_surface(&mut self, id: SurfaceId);

    fn update_textures(&mut self, commands: &mut Vec<TextureCommand>) -> Result<()>;

    fn render(
        &mut self,
        texture_cache: &TextureCache,
        path_cache: &mut PathCache,
        draw_lists: &[DrawList<'_>],
    ) -> Result<()>;

    fn present(&mut self) -> Result<()>;
}

pub trait WindowHandle: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static {}

impl<T: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static> WindowHandle for T {}
