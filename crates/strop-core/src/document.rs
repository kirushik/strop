//! Rich-text schema types and the span/anchor machinery.
//!
//! See docs/document-model.md. Spans are char-indexed ranges over the same
//! text stream the rope and Loro share; `SpanSet::apply_op` keeps them
//! consistent across every edit (including undo/redo, which arrive as
//! ordinary ops). The same adjustment math will anchor annotations.

use std::ops::Range;

use crate::buffer::TextOp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineAttr {
    Emphasis,
    Strong,
    Strikethrough,
    Underline,
    Highlight,
    Code,
    Link(String),
    FootnoteRef(String),
}

impl InlineAttr {
    /// Peritext expansion: typing at the right edge continues the style.
    /// Code and links must not grow under the caret.
    pub fn expands(&self) -> bool {
        !matches!(self, Self::Code | Self::Link(_) | Self::FootnoteRef(_))
    }
}

/// Per-block kind; lives beside the text, keyed by block index.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum BlockKind {
    #[default]
    Paragraph,
    Heading(u8),
    Blockquote,
    ListItem {
        ordered: bool,
        depth: u8,
    },
    Divider,
    Image {
        src: String,
        alt: String,
        caption: String,
    },
    FootnoteDef {
        id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub range: Range<usize>,
    pub attr: InlineAttr,
}

/// Inline formatting as an interval set, kept sorted by start.
#[derive(Debug, Default)]
pub struct SpanSet {
    spans: Vec<Span>,
}

impl SpanSet {
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    pub fn attrs_at(&self, pos: usize) -> impl Iterator<Item = &InlineAttr> {
        self.spans
            .iter()
            .filter(move |s| s.range.start <= pos && pos < s.range.end)
            .map(|s| &s.attr)
    }

    /// Is every position in `range` covered by spans with this attribute?
    /// (Same-attr spans may be adjacent after edits; chains count.)
    pub fn covers(&self, range: Range<usize>, attr: &InlineAttr) -> bool {
        if range.start >= range.end {
            return false;
        }
        let mut covered_to = range.start;
        for s in &self.spans {
            if s.attr != *attr || s.range.end <= covered_to {
                continue;
            }
            if s.range.start > covered_to {
                return false; // spans are sorted: this is a gap
            }
            covered_to = s.range.end;
            if covered_to >= range.end {
                return true;
            }
        }
        false
    }

    /// Apply an attribute over a range, merging with touching/overlapping
    /// spans of the same attribute.
    pub fn add(&mut self, range: Range<usize>, attr: InlineAttr) {
        if range.start >= range.end {
            return;
        }
        let mut merged = range;
        self.spans.retain(|s| {
            if s.attr == attr && s.range.start <= merged.end && merged.start <= s.range.end {
                merged.start = merged.start.min(s.range.start);
                merged.end = merged.end.max(s.range.end);
                false
            } else {
                true
            }
        });
        let at = self
            .spans
            .partition_point(|s| s.range.start < merged.start);
        self.spans.insert(
            at,
            Span {
                range: merged,
                attr,
            },
        );
    }

    /// Clear an attribute from a range, splitting spans that straddle it.
    pub fn remove(&mut self, range: Range<usize>, attr: &InlineAttr) {
        let mut result = Vec::with_capacity(self.spans.len());
        for span in self.spans.drain(..) {
            if span.attr != *attr || span.range.end <= range.start || range.end <= span.range.start
            {
                result.push(span);
                continue;
            }
            if span.range.start < range.start {
                result.push(Span {
                    range: span.range.start..range.start,
                    attr: span.attr.clone(),
                });
            }
            if range.end < span.range.end {
                result.push(Span {
                    range: range.end..span.range.end,
                    attr: span.attr.clone(),
                });
            }
        }
        self.spans = result;
    }

    /// Keep all spans consistent across a text edit: delete `op.delete`
    /// chars at `op.pos`, then insert `op.insert` there.
    pub fn apply_op(&mut self, op: &TextOp) {
        let del_end = op.pos + op.delete;
        let ins = op.insert.chars().count();
        let clamp = |x: usize| {
            if x >= del_end {
                x - op.delete
            } else if x > op.pos {
                op.pos
            } else {
                x
            }
        };
        for span in &mut self.spans {
            if op.delete > 0 {
                span.range.start = clamp(span.range.start);
                span.range.end = clamp(span.range.end);
            }
            if ins > 0 {
                // Typing at the left edge stays outside (start shifts);
                // strictly inside grows the span; at the right edge only
                // expanding styles absorb the insertion.
                if span.range.start >= op.pos {
                    span.range.start += ins;
                }
                if span.range.end > op.pos
                    || (span.range.end == op.pos && span.attr.expands())
                {
                    span.range.end += ins;
                }
            }
        }
        self.spans.retain(|s| s.range.start < s.range.end);
        self.spans.sort_by_key(|s| s.range.start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(pos: usize, delete: usize, insert: &str) -> TextOp {
        TextOp {
            pos,
            delete,
            insert: insert.into(),
        }
    }

    fn strong(range: Range<usize>) -> Span {
        Span {
            range,
            attr: InlineAttr::Strong,
        }
    }

    #[test]
    fn typing_at_right_edge_expands_styles_not_code() {
        let mut set = SpanSet::default();
        set.add(0..4, InlineAttr::Strong);
        set.apply_op(&op(4, 0, "x"));
        assert_eq!(set.spans(), &[strong(0..5)]);

        let mut set = SpanSet::default();
        set.add(0..4, InlineAttr::Code);
        set.apply_op(&op(4, 0, "x"));
        assert_eq!(
            set.spans(),
            &[Span {
                range: 0..4,
                attr: InlineAttr::Code
            }]
        );
    }

    #[test]
    fn typing_at_left_edge_stays_outside() {
        let mut set = SpanSet::default();
        set.add(2..6, InlineAttr::Strong);
        set.apply_op(&op(2, 0, "ab"));
        assert_eq!(set.spans(), &[strong(4..8)]);
    }

    #[test]
    fn typing_inside_grows() {
        let mut set = SpanSet::default();
        set.add(2..6, InlineAttr::Strong);
        set.apply_op(&op(4, 0, "xy"));
        assert_eq!(set.spans(), &[strong(2..8)]);
    }

    #[test]
    fn deletion_clamps_and_drops() {
        // Delete across the left boundary.
        let mut set = SpanSet::default();
        set.add(4..8, InlineAttr::Strong);
        set.apply_op(&op(2, 4, "")); // delete [2,6)
        assert_eq!(set.spans(), &[strong(2..4)]);

        // Delete the entire span: it disappears.
        let mut set = SpanSet::default();
        set.add(4..8, InlineAttr::Strong);
        set.apply_op(&op(3, 6, ""));
        assert!(set.spans().is_empty());
    }

    #[test]
    fn replace_inside_span() {
        // Replacing text inside a span keeps the span around the result.
        let mut set = SpanSet::default();
        set.add(0..10, InlineAttr::Emphasis);
        set.apply_op(&op(3, 4, "xy")); // 10 chars -> 8 chars
        assert_eq!(
            set.spans(),
            &[Span {
                range: 0..8,
                attr: InlineAttr::Emphasis
            }]
        );
    }

    #[test]
    fn add_merges_same_attr() {
        let mut set = SpanSet::default();
        set.add(0..4, InlineAttr::Strong);
        set.add(4..8, InlineAttr::Strong);
        assert_eq!(set.spans(), &[strong(0..8)]);
        // Different attrs never merge.
        set.add(2..6, InlineAttr::Emphasis);
        assert_eq!(set.spans().len(), 2);
    }

    #[test]
    fn remove_splits_straddling_span() {
        let mut set = SpanSet::default();
        set.add(0..10, InlineAttr::Strong);
        set.remove(3..6, &InlineAttr::Strong);
        assert_eq!(set.spans(), &[strong(0..3), strong(6..10)]);
        // Other attrs untouched.
        let mut set = SpanSet::default();
        set.add(0..10, InlineAttr::Strong);
        set.remove(3..6, &InlineAttr::Emphasis);
        assert_eq!(set.spans(), &[strong(0..10)]);
    }

    #[test]
    fn covers_handles_chains_and_gaps() {
        let mut set = SpanSet::default();
        set.add(0..4, InlineAttr::Strong);
        set.add(6..9, InlineAttr::Strong);
        assert!(set.covers(1..3, &InlineAttr::Strong));
        assert!(!set.covers(1..7, &InlineAttr::Strong)); // gap at 4..6
        assert!(!set.covers(1..3, &InlineAttr::Emphasis));
        // Adjacent spans (possible after edits) chain.
        let mut set = SpanSet::default();
        set.add(0..4, InlineAttr::Code);
        set.add(4..8, InlineAttr::Code); // Code merges too via add
        assert!(set.covers(2..6, &InlineAttr::Code));
    }

    #[test]
    fn attrs_at_reports_covering_spans() {
        let mut set = SpanSet::default();
        set.add(0..4, InlineAttr::Strong);
        set.add(2..6, InlineAttr::Emphasis);
        let at3: Vec<_> = set.attrs_at(3).collect();
        assert_eq!(at3.len(), 2);
        assert_eq!(set.attrs_at(5).count(), 1);
        assert_eq!(set.attrs_at(6).count(), 0); // end-exclusive
    }
}
