use std::ops::Range;

use gpui::{Font, FontWeight, Pixels, px};
use ropey::Rope;

use crate::input::RopeExt;

/// Typography for complete buffer lines, including their soft wraps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EditorLineStyle {
    lines: Range<usize>,
    font_size: Option<Pixels>,
    line_height: Option<Pixels>,
    font_weight: Option<FontWeight>,
}

impl EditorLineStyle {
    pub fn new(lines: Range<usize>) -> Self {
        Self {
            lines,
            ..Self::default()
        }
    }

    pub fn with_font_size(mut self, size: Pixels) -> Self {
        self.font_size = Some(size.max(px(1.)));
        self
    }

    pub fn with_line_height(mut self, height: Pixels) -> Self {
        self.line_height = Some(height.max(px(1.)));
        self
    }

    pub fn with_font_weight(mut self, weight: FontWeight) -> Self {
        self.font_weight = Some(weight);
        self
    }

    pub fn lines(&self) -> Range<usize> {
        self.lines.clone()
    }
    pub fn font_size(&self) -> Option<Pixels> {
        self.font_size
    }
    pub fn line_height(&self) -> Option<Pixels> {
        self.line_height
    }
    pub fn font_weight(&self) -> Option<FontWeight> {
        self.font_weight
    }

    pub(crate) fn font(&self, mut font: Font) -> Font {
        if let Some(weight) = self.font_weight {
            font.weight = weight;
        }
        font
    }
}

/// A view of the editor's buffer. Concealed UTF-8 ranges retain their source
/// positions for editing, selection, copy and undo.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EditorPresentation {
    concealed: Vec<Range<usize>>,
    lines: Vec<EditorLineStyle>,
}

impl EditorPresentation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_concealed_ranges(mut self, ranges: Vec<Range<usize>>) -> Self {
        self.concealed = ranges;
        self
    }

    pub fn with_line_styles(mut self, lines: Vec<EditorLineStyle>) -> Self {
        self.lines = lines;
        self
    }

    pub fn concealed_ranges(&self) -> &[Range<usize>] {
        &self.concealed
    }
    pub fn line_styles(&self) -> &[EditorLineStyle] {
        &self.lines
    }

    pub(crate) fn line(&self, row: usize) -> Option<&EditorLineStyle> {
        self.lines
            .iter()
            .rev()
            .find(|style| style.lines.contains(&row))
    }

    pub(crate) fn normalize(&mut self) {
        self.concealed.retain(|range| !range.is_empty());
        self.concealed.sort_by_key(|range| range.start);
        let mut merged: Vec<Range<usize>> = Vec::with_capacity(self.concealed.len());
        for range in self.concealed.drain(..) {
            if let Some(last) = merged.last_mut().filter(|last| last.end >= range.start) {
                last.end = last.end.max(range.end);
            } else {
                merged.push(range);
            }
        }
        self.concealed = merged;
        self.lines.retain(|style| !style.lines.is_empty());
        self.lines.sort_by_key(|style| style.lines.start);
    }

    pub(crate) fn adjust_for_edit(&mut self, old: &Rope, range: &Range<usize>, new: &Rope) {
        if self.concealed.is_empty() && self.lines.is_empty() {
            return;
        }
        let byte_delta = new.len() as isize - range.len() as isize;
        self.concealed.retain_mut(|hidden| {
            if hidden.end < range.start {
                return true;
            }
            if hidden.start <= range.end {
                return false;
            }
            hidden.start = hidden.start.saturating_add_signed(byte_delta);
            hidden.end = hidden.end.saturating_add_signed(byte_delta);
            true
        });
        let start_line = old.offset_to_point(range.start.min(old.len())).row;
        let end_line = old.offset_to_point(range.end.min(old.len())).row;
        let line_delta = new.chars().filter(|&ch| ch == '\n').count() as isize
            - (end_line - start_line) as isize;
        self.lines.retain_mut(|style| {
            if style.lines.end <= start_line {
                return true;
            }
            if style.lines.start > end_line {
                style.lines.start = style.lines.start.saturating_add_signed(line_delta);
                style.lines.end = style.lines.end.saturating_add_signed(line_delta);
                return true;
            }
            // A parser can replace the affected style after the edit. Never
            // apply an old heading style to text joined from another block.
            false
        });
    }
}
