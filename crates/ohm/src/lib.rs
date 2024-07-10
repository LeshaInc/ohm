#![warn(missing_docs)]

//! 2D rendering library, intended for user interfaces.
//!
//! # Features
//!
//! - Can draw rectangles with rounded corners, borders, and shadows.
//!
//! - Can draw text using various rasterization backends (freetype, zeno).
//!
//! - Can fill/stroke vector paths using triangulation through lyon.
//!
//! - Supports layers for implementing things such as group transparency or tint
//!
//! - Everything is pluggable: you can implement your own [`Renderer`],
//!   [`TextShaper`], [`FontDatabase`], [`FontRasterizer`], [`AssetSource`],
//!   [`ImageDecoder`].
//!
//! # Philosophy
//!
//! The philosophy of Ohm is to optimize rendering for the most common
//! primitives in modern UI: rounded rectangles with borders, shadows and text.
//!
//! Ohm is not a general purpose vector graphics renderer. It's not intended for
//! rendering high quality SVG images (for that you'd use resvg, which is
//! integrated with Ohm through `resvg` feature)
//!
//! The default renderer can draw a textured rectangle with rounded corners,
//! borders and a shadow using a single quad. It can also draw text using the
//! same ubershader, minimizing the number draw calls. The renderer doesn't
//! require any special hardware features, such as compute shaders, so should
//! run anywhere.
//!
//! # External dependencies and cargo features
//!
//! - Enable `fontdb` feature for access to system fonts (backed by `fontdb`).
//!
//! - Enable `freetype` feature for the FreeType font renderer
//!
//! - Enable `image` feature for decoding raster images (backed by `image`).
//!
//!   This also includes raster images, embedded in fonts (such as emoji).
//!
//!   You can also enable specific formats using features, such as `image/png`,
//!   `image/jpeg`.
//!
//!  - Enable `resvg` feature for rendering SVG images (backed by `resvg`).
//!
//!  - Enable `rustybuzz` feature for text shaping (backed by `rustybuzz`).
//!
//!  - Enable `wgpu` feature for the default renderer implementation (backed by
//!    `wgpu`).
//!
//!  - Enable `zeno` feature for a portable font renderer (backed by `zeno`).
//!
//! By default, all features are enabled.
//!
//! [`Renderer`]: crate::renderer::Renderer
//! [`TextShaper`]: crate::text::TextShaper
//! [`FontDatabase`]: crate::text::FontDatabase
//! [`FontRasterizer`]: crate::text::FontRasterizer
//! [`AssetSource`]: crate::asset::AssetSource
//! [`ImageDecoder`]: crate::image::ImageDecoder

pub use ohm_core::*;

pub mod text {
    //! Types and traits related font loading, text shaping and layout and glyph
    //! rasterization

    pub use ohm_core::text::*;
    use ohm_core::Result;
    #[cfg(feature = "fontdb")]
    pub use ohm_fontdb::SystemFontDatabase;
    #[cfg(feature = "rustybuzz")]
    pub use ohm_rustybuzz::RustybuzzShaper;
    #[cfg(feature = "zeno")]
    pub use ohm_zeno::ZenoRasterizer;

    /// Default text shaper, based on cargo features
    ///
    /// When `rustybuzz` feature is enabled, uses [`RustybuzzShaper`].
    ///
    /// Otherwise uses [`DummyTextShaper`].
    #[derive(Debug, Default)]
    pub struct DefaultTextShaper {
        #[cfg(feature = "rustybuzz")]
        inner: RustybuzzShaper,
        #[cfg(not(feature = "rustybuzz"))]
        inner: DummyTextShaper,
    }

    impl DefaultTextShaper {
        /// Creates a new default text shaper.
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

    /// Default font database, based on cargo features
    ///
    /// When `fontdb` feature is enabled, uses [`SystemFontDatabase`].
    ///
    /// Otherwise uses [`DummyFontDatabase`].
    #[derive(Debug, Default)]
    pub struct DefaultFontDatabase {
        #[cfg(feature = "fontdb")]
        inner: SystemFontDatabase,
        #[cfg(not(feature = "fontdb"))]
        inner: DummyFontDatabase,
    }

    impl DefaultFontDatabase {
        /// Creates a new default font database.
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
    //! Renderer trait, helpers, and implementations.

    pub use ohm_core::renderer::*;
    #[cfg(feature = "wgpu")]
    pub use ohm_wgpu::WgpuRenderer;
}

mod encoder;
mod graphics;

pub use self::encoder::{Encoder, EncoderScratch};
pub use self::graphics::Graphics;
