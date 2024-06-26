use ohm2d_core::image::{ImageData, ImageDecoder, ImageFormat};
use ohm2d_core::math::{UVec2, Vec2};
use ohm2d_core::text::{FontFace, RasterizedGlyph, Rasterizer, SubpixelBin};
use ohm2d_core::{Error, ErrorKind, Result};
use ttf_parser::GlyphId;

#[derive(Debug, Clone, Copy, Default)]
pub struct ImageImageDecoder;

impl ImageImageDecoder {
    fn probe_format(&self, extension: Option<&str>, data: &[u8]) -> Option<image::ImageFormat> {
        image::guess_format(data)
            .ok()
            .or_else(|| extension.and_then(image::ImageFormat::from_extension))
    }
}

impl ImageDecoder for ImageImageDecoder {
    fn probe(&self, extension: Option<&str>, data: &[u8]) -> bool {
        self.probe_format(extension, data).is_some()
    }

    fn decode(
        &self,
        extension: Option<&str>,
        data: &[u8],
        size: Option<UVec2>,
    ) -> Result<ImageData> {
        let Some(format) = self.probe_format(extension, data) else {
            return Err(Error::new(ErrorKind::InvalidImage, "unrecognized image"));
        };

        let mut image = image::load_from_memory_with_format(data, format).map_err(|e| match e {
            image::ImageError::IoError(e) => e.into(),
            _ => Error::wrap(ErrorKind::InvalidImage, e),
        })?;

        if let Some(size) = size {
            image = image.resize_exact(size.x, size.y, image::imageops::FilterType::Lanczos3);
        }

        let rgba_image = image.to_rgba8();
        let data = rgba_image.into_raw();

        Ok(ImageData {
            format: ImageFormat::Srgba8,
            size: UVec2::new(image.width(), image.height()),
            data,
        })
    }
}

pub struct EmbeddedImageRasterizer;

impl Rasterizer for EmbeddedImageRasterizer {
    fn rasterize(
        &mut self,
        font_face: &FontFace,
        glyph_id: u16,
        size: f32,
        _subpixel_bin: SubpixelBin,
    ) -> Option<RasterizedGlyph> {
        let face = font_face.ttfp_face();

        let raster =
            match face.glyph_raster_image(GlyphId(glyph_id), size.min(u16::MAX.into()) as u16) {
                Some(v) => v,
                None => return None,
            };

        let scale = size / (raster.pixels_per_em as f32);

        let mut image = image::load_from_memory(raster.data).ok()?.into_rgba8();

        let old_size = UVec2::new(image.width(), image.height());
        let size = (old_size.as_vec2() * scale).as_uvec2();

        if size.cmplt(old_size).any() {
            image = image::imageops::resize(
                &image,
                size.x,
                size.y,
                image::imageops::FilterType::Lanczos3,
            );
        }

        let data = image.into_raw();
        Some(RasterizedGlyph {
            image: ImageData {
                format: ImageFormat::Srgba8,
                size,
                data,
            },
            offset: Vec2::new(raster.x as f32, -(raster.height as f32) - (raster.y as f32)) * scale,
        })
    }
}
