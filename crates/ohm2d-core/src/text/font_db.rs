use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::text::{FontAttrs, FontFace, FontId};
use crate::{Error, ErrorKind, Result};

pub trait FontSource: Send + Sync + 'static {
    fn query(&self, attrs: &FontAttrs) -> Option<u64>;

    fn load(&mut self, id: FontId) -> Result<FontFace>;
}

#[derive(Default)]
pub struct FontDatabase {
    sources: Vec<Box<dyn FontSource>>,
    loaded_faces: HashMap<FontId, FontFace>,
    cached_failures: HashSet<FontId>,
}

impl FontDatabase {
    pub fn new() -> FontDatabase {
        FontDatabase::default()
    }

    pub fn add_source<S: FontSource>(&mut self, source: S) {
        self.sources.push(Box::new(source))
    }

    pub fn query(&self, attrs: &FontAttrs) -> Option<FontId> {
        for (source_id, source) in self.sources.iter().enumerate() {
            if let Some(opaque_id) = source.query(attrs) {
                return Some(FontId {
                    source_id,
                    opaque_id,
                });
            }
        }

        None
    }

    pub fn load(&mut self, id: FontId) -> Result<&FontFace> {
        if self.cached_failures.contains(&id) {
            return Err(Error::new(ErrorKind::CachedFailure, "cached failure"));
        }

        let source = self
            .sources
            .get_mut(id.source_id)
            .ok_or_else(|| Error::new(ErrorKind::InvalidId, "invalid font id"))?;

        let face = match source.load(id) {
            Ok(v) => v,
            Err(e) => {
                log::error!("Font failed to load: {}", e);
                self.cached_failures.insert(id);
                return Err(e);
            }
        };

        self.loaded_faces.insert(id, face);
        Ok(&self.loaded_faces[&id])
    }

    pub fn get(&self, id: FontId) -> Option<&FontFace> {
        self.loaded_faces.get(&id)
    }

    pub fn get_or_load(&mut self, id: FontId) -> Result<&FontFace> {
        if self.loaded_faces.contains_key(&id) {
            Ok(&self.loaded_faces[&id])
        } else {
            self.load(id)
        }
    }
}

impl fmt::Debug for FontDatabase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontDatabase")
            .field("loaded_faces", &self.loaded_faces)
            .field("cached_failures", &self.cached_failures)
            .finish_non_exhaustive()
    }
}
