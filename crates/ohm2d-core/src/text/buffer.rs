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
    runs: Vec<Run>,
    glyphs: Vec<ShapedGlyph>,
    lines: Vec<Line>,
    bidi_paragraphs: Vec<BidiParagraph>,
    scratch_indices: Vec<usize>,
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
    pub linebreak: Option<BreakOpportunity>,
    pub font: FontId,
    pub font_size: f32,
    pub line_height: f32,
    pub text_height: f32,
    pub width: f32,
    pub trailing_whitespace_width: f32,
    pub pos: Vec2,
}

#[derive(Debug, Clone, Default)]
struct Line {
    range: Range<usize>,
    run_range: Range<usize>,
    is_rtl: bool,
    width: f32,
    whitespace_width: f32,
    height: f32,
    is_linebreak_forced: bool,
}

impl TextBuffer {
    pub fn new() -> TextBuffer {
        TextBuffer {
            text: String::new(),
            sections: Vec::new(),
            runs: Vec::new(),
            glyphs: Vec::new(),
            lines: Vec::new(),
            bidi_paragraphs: Vec::new(),
            scratch_indices: Vec::new(),
            max_width: f32::INFINITY,
            height: 0.0,
            dirty: true,
        }
    }

    pub fn reset(&mut self) {
        self.text.clear();
        self.sections.clear();
        self.runs.clear();
        self.glyphs.clear();
        self.lines.clear();
        self.bidi_paragraphs.clear();
        self.scratch_indices.clear();
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

        self.runs.clear();
        self.glyphs.clear();
        self.lines.clear();
        self.bidi_paragraphs.clear();
        self.scratch_indices.clear();

        self.split_runs_by_bidi_levels();
        self.shape_runs(font_db, shaper);
        self.split_runs_by_words();
        self.measure_runs();
        self.break_lines();
        self.measure_lines();
        self.bidi_reorder_runs();
        self.layout_lines();

        self.dirty = false;
    }

    fn split_runs_by_bidi_levels(&mut self) {
        let bidi_info = BidiInfo::new(&self.text, None);
        self.bidi_paragraphs = bidi_info.paragraphs;

        for (section_idx, section) in self.sections.iter().enumerate() {
            Self::split_bidi_helper(
                &bidi_info.levels,
                section.range.clone(),
                |range, bidi_level| {
                    self.runs.push(Run {
                        range,
                        glyph_range: 0..0,
                        section_idx,
                        bidi_level,
                        linebreak: None,
                        font: FontId::DUMMY,
                        font_size: 0.0,
                        line_height: 0.0,
                        text_height: 0.0,
                        width: 0.0,
                        trailing_whitespace_width: 0.0,
                        pos: Vec2::ZERO,
                    });
                },
            );
        }
    }

    fn split_bidi_helper(
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
        let index = index.min(section.fonts.len());

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

                shaper.shape(
                    font,
                    text,
                    font_size,
                    run.bidi_level.is_rtl(),
                    &mut self.glyphs,
                );

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

                if prev_glyph_i != glyphs_start && prev_range_end < run.range.end {
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

        self.runs.sort_unstable_by_key(|run| run.range.start);
    }

    fn split_runs_by_words(&mut self) {
        // push splitted words at the end of self.runs, then remove old unsplitted runs

        let mut run_idx = 0;
        let max_run_idx = self.runs.len();

        for (linebreak_idx, linebreak) in unicode_linebreak::linebreaks(&self.text) {
            while run_idx < max_run_idx {
                let run = &mut self.runs[run_idx];
                if run.range.start >= linebreak_idx {
                    break;
                }

                let end = linebreak_idx.min(run.range.end);

                let glyphs = &self.glyphs[run.glyph_range.clone()];
                let glyph_end = glyphs
                    .iter()
                    .position(|v| v.cluster >= end)
                    .map(|v| run.glyph_range.start + v)
                    .unwrap_or(run.glyph_range.end);

                let new_run = Run {
                    range: run.range.start..end,
                    glyph_range: run.glyph_range.start..glyph_end,
                    linebreak: (end == linebreak_idx).then_some(linebreak),
                    ..run.clone()
                };

                run.range.start = end;
                run.glyph_range.start = glyph_end;

                if run.range.is_empty() {
                    run_idx += 1;
                }

                self.runs.push(new_run);
            }
        }

        self.runs.drain(..max_run_idx);
    }

    fn measure_runs(&mut self) {
        for run in &mut self.runs {
            for glyph in &self.glyphs[run.glyph_range.clone()] {
                let char = self.text[glyph.cluster..].chars().next();
                let is_whitespace = char.is_some_and(char::is_whitespace);

                if is_whitespace {
                    run.trailing_whitespace_width += glyph.x_advance;
                } else {
                    run.trailing_whitespace_width = 0.0;
                }

                run.width += glyph.x_advance;
            }

            run.width -= run.trailing_whitespace_width;
        }
    }

    fn break_lines(&mut self) {
        let mut line = Line::default();
        let mut prev_trailing_whitespace = 0.0;
        let mut prev_break_opportunity = None;

        for (run_idx, run) in self.runs.iter().enumerate() {
            let fits = line.width + prev_trailing_whitespace + run.width <= self.max_width;

            if fits {
                line.width += prev_trailing_whitespace + run.width;
                prev_trailing_whitespace = run.trailing_whitespace_width;
            } else if let Some(idx) = prev_break_opportunity {
                let prev_runs = self.runs[idx..run_idx].iter().enumerate();
                let subtract_width = prev_runs
                    .map(|(i, run)| {
                        if i == 0 && idx + 1 != run_idx {
                            run.trailing_whitespace_width
                        } else if i > 0 {
                            run.width + run.trailing_whitespace_width
                        } else {
                            0.0
                        }
                    })
                    .sum::<f32>();

                let prev_runs = self.runs[idx + 1..run_idx].iter();
                let add_width = prev_runs
                    .map(|run| run.width + run.trailing_whitespace_width)
                    .sum::<f32>();

                line.width -= subtract_width;
                line.run_range.end = idx + 1;

                self.lines.push(line.clone());

                line.run_range.start = idx + 1;
                line.width = add_width + run.width;

                prev_trailing_whitespace = run.trailing_whitespace_width;
                prev_break_opportunity = None;
            }

            match run.linebreak {
                Some(BreakOpportunity::Mandatory) => {}
                Some(BreakOpportunity::Allowed) => {}
                None => {}
            }

            if run.linebreak == Some(BreakOpportunity::Mandatory) {
                line.is_linebreak_forced = true;
                line.run_range.end = run_idx + 1;
                self.lines.push(line.clone());
                line.is_linebreak_forced = false;
                line.run_range.start = run_idx + 1;
                line.width = 0.0;
                prev_trailing_whitespace = 0.0;
                prev_break_opportunity = None;
            }

            if run.linebreak == Some(BreakOpportunity::Allowed) {
                prev_break_opportunity = Some(run_idx);
            }
        }

        // handle last line

        line.run_range.end = self.runs.len();
        if !line.run_range.is_empty() {
            self.lines.push(line);
        }

        self.lines.retain(|v| !v.run_range.is_empty());

        // remove trailing whitespace glyphs

        for line in &mut self.lines {
            line.range = self.runs[line.run_range.start].range.start
                ..self.runs[line.run_range.end - 1].range.end;

            let max_cluster = line.range.start + self.text[line.range.clone()].trim_end().len();

            for run in &mut self.runs[line.run_range.clone()] {
                run.glyph_range.end = self.glyphs[run.glyph_range.clone()]
                    .iter()
                    .position(|v| v.cluster >= max_cluster)
                    .map(|v| run.glyph_range.start + v)
                    .unwrap_or(run.glyph_range.end);
            }
        }
    }

    fn measure_lines(&mut self) {
        let mut bidi_paragraph_idx = 0;

        for line in &mut self.lines {
            while let Some(paragraph) = self.bidi_paragraphs.get(bidi_paragraph_idx) {
                if paragraph.range.contains(&line.range.start) {
                    line.is_rtl = paragraph.level.is_rtl();
                    break;
                } else {
                    bidi_paragraph_idx += 1;
                }
            }

            line.height = self.runs[line.run_range.clone()]
                .iter()
                .map(|v| v.line_height)
                .fold(0.0, f32::max);

            let glyphs = self.runs[line.run_range.clone()]
                .iter()
                .flat_map(|run| self.glyphs[run.glyph_range.clone()].iter());

            for glyph in glyphs {
                let char = self.text[glyph.cluster..].chars().next();
                let is_whitespace = char.is_some_and(char::is_whitespace);
                if is_whitespace {
                    line.whitespace_width += glyph.x_advance;
                }
            }
        }
    }

    fn bidi_reorder_runs(&mut self) {
        if self.runs.iter().all(|v| v.bidi_level.is_ltr()) {
            return;
        }

        let num_runs = self.runs.len();

        for line in &self.lines {
            let runs = &self.runs[line.run_range.clone()];

            self.scratch_indices.clear();

            Self::bidi_reorder_visual(
                runs.len(),
                |i| runs.get(i).map(|v| v.bidi_level),
                &mut self.scratch_indices,
            );

            for &run_idx in &self.scratch_indices {
                let run = self.runs[line.run_range.start + run_idx].clone();
                self.runs.push(run);
            }

            // swap old (pre-reordered) and new (reordered) runs

            let (l, r) = self.runs.split_at_mut(line.run_range.end);
            let l_start = line.run_range.start;
            let r_start = r.len() - line.run_range.len();
            l[l_start..].swap_with_slice(&mut r[r_start..]);

            // remove old runs

            self.runs.truncate(num_runs);

            // reverse rtl runs

            for run in &mut self.runs[line.run_range.clone()] {
                if run.bidi_level.is_rtl() {
                    let glyphs = &mut self.glyphs[run.glyph_range.clone()];
                    glyphs.reverse();
                }
            }
        }
    }

    /// adopted from unicode-bidi
    fn bidi_reorder_visual(
        num_levels: usize,
        get_level: impl Fn(usize) -> Option<BidiLevel>,
        result: &mut Vec<usize>,
    ) {
        // Gets the next range of characters after start_index with a level greater
        // than or equal to `max`
        fn next_range(
            num_levels: usize,
            get_level: impl Fn(usize) -> Option<BidiLevel>,
            mut start_index: usize,
            max: BidiLevel,
        ) -> Range<usize> {
            if num_levels == 0 || start_index >= num_levels {
                return start_index..start_index;
            }
            while let Some(l) = get_level(start_index) {
                if l >= max {
                    break;
                }
                start_index += 1;
            }

            if get_level(start_index).is_none() {
                // If at the end of the array, adding one will
                // produce an out-of-range end element
                return start_index..start_index;
            }

            let mut end_index = start_index + 1;
            while let Some(l) = get_level(end_index) {
                if l < max {
                    return start_index..end_index;
                }
                end_index += 1;
            }

            start_index..end_index
        }

        // This implementation is similar to the L2 implementation in `visual_runs()`
        // but it cannot benefit from a precalculated LevelRun vector so needs to be different.

        if num_levels == 0 {
            result.clear();
        }

        let first_level = get_level(0).unwrap();

        // Get the min and max levels
        let (mut min, mut max) =
            (0..num_levels).fold((first_level, first_level), |(min, max), i| {
                let l = get_level(i).unwrap();
                (std::cmp::min(min, l), std::cmp::max(max, l))
            });

        // Initialize an index map
        result.extend(0..num_levels);

        if min == max && min.is_ltr() {
            // Everything is LTR and at the same level, do nothing
            return;
        }

        // Stop at the lowest *odd* level, since everything below that
        // is LTR and does not need further reordering
        min = min.new_lowest_ge_rtl().expect("Level error");

        // For each max level, take all contiguous chunks of
        // levels ≥ max and reverse them
        //
        // We can do this check with the original levels instead of checking reorderings because all
        // prior reorderings will have been for contiguous chunks of levels >> max, which will
        // be a subset of these chunks anyway.
        while min <= max {
            let mut range = 0..0;
            loop {
                range = next_range(num_levels, &get_level, range.end, max);
                result[range.clone()].reverse();

                if range.end >= num_levels {
                    break;
                }
            }

            max.lower(1).expect("Level error");
        }
    }

    fn layout_lines(&mut self) {
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

            let whitespace_stretch = if align == TextAlign::Justify && !line.is_linebreak_forced {
                1.0 + (max_width - line.width) / line.whitespace_width
            } else {
                1.0
            };

            let (start, is_left_aligned) = match (align, line.is_rtl) {
                (TextAlign::Left, _)
                | (TextAlign::Start | TextAlign::Justify, false)
                | (TextAlign::End, true) => (0.0, true),
                (TextAlign::Right, _)
                | (TextAlign::Start | TextAlign::Justify, true)
                | (TextAlign::End, false) => (max_width, false),
                (TextAlign::Center, _) => ((max_width - line.width) * 0.5, true),
            };

            pos.x = start;

            let mut run_idx = if is_left_aligned {
                line.run_range.start
            } else {
                line.run_range.end - 1
            };

            while line.run_range.contains(&run_idx) {
                let run = &mut self.runs[run_idx];
                run.pos.y = pos.y + (line.height - run.line_height) * 0.5 + run.line_height;

                if is_left_aligned {
                    run.pos.x = pos.x;
                    run_idx += 1;
                }

                for glyph in &mut self.glyphs[run.glyph_range.clone()] {
                    let char = self.text[glyph.cluster..].chars().next();
                    let is_whitespace = char.is_some_and(char::is_whitespace);

                    if is_whitespace {
                        glyph.x_advance *= whitespace_stretch;
                    }

                    if is_left_aligned {
                        pos.x += glyph.x_advance;
                    } else {
                        pos.x -= glyph.x_advance;
                    }
                }

                if !is_left_aligned {
                    run.pos.x = pos.x;
                    run_idx = run_idx.wrapping_sub(1);
                }
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

impl Default for TextBuffer {
    fn default() -> Self {
        TextBuffer::new()
    }
}
