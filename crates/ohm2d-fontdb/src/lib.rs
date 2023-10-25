use anyhow::{anyhow, Result};
use ohm2d_core::text::{FontAttrs, FontFace, FontFamily, FontId, FontSource, FontStyle, FontWidth};

pub struct SystemFontSource(FontDbSource);

impl SystemFontSource {
    pub fn new() -> SystemFontSource {
        let mut source = FontDbSource::default();
        source.db.load_system_fonts();
        SystemFontSource(source)
    }
}

impl Default for SystemFontSource {
    fn default() -> Self {
        SystemFontSource::new()
    }
}

impl FontSource for SystemFontSource {
    fn query(&self, attrs: &FontAttrs) -> Option<u64> {
        self.0.query(attrs)
    }

    fn load(&mut self, id: FontId) -> Result<FontFace> {
        self.0.load(id)
    }
}

struct FontDbSource {
    db: fontdb::Database,
}

impl Default for FontDbSource {
    fn default() -> Self {
        FontDbSource {
            db: fontdb::Database::new(),
        }
    }
}

impl FontSource for FontDbSource {
    fn query(&self, attrs: &FontAttrs) -> Option<u64> {
        self.db
            .query(&fontdb::Query {
                families: &[fontdb_family(&attrs.family)],
                weight: fontdb::Weight(attrs.weight.0),
                stretch: fontdb_stretch(attrs.width),
                style: fontdb_style(attrs.style),
            })
            .map(fontdb_id_to_u64)
    }

    fn load(&mut self, id: FontId) -> Result<FontFace> {
        let (data, face_index) = unsafe {
            self.db
                .make_shared_face_data(fontdb_id_from_u64(id.opaque_id))
                .ok_or_else(|| anyhow!("Failed to load font"))?
        };

        FontFace::new(id, data, face_index)
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

fn fontdb_id_to_u64(id: fontdb::ID) -> u64 {
    unsafe { std::mem::transmute(id) }
}

fn fontdb_id_from_u64(id: u64) -> fontdb::ID {
    unsafe { std::mem::transmute(id) }
}
