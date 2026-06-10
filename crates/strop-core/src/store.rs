//! The durable layer: a Loro document mirroring the hot-path buffer.
//!
//! Architecture rule (docs/DECISIONS.md D3): Loro is never on the keystroke
//! path. The rope edits first; committed ops are mirrored here, and the
//! `.strop` file is a Loro snapshot — full edit history plus current state.
//! That history is what later buys checkpoints, time travel, the author's
//! own voice corpus, and (eventually) sync.

use std::fs;
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};

use loro::{ExpandType, ExportMode, LoroDoc, LoroValue, StyleConfig, StyleConfigMap, TextDelta};

use crate::buffer::TextOp;
use crate::document::{BlockKind, BlockMap, InlineAttr, SpanSet};

const TEXT_CONTAINER: &str = "content";
const BLOCKS_CONTAINER: &str = "blocks";

/// One token per block, newline-joined (block text never contains '\n').
/// Wholesale-rebuilt at save, like marks.
fn kind_token(kind: &BlockKind) -> String {
    match kind {
        BlockKind::Paragraph => "p".into(),
        BlockKind::Heading(n) => format!("h{n}"),
        BlockKind::Blockquote => "quote".into(),
        BlockKind::ListItem { ordered, depth } => {
            format!("li:{}:{depth}", if *ordered { "o" } else { "b" })
        }
        BlockKind::Divider => "div".into(),
        BlockKind::CodeBlock { info } => format!("code:{info}"),
        BlockKind::FootnoteDef { id } => format!("fn:{id}"),
        // Image asset plumbing lands with B3; src survives the round trip.
        BlockKind::Image { src, .. } => format!("img:{src}"),
    }
}

fn kind_from_token(token: &str) -> BlockKind {
    match token {
        "p" => BlockKind::Paragraph,
        "quote" => BlockKind::Blockquote,
        "div" => BlockKind::Divider,
        t if t.starts_with('h') => t[1..]
            .parse::<u8>()
            .map(BlockKind::Heading)
            .unwrap_or_default(),
        t if t.starts_with("li:") => {
            let mut parts = t.splitn(3, ':').skip(1);
            let ordered = parts.next() == Some("o");
            let depth = parts.next().and_then(|d| d.parse().ok()).unwrap_or(0);
            BlockKind::ListItem { ordered, depth }
        }
        t if t.starts_with("code:") => BlockKind::CodeBlock {
            info: t[5..].into(),
        },
        t if t.starts_with("fn:") => BlockKind::FootnoteDef { id: t[3..].into() },
        t if t.starts_with("img:") => BlockKind::Image {
            src: t[4..].into(),
            alt: String::new(),
            caption: String::new(),
        },
        _ => BlockKind::Paragraph,
    }
}

/// All style keys we ever write; unmarked wholesale before each rebuild.
const STYLE_KEYS: [&str; 8] = [
    "strong",
    "emphasis",
    "underline",
    "strikethrough",
    "highlight",
    "code",
    "link",
    "footnote",
];

fn attr_key(attr: &InlineAttr) -> &'static str {
    match attr {
        InlineAttr::Strong => "strong",
        InlineAttr::Emphasis => "emphasis",
        InlineAttr::Underline => "underline",
        InlineAttr::Strikethrough => "strikethrough",
        InlineAttr::Highlight => "highlight",
        InlineAttr::Code => "code",
        InlineAttr::Link(_) => "link",
        InlineAttr::FootnoteRef(_) => "footnote",
    }
}

fn attr_value(attr: &InlineAttr) -> LoroValue {
    match attr {
        InlineAttr::Link(href) => LoroValue::String(href.clone().into()),
        InlineAttr::FootnoteRef(id) => LoroValue::String(id.clone().into()),
        _ => LoroValue::Bool(true),
    }
}

fn attr_from(key: &str, value: &LoroValue) -> Option<InlineAttr> {
    if matches!(value, LoroValue::Null | LoroValue::Bool(false)) {
        return None;
    }
    let string = || match value {
        LoroValue::String(s) => s.to_string(),
        _ => String::new(),
    };
    match key {
        "strong" => Some(InlineAttr::Strong),
        "emphasis" => Some(InlineAttr::Emphasis),
        "underline" => Some(InlineAttr::Underline),
        "strikethrough" => Some(InlineAttr::Strikethrough),
        "highlight" => Some(InlineAttr::Highlight),
        "code" => Some(InlineAttr::Code),
        "link" => Some(InlineAttr::Link(string())),
        "footnote" => Some(InlineAttr::FootnoteRef(string())),
        _ => None,
    }
}

/// Expand config mirroring `InlineAttr::expands` — kept in lockstep so any
/// live Loro state between save-time rebuilds behaves like the SpanSet.
fn style_config() -> StyleConfigMap {
    let mut map = StyleConfigMap::new();
    for key in STYLE_KEYS {
        let expand = match key {
            "code" | "link" | "footnote" => ExpandType::None,
            _ => ExpandType::After,
        };
        map.insert(key.into(), StyleConfig { expand });
    }
    map
}

pub struct Store {
    doc: LoroDoc,
    path: PathBuf,
}

impl Store {
    /// Open a `.strop` file. Returns the store and, when the file already
    /// existed, its text, formatting, and block kinds (None = brand-new).
    pub fn open(
        path: impl Into<PathBuf>,
    ) -> io::Result<(Self, Option<(String, SpanSet, BlockMap)>)> {
        let path = path.into();
        let doc = LoroDoc::new();
        doc.config_text_style(style_config());
        let existing = match fs::read(&path) {
            Ok(bytes) => {
                doc.import(&bytes).map_err(io::Error::other)?;
                let text = doc.get_text(TEXT_CONTAINER);
                let mut spans = SpanSet::default();
                let mut pos = 0usize;
                for delta in text.to_delta() {
                    if let TextDelta::Insert { insert, attributes } = delta {
                        let len = insert.chars().count();
                        for (key, value) in attributes.iter().flatten() {
                            if let Some(attr) = attr_from(key, value) {
                                spans.add(pos..pos + len, attr);
                            }
                        }
                        pos += len;
                    }
                }
                let blocks = match doc.get_map(BLOCKS_CONTAINER).get("kinds") {
                    Some(v) => match v.into_value() {
                        Ok(LoroValue::String(tokens)) => BlockMap::from_kinds(
                            tokens.lines().map(kind_from_token).collect(),
                        ),
                        _ => BlockMap::default(),
                    },
                    None => BlockMap::default(),
                };
                Some((text.to_string(), spans, blocks))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };
        Ok((Self { doc, path }, existing))
    }

    /// Rebuild Peritext marks + block kinds from the authoritative state,
    /// then snapshot. Durability only matters at the disk boundary, so
    /// neither is mirrored per-edit — this avoids expand-rule drift.
    pub fn save_with_state(&self, spans: &SpanSet, blocks: &BlockMap) -> io::Result<()> {
        self.rebuild_marks(spans);
        let tokens: Vec<String> = blocks.kinds().iter().map(kind_token).collect();
        if let Err(e) = self
            .doc
            .get_map(BLOCKS_CONTAINER)
            .insert("kinds", tokens.join("\n"))
        {
            eprintln!("strop: persist blocks: {e}");
        }
        self.doc.commit();
        self.save()
    }

    fn rebuild_marks(&self, spans: &SpanSet) {
        let text = self.doc.get_text(TEXT_CONTAINER);
        let len = text.len_unicode();
        if len == 0 {
            return;
        }
        for key in STYLE_KEYS {
            if let Err(e) = text.unmark(0..len, key) {
                eprintln!("strop: unmark {key}: {e}");
            }
        }
        for span in spans.spans() {
            let range: Range<usize> = span.range.start..span.range.end.min(len);
            if range.start >= range.end {
                continue;
            }
            if let Err(e) = text.mark(range, attr_key(&span.attr), attr_value(&span.attr)) {
                eprintln!("strop: mark {}: {e}", attr_key(&span.attr));
            }
        }
        self.doc.commit();
    }

    /// Seed a freshly created document with initial text.
    pub fn seed(&self, text: &str) {
        if !text.is_empty() {
            self.doc
                .get_text(TEXT_CONTAINER)
                .insert(0, text)
                .expect("seeding an empty Loro text cannot fail");
            self.doc.commit();
        }
    }

    /// Mirror buffer ops, in application order. Positions are char-indexed
    /// on both sides (ropey chars == Loro unicode positions); a mismatch is
    /// a programming error and panics loudly.
    pub fn apply(&self, ops: &[TextOp]) {
        let text = self.doc.get_text(TEXT_CONTAINER);
        for op in ops {
            if op.delete > 0 {
                text.delete(op.pos, op.delete).expect("mirrored delete");
            }
            if !op.insert.is_empty() {
                text.insert(op.pos, &op.insert).expect("mirrored insert");
            }
        }
        self.doc.commit();
    }

    pub fn text(&self) -> String {
        self.doc.get_text(TEXT_CONTAINER).to_string()
    }

    /// Atomic snapshot save: full history + state, temp file + rename.
    pub fn save(&self) -> io::Result<()> {
        let bytes = self
            .doc
            .export(ExportMode::Snapshot)
            .map_err(io::Error::other)?;
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("strop.tmp");
        fs::write(&tmp, &bytes)?;
        fs::rename(&tmp, &self.path)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Buffer;

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("strop-test-{}-{tag}.strop", std::process::id()))
    }

    #[test]
    fn mirrors_buffer_and_roundtrips() {
        let path = temp_path("roundtrip");
        let _ = fs::remove_file(&path);

        let (store, existing) = Store::open(&path).unwrap();
        assert!(existing.is_none());
        store.seed("привет");

        let mut buf = Buffer::new("привет");
        buf.take_ops(); // initial text is seeded, not mirrored
        buf.edit(6..6, ", мир");
        buf.edit(0..1, "П");
        buf.undo(); // undo is mirrored as an ordinary op
        store.apply(&buf.take_ops());
        assert_eq!(store.text(), buf.text());
        assert_eq!(buf.text(), "привет, мир");

        store.save().unwrap();
        let (_store2, existing) = Store::open(&path).unwrap();
        assert_eq!(
            existing.map(|(text, _, _)| text).as_deref(),
            Some("привет, мир")
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn edit_history_survives_reopen() {
        // The CRDT contract: ExportMode::Snapshot carries the full op log,
        // so keystroke-level history accumulates across sessions.
        let path = temp_path("history");
        let _ = fs::remove_file(&path);

        let (store, _) = Store::open(&path).unwrap();
        store.seed("ab");
        store.apply(&[TextOp {
            pos: 2,
            delete: 0,
            insert: "c".into(),
        }]);
        store.save().unwrap();

        // Second session: history is present, and grows further.
        let (store2, existing) = Store::open(&path).unwrap();
        assert_eq!(existing.as_ref().unwrap().0, "abc");
        let ops_after_first = store2.doc.len_ops();
        assert!(ops_after_first >= 2, "history lost on reopen");
        store2.apply(&[TextOp {
            pos: 3,
            delete: 0,
            insert: "d".into(),
        }]);
        store2.save().unwrap();

        // Third session: both sessions' ops are in the file.
        let (store3, existing) = Store::open(&path).unwrap();
        assert_eq!(existing.unwrap().0, "abcd");
        assert!(store3.doc.len_ops() > ops_after_first);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn marks_roundtrip() {
        let path = temp_path("marks");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();
        store.seed("жирный и код");

        let mut spans = SpanSet::default();
        spans.add(0..6, InlineAttr::Strong);
        spans.add(9..12, InlineAttr::Code);
        spans.add(2..4, InlineAttr::Link("https://e.x".into()));
        store.save_with_state(&spans, &BlockMap::new(1)).unwrap();

        let (_s2, existing) = Store::open(&path).unwrap();
        let (text, loaded, _blocks) = existing.unwrap();
        assert_eq!(text, "жирный и код");
        assert!(loaded.covers(0..6, &InlineAttr::Strong));
        assert!(loaded.covers(9..12, &InlineAttr::Code));
        assert!(loaded.covers(2..4, &InlineAttr::Link("https://e.x".into())));
        assert!(!loaded.covers(6..9, &InlineAttr::Strong));

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn typing_and_substitutions_mirror_exactly() {
        let path = temp_path("typograph");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();

        let mut buf = Buffer::new("");
        for (i, ch) in "so...".chars().enumerate() {
            buf.edit_bytes_coalescing(i..i, &ch.to_string());
        }
        buf.edit_bytes(2..5, "…"); // typograph substitution
        store.apply(&buf.take_ops());
        assert_eq!(store.text(), "so…");
        assert_eq!(store.text(), buf.text());

        let _ = fs::remove_file(&path);
    }
}
