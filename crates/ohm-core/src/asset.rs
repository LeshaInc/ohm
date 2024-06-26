use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::{Error, ErrorKind, Result};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct AssetPath<'a> {
    str: Cow<'a, str>,
    scheme_len: usize,
}

impl AssetPath<'_> {
    pub fn new<'a>(str: impl Into<Cow<'a, str>>) -> AssetPath<'a> {
        let str: Cow<'_, str> = str.into();

        let Some(scheme_len) = str.find(':') else {
            panic!("invalid asset path");
        };

        AssetPath { str, scheme_len }
    }

    pub fn scheme(&self) -> &str {
        &self.str[..self.scheme_len]
    }

    pub fn path(&self) -> &Path {
        self.str[self.scheme_len + 1..].as_ref()
    }

    pub fn extension(&self) -> Option<&str> {
        self.path().extension().and_then(|v| v.to_str())
    }

    pub fn as_borrowed(&self) -> AssetPath<'_> {
        AssetPath {
            str: Cow::Borrowed(&self.str),
            scheme_len: self.scheme_len,
        }
    }

    pub fn into_owned(self) -> AssetPath<'static> {
        AssetPath {
            str: Cow::Owned(self.str.into_owned()),
            scheme_len: self.scheme_len,
        }
    }
}

impl AsRef<str> for AssetPath<'_> {
    fn as_ref(&self) -> &str {
        self.str.as_ref()
    }
}

impl<'a> From<&'a str> for AssetPath<'a> {
    fn from(str: &'a str) -> AssetPath<'a> {
        AssetPath::new(str)
    }
}

impl<'a> From<&'a String> for AssetPath<'a> {
    fn from(str: &'a String) -> AssetPath<'a> {
        AssetPath::new(str)
    }
}

impl From<String> for AssetPath<'static> {
    fn from(str: String) -> AssetPath<'static> {
        AssetPath::new(str)
    }
}

impl fmt::Display for AssetPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.str.fmt(f)
    }
}

pub trait AssetSource: Send + Sync + 'static {
    fn load(&self, path: AssetPath<'_>) -> Result<Vec<u8>>;
}

#[derive(Default)]
pub struct AssetSources {
    sources: HashMap<String, Box<dyn AssetSource>>,
}

impl AssetSources {
    pub fn new() -> AssetSources {
        AssetSources::default()
    }

    pub fn add_source(&mut self, scheme: impl Into<String>, source: impl AssetSource) {
        self.sources.insert(scheme.into(), Box::new(source));
    }

    pub fn find_source(&self, scheme: &str) -> Option<&dyn AssetSource> {
        self.sources.get(scheme).map(|v| &**v)
    }
}

impl AssetSource for AssetSources {
    fn load(&self, path: AssetPath<'_>) -> Result<Vec<u8>> {
        let Some(source) = self.find_source(path.scheme()) else {
            return Err(Error::new(
                ErrorKind::InvalidPath,
                format!("unknown scheme `{}`", path.scheme()),
            ));
        };

        source.load(path)
    }
}

#[derive(Debug)]
pub struct FileAssetSource {
    root: PathBuf,
}

impl FileAssetSource {
    pub fn new(root: impl Into<PathBuf>) -> Result<FileAssetSource> {
        let root = root.into().canonicalize()?;

        if !root.is_dir() {
            return Err(Error::new(
                ErrorKind::InvalidPath,
                "asset root must be a directory",
            ));
        }

        Ok(FileAssetSource { root })
    }
}

impl AssetSource for FileAssetSource {
    fn load(&self, path: AssetPath<'_>) -> Result<Vec<u8>> {
        let mut file_path = self.root.clone();
        file_path.push(path.path());

        let file_path = file_path.canonicalize()?;

        if !file_path.starts_with(&self.root) {
            return Err(Error::new(
                ErrorKind::InvalidPath,
                "path escapes asset root directory",
            ));
        }

        let data = std::fs::read(file_path)?;
        Ok(data)
    }
}
