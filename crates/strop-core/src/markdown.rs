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
    // '=' is escaped because Highlight exports as ==…== (not CommonMark;
    // imported via the mark_highlights pre-pass) — a literal '=' must
    // never be mistaken for a marker, including against a neighboring
    // marker edge ("==a\=\=\=" vs "==a===").
    if matches!(c, '\\' | '*' | '_' | '`' | '[' | ']' | '~' | '=') {
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
        // At end-of-block every marker MUST close even when the span
        // itself continues across the newline (markdown inline formatting
        // cannot cross blocks; the next block reopens it). Leaving it
        // open emitted literal '**'-garbage — found by the model.rs
        // round-trip property, 2026-06-12.
        let here: Vec<InlineAttr> = if i == chars.len() {
            Vec::new()
        } else {
            spans.attrs_at(base + i).cloned().collect()
        };
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
            // A footnote ref's marker replaces its carrier text entirely.
            if !here
                .iter()
                .any(|a| matches!(a, InlineAttr::FootnoteRef(_)))
            {
                escape_into(*c, &mut out);
            }
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

/// Markdown -> document state. Lossy where the schema is deliberately
/// smaller than Markdown (tables, raw HTML beyond `<u>` import as visible
/// literal text — never silently dropped).
/// Replace paired, unescaped `==` highlight markers with `<mark>` tags so
/// the (extension-less) CommonMark parser can carry them — the import half
/// of InlineAttr::Highlight, which pulldown-cmark has no extension for.
/// Pairs are resolved per line (the exporter never spans a marker across
/// lines), skipping fenced code blocks and inline backtick spans; an
/// unpaired marker stays literal text.
fn mark_highlights(md: &str) -> String {
    fn mark_line(line: &str) -> String {
        let chars: Vec<char> = line.chars().collect();
        let mut marks = Vec::new();
        let mut i = 0;
        let mut in_code = false;
        while i < chars.len() {
            match chars[i] {
                '\\' => i += 2, // backslash escape (incl. \=)
                '`' => {
                    in_code = !in_code;
                    i += 1;
                }
                '=' if !in_code && chars.get(i + 1) == Some(&'=') => {
                    marks.push(i);
                    i += 2;
                }
                _ => i += 1,
            }
        }
        let paired = marks.len() / 2 * 2;
        if paired == 0 {
            return line.to_string();
        }
        let mut out = String::new();
        let mut next = 0usize; // index into marks
        let mut i = 0usize;
        while i < chars.len() {
            if next < paired && marks[next] == i {
                out.push_str(if next % 2 == 0 { "<mark>" } else { "</mark>" });
                next += 1;
                i += 2;
            } else {
                out.push(chars[i]);
                i += 1;
            }
        }
        out
    }

    let mut out = String::with_capacity(md.len());
    let mut in_fence = false;
    for (ix, line) in md.split('\n').enumerate() {
        if ix > 0 {
            out.push('\n');
        }
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            out.push_str(line);
        } else if in_fence || line.starts_with("    ") {
            out.push_str(line); // fenced or indented code: raw
        } else {
            out.push_str(&mark_line(line));
        }
    }
    out
}

pub fn from_markdown(md: &str) -> (String, SpanSet, BlockMap) {
    use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

    let md = &mark_highlights(md);
    let mut text = String::new();
    let mut chars = 0usize; // char length of `text`
    let mut spans = SpanSet::default();
    let mut kinds: Vec<BlockKind> = Vec::new();

    let mut quote = 0usize;
    let mut lists: Vec<bool> = Vec::new(); // ordered?
    let mut item_fresh = false; // suppress begin_block for the item's first para
    let mut code_info: Option<String> = None;
    let mut footnote_def: Option<String> = None;
    let mut image_alt: Option<String> = None; // capturing alt text
    let mut inline_starts: Vec<(usize, InlineAttr)> = Vec::new();
    let mut underline_start: Option<usize> = None;
    let mut highlight_start: Option<usize> = None;

    macro_rules! push_str {
        ($s:expr) => {{
            let s: &str = $s;
            text.push_str(s);
            chars += s.chars().count();
        }};
    }

    let begin_block = |text: &mut String,
                           chars: &mut usize,
                           kinds: &mut Vec<BlockKind>,
                           kind: BlockKind| {
        if !kinds.is_empty() {
            text.push('\n');
            *chars += 1;
        }
        kinds.push(kind);
    };

    let current_kind = |quote: usize,
                        lists: &[bool],
                        footnote_def: &Option<String>|
     -> BlockKind {
        if let Some(id) = footnote_def {
            BlockKind::FootnoteDef { id: id.clone() }
        } else if let Some(&ordered) = lists.last() {
            BlockKind::ListItem {
                ordered,
                depth: (lists.len() - 1).min(1) as u8,
            }
        } else if quote > 0 {
            BlockKind::Blockquote
        } else {
            BlockKind::Paragraph
        }
    };

    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_FOOTNOTES;
    for event in Parser::new_ext(md, options) {
        match event {
            Event::Start(Tag::Paragraph) => {
                if item_fresh {
                    item_fresh = false;
                } else {
                    begin_block(
                        &mut text,
                        &mut chars,
                        &mut kinds,
                        current_kind(quote, &lists, &footnote_def),
                    );
                }
            }
            Event::Start(Tag::Heading { level, .. }) => {
                let n = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                begin_block(&mut text, &mut chars, &mut kinds, BlockKind::Heading(n));
            }
            Event::Start(Tag::BlockQuote(_)) => quote += 1,
            Event::End(TagEnd::BlockQuote(_)) => quote = quote.saturating_sub(1),
            Event::Start(Tag::List(start)) => lists.push(start.is_some()),
            Event::End(TagEnd::List(_)) => {
                lists.pop();
            }
            Event::Start(Tag::Item) => {
                begin_block(
                    &mut text,
                    &mut chars,
                    &mut kinds,
                    current_kind(quote, &lists, &footnote_def),
                );
                item_fresh = true;
            }
            Event::End(TagEnd::Item) => item_fresh = false,
            Event::Start(Tag::CodeBlock(kind)) => {
                code_info = Some(match kind {
                    CodeBlockKind::Fenced(info) => info.to_string(),
                    CodeBlockKind::Indented => String::new(),
                });
            }
            Event::End(TagEnd::CodeBlock) => code_info = None,
            Event::Start(Tag::FootnoteDefinition(id)) => {
                footnote_def = Some(id.to_string());
            }
            Event::End(TagEnd::FootnoteDefinition) => footnote_def = None,
            Event::Rule => {
                begin_block(&mut text, &mut chars, &mut kinds, BlockKind::Divider);
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                image_alt = Some(String::new());
                begin_block(
                    &mut text,
                    &mut chars,
                    &mut kinds,
                    BlockKind::Image {
                        src: dest_url.to_string(),
                        alt: String::new(),
                        caption: String::new(),
                    },
                );
            }
            Event::End(TagEnd::Image) => {
                let alt = image_alt.take().unwrap_or_default();
                if let Some(BlockKind::Image { alt: slot, .. }) = kinds.last_mut() {
                    *slot = alt;
                }
            }
            Event::Start(Tag::Emphasis) => inline_starts.push((chars, InlineAttr::Emphasis)),
            Event::Start(Tag::Strong) => inline_starts.push((chars, InlineAttr::Strong)),
            Event::Start(Tag::Strikethrough) => {
                inline_starts.push((chars, InlineAttr::Strikethrough))
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                inline_starts.push((chars, InlineAttr::Link(dest_url.to_string())))
            }
            Event::End(TagEnd::Emphasis)
            | Event::End(TagEnd::Strong)
            | Event::End(TagEnd::Strikethrough)
            | Event::End(TagEnd::Link) => {
                if let Some((start, attr)) = inline_starts.pop() {
                    spans.add(start..chars, attr);
                }
            }
            Event::Code(code) => {
                let start = chars;
                push_str!(code.as_ref());
                spans.add(start..chars, InlineAttr::Code);
            }
            Event::Text(t) => {
                if let Some(alt) = image_alt.as_mut() {
                    alt.push_str(&t);
                } else if let Some(info) = &code_info {
                    // Each code line is its own CodeBlock block.
                    let body = t.strip_suffix('\n').unwrap_or(&t);
                    for (i, line) in body.split('\n').enumerate() {
                        if i > 0 || kinds.last().map(|k| !matches!(k, BlockKind::CodeBlock { .. }))
                            != Some(false)
                        {
                            begin_block(
                                &mut text,
                                &mut chars,
                                &mut kinds,
                                BlockKind::CodeBlock { info: info.clone() },
                            );
                        }
                        push_str!(line);
                    }
                } else {
                    push_str!(t.as_ref());
                }
            }
            Event::Html(h) | Event::InlineHtml(h) => match h.as_ref().trim() {
                "<u>" => underline_start = Some(chars),
                "</u>" => {
                    if let Some(start) = underline_start.take() {
                        spans.add(start..chars, InlineAttr::Underline);
                    }
                }
                // Produced by the mark_highlights pre-pass (==…==).
                "<mark>" => highlight_start = Some(chars),
                "</mark>" => {
                    if let Some(start) = highlight_start.take() {
                        spans.add(start..chars, InlineAttr::Highlight);
                    }
                }
                other => push_str!(other),
            },
            Event::FootnoteReference(id) => {
                // The ref is a span over its carrier digits (the id text).
                let start = chars;
                push_str!(id.as_ref());
                spans.add(start..chars, InlineAttr::FootnoteRef(id.to_string()));
            }
            Event::SoftBreak => push_str!(" "),
            Event::HardBreak => push_str!("\u{2028}"),
            _ => {}
        }
    }

    (text, spans, BlockMap::from_kinds(kinds))
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
    fn import_roundtrips_through_export() {
        let md = "## Заголовок\n\n**Жирный** *курсив* и `код`.\n\n> цитата\n\n\
                  1. первый\n2. второй\n\n```rust\nlet x = 1;\nlet y = 2;\n```\n\n\
                  ***\n\nСсылка на [сайт](https://e.x) и <u>подчёркнутое</u>.\n";
        let (text, spans, blocks) = from_markdown(md);
        assert_eq!(
            blocks.kinds(),
            &[
                BlockKind::Heading(2),
                BlockKind::Paragraph,
                BlockKind::Blockquote,
                BlockKind::ListItem {
                    ordered: true,
                    depth: 0
                },
                BlockKind::ListItem {
                    ordered: true,
                    depth: 0
                },
                BlockKind::CodeBlock {
                    info: "rust".into()
                },
                BlockKind::CodeBlock {
                    info: "rust".into()
                },
                BlockKind::Divider,
                BlockKind::Paragraph,
            ]
        );
        assert!(text.contains("Жирный курсив и код."));
        assert!(spans.covers(10..16, &InlineAttr::Strong));
        assert!(spans.covers(17..23, &InlineAttr::Emphasis));
        // Full circle: export of the import matches the original.
        assert_eq!(to_markdown(&text, &spans, &blocks), md.replace("                  ", ""));
    }

    #[test]
    fn import_handles_soft_and_hard_breaks() {
        let (text, _, blocks) = from_markdown("строка раз\nстрока два\\\nстрока три\n");
        // Soft wrap joins with a space; hard break becomes U+2028.
        assert_eq!(text, "строка раз строка два\u{2028}строка три");
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn footnotes_roundtrip() {
        let md = "Текст[^1] продолжается.\n\n[^1]: Сама сноска.\n";
        let (text, spans, blocks) = from_markdown(md);
        assert_eq!(text, "Текст1 продолжается.\nСама сноска.");
        assert!(spans.covers(5..6, &InlineAttr::FootnoteRef("1".into())));
        assert_eq!(
            blocks.kinds()[1],
            BlockKind::FootnoteDef { id: "1".into() }
        );
        assert_eq!(to_markdown(&text, &spans, &blocks), md);
    }

    #[test]
    fn escapes_markdown_specials() {
        let text = "a*b_c`d[e]";
        let md = to_markdown(text, &SpanSet::default(), &BlockMap::new(1));
        assert_eq!(md, "a\\*b\\_c\\`d\\[e\\]\n");
    }

    #[test]
    fn highlight_roundtrips() {
        // The bug the model.rs property found (2026-06-12): Highlight
        // exported as ==…== but import had no parser for it — exported
        // highlights silently became literal equals signs.
        let mut spans = SpanSet::default();
        spans.add(2..7, InlineAttr::Highlight);
        let text = "до выделено после";
        let md = to_markdown(text, &spans, &BlockMap::new(1));
        assert!(md.contains("=="), "highlight must export as ==…==: {md:?}");
        let (text2, spans2, _) = from_markdown(&md);
        assert_eq!(text2, text);
        assert!(spans2.covers(2..7, &InlineAttr::Highlight));
        assert!(!spans2.covers(1..2, &InlineAttr::Highlight));
        assert!(!spans2.covers(7..8, &InlineAttr::Highlight));
    }

    #[test]
    fn literal_equals_signs_survive() {
        // "==" in prose must not be eaten as a highlight marker.
        let text = "a == b == c";
        let md = to_markdown(text, &SpanSet::default(), &BlockMap::new(1));
        let (text2, spans2, _) = from_markdown(&md);
        assert_eq!(text2, text);
        assert!(spans2.spans().is_empty());
    }

    #[test]
    fn cross_block_span_closes_at_block_end() {
        // Bold across a paragraph break (selection over Enter): each
        // block must carry balanced markers; the second reopens. The
        // unclosed-marker export was found by the model.rs property.
        let mut spans = SpanSet::default();
        spans.add(1..5, InlineAttr::Strong); // "b\ncd" incl. the newline
        let text = "ab\ncde";
        let md = to_markdown(text, &spans, &BlockMap::new(2));
        assert_eq!(md, "a**b**\n\n**cd**e\n");
        let (text2, spans2, _) = from_markdown(&md);
        assert_eq!(text2, text);
        assert!(spans2.covers(1..2, &InlineAttr::Strong));
        assert!(spans2.covers(3..5, &InlineAttr::Strong));
    }

    #[test]
    fn highlight_beside_literal_equals() {
        // Highlighted text whose content touches '=' itself.
        let mut spans = SpanSet::default();
        spans.add(0..2, InlineAttr::Highlight);
        let text = "a= rest";
        let md = to_markdown(text, &spans, &BlockMap::new(1));
        let (text2, spans2, _) = from_markdown(&md);
        assert_eq!(text2, text);
        assert!(spans2.covers(0..2, &InlineAttr::Highlight));
    }
}
