//! Markdown boundary (docs/document-model.md §6). Export is a total
//! function over the schema; import lands with B1's second half.

use crate::document::{BlockKind, BlockMap, InlineAttr, SpanSet};

fn open_marker(attr: &InlineAttr) -> String {
    match attr {
        InlineAttr::Strong => "**".into(),
        InlineAttr::Emphasis => "*".into(),
        InlineAttr::Strikethrough => "~~".into(),
        InlineAttr::Underline => "<u>".into(),
        InlineAttr::Highlight => "==".into(),
        InlineAttr::Code => "`".into(),
        InlineAttr::Link(_) => "[".into(),
        InlineAttr::FootnoteRef(id) => format!("[^{id}]"),
    }
}

fn close_marker(attr: &InlineAttr) -> String {
    match attr {
        InlineAttr::Strong => "**".into(),
        InlineAttr::Emphasis => "*".into(),
        InlineAttr::Strikethrough => "~~".into(),
        InlineAttr::Underline => "</u>".into(),
        InlineAttr::Highlight => "==".into(),
        InlineAttr::Code => "`".into(),
        InlineAttr::Link(href) => format!("]({href})"),
        InlineAttr::FootnoteRef(_) => String::new(),
    }
}

fn escape_into(c: char, out: &mut String) {
    if matches!(c, '\\' | '*' | '_' | '`' | '[' | ']' | '~') {
        out.push('\\');
    }
    if c == '\u{2028}' {
        out.push_str("\\\n"); // soft break -> trailing backslash
        return;
    }
    out.push(c);
}

/// Inline spans of one block to Markdown, with proper nesting (overlapping
/// spans are re-opened as needed — Markdown cannot express true overlap).
fn inline_md(line: &str, base: usize, spans: &SpanSet) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut stack: Vec<InlineAttr> = Vec::new();
    for i in 0..=chars.len() {
        let here: Vec<InlineAttr> = spans.attrs_at(base + i).cloned().collect();
        // Close everything above (and including) any attr that ended here,
        // remembering still-active ones to reopen.
        let mut reopen = Vec::new();
        while stack.iter().any(|a| !here.contains(a)) {
            let top = stack.pop().unwrap();
            out.push_str(&close_marker(&top));
            if here.contains(&top) {
                reopen.push(top);
            }
        }
        for attr in reopen.into_iter().rev() {
            out.push_str(&open_marker(&attr));
            stack.push(attr);
        }
        for attr in &here {
            if !stack.contains(attr) {
                out.push_str(&open_marker(attr));
                stack.push(attr.clone());
            }
        }
        if let Some(c) = chars.get(i) {
            escape_into(*c, &mut out);
        }
    }
    out
}

pub fn to_markdown(text: &str, spans: &SpanSet, blocks: &BlockMap) -> String {
    let mut out = String::new();
    let mut base = 0usize; // char offset of the current block
    let mut ordered_no = 0usize;
    let mut in_code = false;
    let lines: Vec<&str> = text.split('\n').collect();
    for (ix, line) in lines.iter().enumerate() {
        let kind = blocks.kind(ix).clone();
        let is_code = matches!(kind, BlockKind::CodeBlock { .. });
        if in_code && !is_code {
            out.push_str("```\n\n");
            in_code = false;
        }
        if !matches!(kind, BlockKind::ListItem { ordered: true, .. }) {
            ordered_no = 0;
        }
        match &kind {
            BlockKind::CodeBlock { info } => {
                if !in_code {
                    out.push_str("```");
                    out.push_str(info);
                    out.push('\n');
                    in_code = true;
                }
                out.push_str(line); // raw — no inline processing in code
                out.push('\n');
            }
            BlockKind::Divider => out.push_str("***\n\n"),
            BlockKind::Heading(n) => {
                for _ in 0..*n {
                    out.push('#');
                }
                out.push(' ');
                out.push_str(&inline_md(line, base, spans));
                out.push_str("\n\n");
            }
            BlockKind::Blockquote => {
                out.push_str("> ");
                out.push_str(&inline_md(line, base, spans));
                // Consecutive quote blocks stay one quote.
                if matches!(blocks.kind(ix + 1), BlockKind::Blockquote) && ix + 1 < lines.len() {
                    out.push_str("\n>\n");
                } else {
                    out.push_str("\n\n");
                }
            }
            BlockKind::ListItem { ordered, .. } => {
                if *ordered {
                    ordered_no += 1;
                    out.push_str(&format!("{ordered_no}. "));
                } else {
                    out.push_str("- ");
                }
                out.push_str(&inline_md(line, base, spans));
                out.push('\n');
                let next_is_item =
                    ix + 1 < lines.len() && matches!(blocks.kind(ix + 1), BlockKind::ListItem { .. });
                if !next_is_item {
                    out.push('\n');
                }
            }
            BlockKind::FootnoteDef { id } => {
                out.push_str(&format!("[^{id}]: "));
                out.push_str(&inline_md(line, base, spans));
                out.push_str("\n\n");
            }
            BlockKind::Image { src, alt, .. } => {
                out.push_str(&format!("![{alt}]({src})\n\n"));
            }
            BlockKind::Paragraph => {
                out.push_str(&inline_md(line, base, spans));
                out.push_str("\n\n");
            }
        }
        base += line.chars().count() + 1;
    }
    if in_code {
        out.push_str("```\n");
    }
    let trimmed = out.trim_end();
    format!("{trimmed}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_blocks_and_inline_formatting() {
        let text = "Заголовок\nЖирный курсив и код.\nцитата\nпервый\nвторой\nlet x = 1;\nlet y = 2;";
        let mut spans = SpanSet::default();
        spans.add(10..16, InlineAttr::Strong); // "Жирный"
        spans.add(17..23, InlineAttr::Emphasis); // "курсив"
        spans.add(26..29, InlineAttr::Code); // "код"
        let blocks = BlockMap::from_kinds(vec![
            BlockKind::Heading(2),
            BlockKind::Paragraph,
            BlockKind::Blockquote,
            BlockKind::ListItem {
                ordered: true,
                depth: 0,
            },
            BlockKind::ListItem {
                ordered: true,
                depth: 0,
            },
            BlockKind::CodeBlock {
                info: "rust".into(),
            },
            BlockKind::CodeBlock {
                info: "rust".into(),
            },
        ]);
        let md = to_markdown(text, &spans, &blocks);
        assert_eq!(
            md,
            "## Заголовок\n\n**Жирный** *курсив* и `код`.\n\n> цитата\n\n\
             1. первый\n2. второй\n\n```rust\nlet x = 1;\nlet y = 2;\n```\n"
        );
    }

    #[test]
    fn escapes_markdown_specials() {
        let text = "a*b_c`d[e]";
        let md = to_markdown(text, &SpanSet::default(), &BlockMap::new(1));
        assert_eq!(md, "a\\*b\\_c\\`d\\[e\\]\n");
    }
}
