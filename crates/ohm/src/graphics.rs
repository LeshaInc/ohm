use ohm_core::renderer::PathCache;

use crate::asset::AssetSources;
use crate::encoder::EncoderScratch;
use crate::image::ImageDecoders;
use crate::renderer::{Renderer, SurfaceId};
use crate::text::{
    DefaultFontDatabase, DefaultTextShaper, FontDatabase, FontRasterizers, TextShaper,
};
use crate::texture::TextureCache;
use crate::{DrawList, Encoder, Result};

pub struct Graphics {
    pub renderer: Box<dyn Renderer>,
    pub asset_sources: AssetSources,
    pub image_decoders: ImageDecoders,
    pub texture_cache: TextureCache,
    pub path_cache: PathCache,
    pub font_db: Box<dyn FontDatabase>,
    pub font_rasterizers: FontRasterizers,
    pub text_shaper: Box<dyn TextShaper>,
}

#[cfg(feature = "wgpu")]
impl Graphics {
    pub fn new_wgpu() -> Graphics {
        Graphics::new(crate::renderer::WgpuRenderer::new())
    }
}

impl Graphics {
    pub fn new<R: Renderer>(renderer: R) -> Graphics {
        let mut graphics = Graphics {
            renderer: Box::new(renderer),
            asset_sources: AssetSources::new(),
            image_decoders: ImageDecoders::new(),
            texture_cache: TextureCache::new(),
            path_cache: PathCache::new(),
            font_db: Box::new(DefaultFontDatabase::new()),
            font_rasterizers: FontRasterizers::new(),
            text_shaper: Box::new(DefaultTextShaper::new()),
        };

        graphics.default_init();

        graphics
    }

    fn default_init(&mut self) {
        #[cfg(feature = "image")]
        self.image_decoders
            .add_decoder(ohm_image::ImageImageDecoder);

        #[cfg(feature = "resvg")]
        self.image_decoders
            .add_decoder(ohm_resvg::ResvgImageDecoder);

        #[cfg(feature = "image")]
        self.font_rasterizers
            .add_rasterizer(ohm_image::EmbeddedImageRasterizer);

        #[cfg(feature = "freetype")]
        self.font_rasterizers
            .add_rasterizer(ohm_freetype::FreetypeRasterizer::new());

        #[cfg(feature = "zeno")]
        self.font_rasterizers
            .add_rasterizer(ohm_zeno::ZenoRasterizer::new());
    }

    pub fn create_encoder<'g, 's>(
        &'g mut self,
        scratch: &'s EncoderScratch,
        surface: SurfaceId,
    ) -> Encoder<'g, 's> {
        Encoder::new(
            scratch,
            &mut *self.font_db,
            &mut *self.text_shaper,
            &mut self.texture_cache,
            surface,
        )
    }

    pub fn render(&mut self, draw_lists: &[DrawList]) -> Result<()> {
        {
            let mut commands = Vec::new();
            self.texture_cache.add_glyphs_from_lists(draw_lists);
            self.texture_cache
                .set_image_sizes_from_lists(&mut self.path_cache, draw_lists);
            self.texture_cache.load_glyphs(
                &*self.font_db,
                &mut self.font_rasterizers,
                &mut commands,
            )?;
            self.texture_cache.load_images(
                &self.asset_sources,
                &self.image_decoders,
                &mut commands,
            )?;
            self.renderer.update_textures(&mut commands)?;
        }

        self.renderer
            .render(&self.texture_cache, &mut self.path_cache, draw_lists)
    }

    pub fn present(&mut self) -> Result<()> {
        self.renderer.present()
    }
}
