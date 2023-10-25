use std::fmt;
use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};

use crate::math::UVec2;
use crate::AssetPath;

slotmap::new_key_type! {
    pub struct ImageId;
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

pub trait ImageSource: Send + Sync + 'static {
    fn scheme(&self) -> &'static str;

    fn load(&mut self, path: AssetPath<'_>, size: Option<UVec2>) -> Result<ImageData>;
}

#[derive(Debug)]
pub struct FileImageSource {
    root: PathBuf,
}

impl FileImageSource {
    pub fn new(root: impl Into<PathBuf>) -> FileImageSource {
        FileImageSource { root: root.into() }
    }
}

impl ImageSource for FileImageSource {
    fn scheme(&self) -> &'static str {
        "file"
    }

    fn load(&mut self, asset_path: AssetPath<'_>, size: Option<UVec2>) -> Result<ImageData> {
        let mut path = self.root.clone();
        path.push(asset_path.path());

        let file =
            File::open(&path).with_context(|| format!("Failed to open {}", path.display()))?;
        let mut reader = BufReader::new(file);

        let mut buf = [0; 256];
        reader.read(&mut buf)?;

        let format = image::guess_format(&buf)
            .ok()
            .or_else(|| {
                path.extension()
                    .and_then(|ext| image::ImageFormat::from_extension(ext))
            })
            .ok_or_else(|| anyhow!("unknown image format"))?;

        reader.seek(std::io::SeekFrom::Start(0))?;

        let mut image = image::load(reader, format)?;

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
