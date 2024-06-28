use std::fmt;
use std::sync::Arc;

use crossbeam_queue::SegQueue;

use crate::math::UVec2;
use crate::{Error, ErrorKind, Result};

slotmap::new_key_type! {
    pub struct ImageId;
}

#[derive(Debug, Clone)]
pub struct ImageHandle {
    id: ImageId,
    cleanup_queue: Arc<SegQueue<ImageId>>,
}

impl ImageHandle {
    pub(crate) fn new(id: ImageId, cleanup_queue: Arc<SegQueue<ImageId>>) -> ImageHandle {
        ImageHandle { id, cleanup_queue }
    }

    pub fn id(&self) -> ImageId {
        self.id
    }
}

impl Drop for ImageHandle {
    fn drop(&mut self) {
        self.cleanup_queue.push(self.id);
    }
}

#[derive(Clone)]
pub struct ImageData {
    pub format: ImageFormat,
    pub size: UVec2,
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ImageFormat {
    Srgba8,
    Gray8,
}

pub trait ImageDecoder: Send + Sync + 'static {
    fn probe(&self, extension: Option<&str>, data: &[u8]) -> bool;

    fn decode(
        &self,
        extension: Option<&str>,
        data: &[u8],
        requested_size: Option<UVec2>,
    ) -> Result<ImageData>;
}

#[derive(Default)]
pub struct ImageDecoders {
    decoders: Vec<Box<dyn ImageDecoder>>,
}

impl ImageDecoders {
    pub fn new() -> ImageDecoders {
        Default::default()
    }

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
