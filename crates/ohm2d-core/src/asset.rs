use std::borrow::Cow;
use std::fmt;
use std::path::Path;

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

    pub fn url(&self) -> &str {
        self.as_ref()
    }

    pub fn scheme(&self) -> &str {
        &self.str[..self.scheme_len]
    }

    pub fn path(&self) -> &Path {
        self.str[self.scheme_len + 1..].as_ref()
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
