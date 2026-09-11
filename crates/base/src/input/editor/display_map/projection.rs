use std::{borrow::Cow, ops::Range};

use gpui::TextRun;

/// Byte mapping for a single source line. Newlines are never concealed.
#[derive(Clone, Debug, Default)]
pub(crate) struct TextProjection {
    source_len: usize,
    hidden: Vec<Range<usize>>,
}

impl TextProjection {
    pub(crate) fn new(source: &str, offset: usize, concealed: &[Range<usize>]) -> Self {
        let end = offset + source.len();
        let first = concealed.partition_point(|range| range.end <= offset);
        let hidden = concealed[first..]
            .iter()
            .take_while(|range| range.start < end)
            .filter_map(|range| {
                let local = range.start.saturating_sub(offset)..(range.end.min(end) - offset);
                source.get(local.clone()).map(|_| local)
            })
            .collect();
        Self {
            source_len: source.len(),
            hidden,
        }
    }

    pub(crate) fn source_len(&self) -> usize {
        self.source_len
    }

    pub(crate) fn display_offset(&self, source: usize) -> usize {
        let source = source.min(self.source_len);
        source
            - self
                .hidden
                .iter()
                .take_while(|range| range.start < source)
                .map(|range| source.min(range.end) - range.start)
                .sum::<usize>()
    }

    pub(crate) fn source_offset(&self, display: usize) -> usize {
        let mut source = display;
        for range in &self.hidden {
            if source < range.start {
                break;
            }
            source += range.len();
        }
        source.min(self.source_len)
    }

    pub(crate) fn text<'a>(&self, source: &'a str) -> Cow<'a, str> {
        if self.hidden.is_empty() {
            return Cow::Borrowed(source);
        }
        let mut output = String::with_capacity(source.len());
        let mut at = 0;
        for range in &self.hidden {
            output.push_str(&source[at..range.start]);
            at = range.end;
        }
        output.push_str(&source[at..]);
        Cow::Owned(output)
    }

    pub(crate) fn runs(&self, runs: Vec<TextRun>, start: usize) -> Vec<TextRun> {
        if self.hidden.is_empty() {
            return runs;
        }
        let mut at = start;
        runs.into_iter()
            .filter_map(|mut run| {
                let end = at + run.len;
                run.len = self.display_offset(end) - self.display_offset(at);
                at = end;
                (run.len > 0).then_some(run)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concealed_syntax_maps_visible_unicode_back_to_the_original_buffer() {
        let source = "**æ🌲** and [日本語](destination)";
        let label = source.find('[').expect("link");
        let destination = source.find(']').expect("destination");
        let projection = TextProjection::new(
            source,
            0,
            &[0..2, 8..10, label..label + 1, destination..source.len()],
        );
        let visible = projection.text(source);
        assert_eq!(visible, "æ🌲 and 日本語");
        for (display, _) in visible.char_indices() {
            let original = projection.source_offset(display);
            assert!(source.is_char_boundary(original));
            assert_eq!(projection.display_offset(original), display);
            assert_eq!(
                source[original..].chars().next(),
                visible[display..].chars().next()
            );
        }
        assert_eq!(projection.display_offset(0), 0);
        assert_eq!(projection.display_offset(1), 0);
        assert_eq!(projection.source_offset(visible.len()), source.len());
        // An obsolete range inside a UTF-8 character must not panic before
        // the caller has had a chance to reparse a programmatic edit.
        let stale = TextProjection::new("🌲", 0, &[1..3]);
        assert_eq!(stale.text("🌲"), "🌲");
    }
}
