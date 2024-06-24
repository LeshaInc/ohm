use crate::math::UVec2;
use crate::{DrawList, TextureCache, TextureCommand};

mod batcher;
pub use self::batcher::*;

slotmap::new_key_type! {
    pub struct SurfaceId;
}

pub trait Renderer {
    fn get_surface_size(&self, surface: SurfaceId) -> UVec2;

    fn update_textures(&mut self, commands: &[TextureCommand]);

    fn render(&mut self, texture_cache: &TextureCache, draw_lists: &[DrawList<'_>]);

    fn present(&mut self);
}
