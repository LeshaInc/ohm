use ohm_core::renderer::PathCache;

use crate::asset::AssetSources;
use crate::image::ImageDecoders;
use crate::renderer::{Renderer, SurfaceId};
use crate::text::{
    DefaultFontDatabase, DefaultTextShaper, FontDatabase, FontRasterizers, TextShaper,
};
use crate::texture::TextureCache;
use crate::{DrawList, Encoder, EncoderScratch, Result};

/// A convenience structure, encompassing all graphics related objects.
///
/// You can create it using the default constructor, or manually.
pub struct Graphics {
    /// Renderer.
    pub renderer: Box<dyn Renderer>,
    /// Asset sources (for loading images).
    ///
    /// By default, the list will be empty. You will have to add your own
    /// sources (e.g.
    /// [`DirAssetSource`](crate::asset::DirAssetSource)).
    pub asset_sources: AssetSources,
    /// Image decoders
    ///
    /// This will include `image` and `resvg` based decoders, if the
    /// corresponding features are enabled.
    pub image_decoders: ImageDecoders,
    /// Texture cache, used for allocating space for images and glyphs in GPU
    /// textures.
    pub texture_cache: TextureCache,
    /// Path cache, which stores triangualted path strokes and fills.
    pub path_cache: PathCache,
    /// Font database.
    ///
    /// By default, this will be a [`DefaultFontDatabase`].
    pub font_db: Box<dyn FontDatabase>,
    /// Font rasterizers.
    ///
    /// By default, this will include `image`, `freetype`, and `zeno`
    /// rasterizers.
    pub font_rasterizers: FontRasterizers,
    /// Text shaper
    ///
    /// By default, this will be a [`DefaultTextShaper`].
    pub text_shaper: Box<dyn TextShaper>,
}

#[cfg(feature = "wgpu")]
impl Graphics {
    /// Creates a [`Graphics`] instance with `wgpu` based renderer.
    pub fn new_wgpu() -> Graphics {
        Graphics::new(crate::renderer::WgpuRenderer::new())
    }
}

impl Graphics {
    /// Creates a [`Graphics`] instance with the specified renderer.
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

    /// Creates a command encoder, for encoding a [`DrawList`] for a specific
    /// surface.
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

    /// Renders the provided [`DrawList`]'s.
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

    /// Presents the frame to all touched surfaces.
    pub fn present(&mut self) -> Result<()> {
        self.renderer.present()
    }
}
