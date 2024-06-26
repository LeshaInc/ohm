use crate::text::{FontAttrs, FontFace, FontId};
use crate::Result;

pub trait FontDatabase: Send + Sync + 'static {
    fn query(&self, attrs: &FontAttrs) -> Option<FontId>;

    fn load(&mut self, id: FontId) -> Result<&FontFace>;

    fn get(&self, id: FontId) -> Option<&FontFace>;

    fn get_or_load(&mut self, id: FontId) -> Result<&FontFace>;
}

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
