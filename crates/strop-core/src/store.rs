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

use loro::{
    ExpandType, ExportMode, Frontiers, LoroDoc, LoroValue, StyleConfig, StyleConfigMap, TextDelta,
};
use serde::{Deserialize, Serialize};

use crate::buffer::TextOp;
use crate::document::{Annotations, BlockKind, BlockMap, History, InlineAttr, SpanSet};

const TEXT_CONTAINER: &str = "content";
const BLOCKS_CONTAINER: &str = "blocks";
const SESSION_CONTAINER: &str = "session";
const ANNOTATIONS_CONTAINER: &str = "annotations";
const CHECKPOINTS_CONTAINER: &str = "checkpoints";
const ASSETS_CONTAINER: &str = "assets";

/// Everything a reopened document restores.
pub struct Loaded {
    pub text: String,
    pub spans: SpanSet,
    pub blocks: BlockMap,
    pub history: Option<History>,
    pub annotations: Annotations,
}

/// A named version snapshot: a Loro frontier (version vector position) the
/// document can be rewound to. Lives inside the .strop file — Google-Docs
/// version history, local-first and self-contained.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub name: String,
    pub created_unix: i64,
    pub frontiers: Vec<u8>,
    /// Named-by-the-author (vs automatic session markers).
    #[serde(default)]
    pub manual: bool,
}

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
    /// Rename/move the on-disk file; subsequent saves follow. Refuses to
    /// overwrite — renaming is never allowed to destroy another document.
    pub fn rename_file(&mut self, new_path: impl Into<PathBuf>) -> io::Result<()> {
        let new_path = new_path.into();
        if new_path == self.path {
            return Ok(());
        }
        if new_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} already exists", new_path.display()),
            ));
        }
        if let Some(dir) = new_path.parent() {
            fs::create_dir_all(dir)?;
        }
        // The file may not exist yet (brand-new doc before first save) —
        // that's fine, the path is still just where saves will land.
        match fs::rename(&self.path, &new_path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        self.path = new_path;
        Ok(())
    }

    /// Open a `.strop` file. Returns the store and, when the file already
    /// existed, its text, formatting, and block kinds (None = brand-new).
    pub fn open(path: impl Into<PathBuf>) -> io::Result<(Self, Option<Loaded>)> {
        let path = path.into();
        let doc = LoroDoc::new();
        doc.config_text_style(style_config());
        match fs::read(&path) {
            Ok(bytes) => {
                doc.import(&bytes).map_err(io::Error::other)?;
                let store = Self { doc, path };
                let (text, spans, blocks) = store.read_state();
                let history = match store.doc.get_map(SESSION_CONTAINER).get("history") {
                    Some(v) => match v.into_value() {
                        Ok(LoroValue::String(json)) => serde_json::from_str(&json).ok(),
                        _ => None,
                    },
                    None => None,
                };
                let annotations = match store.doc.get_map(ANNOTATIONS_CONTAINER).get("list") {
                    Some(v) => match v.into_value() {
                        Ok(LoroValue::String(json)) => {
                            serde_json::from_str(&json).unwrap_or_default()
                        }
                        _ => Annotations::default(),
                    },
                    None => Annotations::default(),
                };
                Ok((
                    store,
                    Some(Loaded {
                        text,
                        spans,
                        blocks,
                        history,
                        annotations,
                    }),
                ))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok((Self { doc, path }, None)),
            Err(e) => Err(e),
        }
    }

    /// Text + formatting + block kinds at the doc's *current* version
    /// (which `state_at` temporarily moves).
    fn read_state(&self) -> (String, SpanSet, BlockMap) {
        let text = self.doc.get_text(TEXT_CONTAINER);
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
        let blocks = match self.doc.get_map(BLOCKS_CONTAINER).get("kinds") {
            Some(v) => match v.into_value() {
                Ok(LoroValue::String(tokens)) => {
                    BlockMap::from_kinds(tokens.lines().map(kind_from_token).collect())
                }
                _ => BlockMap::default(),
            },
            None => BlockMap::default(),
        };
        (text.to_string(), spans, blocks)
    }

    /// Store an image asset in-file; returns its id for Image{src}.
    /// Content-addressed (blake3) — identical pastes dedupe; the document
    /// stays a single portable file.
    pub fn put_asset(&self, bytes: Vec<u8>, ext: &str) -> String {
        let id = format!("asset:{}.{ext}", blake3::hash(&bytes).to_hex());
        let assets = self.doc.get_map(ASSETS_CONTAINER);
        if assets.get(&id).is_none() {
            if let Err(e) = assets.insert(&id, bytes) {
                eprintln!("strop: store asset: {e}");
            }
            self.doc.commit();
        }
        id
    }

    pub fn get_asset(&self, id: &str) -> Option<Vec<u8>> {
        match self.doc.get_map(ASSETS_CONTAINER).get(id)?.into_value() {
            Ok(LoroValue::Binary(b)) => Some(b.to_vec()),
            _ => None,
        }
    }

    /// Record a named checkpoint at the current version.
    pub fn add_checkpoint(&self, name: &str, manual: bool) {
        self.doc.commit();
        let checkpoint = Checkpoint {
            name: name.to_owned(),
            created_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
            frontiers: self.doc.oplog_frontiers().encode(),
            manual,
        };
        match serde_json::to_string(&checkpoint) {
            Ok(json) => {
                let list = self.doc.get_list(CHECKPOINTS_CONTAINER);
                if let Err(e) = list.push(json) {
                    eprintln!("strop: record checkpoint: {e}");
                }
                self.doc.commit();
            }
            Err(e) => eprintln!("strop: encode checkpoint: {e}"),
        }
    }

    /// Rename a checkpoint; renaming an automatic entry makes it named
    /// (manual), per the rewind research.
    pub fn rename_checkpoint(&self, ix: usize, name: &str) {
        let list = self.doc.get_list(CHECKPOINTS_CONTAINER);
        let Some(v) = list.get(ix) else { return };
        let Ok(LoroValue::String(json)) = v.into_value() else {
            return;
        };
        let Ok(mut cp) = serde_json::from_str::<Checkpoint>(&json) else {
            return;
        };
        cp.name = name.to_owned();
        cp.manual = true;
        match serde_json::to_string(&cp) {
            Ok(json) => {
                let _ = list.delete(ix, 1);
                if let Err(e) = list.insert(ix, json) {
                    eprintln!("strop: rename checkpoint: {e}");
                }
                self.doc.commit();
            }
            Err(e) => eprintln!("strop: encode checkpoint: {e}"),
        }
    }

    /// Add a checkpoint only if the document state actually differs from
    /// the most recent checkpoint — empty sessions never clutter the rail
    /// (the research's top Docs complaint).
    pub fn add_checkpoint_if_changed(&self, name: &str, manual: bool) {
        if let Some(last) = self.checkpoints().last()
            && let Some(at_last) = self.state_at(&last.frontiers)
        {
            // Full (text, spans, blocks) comparison: a session that only
            // bolded or restructured headings still deserves a rewind
            // point — text-only comparison made such work unreachable.
            if at_last == self.read_state() {
                return;
            }
        }
        self.add_checkpoint(name, manual);
    }

    pub fn checkpoints(&self) -> Vec<Checkpoint> {
        let list = self.doc.get_list(CHECKPOINTS_CONTAINER);
        (0..list.len())
            .filter_map(|i| list.get(i))
            .filter_map(|v| match v.into_value() {
                Ok(LoroValue::String(json)) => serde_json::from_str(&json).ok(),
                _ => None,
            })
            .collect()
    }

    /// Document state as of a checkpoint: time-travel there, read, return
    /// to the present. Restoring is the caller's ordinary (undoable) edit —
    /// history is append-only, never rewritten.
    pub fn state_at(&self, frontiers: &[u8]) -> Option<(String, SpanSet, BlockMap)> {
        let frontiers = Frontiers::decode(frontiers).ok()?;
        self.doc.checkout(&frontiers).ok()?;
        let state = self.read_state();
        self.doc.checkout_to_latest();
        Some(state)
    }

    /// Rebuild Peritext marks + block kinds from the authoritative state,
    /// then snapshot. Durability only matters at the disk boundary, so
    /// neither is mirrored per-edit — this avoids expand-rule drift.
    pub fn save_with_state(
        &self,
        spans: &SpanSet,
        blocks: &BlockMap,
        history: &History,
        annotations: &Annotations,
    ) -> io::Result<()> {
        match serde_json::to_string(annotations) {
            Ok(json) => {
                if let Err(e) = self.doc.get_map(ANNOTATIONS_CONTAINER).insert("list", json) {
                    eprintln!("strop: persist annotations: {e}");
                }
            }
            Err(e) => eprintln!("strop: encode annotations: {e}"),
        }
        self.rebuild_marks(spans);
        let tokens: Vec<String> = blocks.kinds().iter().map(kind_token).collect();
        if let Err(e) = self
            .doc
            .get_map(BLOCKS_CONTAINER)
            .insert("kinds", tokens.join("\n"))
        {
            eprintln!("strop: persist blocks: {e}");
        }
        match serde_json::to_string(history) {
            Ok(json) => {
                if let Err(e) = self.doc.get_map(SESSION_CONTAINER).insert("history", json) {
                    eprintln!("strop: persist history: {e}");
                }
            }
            Err(e) => eprintln!("strop: encode history: {e}"),
        }
        self.collect_unreachable_assets(blocks, history);
        self.doc.commit();
        self.save()
    }

    /// Save-time asset GC: an asset survives if the current document, any
    /// persisted undo/redo state, or any checkpoint's document still
    /// references it. (Deleting an image block orphans its bytes only once
    /// every survivor path has rotated away.)
    fn collect_unreachable_assets(&self, blocks: &BlockMap, history: &History) {
        let assets = self.doc.get_map(ASSETS_CONTAINER);
        if assets.is_empty() {
            return;
        }
        let mut reachable: std::collections::HashSet<String> = blocks
            .asset_refs()
            .chain(history.asset_refs())
            .map(str::to_owned)
            .collect();
        for cp in self.checkpoints() {
            if let Some((_, _, cp_blocks)) = self.state_at(&cp.frontiers) {
                reachable.extend(cp_blocks.asset_refs().map(str::to_owned));
            }
        }
        let stored: Vec<String> = assets.keys().map(|k| k.to_string()).collect();
        for id in stored {
            if !reachable.contains(&id) {
                if let Err(e) = assets.delete(&id) {
                    eprintln!("strop: gc asset {id}: {e}");
                } else {
                    eprintln!("strop: gc'd unreferenced asset {id}");
                }
            }
        }
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

    /// Write a snapshot copy (full history) to another path; the open
    /// document keeps saving to its own.
    pub fn save_copy_to(&self, path: &Path) -> io::Result<()> {
        let bytes = self
            .doc
            .export(ExportMode::Snapshot)
            .map_err(io::Error::other)?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, bytes)
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
            existing.map(|l| l.text).as_deref(),
            Some("привет, мир")
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn formatting_only_change_still_seals_a_checkpoint() {
        let path = temp_path("fmt-checkpoint");
        let _ = fs::remove_file(&path);

        let (store, _) = Store::open(&path).unwrap();
        store.seed("слово и слово");
        store.add_checkpoint("base", false);

        // Identical call with nothing changed: skipped.
        store.add_checkpoint_if_changed("noop", false);
        assert_eq!(store.checkpoints().len(), 1);

        // Bold a word without touching the text.
        let mut spans = SpanSet::default();
        spans.add(0..5, crate::document::InlineAttr::Strong);
        store
            .save_with_state(
                &spans,
                &BlockMap::from_kinds(vec![crate::document::BlockKind::Paragraph]),
                &History::default(),
                &Annotations::default(),
            )
            .unwrap();
        store.add_checkpoint_if_changed("bolded", false);
        assert_eq!(
            store.checkpoints().len(),
            2,
            "formatting-only work must be reachable by rewind"
        );

        // And again with no further change: skipped.
        store.add_checkpoint_if_changed("noop2", false);
        assert_eq!(store.checkpoints().len(), 2);

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
        assert_eq!(existing.as_ref().unwrap().text, "abc");
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
        assert_eq!(existing.unwrap().text, "abcd");
        assert!(store3.doc.len_ops() > ops_after_first);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn assets_roundtrip_and_dedupe() {
        let path = temp_path("assets");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();
        let bytes = vec![1u8, 2, 3, 4, 5];
        let id = store.put_asset(bytes.clone(), "png");
        let id2 = store.put_asset(bytes.clone(), "png");
        assert_eq!(id, id2); // dedupe by content
        assert!(id.ends_with(".png"));
        store.save().unwrap();
        let (store2, _) = Store::open(&path).unwrap();
        assert_eq!(store2.get_asset(&id), Some(bytes));
        assert_eq!(store2.get_asset("asset:missing"), None);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn asset_gc_keeps_reachable_drops_orphans() {
        use crate::document::BlockKind;
        let path = temp_path("gc");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();
        store.seed("картинка\n");
        let kept_id = store.put_asset(vec![1, 2, 3], "png");
        let orphan_id = store.put_asset(vec![9, 9, 9], "png");
        let blocks = BlockMap::from_kinds(vec![
            BlockKind::Image {
                src: kept_id.clone(),
                alt: String::new(),
                caption: String::new(),
            },
            BlockKind::Paragraph,
        ]);
        store
            .save_with_state(
                &SpanSet::default(),
                &blocks,
                &History::default(),
                &Annotations::default(),
            )
            .unwrap();
        assert!(store.get_asset(&kept_id).is_some(), "referenced asset kept");
        assert!(store.get_asset(&orphan_id).is_none(), "orphan collected");
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
        store
            .save_with_state(&spans, &BlockMap::new(1), &History::default(), &Annotations::default())
            .unwrap();

        let (_s2, existing) = Store::open(&path).unwrap();
        let loaded = existing.unwrap();
        assert_eq!(loaded.text, "жирный и код");
        assert!(loaded.spans.covers(0..6, &InlineAttr::Strong));
        assert!(loaded.spans.covers(9..12, &InlineAttr::Code));
        assert!(
            loaded
                .spans
                .covers(2..4, &InlineAttr::Link("https://e.x".into()))
        );
        assert!(!loaded.spans.covers(6..9, &InlineAttr::Strong));

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn undo_history_and_checkpoints_roundtrip() {
        use crate::document::Document;
        let path = temp_path("undo-roundtrip");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();

        // Session 1: type, format, checkpoint, note, type more, save.
        let mut doc = Document::new("", SpanSet::default(), BlockMap::default());
        doc.edit_bytes_coalescing(0..0, "v1");
        doc.add_note(0..2, "заметка".into(), 7);
        store.apply(&doc.take_ops());
        store.add_checkpoint("first draft", true);
        doc.edit_bytes(2..2, " v2");
        doc.toggle_format(0..2, InlineAttr::Strong);
        store.apply(&doc.take_ops());
        store
            .save_with_state(
                doc.spans(),
                doc.blocks(),
                &doc.export_history(200),
                doc.notes(),
            )
            .unwrap();

        // Session 2: undo works across the restart, typing AND formatting.
        let (store2, loaded) = Store::open(&path).unwrap();
        let loaded = loaded.unwrap();
        assert_eq!(loaded.text, "v1 v2");
        assert_eq!(loaded.annotations.notes().len(), 1);
        assert_eq!(loaded.annotations.notes()[0].body, "заметка");
        let mut doc2 = Document::new(&loaded.text, loaded.spans, loaded.blocks);
        doc2.set_notes(loaded.annotations.clone());
        doc2.import_history(loaded.history.unwrap());
        assert_eq!(doc2.undo(), Some(None)); // the format toggle
        assert!(doc2.spans().spans().is_empty());
        doc2.undo().unwrap(); // " v2"
        assert_eq!(doc2.text(), "v1");
        doc2.redo().unwrap();
        assert_eq!(doc2.text(), "v1 v2");

        // Checkpoint rewind: state as of "first draft".
        let cps = store2.checkpoints();
        assert_eq!(cps.len(), 1);
        assert_eq!(cps[0].name, "first draft");
        let (text_then, _, _) = store2.state_at(&cps[0].frontiers).unwrap();
        assert_eq!(text_then, "v1");
        // And the present is untouched after time travel.
        assert_eq!(store2.text(), "v1 v2");

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
