use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use smallvec::{smallvec, SmallVec};
pub use ttf_parser::GlyphId;
use ttf_parser::{name_id, Face, Language, Tag};

use crate::{Error, ErrorKind, Result};

/// ID of a font face inside a [`FontDatabase`](super::FontDatabase).
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct FontId(pub u64);

impl FontId {
    /// A dummy [`FontId`], can be used as a placeholder.
    pub const DUMMY: FontId = FontId(u64::MAX);
}

self_cell::self_cell! {
    struct FaceRef {
        owner: Arc<dyn AsRef<[u8]> + Send + Sync>,
        #[covariant]
        dependent: Face,
    }
}

/// A single font face with a specific style and weight.
pub struct FontFace {
    id: FontId,
    face_ref: FaceRef,
    face_index: u32,
    attrs: FontAttrs,
    metrics: FontMetrics,
}

impl FontFace {
    /// Parses the data as a TTF/OTF font and returns the font face at a given
    /// index.
    pub fn new(
        id: FontId,
        data: Arc<dyn AsRef<[u8]> + Send + Sync>,
        face_index: u32,
    ) -> Result<FontFace> {
        let face_ref = FaceRef::try_new(data, |data| Face::parse((**data).as_ref(), face_index))
            .map_err(|e| Error::wrap(ErrorKind::InvalidFont, e))?;
        let face = face_ref.borrow_dependent();

        let attrs = FontAttrs::from_ttfp_face(face)
            .ok_or_else(|| Error::new(ErrorKind::InvalidFont, "invalid font attrs"))?;
        let metrics = FontMetrics::from_ttfp_face(face);

        Ok(FontFace {
            id,
            face_ref,
            face_index,
            attrs,
            metrics,
        })
    }

    /// Returns the [`FontId`] of this face.
    pub fn id(&self) -> FontId {
        self.id
    }

    /// Returns the original TTF/OTF data.
    pub fn data(&self) -> &Arc<dyn AsRef<[u8]> + Send + Sync> {
        self.face_ref.borrow_owner()
    }

    /// Returns the underlying [`ttf_parser::Face`].
    pub fn ttfp_face(&self) -> &Face {
        self.face_ref.borrow_dependent()
    }

    /// Returns the index of this face in the collection.
    pub fn face_index(&self) -> u32 {
        self.face_index
    }

    /// Returns attributes of the face (style, weight, etc).
    pub fn attrs(&self) -> &FontAttrs {
        &self.attrs
    }

    /// Returns the metrics of the face, useful for layout.
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

/// Font attributes.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default)]
pub struct FontAttrs {
    /// Name of the font family.
    pub family: FontFamily,
    /// Font weight (how thick the letters are).
    pub weight: FontWeight,
    /// Font width (how wide the letters are).
    pub width: FontWidth,
    /// Font style (italic, oblique, normal).
    pub style: FontStyle,
    /// Whether the font has variable weight (WGHT variation axis).
    pub is_variable_weight: bool,
    /// Whether the font has variable width (WDTH variation axis).
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

/// Name of the font family
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct FontFamily {
    name: Cow<'static, str>,
}

impl FontFamily {
    /// Creates the [`FontFamily`] from the specified string.
    pub fn new(name: impl Into<Cow<'static, str>>) -> FontFamily {
        FontFamily { name: name.into() }
    }

    /// Returns the default "serif" font family.
    pub fn serif() -> FontFamily {
        FontFamily::new("serif")
    }

    /// Returns the default "sans-serif" font family.
    pub fn sans_serif() -> FontFamily {
        FontFamily::new("sans-serif")
    }

    /// Returns the default "cursive" font family.
    pub fn cursive() -> FontFamily {
        FontFamily::new("cursive")
    }

    /// Returns the default "fantasy" font family.
    pub fn fantasy() -> FontFamily {
        FontFamily::new("fantasy")
    }

    /// Returns the default "monospace" font family.
    pub fn monospace() -> FontFamily {
        FontFamily::new("monospace")
    }

    /// Returns a string representation of this font family.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Default for FontFamily {
    fn default() -> Self {
        FontFamily::sans_serif()
    }
}

/// A list of font families, in the order of preference
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct FontFamilies {
    list: SmallVec<[FontFamily; 2]>,
}

impl FontFamilies {
    /// Creates a single element list with `base` as a [`FontFamily`].
    pub fn new(base: FontFamily) -> FontFamilies {
        FontFamilies {
            list: smallvec![base],
        }
    }

    /// Adds a fallback [`FontFamily`] to the end of the list.
    pub fn with(mut self, family: FontFamily) -> Self {
        self.list.push(family);
        self
    }

    /// Returns the first [`FontFamily`] in the chain.
    pub fn base(&self) -> &FontFamily {
        &self.list[0]
    }

    /// Returns the length of the chain. Guaranteed to be larger than 1.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.list.len()
    }

    /// Returns an ordered iterator over [`FontFamily`].
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

/// Font weight, i.e. how bold or thin the characters are.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum FontWeight {
    /// Thin (100).
    Thin,
    /// Extra Light (200).
    ExtraLight,
    /// Light (300).
    Light,
    /// Normal (400) --- default.
    #[default]
    Normal,
    /// Medium (500).
    Medium,
    /// Semi-bold (600).
    SemiBold,
    /// Bold (700).
    Bold,
    /// Extra Bold (800).
    ExtraBold,
    /// Black (900).
    Black,
    /// Any other value.
    Other(u16),
}

impl FontWeight {
    /// Creates a [`FontWeight`] from the numeric representation.
    pub fn from_number(v: u16) -> FontWeight {
        match v {
            100 => FontWeight::Thin,
            200 => FontWeight::ExtraLight,
            300 => FontWeight::Light,
            400 => FontWeight::Normal,
            500 => FontWeight::Medium,
            600 => FontWeight::SemiBold,
            700 => FontWeight::Bold,
            800 => FontWeight::ExtraBold,
            900 => FontWeight::Black,
            v => FontWeight::Other(v),
        }
    }

    /// Converts this [`FontWeight`] into its numeric representation.
    pub fn to_number(self) -> u16 {
        match self {
            FontWeight::Thin => 100,
            FontWeight::ExtraLight => 200,
            FontWeight::Light => 300,
            FontWeight::Normal => 400,
            FontWeight::Medium => 500,
            FontWeight::SemiBold => 600,
            FontWeight::Bold => 700,
            FontWeight::ExtraBold => 800,
            FontWeight::Black => 900,
            FontWeight::Other(v) => v,
        }
    }
}

fn get_font_weight(face: &Face<'_>) -> FontWeight {
    FontWeight::from_number(face.weight().to_number())
}

/// Font width, i.e. how wide the characters are.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum FontWidth {
    /// Ultra Condensed.
    UltraCondensed,
    /// Extra Condensed.
    ExtraCondensed,
    /// Condensed.
    Condensed,
    /// Semi-condensed.
    SemiCondensed,
    /// Normal --- default.
    #[default]
    Normal,
    /// Semi-expanded.
    SemiExpanded,
    /// Expanded.
    Expanded,
    /// Extra Expanded.
    ExtraExpanded,
    /// Ultra expanded.
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

/// Font style.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum FontStyle {
    /// Normal --- default.
    #[default]
    Normal,
    /// Italic --- usually a cursive variant of the font.
    Italic,
    /// Oblique --- usually just a sloped version of the normal style.
    Oblique,
}

fn get_font_style(face: &Face<'_>) -> FontStyle {
    match face.style() {
        ttf_parser::Style::Normal => FontStyle::Normal,
        ttf_parser::Style::Italic => FontStyle::Italic,
        ttf_parser::Style::Oblique => FontStyle::Oblique,
    }
}

/// Font metrics.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub struct FontMetrics {
    /// Ascender, i.e. the maximum distance between the baseline and the
    /// top-most point of a letter.
    ///
    /// Measured in units. To get the size in pixels, divide by `units_per_em`
    /// and multiply by font size.
    pub ascender: i16,
    /// Descender, i.e. the maximum distance between the baseline and the
    /// bottom-most point of a letter.
    ///
    /// Measured in units. To get the size in pixels, divide by `units_per_em`
    /// and multiply by font size.
    pub descender: i16,
    /// Distance between the descender of this line and the ascender of the next
    /// line, assuming a standard line height.
    ///
    /// Measured in units. To get the size in pixels, divide by `units_per_em`
    /// and multiply by font size.
    pub line_gap: i16,
    /// Scale of the font grid.
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
