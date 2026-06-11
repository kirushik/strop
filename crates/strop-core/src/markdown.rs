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

/// Markdown -> document state. Lossy where the schema is deliberately
/// smaller than Markdown (tables, raw HTML beyond `<u>` import as visible
/// literal text — never silently dropped). Footnote *references* import as
/// literal `[^id]` until B2 gives them an atom representation.
pub fn from_markdown(md: &str) -> (String, SpanSet, BlockMap) {
    use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

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
                other => push_str!(other),
            },
            Event::FootnoteReference(id) => {
                push_str!(&format!("[^{id}]"));
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
    fn escapes_markdown_specials() {
        let text = "a*b_c`d[e]";
        let md = to_markdown(text, &SpanSet::default(), &BlockMap::new(1));
        assert_eq!(md, "a\\*b\\_c\\`d\\[e\\]\n");
    }
}
