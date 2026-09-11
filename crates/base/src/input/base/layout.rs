use std::{ops::Range, rc::Rc};

use gpui::{Bounds, Half, Pixels, ShapedLine, Size, TextAlign, px};

use super::{WrappingIndent, display_map::LineLayout};

#[derive(Clone, Default)]
pub(crate) struct WhitespaceIndicators {
    pub(crate) space: ShapedLine,
    pub(crate) tab: ShapedLine,
}

#[derive(Clone, Default)]
pub(super) struct RowGeometry {
    line_height: Pixels,
    row_count: usize,
    rows: Rc<Vec<RowMetrics>>,
}

#[derive(Clone)]
struct RowMetrics {
    top: Pixels,
    text_top: Pixels,
    height: Pixels,
}

impl RowGeometry {
    pub(super) fn new(
        line_height: Pixels,
        row_count: usize,
        blocks: Vec<(usize, Pixels)>,
        heights: Vec<(Range<usize>, Pixels)>,
    ) -> Self {
        // Ordinary inputs keep their constant-time, allocation-free geometry.
        if blocks.is_empty() && heights.is_empty() {
            return Self {
                line_height,
                row_count,
                rows: Rc::new(Vec::new()),
            };
        }
        // The extra entry owns blocks after the last visible source line.
        let mut rows = vec![
            RowMetrics {
                top: px(0.),
                text_top: px(0.),
                height: line_height
            };
            row_count + 1
        ];
        for (range, height) in heights {
            for row in rows
                .iter_mut()
                .take(range.end.min(row_count))
                .skip(range.start)
            {
                row.height = height;
            }
        }
        for (row, height) in blocks {
            rows[row.min(row_count)].text_top += height;
        }
        let mut top = px(0.);
        for row in &mut rows {
            row.top = top;
            row.text_top += top;
            top = row.text_top + row.height;
        }
        Self {
            line_height,
            row_count,
            rows: Rc::new(rows),
        }
    }

    pub(super) fn space_at(&self, row: usize) -> Pixels {
        self.rows
            .get(row)
            .map_or(px(0.), |row| row.text_top - row.top)
    }

    pub(super) fn total(&self) -> Pixels {
        self.y_of(self.row_count)
    }

    pub(super) fn is_uniform(&self) -> bool {
        self.rows.is_empty()
    }

    pub(super) fn top_of(&self, row: usize) -> Pixels {
        self.rows
            .get(row)
            .map_or_else(|| self.tail_y(row), |row| row.top)
    }

    pub(super) fn y_of(&self, row: usize) -> Pixels {
        self.rows
            .get(row)
            .map_or_else(|| self.tail_y(row), |row| row.text_top)
    }

    fn tail_y(&self, row: usize) -> Pixels {
        self.rows
            .last()
            .map_or(self.line_height * row as f32, |last| {
                last.text_top + self.line_height * row.saturating_sub(self.row_count) as f32
            })
    }

    pub(super) fn row_height(&self, row: usize) -> Pixels {
        self.rows
            .get(row)
            .map_or(self.line_height, |row| row.height)
    }

    pub(super) fn row_at(&self, y: Pixels) -> usize {
        if self.rows.is_empty() {
            return (y.max(px(0.)) / self.line_height.max(px(1.))).floor() as usize;
        }
        self.rows
            .partition_point(|row| row.top <= y)
            .saturating_sub(1)
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
    /// Where the blocks stand, which is what turns a display row into a y.
    pub(super) placement: RowGeometry,
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
        let placement = RowGeometry::new(
            line_height,
            8,
            vec![(5, px(30.)), (2, px(50.)), (5, px(10.))],
            vec![],
        );

        // Rows before any block are where they always were.
        assert_eq!(placement.top_of(0), px(0.));
        assert_eq!(placement.y_of(1), px(20.));

        // A block stands between the row before it and the row it belongs to,
        // and two at one row stack in the order they were given.
        assert_eq!(placement.top_of(2), px(40.));
        assert_eq!(placement.y_of(2), px(90.));
        assert_eq!(placement.y_of(5), px(190.));
        assert_eq!(placement.space_at(5), px(40.));
        assert_eq!(placement.total(), px(250.));

        // Pointing at a block means pointing at the row it stands before.
        assert_eq!(placement.row_at(px(45.)), 2);
        assert_eq!(placement.row_at(px(90.)), 2);
        assert_eq!(placement.row_at(px(110.)), 3);
        assert_eq!(placement.row_at(px(155.)), 5);
        assert_eq!(placement.row_at(px(195.)), 5);

        // With nothing placed the geometry is the uniform one, untouched.
        let none = RowGeometry::new(line_height, 8, vec![], vec![]);
        assert_eq!(none.y_of(4), px(80.));
        assert_eq!(none.row_at(px(81.)), 4);
        assert_eq!(none.total(), px(160.));

        let heading = RowGeometry::new(
            line_height,
            3,
            vec![(1, px(50.)), (3, px(60.))],
            vec![(0..1, px(40.))],
        );
        assert_eq!(heading.y_of(1), px(90.));
        assert_eq!(heading.row_at(px(30.)), 0);
        assert_eq!(heading.row_at(px(80.)), 1);
        assert_eq!(heading.top_of(3), px(130.));
        assert_eq!(heading.total(), px(190.));
        let all_blocks = RowGeometry::new(line_height, 0, vec![(0, px(75.)), (0, px(40.))], vec![]);
        assert_eq!(all_blocks.total(), px(115.));
        assert_eq!(all_blocks.row_at(px(100.)), 0);
    }
}
