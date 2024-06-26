pub use ohm2d_core::*;

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

pub mod renderer {
    pub use ohm2d_core::renderer::*;
    #[cfg(feature = "wgpu")]
    pub use ohm2d_wgpu::WgpuRenderer;
}

mod graphics;

pub use self::graphics::Graphics;
