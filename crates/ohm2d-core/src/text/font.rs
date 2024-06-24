use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use smallvec::{smallvec, SmallVec};
use ttf_parser::{name_id, Face, Language, Tag};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct FontId {
    pub source_id: usize,
    pub opaque_id: u64,
}

impl FontId {
    pub const DUMMY: FontId = FontId {
        source_id: usize::MAX,
        opaque_id: u64::MAX,
    };
}

self_cell::self_cell! {
    struct FaceRef {
        owner: Arc<dyn AsRef<[u8]> + Send + Sync>,
        #[covariant]
        dependent: Face,
    }
}

pub struct FontFace {
    id: FontId,
    face_ref: FaceRef,
    face_index: u32,
    attrs: FontAttrs,
    metrics: FontMetrics,
}

impl FontFace {
    pub fn new(
        id: FontId,
        data: Arc<dyn AsRef<[u8]> + Send + Sync>,
        face_index: u32,
    ) -> Result<FontFace> {
        let face_ref = FaceRef::try_new(data, |data| Face::parse((**data).as_ref(), face_index))?;
        let face = face_ref.borrow_dependent();

        let attrs = FontAttrs::from_ttfp_face(face).ok_or_else(|| anyhow!("invalid font attrs"))?;
        let metrics = FontMetrics::from_ttfp_face(face);

        Ok(FontFace {
            id,
            face_ref,
            face_index,
            attrs,
            metrics,
        })
    }

    pub fn id(&self) -> FontId {
        self.id
    }

    pub fn data(&self) -> &Arc<dyn AsRef<[u8]> + Send + Sync> {
        self.face_ref.borrow_owner()
    }

    pub fn ttfp_face(&self) -> &Face {
        self.face_ref.borrow_dependent()
    }

    pub fn face_index(&self) -> u32 {
        self.face_index
    }

    pub fn attrs(&self) -> &FontAttrs {
        &self.attrs
    }

    pub fn metrics(&self) -> &FontMetrics {
        &self.metrics
    }
}

impl fmt::Debug for FontFace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontFace")
            .field("id", &self.id)
            .field("face_index", &self.face_index)
            .field("attrs", &self.attrs)
            .field("metrics", &self.metrics)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Default)]
pub struct FontAttrs {
    pub family: FontFamily,
    pub weight: FontWeight,
    pub width: FontWidth,
    pub style: FontStyle,
    pub is_variable_weight: bool,
    pub is_variable_width: bool,
}

impl FontAttrs {
    pub(crate) const WGHT_AXIS: Tag = Tag::from_bytes(b"wght");
    pub(crate) const WDTH_AXIS: Tag = Tag::from_bytes(b"wdth");

    fn from_ttfp_face(face: &Face<'_>) -> Option<FontAttrs> {
        let mut attrs = FontAttrs {
            family: get_font_family(face)?,
            weight: get_font_weight(face),
            width: get_font_stretch(face),
            style: get_font_style(face),
            is_variable_weight: false,
            is_variable_width: false,
        };

        for axis in face.variation_axes() {
            if axis.tag == Self::WGHT_AXIS {
                attrs.is_variable_weight = true;
            }
            if axis.tag == Self::WDTH_AXIS {
                attrs.is_variable_width = true;
            }
        }

        Some(attrs)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct FontFamily {
    name: Cow<'static, str>,
}

impl FontFamily {
    pub fn new(name: impl Into<Cow<'static, str>>) -> FontFamily {
        FontFamily { name: name.into() }
    }

    pub fn serif() -> FontFamily {
        FontFamily::new("serif")
    }

    pub fn sans_serif() -> FontFamily {
        FontFamily::new("sans-serif")
    }

    pub fn cursive() -> FontFamily {
        FontFamily::new("cursive")
    }

    pub fn fantasy() -> FontFamily {
        FontFamily::new("fantasy")
    }

    pub fn monospace() -> FontFamily {
        FontFamily::new("monospace")
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Default for FontFamily {
    fn default() -> Self {
        FontFamily::sans_serif()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct FontFamilies {
    list: SmallVec<[FontFamily; 2]>,
}

impl FontFamilies {
    pub fn new(base: FontFamily) -> FontFamilies {
        FontFamilies {
            list: smallvec![base],
        }
    }

    pub fn with(mut self, family: FontFamily) -> Self {
        self.list.push(family);
        self
    }

    pub fn base(&self) -> &FontFamily {
        &self.list[0]
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &FontFamily> + '_ {
        self.list.iter()
    }
}

impl From<FontFamily> for FontFamilies {
    fn from(v: FontFamily) -> FontFamilies {
        FontFamilies::new(v)
    }
}

impl Default for FontFamilies {
    fn default() -> FontFamilies {
        FontFamilies::new(FontFamily::default())
    }
}

fn get_font_family(face: &Face<'_>) -> Option<FontFamily> {
    face.names()
        .into_iter()
        .filter(|name| {
            name.name_id == name_id::TYPOGRAPHIC_FAMILY || name.name_id == name_id::FAMILY
        })
        .flat_map(|name| {
            name.to_string()
                .map(|str| (name.name_id, str, name.language()))
        })
        .max_by_key(|&(id, _, language)| {
            let mut points = 0;
            if id == name_id::TYPOGRAPHIC_FAMILY {
                points += 10;
            }
            if language == Language::English_UnitedStates {
                points += 10;
            }
            points
        })
        .map(|(_, name, _)| FontFamily::new(name))
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct FontWeight(pub u16);

impl FontWeight {
    pub const THIN: FontWeight = FontWeight(100);
    pub const EXTRA_LIGHT: FontWeight = FontWeight(200);
    pub const LIGHT: FontWeight = FontWeight(300);
    pub const NORMAL: FontWeight = FontWeight(400);
    pub const MEDIUM: FontWeight = FontWeight(500);
    pub const SEMI_BOLD: FontWeight = FontWeight(600);
    pub const BOLD: FontWeight = FontWeight(700);
    pub const EXTRA_BOLD: FontWeight = FontWeight(800);
    pub const BLACK: FontWeight = FontWeight(900);
}

impl Default for FontWeight {
    fn default() -> Self {
        FontWeight::NORMAL
    }
}

fn get_font_weight(face: &Face<'_>) -> FontWeight {
    FontWeight(face.weight().to_number())
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum FontWidth {
    UltraCondensed,
    ExtraCondensed,
    Condensed,
    SemiCondensed,
    #[default]
    Normal,
    SemiExpanded,
    Expanded,
    ExtraExpanded,
    UltraExpanded,
}

fn get_font_stretch(face: &Face<'_>) -> FontWidth {
    match face.width() {
        ttf_parser::Width::UltraCondensed => FontWidth::UltraCondensed,
        ttf_parser::Width::ExtraCondensed => FontWidth::ExtraCondensed,
        ttf_parser::Width::Condensed => FontWidth::Condensed,
        ttf_parser::Width::SemiCondensed => FontWidth::SemiCondensed,
        ttf_parser::Width::Normal => FontWidth::Normal,
        ttf_parser::Width::SemiExpanded => FontWidth::SemiExpanded,
        ttf_parser::Width::Expanded => FontWidth::Expanded,
        ttf_parser::Width::ExtraExpanded => FontWidth::ExtraExpanded,
        ttf_parser::Width::UltraExpanded => FontWidth::UltraExpanded,
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}

fn get_font_style(face: &Face<'_>) -> FontStyle {
    match face.style() {
        ttf_parser::Style::Normal => FontStyle::Normal,
        ttf_parser::Style::Italic => FontStyle::Italic,
        ttf_parser::Style::Oblique => FontStyle::Oblique,
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub struct FontMetrics {
    pub ascender: i16,
    pub descender: i16,
    pub line_gap: i16,
    pub units_per_em: u16,
}

impl FontMetrics {
    fn from_ttfp_face(face: &Face<'_>) -> FontMetrics {
        FontMetrics {
            ascender: face.ascender(),
            descender: face.descender(),
            line_gap: face.line_gap(),
            units_per_em: face.units_per_em(),
        }
    }
}
