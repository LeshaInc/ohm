use std::collections::HashMap;

use ohm_core::text::{
    FontAttrs, FontDatabase, FontFace, FontFamily, FontId, FontStyle, FontWidth,
};
use ohm_core::{Error, ErrorKind, Result};

#[derive(Debug)]
pub struct SystemFontDatabase {
    db: fontdb::Database,
    loaded_faces: HashMap<FontId, FontFace>,
}

impl SystemFontDatabase {
    pub fn new() -> SystemFontDatabase {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        SystemFontDatabase {
            db,
            loaded_faces: HashMap::new(),
        }
    }
}

impl Default for SystemFontDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl FontDatabase for SystemFontDatabase {
    fn query(&self, attrs: &FontAttrs) -> Option<FontId> {
        self.db
            .query(&fontdb::Query {
                families: &[fontdb_family(&attrs.family)],
                weight: fontdb::Weight(attrs.weight.0),
                stretch: fontdb_stretch(attrs.width),
                style: fontdb_style(attrs.style),
            })
            .map(fontdb_id_to_u64)
    }

    fn load(&mut self, id: FontId) -> Result<&FontFace> {
        let (data, face_index) = unsafe {
            self.db
                .make_shared_face_data(fontdb_id_from_u64(id))
                .ok_or_else(|| Error::new(ErrorKind::InvalidFont, "Failed to load font"))?
        };

        let face = FontFace::new(id, data, face_index)?;
        self.loaded_faces.insert(id, face);
        Ok(&self.loaded_faces[&id])
    }

    fn get(&self, id: FontId) -> Option<&FontFace> {
        self.loaded_faces.get(&id)
    }

    fn get_or_load(&mut self, id: FontId) -> Result<&FontFace> {
        if self.loaded_faces.contains_key(&id) {
            Ok(&self.loaded_faces[&id])
        } else {
            self.load(id)
        }
    }
}

fn fontdb_family(family: &FontFamily) -> fontdb::Family<'_> {
    match family.name() {
        "serif" => fontdb::Family::Serif,
        "sans-serif" => fontdb::Family::SansSerif,
        "cursive" => fontdb::Family::Cursive,
        "fantasy" => fontdb::Family::Fantasy,
        "monospace" => fontdb::Family::Monospace,
        name => fontdb::Family::Name(name),
    }
}

fn fontdb_stretch(stretch: FontWidth) -> fontdb::Stretch {
    match stretch {
        FontWidth::UltraCondensed => fontdb::Stretch::UltraCondensed,
        FontWidth::ExtraCondensed => fontdb::Stretch::ExtraCondensed,
        FontWidth::Condensed => fontdb::Stretch::Condensed,
        FontWidth::SemiCondensed => fontdb::Stretch::SemiCondensed,
        FontWidth::Normal => fontdb::Stretch::Normal,
        FontWidth::SemiExpanded => fontdb::Stretch::SemiExpanded,
        FontWidth::Expanded => fontdb::Stretch::Expanded,
        FontWidth::ExtraExpanded => fontdb::Stretch::ExtraExpanded,
        FontWidth::UltraExpanded => fontdb::Stretch::UltraExpanded,
    }
}

fn fontdb_style(style: FontStyle) -> fontdb::Style {
    match style {
        FontStyle::Normal => fontdb::Style::Normal,
        FontStyle::Italic => fontdb::Style::Italic,
        FontStyle::Oblique => fontdb::Style::Oblique,
    }
}

fn fontdb_id_to_u64(id: fontdb::ID) -> FontId {
    unsafe { std::mem::transmute(id) }
}

fn fontdb_id_from_u64(id: FontId) -> fontdb::ID {
    unsafe { std::mem::transmute(id) }
}
