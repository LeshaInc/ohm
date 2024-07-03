use crate::text::{FontAttrs, FontFace, FontId};
use crate::Result;

/// A font database which stores and queries font faces.
pub trait FontDatabase: Send + Sync + 'static {
    /// Finds a font face best matching the attributes.
    fn query(&self, attrs: &FontAttrs) -> Option<FontId>;

    /// Loads a previously queried font face by ID.
    fn load(&mut self, id: FontId) -> Result<&FontFace>;

    /// Returns a previously loaded font face by ID.
    fn get(&self, id: FontId) -> Option<&FontFace>;

    /// Returns a previously loaded font face, or loads it if it hasn't been
    /// already.
    fn get_or_load(&mut self, id: FontId) -> Result<&FontFace>;
}

/// A dummy [`FontDatabase`], where all methods are unimplemented.
#[derive(Debug, Clone, Copy, Default)]
pub struct DummyFontDatabase;

impl FontDatabase for DummyFontDatabase {
    fn query(&self, _attrs: &FontAttrs) -> Option<FontId> {
        unimplemented!()
    }

    fn load(&mut self, _id: FontId) -> Result<&FontFace> {
        unimplemented!()
    }

    fn get(&self, _id: FontId) -> Option<&FontFace> {
        unimplemented!()
    }

    fn get_or_load(&mut self, _id: FontId) -> Result<&FontFace> {
        unimplemented!()
    }
}
