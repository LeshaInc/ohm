use std::ops::Range;

use smallvec::SmallVec;
use unicode_bidi::{BidiInfo, Level as BidiLevel, ParagraphInfo as BidiParagraph};
use unicode_linebreak::BreakOpportunity;

use crate::math::Vec2;
use crate::text::{
    FontAttrs, FontDatabase, FontFace, FontFamily, FontId, LineHeight, ShapedGlyph, TextAlign,
    TextAttrs, TextShaper,
};

#[derive(Debug)]
pub struct TextBuffer {
    text: String,
    sections: Vec<Section>,
    bidi_levels: Vec<BidiLevel>,
    bidi_paragraphs: Vec<BidiParagraph>,
    runs: Vec<Run>,
    tmp_runs: Vec<Run>,
    glyphs: Vec<ShapedGlyph>,
    lines: Vec<Line>,
    max_width: f32,
    height: f32,
    dirty: bool,
}

#[derive(Debug, Clone)]
struct Section {
    range: Range<usize>,
    attrs: TextAttrs,
    fonts: SmallVec<[FontId; 2]>,
}

#[derive(Debug, Clone)]
pub struct Run {
    pub range: Range<usize>,
    pub glyph_range: Range<usize>,
    pub section_idx: usize,
    pub bidi_level: BidiLevel,
    pub font: FontId,
    pub font_size: f32,
    pub line_height: f32,
    pub text_height: f32,
    pub pos: Vec2,
}

#[derive(Debug, Clone, Default)]
struct Line {
    range: Range<usize>,
    run_range: Range<usize>,
    width: f32,
    whitespace_width: f32,
    height: f32,
}

impl TextBuffer {
    pub fn new() -> TextBuffer {
        TextBuffer {
            text: String::new(),
            sections: Vec::new(),
            bidi_levels: Vec::new(),
            bidi_paragraphs: Vec::new(),
            runs: Vec::new(),
            tmp_runs: Vec::new(),
            glyphs: Vec::new(),
            lines: Vec::new(),
            max_width: f32::INFINITY,
            height: 0.0,
            dirty: true,
        }
    }

    pub fn reset(&mut self) {
        self.text.clear();
        self.sections.clear();
        self.bidi_levels.clear();
        self.bidi_paragraphs.clear();
        self.runs.clear();
        self.tmp_runs.clear();
        self.glyphs.clear();
        self.lines.clear();
        self.max_width = f32::INFINITY;
        self.height = 0.0;
        self.dirty = false;
    }

    pub fn push(&mut self, attrs: TextAttrs, text: &str) {
        self.text.push_str(text);
        self.sections.push(Section {
            attrs,
            range: self.text.len() - text.len()..self.text.len(),
            fonts: SmallVec::new(),
        });

        self.dirty = true;
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_max_width(&mut self, max_width: f32) {
        if self.max_width == max_width {
            return;
        }

        self.max_width = max_width;
        self.dirty = true;
    }

    pub fn compute_layout(&mut self, font_db: &mut FontDatabase, shaper: &mut dyn TextShaper) {
        if !self.dirty {
            return;
        }

        self.bidi_levels.clear();
        self.bidi_paragraphs.clear();
        self.runs.clear();
        self.tmp_runs.clear();
        self.glyphs.clear();
        self.lines.clear();

        self.compute_bidi();
        self.split_runs();
        self.shape_runs(font_db, shaper);
        self.sort_runs();
        self.break_words();
        self.split_runs_by_lines();
        self.measure_line_heights();
        self.layout();

        self.dirty = false;
    }

    fn compute_bidi(&mut self) {
        let bidi_info = BidiInfo::new(&self.text, None);
        self.bidi_levels = bidi_info.levels;
        self.bidi_paragraphs = bidi_info.paragraphs;
    }

    fn split_runs(&mut self) {
        for (section_idx, section) in self.sections.iter().enumerate() {
            Self::split_bidi(
                &self.bidi_levels,
                section.range.clone(),
                |range, bidi_level| {
                    self.runs.push(Run {
                        range,
                        glyph_range: 0..0,
                        section_idx,
                        bidi_level,
                        font: FontId::DUMMY,
                        font_size: 0.0,
                        line_height: 0.0,
                        text_height: 0.0,
                        pos: Vec2::ZERO,
                    });
                },
            );
        }
    }

    fn split_bidi(
        levels: &[BidiLevel],
        range: Range<usize>,
        mut callback: impl FnMut(Range<usize>, BidiLevel),
    ) {
        let mut subrange_start = range.start;
        let mut prev_level = levels[range.start];

        loop {
            let Some((subrange_end, next_level)) = levels[subrange_start + 1..range.end]
                .iter()
                .enumerate()
                .map(|(i, &l)| (subrange_start + i + 1, l))
                .find(|&(_, v)| v != prev_level)
            else {
                break;
            };

            callback(subrange_start..subrange_end, prev_level);

            prev_level = next_level;
            subrange_start = subrange_end;
        }

        if subrange_start != range.end {
            callback(subrange_start..range.end, prev_level);
        }
    }

    fn get_section_font<'a>(
        font_db: &'a mut FontDatabase,
        section: &mut Section,
        index: usize,
    ) -> Option<&'a FontFace> {
        if let Some(&font) = section.fonts.get(index) {
            return font_db.get_or_load(font).ok();
        }

        let font = font_db.query(&Self::font_attrs(&section.attrs, index))?;
        let index = index.max(section.fonts.len());

        section.fonts.insert(index, font);

        font_db.get_or_load(font).ok()
    }

    fn font_attrs(attrs: &TextAttrs, font_index: usize) -> FontAttrs {
        let mut fonts = attrs.fonts.iter().cloned();
        FontAttrs {
            family: fonts.nth(font_index).unwrap_or_else(FontFamily::sans_serif),
            weight: attrs.weight,
            width: attrs.width,
            style: attrs.style,
            ..Default::default()
        }
    }

    fn shape_runs(&mut self, font_db: &mut FontDatabase, shaper: &mut dyn TextShaper) {
        let mut run_idx = 0;
        'outer: while run_idx < self.runs.len() {
            let run = self.runs[run_idx].clone();
            let section = &mut self.sections[run.section_idx];
            let font_size = section.attrs.size;
            let line_height = match section.attrs.line_height {
                LineHeight::Px(v) => v,
                LineHeight::Relative(v) => v * font_size,
            };
            let text = &self.text[run.range.clone()];

            // try shaping with each font until success
            for font_index in 0..section.attrs.fonts.len() {
                let Some(font) = Self::get_section_font(font_db, section, font_index) else {
                    continue;
                };

                let glyphs_start = self.glyphs.len();
                shaper.shape(font, text, font_size, &mut self.glyphs);
                let glyphs_end = self.glyphs.len();

                let glyphs = &mut self.glyphs[glyphs_start..glyphs_end];

                for glyph in glyphs.iter_mut() {
                    glyph.cluster += run.range.start;
                }

                let is_missing = |glyph: &ShapedGlyph| {
                    // ignore missing whitespace glyphs
                    glyph.glyph_id == 0
                        && !self.text[glyph.cluster..]
                            .chars()
                            .next()
                            .is_some_and(char::is_whitespace)
                };

                if glyphs.iter().all(|v| !is_missing(v)) {
                    // no missing glyphs, success
                    let run = &mut self.runs[run_idx];
                    run.glyph_range = glyphs_start..glyphs_end;

                    let metrics = font.metrics();
                    run.font = font.id();
                    run.font_size = font_size;
                    run.text_height = ((metrics.ascender + metrics.descender) as f32)
                        / (metrics.units_per_em as f32)
                        * font_size;
                    run.line_height = line_height.max(run.text_height);
                    break;
                }

                // split current run by missing glyph/cluster boundaries
                // for example, if uppercase glyphs are missing:
                // abcXYZabcX is split into [abc, XYZ, abc, X]
                // new runs are inserted at the end (they will be sorted after shaping)

                let mut prev_is_missing = None;
                let mut prev_range_end = run.range.start;
                let mut glyph_i = glyphs_start;
                let mut prev_glyph_i = glyphs_start;
                let mut num_splits = 0;

                while glyph_i < glyphs_end {
                    let mut is_missing = false;
                    let mut cluster = None;

                    while glyph_i < glyphs_end {
                        let glyph = glyphs[glyph_i - glyphs_start];

                        if let Some(prev_cluster) = cluster {
                            if glyph.cluster != prev_cluster {
                                break;
                            }
                        } else {
                            cluster = Some(glyph.cluster);
                        }

                        if glyph.glyph_id == 0 {
                            is_missing = true;
                        }

                        glyph_i += 1;
                    }

                    let Some(cluster) = cluster else {
                        continue;
                    };

                    let char = self.text[cluster..].chars().next();
                    if char.is_some_and(char::is_whitespace) {
                        is_missing = false;
                    }

                    if prev_is_missing.is_none() {
                        prev_is_missing = Some(is_missing);
                    }

                    if Some(is_missing) == prev_is_missing {
                        continue;
                    }

                    let run = if prev_glyph_i == glyphs_start {
                        &mut self.runs[run_idx]
                    } else {
                        let idx = self.runs.len();
                        self.runs.push(run.clone());
                        &mut self.runs[idx]
                    };

                    run.range = prev_range_end..cluster;
                    run.glyph_range = 0..0;

                    prev_range_end = cluster;
                    prev_glyph_i = glyph_i;
                    prev_is_missing = Some(is_missing);
                    num_splits += 1;
                }

                if prev_glyph_i != glyphs_start && prev_glyph_i != glyphs_end {
                    self.runs.push(Run {
                        range: prev_range_end..run.range.end,
                        glyph_range: 0..0,
                        ..run.clone()
                    });
                    num_splits += 1;
                }

                self.glyphs.truncate(glyphs_start);

                if num_splits > 0 {
                    // restart shaping of this run
                    continue 'outer;
                } else {
                    // try next font
                    continue;
                }
            }

            run_idx += 1;
        }

        self.runs
            .retain(|run| !run.range.is_empty() && !run.glyph_range.is_empty());
    }

    fn sort_runs(&mut self) {
        self.runs.sort_unstable_by_key(|run| run.range.start);
    }

    fn break_words(&mut self) {
        let mut line = Line::default();
        let mut run_idx = 0;
        let mut glyph_idx = 0;

        let mut prev_break_opportunity = None;
        let mut prev_trailing_whitespace_width = 0.0;

        // dbg!(&self.runs);
        // dbg!(&self.glyphs);

        for (linebreak_idx, mut linebreak) in unicode_linebreak::linebreaks(&self.text) {
            // measure text until linebreak

            // dbg!(&self.text[..linebreak_idx]);
            // dbg!(linebreak_idx);

            let mut width = 0.0;
            let mut trailing_whitespace_width = 0.0;

            'measure: while run_idx < self.runs.len() {
                let run = &self.runs[run_idx];
                while glyph_idx < run.glyph_range.end {
                    let glyph = &self.glyphs[glyph_idx];
                    let cluster = glyph.cluster;
                    // dbg!(cluster);
                    if cluster >= linebreak_idx {
                        break 'measure;
                    }

                    let char = self.text[glyph.cluster..].chars().next();
                    let is_whitespace = char.is_some_and(char::is_whitespace);

                    while glyph_idx < run.glyph_range.end {
                        let glyph = &self.glyphs[glyph_idx];
                        if glyph.cluster != cluster {
                            break;
                        }

                        // dbg!(glyph_idx, glyph.x_advance);
                        width += glyph.x_advance;

                        if is_whitespace {
                            trailing_whitespace_width = glyph.x_advance;
                        } else {
                            trailing_whitespace_width = 0.0;
                        }

                        glyph_idx += 1;
                    }
                }

                run_idx += 1;

                if let Some(run) = self.runs.get(run_idx) {
                    glyph_idx = run.glyph_range.start;
                }
            }

            width -= trailing_whitespace_width;

            // if the segment fits and line break is not mandatory
            //   add segment to this line
            // if this segment doesn't fit
            //   try to break line before this segment
            //   otherwise break line after this segment, text will overflow

            let fits = line.width + prev_trailing_whitespace_width + width <= self.max_width;

            if fits {
                line.width += prev_trailing_whitespace_width + width;
                line.whitespace_width += prev_trailing_whitespace_width;
            } else if !fits {
                if let Some(idx) = prev_break_opportunity {
                    line.range.end = idx;
                    self.lines.push(line.clone());
                    line.width = width;
                    line.whitespace_width = 0.0;
                    line.range.start = idx;
                } else {
                    linebreak = BreakOpportunity::Mandatory;
                }
            }

            if linebreak == BreakOpportunity::Allowed {
                prev_break_opportunity = Some(linebreak_idx);
                prev_trailing_whitespace_width = trailing_whitespace_width;
            } else {
                line.range.end = linebreak_idx;
                self.lines.push(line.clone());
                line.width = 0.0;
                line.whitespace_width = 0.0;
                line.range.start = linebreak_idx;
                prev_break_opportunity = None;
                prev_trailing_whitespace_width = 0.0;
            }
        }
    }

    fn split_runs_by_lines(&mut self) {
        let mut run_idx = 0;
        let mut tmp_run_idx = 0;

        for line in &mut self.lines {
            let start_tmp_run_idx = tmp_run_idx;
            let mut num_tmp_runs = 0;

            while run_idx < self.runs.len() {
                let run = &mut self.runs[run_idx];
                if run.range.start >= line.range.end {
                    break;
                }

                if run.range.end <= line.range.end {
                    self.tmp_runs.push(run.clone());
                    run_idx += 1;
                    tmp_run_idx += 1;
                    num_tmp_runs += 1;
                } else if run.range.end > line.range.end {
                    let old_end = run.range.end;
                    let new_end = line.range.end;

                    let old_glyph_end = run.glyph_range.end;
                    let new_glyph_end = self.glyphs[run.glyph_range.clone()]
                        .iter()
                        .position(|g| g.cluster >= new_end)
                        .map(|v| v + run.glyph_range.start)
                        .unwrap_or(run.glyph_range.end);

                    run.range.end = new_end;
                    run.glyph_range.end = new_glyph_end;

                    self.tmp_runs.push(run.clone());
                    tmp_run_idx += 1;
                    num_tmp_runs += 1;

                    run.range.start = new_end;
                    run.range.end = old_end;

                    run.glyph_range.start = new_glyph_end;
                    run.glyph_range.end = old_glyph_end;

                    if run.range.is_empty() {
                        run_idx += 1;
                    }
                }
            }

            line.run_range = start_tmp_run_idx..start_tmp_run_idx + num_tmp_runs;
        }

        std::mem::swap(&mut self.runs, &mut self.tmp_runs);
    }

    fn measure_line_heights(&mut self) {
        for line in &mut self.lines {
            line.height = self.runs[line.run_range.clone()]
                .iter()
                .map(|v| v.line_height)
                .fold(0.0, f32::max);
        }
    }

    fn layout(&mut self) {
        let max_width = if self.max_width.is_finite() {
            self.max_width
        } else {
            self.lines.iter().map(|l| l.width).fold(0.0, f32::max)
        };

        let mut pos = Vec2::ZERO;

        for line in &self.lines {
            if line.run_range.is_empty() {
                continue;
            }

            let align = self.sections[self.runs[line.run_range.start].section_idx]
                .attrs
                .align;

            pos.x = match align {
                TextAlign::Start | TextAlign::Left => 0.0,
                TextAlign::End | TextAlign::Right => max_width - line.width,
                TextAlign::Center => (max_width - line.width) * 0.5,
                TextAlign::Justify => 0.0,
            };

            let whitespace_stretch = if align == TextAlign::Justify {
                1.0 + (max_width - line.width) / line.whitespace_width
            } else {
                1.0
            };

            for run in &mut self.runs[line.run_range.clone()] {
                run.pos.x = pos.x;
                run.pos.y = pos.y + (line.height - run.line_height) * 0.5 + run.line_height;

                let text = &self.text[run.range.clone()];
                let max_cluster = run.range.start + text.trim_end().len();
                let mut glyph_range_end = run.glyph_range.end;

                for (i, glyph) in self.glyphs[run.glyph_range.clone()].iter().enumerate() {
                    if glyph.cluster >= max_cluster {
                        glyph_range_end = run.glyph_range.start + i;
                    }

                    let is_whitespace = self.text[glyph.cluster..]
                        .chars()
                        .next()
                        .is_some_and(char::is_whitespace);
                    pos.x += if is_whitespace {
                        glyph.x_advance * whitespace_stretch
                    } else {
                        glyph.x_advance
                    };
                }

                // skip rendering whitespace characters at the end of words
                run.glyph_range.end = glyph_range_end;
            }

            pos.y += line.height;
        }
    }

    pub fn glyphs(&self) -> &[ShapedGlyph] {
        &self.glyphs
    }

    pub fn runs(&self) -> &[Run] {
        &self.runs
    }
}
