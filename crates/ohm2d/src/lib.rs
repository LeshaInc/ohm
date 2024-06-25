pub use ohm2d_core::*;
#[cfg(feature = "wgpu")]
pub use ohm2d_wgpu::WgpuRenderer;

pub mod text {
    pub use ohm2d_core::text::*;
    #[cfg(feature = "fontdb")]
    pub use ohm2d_fontdb::SystemFontSource;
    #[cfg(feature = "rustybuzz")]
    pub use ohm2d_rustybuzz::RustybuzzShaper;
    #[cfg(feature = "zeno")]
    pub use ohm2d_zeno::ZenoRasterizer;

    #[derive(Default)]
    pub struct DefaultTextShaper {
        inner: ohm2d_rustybuzz::RustybuzzShaper,
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
}

use crate::text::{
    DefaultTextShaper, EmbeddedImageRasterizer, FontDatabase, FontRasterizers, TextShaper,
};

pub struct Graphics {
    pub renderer: Box<dyn Renderer>,
    pub texture_cache: TextureCache,
    pub font_db: FontDatabase,
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
            texture_cache: TextureCache::new(),
            font_db: FontDatabase::new(),
            font_rasterizers: FontRasterizers::new(),
            text_shaper: Box::new(DefaultTextShaper::new()),
        };

        graphics.default_init();

        graphics
    }

    fn default_init(&mut self) {
        #[cfg(feature = "fontdb")]
        self.font_db
            .add_source(ohm2d_fontdb::SystemFontSource::new());
        self.font_rasterizers.add(EmbeddedImageRasterizer);

        #[cfg(feature = "freetype")]
        self.font_rasterizers
            .add(ohm2d_freetype::FreetypeRasterizer::new());

        #[cfg(feature = "zeno")]
        self.font_rasterizers.add(ohm2d_zeno::ZenoRasterizer::new());
    }

    pub fn render(&mut self, draw_lists: &[DrawList]) -> Result<()> {
        {
            let mut commands = Vec::new();
            self.texture_cache.add_glyphs_from_lists(draw_lists);
            self.texture_cache.load_glyphs(
                &self.font_db,
                &mut self.font_rasterizers,
                &mut commands,
            )?;
            self.texture_cache.load_images(&mut commands)?;
            self.renderer.update_textures(&mut commands)?;
        }

        self.renderer.render(&self.texture_cache, draw_lists)
    }

    pub fn present(&mut self) -> Result<()> {
        self.renderer.present()
    }
}
