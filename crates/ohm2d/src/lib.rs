pub use ohm2d_core::*;
#[cfg(feature = "wgpu")]
pub use ohm2d_wgpu::WgpuRenderer;
use text::DefaultFontDatabase;

pub mod text {
    pub use ohm2d_core::text::*;
    use ohm2d_core::Result;
    #[cfg(feature = "fontdb")]
    pub use ohm2d_fontdb::SystemFontDatabase;
    #[cfg(feature = "rustybuzz")]
    pub use ohm2d_rustybuzz::RustybuzzShaper;
    #[cfg(feature = "zeno")]
    pub use ohm2d_zeno::ZenoRasterizer;

    #[derive(Debug, Default)]
    pub struct DefaultTextShaper {
        #[cfg(feature = "rustybuzz")]
        inner: ohm2d_rustybuzz::RustybuzzShaper,
        #[cfg(not(feature = "rustybuzz"))]
        inner: DummyTextShaper,
    }

    impl DefaultTextShaper {
        pub fn new() -> DefaultTextShaper {
            DefaultTextShaper::default()
        }
    }

    impl TextShaper for DefaultTextShaper {
        fn shape(
            &mut self,
            font_face: &FontFace,
            text: &str,
            size: f32,
            is_rtl: bool,
            buf: &mut Vec<ShapedGlyph>,
        ) {
            self.inner.shape(font_face, text, size, is_rtl, buf);
        }
    }

    #[derive(Debug, Default)]
    pub struct DefaultFontDatabase {
        #[cfg(feature = "fontdb")]
        inner: ohm2d_fontdb::SystemFontDatabase,
        #[cfg(not(feature = "fontdb"))]
        inner: DummyFontDatabase,
    }

    impl DefaultFontDatabase {
        pub fn new() -> DefaultFontDatabase {
            DefaultFontDatabase::default()
        }
    }

    impl FontDatabase for DefaultFontDatabase {
        fn query(&self, attrs: &FontAttrs) -> Option<FontId> {
            self.inner.query(attrs)
        }

        fn load(&mut self, id: FontId) -> Result<&FontFace> {
            self.inner.load(id)
        }

        fn get(&self, id: FontId) -> Option<&FontFace> {
            self.inner.get(id)
        }

        fn get_or_load(&mut self, id: FontId) -> Result<&FontFace> {
            self.inner.get_or_load(id)
        }
    }
}

use crate::text::{DefaultTextShaper, FontDatabase, FontRasterizers, TextShaper};

pub struct Graphics {
    pub renderer: Box<dyn Renderer>,
    pub asset_sources: AssetSources,
    pub image_decoders: ImageDecoders,
    pub texture_cache: TextureCache,
    pub font_db: Box<dyn FontDatabase>,
    pub font_rasterizers: FontRasterizers,
    pub text_shaper: Box<dyn TextShaper>,
}

#[cfg(feature = "wgpu")]
impl Graphics {
    pub fn new_wgpu() -> Graphics {
        Graphics::new(WgpuRenderer::new())
    }
}

impl Graphics {
    pub fn new<R: Renderer>(renderer: R) -> Graphics {
        let mut graphics = Graphics {
            renderer: Box::new(renderer),
            asset_sources: AssetSources::new(),
            image_decoders: ImageDecoders::new(),
            texture_cache: TextureCache::new(),
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
            .add_decoder(ohm2d_image::ImageImageDecoder);

        #[cfg(feature = "image")]
        self.font_rasterizers
            .add_rasterizer(ohm2d_image::EmbeddedImageRasterizer);

        #[cfg(feature = "freetype")]
        self.font_rasterizers
            .add_rasterizer(ohm2d_freetype::FreetypeRasterizer::new());

        #[cfg(feature = "zeno")]
        self.font_rasterizers
            .add_rasterizer(ohm2d_zeno::ZenoRasterizer::new());
    }

    pub fn render(&mut self, draw_lists: &[DrawList]) -> Result<()> {
        {
            let mut commands = Vec::new();
            self.texture_cache.add_glyphs_from_lists(draw_lists);
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

        self.renderer.render(&self.texture_cache, draw_lists)
    }

    pub fn present(&mut self) -> Result<()> {
        self.renderer.present()
    }
}
