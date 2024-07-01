//! Types and traits related to images.

use std::fmt;
use std::sync::Arc;

use crossbeam_queue::SegQueue;

use crate::math::UVec2;
use crate::{Error, ErrorKind, Result};

slotmap::new_key_type! {
    /// ID of an image.
    pub struct ImageId;
}

/// Handle to an image. Will schedule cleanup on `Drop`.
#[derive(Debug, Clone)]
pub struct ImageHandle {
    id: ImageId,
    cleanup_queue: Arc<SegQueue<ImageId>>,
}

impl ImageHandle {
    pub(crate) fn new(id: ImageId, cleanup_queue: Arc<SegQueue<ImageId>>) -> ImageHandle {
        ImageHandle { id, cleanup_queue }
    }

    /// Returns the corresponding [`ImageId`].
    pub fn id(&self) -> ImageId {
        self.id
    }
}

impl Drop for ImageHandle {
    fn drop(&mut self) {
        self.cleanup_queue.push(self.id);
    }
}

/// Fully decoded image data.
#[derive(Clone)]
pub struct ImageData {
    /// Format of the image.
    pub format: ImageFormat,
    /// Size of the image (width and height).
    pub size: UVec2,
    /// Tightly packed bytes for each pixel, row-major, without padding.
    pub data: Vec<u8>,
}

impl fmt::Debug for ImageData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageData")
            .field("format", &self.format)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

/// Format of image pixels.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ImageFormat {
    /// sRGB (non-linear, non-premultiplied), 8 bits per channel.
    Srgba8,
    /// Grayscale (linear).
    Gray8,
}

/// An image decoder. Can support one or multiple formats
/// (such as PNG, JPG, SVG, etc).
pub trait ImageDecoder: Send + Sync + 'static {
    /// Checks the extension and magic bytes in the provided `data`, returning
    /// `true` if the image can potentially be decoded by this decoder.
    fn probe(&self, extension: Option<&str>, data: &[u8]) -> bool;

    /// Decodes an image from raw bytes, optionally resizing it to a given size.
    ///
    /// If `requested_size` is not `None`:
    ///  - For raster formats, such as PNG, the image should be shrank to the
    ///    specified size, if necessary.
    ///  - For vector formats, such as SVG, the image should be rendered at
    ///    exactly the given size.
    ///
    /// This is merely a suggestion that shouldn't be relied upon.
    ///
    /// `requested_size` is useful for cases when:
    ///   - The image is large, but we want to render only a scaled down version
    ///     of it. There's no need to keep the entire image in memory.
    ///   - The image is vector, and we want to render it at a larger size than
    ///     specified in the image, without causing blurriness.
    fn decode(
        &self,
        extension: Option<&str>,
        data: &[u8],
        requested_size: Option<UVec2>,
    ) -> Result<ImageData>;
}

/// A set of [`ImageDecoders`]'s, that is also an [`ImageDecoders`] that tries
/// to decode the provided image using one of the decoders based on the
/// [`ImageDecoder::probe`] method.
#[derive(Default)]
pub struct ImageDecoders {
    decoders: Vec<Box<dyn ImageDecoder>>,
}

impl ImageDecoders {
    /// Creates an empty set of [`ImageDecoder`]'s.
    pub fn new() -> ImageDecoders {
        Default::default()
    }

    /// Registers an [`ImageDecoder`] into the set.
    pub fn add_decoder(&mut self, decoder: impl ImageDecoder) {
        self.decoders.push(Box::new(decoder));
    }
}

impl ImageDecoder for ImageDecoders {
    fn probe(&self, extension: Option<&str>, data: &[u8]) -> bool {
        for decoder in &self.decoders {
            if decoder.probe(extension, data) {
                return true;
            }
        }

        false
    }

    fn decode(
        &self,
        extension: Option<&str>,
        data: &[u8],
        requested_size: Option<UVec2>,
    ) -> Result<ImageData> {
        for decoder in &self.decoders {
            if decoder.probe(extension, data) {
                return decoder.decode(extension, data, requested_size);
            }
        }

        Err(Error::new(ErrorKind::InvalidImage, "unrecognized image"))
    }
}

impl fmt::Debug for ImageDecoders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageDecoders").finish_non_exhaustive()
    }
}
