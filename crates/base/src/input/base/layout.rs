use std::{ops::Range, rc::Rc};

use gpui::{Bounds, Half, Pixels, ShapedLine, Size, TextAlign, px};

use super::{WrappingIndent, display_map::LineLayout};

#[derive(Clone, Default)]
pub(crate) struct WhitespaceIndicators {
    pub(crate) space: ShapedLine,
    pub(crate) tab: ShapedLine,
}

/// Where the blocks stand: for each one, the display row it is drawn before
/// and the height it measured. Sorted by row, and empty for every input that
/// asks for none, which is where the uniform row geometry still holds.
#[derive(Clone, Default)]
pub(super) struct BlockPlacement {
    placed: Vec<(usize, Pixels)>,
}

impl BlockPlacement {
    pub(super) fn new(mut placed: Vec<(usize, Pixels)>) -> Self {
        placed.sort_by_key(|(row, _)| *row);
        Self { placed }
    }

    /// Height of the blocks standing before a display row, the ones at it
    /// excluded: the y where those begin.
    pub(super) fn space_before(&self, display_row: usize) -> Pixels {
        self.placed
            .iter()
            .take_while(|(row, _)| *row < display_row)
            .map(|(_, height)| *height)
            .fold(px(0.), |total, height| total + height)
    }

    /// Height of the blocks drawn immediately before this display row.
    pub(super) fn space_at(&self, display_row: usize) -> Pixels {
        self.placed
            .iter()
            .skip_while(|(row, _)| *row < display_row)
            .take_while(|(row, _)| *row == display_row)
            .map(|(_, height)| *height)
            .fold(px(0.), |total, height| total + height)
    }

    pub(super) fn total(&self) -> Pixels {
        self.placed
            .iter()
            .map(|(_, height)| *height)
            .fold(px(0.), |total, height| total + height)
    }

    /// The y where the blocks at a display row begin, which is where the row
    /// itself begins when it has none.
    pub(super) fn top_of(&self, display_row: usize, line_height: Pixels) -> Pixels {
        line_height * display_row as f32 + self.space_before(display_row)
    }

    /// The y the text of a display row is drawn at.
    pub(super) fn y_of(&self, display_row: usize, line_height: Pixels) -> Pixels {
        self.top_of(display_row, line_height) + self.space_at(display_row)
    }

    /// The display row at a y. A block belongs to the row it stands before, so
    /// a pointer over one lands on that row.
    pub(super) fn row_at(&self, y: Pixels, line_height: Pixels) -> usize {
        let mut space = px(0.);
        for (row, height) in &self.placed {
            if y < line_height * *row as f32 + space {
                break;
            }
            space += *height;
            if y < line_height * *row as f32 + space {
                return *row;
            }
        }
        (f32::from((y - space).max(px(0.))) / f32::from(line_height)).floor() as usize
    }
}

#[derive(Clone)]
pub(super) struct LastLayout {
    pub(super) visible_range: Range<usize>,
    pub(super) visible_buffer_lines: Vec<usize>,
    pub(super) visible_line_byte_offsets: Vec<usize>,
    /// Height standing before each visible line, where a block is drawn in
    /// place of the lines it hides. Zero everywhere for an input with none,
    /// which is every input but a note.
    pub(super) block_space: Vec<Pixels>,
    /// The display row each visible line starts at.
    pub(super) visible_display_rows: Vec<usize>,
    /// Where the blocks stand, which is what turns a display row into a y.
    pub(super) placement: BlockPlacement,
    pub(super) visible_top: Pixels,
    pub(super) visible_range_offset: Range<usize>,
    pub(super) lines: Rc<Vec<LineLayout>>,
    pub(super) line_height: Pixels,
    pub(super) wrap_width: Option<Pixels>,
    pub(super) wrapping_indent: WrappingIndent,
    pub(super) line_number_width: Pixels,
    pub(super) cursor_size: Size<Pixels>,
    pub(super) cursor_bounds: Option<Bounds<Pixels>>,
    pub(super) text_align: TextAlign,
    pub(super) content_width: Pixels,
}

impl LastLayout {
    /// The height to insert before the visible line at this index.
    #[inline]
    pub(super) fn block_space_at(&self, index: usize) -> Pixels {
        self.block_space.get(index).copied().unwrap_or(px(0.))
    }

    pub(crate) fn line(&self, row: usize) -> Option<&LineLayout> {
        let pos = self.visible_buffer_lines.binary_search(&row).ok()?;
        self.lines.get(pos)
    }

    pub(super) fn alignment_offset(&self, line_width: Pixels) -> Pixels {
        match self.text_align {
            TextAlign::Left => px(0.),
            TextAlign::Center => (self.content_width - line_width).half().max(px(0.)),
            TextAlign::Right => (self.content_width - line_width).max(px(0.)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_sits_below_the_blocks_drawn_before_it() {
        let line_height = px(20.);
        let placement = BlockPlacement::new(vec![(5, px(30.)), (2, px(50.)), (5, px(10.))]);

        // Rows before any block are where they always were.
        assert_eq!(placement.top_of(0, line_height), px(0.));
        assert_eq!(placement.y_of(1, line_height), px(20.));

        // A block stands between the row before it and the row it belongs to,
        // and two at one row stack in the order they were given.
        assert_eq!(placement.top_of(2, line_height), px(40.));
        assert_eq!(placement.y_of(2, line_height), px(90.));
        assert_eq!(placement.y_of(5, line_height), px(190.));
        assert_eq!(placement.space_at(5), px(40.));
        assert_eq!(placement.total(), px(90.));

        // Pointing at a block means pointing at the row it stands before.
        assert_eq!(placement.row_at(px(45.), line_height), 2);
        assert_eq!(placement.row_at(px(90.), line_height), 2);
        assert_eq!(placement.row_at(px(110.), line_height), 3);
        assert_eq!(placement.row_at(px(155.), line_height), 5);
        assert_eq!(placement.row_at(px(195.), line_height), 5);

        // With nothing placed the geometry is the uniform one, untouched.
        let none = BlockPlacement::default();
        assert_eq!(none.y_of(4, line_height), px(80.));
        assert_eq!(none.row_at(px(81.), line_height), 4);
        assert_eq!(none.total(), px(0.));
    }
}
