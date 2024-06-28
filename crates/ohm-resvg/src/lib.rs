use ohm_core::image::{ImageData, ImageDecoder, ImageFormat};
use ohm_core::math::UVec2;
use ohm_core::{Error, ErrorKind, Result};
use resvg::tiny_skia::Pixmap;
use resvg::usvg::{Options, Transform, Tree};

#[derive(Debug, Clone, Copy, Default)]
pub struct ResvgImageDecoder;

impl ImageDecoder for ResvgImageDecoder {
    fn probe(&self, extension: Option<&str>, data: &[u8]) -> bool {
        if extension == Some("svg") || extension == Some("svgz") {
            return true;
        }

        data.starts_with(b"<?xml") || data.starts_with(b"\x1f\x8b")
    }

    fn decode(
        &self,
        _extension: Option<&str>,
        data: &[u8],
        size: Option<UVec2>,
    ) -> Result<ImageData> {
        let tree = Tree::from_data(data, &Options::default())
            .map_err(|e| Error::wrap(ErrorKind::InvalidImage, e))?;

        let size = size.unwrap_or_else(|| {
            let size = tree.size();
            UVec2::new(size.width().ceil() as u32, size.height().ceil() as u32)
        });

        let mut pixmap = Pixmap::new(size.x, size.y)
            .ok_or_else(|| Error::new(ErrorKind::InvalidImage, "zero-size svg"))?;
        let mut pixmap_mut = pixmap.as_mut();

        let transform = Transform::from_scale(
            (size.x as f32) / tree.size().width(),
            (size.y as f32) / tree.size().height(),
        );

        resvg::render(&tree, transform, &mut pixmap_mut);

        Ok(ImageData {
            format: ImageFormat::Srgba8,
            size,
            data: pixmap.take(),
        })
    }
}
