use ohm_core::image::{ImageData, ImageDecoder, ImageFormat};
use ohm_core::math::{UVec2, Vec2};
use ohm_core::text::{FontFace, GlyphId, RasterizedGlyph, Rasterizer, SubpixelBin};
use ohm_core::{Error, ErrorKind, Result};

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

        let image = image::load_from_memory_with_format(data, format).map_err(|e| match e {
            image::ImageError::IoError(e) => e.into(),
            _ => Error::wrap(ErrorKind::InvalidImage, e),
        })?;

        Ok(convert_image(image.to_rgba8(), size))
    }
}

pub struct EmbeddedImageRasterizer;

impl Rasterizer for EmbeddedImageRasterizer {
    fn rasterize(
        &mut self,
        font_face: &FontFace,
        glyph_id: GlyphId,
        size: f32,
        _subpixel_bin: SubpixelBin,
    ) -> Option<RasterizedGlyph> {
        let face = font_face.ttfp_face();

        let raster = match face.glyph_raster_image(glyph_id, size.min(u16::MAX.into()) as u16) {
            Some(v) => v,
            None => return None,
        };

        let scale = size / (raster.pixels_per_em as f32);

        let image = image::load_from_memory(raster.data).ok()?.into_rgba8();
        let old_size = UVec2::new(image.width(), image.height());
        let size = (old_size.as_vec2() * scale).as_uvec2();

        Some(RasterizedGlyph {
            image: convert_image(image, Some(size)),
            offset: Vec2::new(raster.x as f32, -(raster.height as f32) - (raster.y as f32)) * scale,
        })
    }
}

fn convert_image(mut image: image::RgbaImage, size: Option<UVec2>) -> ImageData {
    let old_size = UVec2::new(image.width(), image.height());

    if let Some(size) = size.filter(|v| v.cmplt(old_size).any()) {
        image = image::imageops::resize(
            &image,
            size.x,
            size.y,
            image::imageops::FilterType::Lanczos3,
        );
    }

    for pixel in image.pixels_mut() {
        pixel.0[0] = (u16::from(pixel.0[0]) * u16::from(pixel.0[3]) / 255) as u8;
        pixel.0[1] = (u16::from(pixel.0[1]) * u16::from(pixel.0[3]) / 255) as u8;
        pixel.0[2] = (u16::from(pixel.0[2]) * u16::from(pixel.0[3]) / 255) as u8;
    }

    ImageData {
        format: ImageFormat::Srgba8,
        size: UVec2::new(image.width(), image.height()),
        data: image.into_raw(),
    }
}
