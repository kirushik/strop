//! The durable layer: a Loro document mirroring the hot-path buffer.
//!
//! Architecture rule (docs/DECISIONS.md D3): Loro is never on the keystroke
//! path. The rope edits first; committed ops are mirrored here, and the
//! `.strop` file is a Loro snapshot — full edit history plus current state.
//! That history is what later buys checkpoints, time travel, the author's
//! own voice corpus, and (eventually) sync.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use loro::{
    ExpandType, ExportMode, Frontiers, LoroDoc, LoroValue, StyleConfig, StyleConfigMap, TextDelta,
};
use serde::{Deserialize, Serialize};

use crate::buffer::TextOp;
use crate::document::{
    Annotations, BlockKind, BlockMap, Graveyard, History, InlineAttr, Provenance, SpanSet,
};
use crate::journal::{EditRun, Journal, JournalEvent};

const TEXT_CONTAINER: &str = "content";
const BLOCKS_CONTAINER: &str = "blocks";
const SESSION_CONTAINER: &str = "session";
const ANNOTATIONS_CONTAINER: &str = "annotations";
const CHECKPOINTS_CONTAINER: &str = "checkpoints";
const ASSETS_CONTAINER: &str = "assets";
const META_CONTAINER: &str = "meta";
const SCHEMA_VERSION_KEY: &str = "schema_version";
/// Version of the durable container/schema contract, independent of the app
/// release number. Files without a marker are legacy v0 and migrate through
/// the same reader paths that have always used serde defaults/fallbacks.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;
// The graveyard (docs/impl/02-asides.md §4/§5) rides its own map + fingerprint
// channel, exactly like annotations (review B12): an unguarded blob of verbatim
// cut text rewriting per idle save is the 4.8 MB class.
const GRAVEYARD_CONTAINER: &str = "graveyard";
// Parked blocks' provenance records (the Scraps build, seam-mechanics 7):
// same lifecycle and same bloat discipline as the graveyard.
const PROVENANCE_CONTAINER: &str = "provenance";
// The journal persists as LISTS, not as re-inserted JSON blobs: a blob that
// changes every edit misses its fingerprint on every save and rewrites into
// the append-only oplog forever (the 4.8 MB class). List pushes append only
// the new items — and a list's current value survives shallow compaction as
// state, exactly like checkpoints do.
const JOURNAL_RUNS_CONTAINER: &str = "journal.runs";
const JOURNAL_EVENTS_CONTAINER: &str = "journal.events";

fn schema_version_of(doc: &LoroDoc) -> io::Result<u32> {
    let Some(value) = doc.get_map(META_CONTAINER).get(SCHEMA_VERSION_KEY) else {
        return Ok(0);
    };
    match value.into_value() {
        Ok(LoroValue::I64(version)) => u32::try_from(version).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "invalid .strop schema version")
        }),
        Ok(LoroValue::String(version)) => version.parse::<u32>().map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "invalid .strop schema version")
        }),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid .strop schema version",
        )),
    }
}

/// One explicit gate for every future durable migration. Version zero is the
/// unmarked 0.1-era format; its compatibility readers already live in
/// `read_state_of` and the serde defaults on the side structures.
fn migrate_schema(doc: &LoroDoc) -> io::Result<()> {
    let version = schema_version_of(doc)?;
    if version > CURRENT_SCHEMA_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "this .strop file uses schema {version}, but this build supports up to {CURRENT_SCHEMA_VERSION}"
            ),
        ));
    }
    match version {
        0 | CURRENT_SCHEMA_VERSION => Ok(()),
        // Kept explicit: adding v2 means adding a v1 arm here rather than
        // accidentally treating a new durable shape as the current one.
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("no migration path from .strop schema {version}"),
        )),
    }
}

fn stamp_schema_version(doc: &LoroDoc) -> io::Result<()> {
    let meta = doc.get_map(META_CONTAINER);
    if schema_version_of(doc)? != CURRENT_SCHEMA_VERSION {
        meta.insert(SCHEMA_VERSION_KEY, CURRENT_SCHEMA_VERSION as i64)
            .map_err(io::Error::other)?;
        doc.commit();
    }
    Ok(())
}

/// Everything a reopened document restores.
pub struct Loaded {
    pub text: String,
    pub spans: SpanSet,
    pub blocks: BlockMap,
    pub history: Option<History>,
    pub annotations: Annotations,
    pub journal: Journal,
    pub graveyard: Graveyard,
    pub provenance: Provenance,
}

/// A named version snapshot. Lives inside the .strop file — Google-Docs
/// version history, local-first and self-contained.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub name: String,
    pub created_unix: i64,
    /// Exact journal-axis timestamp for checkpoints written by current
    /// builds. Older files only have `created_unix`; `timestamp_ms` provides
    /// the backward-compatible seconds fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_ms: Option<i64>,
    pub frontiers: Vec<u8>,
    /// Named-by-the-author (vs automatic session markers).
    #[serde(default)]
    pub manual: bool,
    /// The checkpoint's document state, MATERIALIZED at creation — when it is
    /// one cheap read of the live doc. Rewind, previews, and asset GC read
    /// this instead of time-travelling (`state_at`): a Loro historical
    /// checkout replays the whole oplog and cost 5–7 s PER CHECKPOINT on a
    /// long-lived file — the history-sidebar hang. `None` only on checkpoints
    /// recorded by older builds; those are backfilled once, in the
    /// background, and persisted (see `set_checkpoint_state`). Self-contained
    /// states are also what make shallow-snapshot compaction safe: the
    /// rewind feature no longer needs any oplog history at all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<CheckpointState>,
}

impl Checkpoint {
    pub fn timestamp_ms(&self) -> i64 {
        self.created_ms
            .unwrap_or_else(|| self.created_unix.saturating_mul(1000))
    }
}

/// A checkpoint's frozen document state (see `Checkpoint::state`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckpointState {
    pub text: String,
    pub spans: SpanSet,
    pub blocks: BlockMap,
}

/// Legacy reader for the pre-JSON newline-joined token format. Kept so old
/// `.strop` files (and older Loro frontiers reached by `state_at`) still load;
/// new saves write JSON (see `save_with_state`). One token per block.
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

/// Text + formatting + block kinds at `doc`'s current version. Free of
/// `Store` so the backfill can read a private background-thread doc.
fn read_state_of(doc: &LoroDoc) -> (String, SpanSet, BlockMap) {
    let text = doc.get_text(TEXT_CONTAINER);
    let text_string = text.to_string();
    // Formatting: the spans JSON when present (one map read), else derive it
    // from the legacy Peritext marks. The marks path is `to_delta()`, and on
    // a file that lived through months of unmark-everything/remark-everything
    // save cycles the text state carries THOUSANDS of dead style anchors —
    // measured 4.7 s for a 5.7 KB text, the dominant cost of open, of every
    // historical checkout, and of sealing a checkpoint. Spans persist as
    // JSON now (save_with_state); marks are read-only legacy.
    let mut spans = match doc.get_map(BLOCKS_CONTAINER).get("spans") {
        Some(v) => match v.into_value() {
            Ok(LoroValue::String(s)) => serde_json::from_str(&s).unwrap_or_default(),
            _ => SpanSet::default(),
        },
        None => {
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
            spans
        }
    };
    let mut blocks = match doc.get_map(BLOCKS_CONTAINER).get("kinds") {
        Some(v) => match v.into_value() {
            // JSON first; fall back to the legacy newline-joined token
            // format so pre-existing .strop files and older Loro frontiers
            // (read by state_at) still load.
            Ok(LoroValue::String(s)) => match serde_json::from_str::<Vec<BlockKind>>(&s) {
                Ok(kinds) => BlockMap::from_kinds(kinds),
                Err(_) => BlockMap::from_kinds(s.lines().map(kind_from_token).collect()),
            },
            _ => BlockMap::default(),
        },
        None => BlockMap::default(),
    };
    // The boundary rides the SAME versioned map, so a checkpoint state
    // (materialized through this reader) reproduces the geometry of ITS
    // moment. Without this, restoring any version silently merged the pile
    // into the manuscript — exported, counted, and sent to the AI (wave-1
    // review, correctness/high) — and the next save persisted the loss.
    // ERA (adjudications, "the foundation"): the tail boundary persists
    // under its OWN key (`scrap_line`) beside the legacy `boundary` key,
    // which migrated saves write as null — a top-era build reading a
    // tail-era file degrades to no-boundary, never misreads; and this
    // reader interprets each state per the field it actually carries.
    if let Some(v) = doc.get_map(BLOCKS_CONTAINER).get("boundary")
        && let Ok(LoroValue::String(s)) = v.into_value()
        && let Ok(boundary) = serde_json::from_str::<Option<usize>>(&s)
    {
        blocks.set_aside_boundary(boundary);
    }
    if let Some(v) = doc.get_map(BLOCKS_CONTAINER).get("scrap_line")
        && let Ok(LoroValue::String(s)) = v.into_value()
        && let Ok(boundary) = serde_json::from_str::<Option<usize>>(&s)
        && boundary.is_some()
    {
        blocks.set_scrap_line(boundary);
    }
    spans.normalize(text_string.chars().count());
    (text_string, spans, blocks)
}

/// Compaction only makes sense once the oplog dwarfs the state; below this
/// a file is healthy and the extra open-time work buys nothing.
const COMPACT_MIN_BYTES: usize = 512 * 1024;

/// Opportunistic oplog compaction, at open. A long-lived file accretes
/// history the app no longer reads: with checkpoint states materialized
/// (`Checkpoint::state`), NOTHING needs Loro time-travel — rewind, previews,
/// restore, asset GC and the undo stacks are all plain state. The oplog was
/// still making open take seconds and every rewrite permanent. So: when the
/// file is big, every checkpoint is self-contained, and a shallow snapshot
/// (current state + truncated history) is at least twice smaller, adopt it —
/// after round-tripping it through a fresh doc and comparing the text, and
/// after writing a one-time `*.pre-compact.bak` of the original bytes.
/// Every failure path keeps the original doc: compaction is strictly
/// opportunistic, never load-bearing.
fn compact_on_open(doc: LoroDoc, original: &[u8], path: &Path) -> LoroDoc {
    if original.len() < COMPACT_MIN_BYTES
        || !checkpoints_of(&doc).iter().all(|cp| cp.state.is_some())
    {
        return doc;
    }
    let frontiers = doc.oplog_frontiers();
    let Ok(shallow) =
        doc.export(ExportMode::ShallowSnapshot(std::borrow::Cow::Borrowed(&frontiers)))
    else {
        return doc;
    };
    if shallow.len().saturating_mul(2) > original.len() {
        return doc;
    }
    // The shallow bytes must stand on their own before they touch disk.
    let fresh = LoroDoc::new();
    fresh.config_text_style(style_config());
    if fresh.import(&shallow).is_err()
        || fresh.get_text(TEXT_CONTAINER).to_string() != doc.get_text(TEXT_CONTAINER).to_string()
    {
        return doc;
    }
    let bak = sidecar_path(path, ".pre-compact.bak");
    if !bak.exists() && fs::write(&bak, original).is_err() {
        return doc;
    }
    let tmp = sidecar_path(path, ".tmp");
    if fs::write(&tmp, &shallow).is_err() || fs::rename(&tmp, path).is_err() {
        let _ = fs::remove_file(&tmp);
        return doc;
    }
    eprintln!(
        "strop: compacted {} → {} bytes (original kept once at {})",
        original.len(),
        shallow.len(),
        bak.display()
    );
    fresh
}

/// Open-time caption migration (docs/inline-images.md §10). The runtime
/// enum dropped `Image.caption` — the block's own line IS the caption —
/// so a surviving non-empty stored caption on a block whose line is still
/// empty moves INTO the line, here, before the `Document` exists: one-way,
/// never undoable, never journaled — an open-time normalization of the
/// LIVE doc only, placed like `compact_on_open`. Checkpoint and history
/// states are read-only past and are never rewritten (build plan,
/// adjudicated pushback 3): a pre-migration checkpoint's preview simply
/// shows the picture without its legacy caption, the recorded cost.
///
/// Positions are resolved at APPLY time, one insertion at a time — an
/// earlier caption shifts every later line, so nothing captured up front
/// can be trusted (the index-drift trap). Line-break characters in a
/// legacy caption flatten to spaces: any of ropey's break set would split
/// the image block and desync the whole map, and the Markdown boundary
/// flattens caption breaks to spaces anyway. Every in-memory anchor
/// family shifts through its own `apply_op`; the caller zeroes the save
/// guards so the shifted JSON actually reaches disk, and drops the
/// persisted undo stacks (their positions predate the inserted text —
/// the History arity guard set the one-time-loss precedent).
#[allow(clippy::too_many_arguments)]
fn migrate_image_captions(
    doc: &LoroDoc,
    text: &mut String,
    spans: &mut SpanSet,
    blocks: &BlockMap,
    annotations: &mut Annotations,
    graveyard: &mut Graveyard,
    provenance: &mut Provenance,
) -> bool {
    // Captions live only in the raw kinds JSON — the runtime deserializer
    // drops the field — so read the wire itself. Token-format files carry
    // no captions and fail this parse: nothing to migrate.
    let Some(v) = doc.get_map(BLOCKS_CONTAINER).get("kinds") else {
        return false;
    };
    let Ok(LoroValue::String(raw)) = v.into_value() else {
        return false;
    };
    let Ok(entries) = serde_json::from_str::<Vec<serde_json::Value>>(&raw) else {
        return false;
    };
    let legacy: Vec<(usize, String)> = entries
        .iter()
        .enumerate()
        .filter_map(|(ix, e)| {
            let cap = e.get("Image")?.get("caption")?.as_str()?;
            (!cap.is_empty()).then(|| (ix, cap.to_owned()))
        })
        .collect();
    if legacy.is_empty() {
        return false;
    }
    // (byte, char) offsets of `line`'s start in `text`, None past the end —
    // resolved by ropey, the SAME break metric the block indices were
    // minted under (CR/CRLF/VT/FF/NEL/U+2028/2029 all split; see
    // pre_edit_info's doc comment). A hand-rolled '\n' counter drifted one
    // line per earlier non-LF break, so a legacy classic-Mac/PDF paste
    // above the picture migrated the caption onto the WRONG block — and
    // the one-way clear below made the misplacement permanent.
    let line_start = |text: &str, line: usize| -> Option<(usize, usize)> {
        let rope = ropey::Rope::from_str(text);
        (line < rope.len_lines()).then(|| (rope.line_to_byte(line), rope.line_to_char(line)))
    };
    // Any of the rope's break set ends an empty line (the line's own
    // terminator is its first char exactly when it holds no words).
    let is_break = |c: char| {
        matches!(
            c,
            '\u{000A}' | '\u{000B}' | '\u{000C}' | '\u{000D}' | '\u{0085}' | '\u{2028}'
                | '\u{2029}'
        )
    };
    let ltext = doc.get_text(TEXT_CONTAINER);
    let mut migrated = false;
    for (block, caption) in legacy {
        let Some((byte, chars)) = line_start(text, block) else {
            continue; // a desynced map: never guess, never touch text
        };
        match text[byte..].chars().next() {
            None => {}
            Some(c) if is_break(c) => {}
            _ => continue, // the line already has words — spec: empty lines only
        }
        let flat: String = caption
            .replace("\r\n", "\n")
            .chars()
            .map(|c| match c {
                '\u{000A}' | '\u{000B}' | '\u{000C}' | '\u{000D}' | '\u{0085}'
                | '\u{2028}' | '\u{2029}' => ' ',
                c => c,
            })
            .collect();
        if ltext.insert(chars, &flat).is_err() {
            continue;
        }
        text.insert_str(byte, &flat);
        let op = TextOp { pos: chars, delete: 0, insert: flat };
        spans.apply_op_verbatim(&op);
        annotations.apply_op(&op);
        graveyard.apply_op(&op);
        provenance.apply_op(&op);
        migrated = true;
    }
    if migrated {
        // Clear the vestigial stored values (the new serializer emits the
        // key as "" — wire compat holds), so a caption line the writer
        // later empties can never resurrect its legacy text on a reopen.
        match serde_json::to_string(blocks.kinds()) {
            Ok(kinds) => {
                if let Err(e) = doc.get_map(BLOCKS_CONTAINER).insert("kinds", kinds) {
                    eprintln!("strop: migrate captions: {e}");
                }
            }
            Err(e) => eprintln!("strop: encode migrated kinds: {e}"),
        }
        doc.commit();
    }
    migrated
}

/// The checkpoint list of `doc` (see `Store::checkpoints`).
fn checkpoints_of(doc: &LoroDoc) -> Vec<Checkpoint> {
    let list = doc.get_list(CHECKPOINTS_CONTAINER);
    (0..list.len())
        .filter_map(|i| list.get(i))
        .filter_map(|v| match v.into_value() {
            Ok(LoroValue::String(json)) => serde_json::from_str(&json).ok(),
            _ => None,
        })
        .map(|mut cp: Checkpoint| {
            if let Some(state) = &mut cp.state {
                state.spans.normalize(state.text.chars().count());
            }
            cp
        })
        .collect()
}

/// The persisted journal of `doc`: two append-only lists, one JSON item per
/// settled run/event. Damaged items are skipped, never trusted into a panic.
fn journal_of(doc: &LoroDoc) -> Journal {
    let parse_list = |name: &str| -> Vec<String> {
        let list = doc.get_list(name);
        (0..list.len())
            .filter_map(|i| list.get(i))
            .filter_map(|v| match v.into_value() {
                Ok(LoroValue::String(json)) => Some(json.to_string()),
                _ => None,
            })
            .collect()
    };
    let runs: Vec<EditRun> = parse_list(JOURNAL_RUNS_CONTAINER)
        .iter()
        .filter_map(|json| serde_json::from_str(json).ok())
        .collect();
    let events: Vec<JournalEvent> = parse_list(JOURNAL_EVENTS_CONTAINER)
        .iter()
        .filter_map(|json| serde_json::from_str(json).ok())
        .collect();
    Journal::from_parts(runs, events)
}

/// Legacy-marks reader (spans persist as JSON now; see `read_state_of`).
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
    /// Fingerprints of what the last save actually wrote (or what the file
    /// held at open). `save_with_state` used to rewrite the annotations,
    /// blocks and undo-history JSON — and unmark+remark every formatting
    /// span — on EVERY idle save, unchanged or not; in an append-only CRDT
    /// every rewrite is new oplog forever (the reporter's 4.8 MB file held
    /// 5.7 KB of prose). Now each piece writes only when its content
    /// actually changed.
    saved: std::cell::RefCell<SavedHashes>,
    /// Journal items already pushed to the list containers (runs, events):
    /// a save appends only the tail past these counts. Seeded from the file
    /// at open; a fresh document starts at zero.
    journal_saved: std::cell::RefCell<(usize, usize)>,
    /// The doc's frontiers as of open / the last prepared save — the quit
    /// guard's clean predicate (`needs_save`). The editor's `store_dirty`
    /// flag only knows about edits routed through its dirty chokepoint;
    /// open-time session seals, idle-gap "Session" checkpoints and the
    /// cold-read seal mutate the doc directly, and a close must still
    /// flush them (papercuts LAW 2: never exit with unflushed bytes).
    saved_frontiers: std::cell::RefCell<Frontiers>,
}

/// See `Store::saved`. Zeroes mean "unknown" — the next save writes.
#[derive(Default)]
struct SavedHashes {
    annotations: u64,
    blocks: u64,
    history: u64,
    spans: u64,
    /// The graveyard's own fingerprint channel (review B12). The `blocks`
    /// channel also covers the out-of-band boundary indexes, since they move
    /// only when block structure does (both driven by `BlockMap::on_edit`) and
    /// are a few bytes — so their JSON is folded into the blocks fingerprint.
    graveyard: u64,
    /// The provenance records' channel (same discipline as the graveyard).
    provenance: u64,
}

/// Content fingerprint for the save guards (not cryptographic — this only
/// ever compares a process's own serializations with each other).
fn fingerprint(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// The blocks channel's fingerprint over the kinds JSON and BOTH boundary
/// indexes, so a boundary-only change (a seam born or evaporated without
/// touching kinds) still triggers exactly one write of all three keys. An
/// old file that never uses asides fingerprints `kinds` + `null` + `null`,
/// matching a save that stays `None`, so nothing is written.
fn blocks_fingerprint(blocks: &BlockMap) -> u64 {
    let kinds = serde_json::to_string(blocks.kinds()).unwrap_or_default();
    let boundary = serde_json::to_string(&blocks.aside_boundary()).unwrap_or_default();
    let scrap_line = serde_json::to_string(&blocks.scrap_line()).unwrap_or_default();
    fingerprint(&format!("{kinds}\u{1}{boundary}\u{1}{scrap_line}"))
}

/// An immutable Loro snapshot which is safe to move to a filesystem thread.
/// `LoroDoc` itself remains confined to the UI thread; after export, the
/// worker owns only bytes and a path.
pub struct PreparedSave {
    path: PathBuf,
    bytes: Vec<u8>,
}

impl PreparedSave {
    pub fn write(self) -> io::Result<()> {
        atomic_write(&self.path, &self.bytes)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SaveGeneration(u64);

pub struct SaveCompletion {
    pub generation: SaveGeneration,
    pub result: io::Result<()>,
}

/// A single-consumer queue proves generation N reaches replacement before
/// N+1, so an older snapshot can never overwrite a newer one.
pub struct SaveWorker {
    requests: mpsc::Sender<(SaveGeneration, PreparedSave)>,
    completions: mpsc::Receiver<SaveCompletion>,
    next_generation: u64,
}

impl SaveWorker {
    pub fn new() -> Self {
        Self::with_notify(|| {})
    }

    /// A worker whose thread calls `notify` after posting each completion.
    /// The hook is how a save's arrival WAKES the owning thread (the editor
    /// parks a waker behind it) instead of the completion sitting in the
    /// channel until the next 1s heartbeat — the quit guard's "a fast final
    /// write closes the window in milliseconds, not a second" requirement.
    pub fn with_notify(notify: impl Fn() + Send + 'static) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<(SaveGeneration, PreparedSave)>();
        let (completion_tx, completion_rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("strop-save".into())
            .spawn(move || {
                while let Ok((generation, save)) = request_rx.recv() {
                    let result = save.write();
                    if completion_tx
                        .send(SaveCompletion { generation, result })
                        .is_err()
                    {
                        break;
                    }
                    notify();
                }
            })
            .expect("spawn filesystem save worker");
        Self {
            requests: request_tx,
            completions: completion_rx,
            next_generation: 0,
        }
    }

    pub fn request(&mut self, save: PreparedSave) -> SaveGeneration {
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .expect("save generation overflow");
        let generation = SaveGeneration(self.next_generation);
        self.requests
            .send((generation, save))
            .expect("save worker unexpectedly stopped");
        generation
    }

    pub fn try_completion(&self) -> Option<SaveCompletion> {
        self.completions.try_recv().ok()
    }

    pub fn wait_for(&self, generation: SaveGeneration) -> SaveCompletion {
        loop {
            let completion = self
                .completions
                .recv()
                .expect("save worker unexpectedly stopped");
            if completion.generation == generation {
                return completion;
            }
        }
    }
}

impl Default for SaveWorker {
    fn default() -> Self {
        Self::new()
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = sidecar_path(path, ".tmp");
    let mut file = fs::File::create(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    atomic_replace(&tmp, path)?;
    #[cfg(unix)]
    if let Some(dir) = path.parent() {
        fs::File::open(dir)?.sync_all()?;
    }
    Ok(())
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

#[cfg(not(windows))]
fn atomic_replace(from: &Path, to: &Path) -> io::Result<()> {
    fs::rename(from, to)
}

#[cfg(windows)]
fn atomic_replace(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: both buffers are live, NUL-terminated UTF-16 paths; the temp is
    // in the destination directory and therefore on the same volume.
    let ok = unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
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
            // Escaping a portal mount crosses filesystems (FUSE → host);
            // rename can't, copy-then-remove can. If the remove fails,
            // keep both files — two copies beat zero.
            Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {
                fs::copy(&self.path, &new_path)?;
                if let Err(e) = fs::remove_file(&self.path) {
                    eprintln!("strop: rename left the old file behind: {e}");
                }
            }
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
                migrate_schema(&doc)?;
                let doc = compact_on_open(doc, &bytes, &path);
                let store = Self {
                    doc,
                    path,
                    saved: Default::default(),
                    journal_saved: Default::default(),
                    saved_frontiers: Default::default(),
                };
                // The aside boundary persists as its OWN key beside "kinds"
                // (review B13/H42): an older build reads only "kinds" and so
                // ignores it — compost folds into the manuscript, nothing
                // resets. `read_state_of` applies it, HERE and for every
                // materialized checkpoint state alike.
                let (mut text, mut spans, blocks) = store.read_state();
                let journal = journal_of(&store.doc);
                *store.journal_saved.borrow_mut() =
                    (journal.runs.len(), journal.events.len());
                let mut graveyard = match store.doc.get_map(GRAVEYARD_CONTAINER).get("list") {
                    Some(v) => match v.into_value() {
                        Ok(LoroValue::String(json)) => {
                            serde_json::from_str(&json).unwrap_or_default()
                        }
                        _ => Graveyard::default(),
                    },
                    None => Graveyard::default(),
                };
                let mut provenance = match store.doc.get_map(PROVENANCE_CONTAINER).get("list") {
                    Some(v) => match v.into_value() {
                        Ok(LoroValue::String(json)) => {
                            serde_json::from_str(&json).unwrap_or_default()
                        }
                        _ => Provenance::default(),
                    },
                    None => Provenance::default(),
                };
                let history = match store.doc.get_map(SESSION_CONTAINER).get("history") {
                    Some(v) => match v.into_value() {
                        Ok(LoroValue::String(json)) => serde_json::from_str(&json).ok(),
                        _ => None,
                    },
                    None => None,
                };
                let mut annotations = match store.doc.get_map(ANNOTATIONS_CONTAINER).get("list") {
                    Some(v) => match v.into_value() {
                        Ok(LoroValue::String(json)) => {
                            serde_json::from_str(&json).unwrap_or_default()
                        }
                        _ => Annotations::default(),
                    },
                    None => Annotations::default(),
                };
                // Open-time caption migration (docs/inline-images.md §10):
                // a legacy non-empty caption moves into the block's line.
                // The persisted undo stacks predate the inserted text and
                // their positions cannot be trusted across it, so a
                // migrating open drops them — the one-time loss the
                // History arity guard already set precedent for.
                let migrated = migrate_image_captions(
                    &store.doc,
                    &mut text,
                    &mut spans,
                    &blocks,
                    &mut annotations,
                    &mut graveyard,
                    &mut provenance,
                );
                let history = if migrated { None } else { history };
                // Repair untrusted side state against the text it describes —
                // AFTER the migration, whose caption insert is the last thing
                // to move the text and the offsets that index it.
                let len_chars = text.chars().count();
                annotations.normalize(len_chars);
                graveyard.normalize(len_chars);
                provenance.normalize(len_chars);
                // Seed the save guards with what the file already holds, so
                // the session's first unchanged save rewrites nothing.
                *store.saved.borrow_mut() = SavedHashes {
                    annotations: fingerprint(
                        &serde_json::to_string(&annotations).unwrap_or_default(),
                    ),
                    blocks: blocks_fingerprint(&blocks),
                    graveyard: fingerprint(
                        &serde_json::to_string(&graveyard).unwrap_or_default(),
                    ),
                    provenance: fingerprint(
                        &serde_json::to_string(&provenance).unwrap_or_default(),
                    ),
                    history: history
                        .as_ref()
                        .map(|h: &History| {
                            fingerprint(&serde_json::to_string(h).unwrap_or_default())
                        })
                        .unwrap_or_default(),
                    // Seed from the file's spans JSON when it has one. A
                    // legacy MARKS file seeds 0 ("unknown") instead — its
                    // loaded spans would re-serialize to the very hash we'd
                    // seed, and the upgrade write must not be skipped.
                    spans: if store.doc.get_map(BLOCKS_CONTAINER).get("spans").is_some() {
                        fingerprint(&serde_json::to_string(&spans).unwrap_or_default())
                    } else {
                        0
                    },
                };
                if migrated {
                    // The file still holds the PRE-shift anchors: zero every
                    // guard (0 = "unknown, next save writes") so the shifted
                    // spans/notes/graveyard/provenance and the emptied
                    // history all reach disk — except blocks, whose cleared
                    // kinds the migration already wrote itself.
                    let blocks_hash = store.saved.borrow().blocks;
                    *store.saved.borrow_mut() = SavedHashes {
                        blocks: blocks_hash,
                        ..Default::default()
                    };
                }
                // The clean-close baseline is the doc AS OPENED — after the
                // open-time normalizations above (schema/caption migration,
                // compaction). A read-only session therefore closes without
                // a write; a legacy file's migrations simply re-run at the
                // next open (deterministic) and persist with the first real
                // save, instead of every glance rewriting the file.
                *store.saved_frontiers.borrow_mut() = store.doc.oplog_frontiers();
                Ok((
                    store,
                    Some(Loaded {
                        text,
                        spans,
                        blocks,
                        history,
                        annotations,
                        journal,
                        graveyard,
                        provenance,
                    }),
                ))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok((
                Self {
                    doc,
                    path,
                    saved: Default::default(),
                    journal_saved: Default::default(),
                    // An empty doc over a missing file: the default (empty)
                    // frontiers already say "nothing to flush".
                    saved_frontiers: Default::default(),
                },
                None,
            )),
            Err(e) => Err(e),
        }
    }

    /// Text + formatting + block kinds at the doc's *current* version
    /// (which `state_at` temporarily moves).
    fn read_state(&self) -> (String, SpanSet, BlockMap) {
        read_state_of(&self.doc)
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

    /// Does `id` resolve in THIS document's store? The §9 paste precedence
    /// asks existence only — no byte copy out of Loro for a yes/no.
    pub fn has_asset(&self, id: &str) -> bool {
        self.doc.get_map(ASSETS_CONTAINER).get(id).is_some()
    }

    /// Record a named checkpoint at the current version. The state is
    /// materialized NOW — one cheap read of the live doc — so nothing ever
    /// has to time-travel back here (Checkpoint::state).
    pub fn add_checkpoint(&self, name: &str, manual: bool) {
        let state = self.read_state();
        self.add_checkpoint_with_state(name, manual, state);
    }

    /// Name a caller-reconstructed moment without checking out or mutating
    /// the live document. Names are intentionally not unique.
    pub fn add_checkpoint_at(
        &self,
        name: &str,
        timestamp_ms: i64,
        state: CheckpointState,
    ) {
        self.doc.commit();
        let checkpoint = Checkpoint {
            name: name.to_owned(),
            created_unix: timestamp_ms / 1000,
            created_ms: Some(timestamp_ms),
            frontiers: self.doc.oplog_frontiers().encode(),
            manual: true,
            state: Some(state),
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

    /// `add_checkpoint` with the state supplied by a caller that already
    /// holds it (open-time sealing) — skips re-deriving it from the doc.
    fn add_checkpoint_with_state(
        &self,
        name: &str,
        manual: bool,
        (text, spans, blocks): (String, SpanSet, BlockMap),
    ) {
        self.doc.commit();
        let created_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
            .unwrap_or(0);
        let checkpoint = Checkpoint {
            name: name.to_owned(),
            created_unix: created_ms / 1000,
            created_ms: Some(created_ms),
            frontiers: self.doc.oplog_frontiers().encode(),
            manual,
            state: Some(CheckpointState { text, spans, blocks }),
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

    /// Rig hook (`seed:legacy`): push a checkpoint at an EXPLICIT past
    /// timestamp with a materialized state — synthesizes a legacy file's
    /// history (checkpoints only, empty journal) without a fortnight of real
    /// editing. The public API always stamps `now`; only this test seam can
    /// backdate. Never called outside the smoke rig.
    pub fn debug_push_checkpoint(
        &self,
        name: &str,
        created_unix: i64,
        manual: bool,
        state: CheckpointState,
    ) {
        self.doc.commit();
        let checkpoint = Checkpoint {
            name: name.to_owned(),
            created_unix,
            created_ms: None,
            frontiers: self.doc.oplog_frontiers().encode(),
            manual,
            state: Some(state),
        };
        match serde_json::to_string(&checkpoint) {
            Ok(json) => {
                let list = self.doc.get_list(CHECKPOINTS_CONTAINER);
                if let Err(e) = list.push(json) {
                    eprintln!("strop: seed checkpoint: {e}");
                }
                self.doc.commit();
            }
            Err(e) => eprintln!("strop: encode seed checkpoint: {e}"),
        }
    }

    /// Rename a checkpoint; renaming an automatic entry makes it named
    /// (manual), per the rewind research. An EMPTY name is the inverse verb
    /// — demotion: the version keeps its recorded state (nothing is lost)
    /// and returns to an unnamed station, off the strip's label lane.
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
        cp.manual = !name.is_empty();
        match serde_json::to_string(&cp) {
            Ok(json) => {
                // Insert-before-delete (see `set_checkpoint_state`): a failed
                // insert must not drop the checkpoint being renamed.
                if let Err(e) = list.insert(ix, json) {
                    eprintln!("strop: rename checkpoint: {e}");
                    return;
                }
                let _ = list.delete(ix + 1, 1);
                self.doc.commit();
            }
            Err(e) => eprintln!("strop: encode checkpoint: {e}"),
        }
    }

    /// Add a checkpoint only if the document state actually differs from
    /// the most recent checkpoint — empty sessions never clutter the rail
    /// (the research's top Docs complaint).
    pub fn add_checkpoint_if_changed(&self, name: &str, manual: bool) {
        let current = self.read_state();
        self.seal_session_with(name, manual, current);
    }

    /// Open-time session sealing with the state `open` already produced —
    /// no re-derivation. While the file is MID-MIGRATION (its last
    /// checkpoint has no materialized state yet), sealing defers to the
    /// next launch: the comparison would need a multi-second historical
    /// checkout, and the background backfill is about to make it free.
    pub fn seal_session_with(
        &self,
        name: &str,
        manual: bool,
        current: (String, SpanSet, BlockMap),
    ) {
        if let Some(last) = self.checkpoints().last() {
            let Some(state) = &last.state else {
                return; // legacy checkpoint mid-migration — seal next launch
            };
            // Full (text, spans, blocks) comparison: a session that only
            // bolded or restructured headings still deserves a rewind
            // point — text-only comparison made such work unreachable.
            if (&state.text, &state.spans, &state.blocks)
                == (&current.0, &current.1, &current.2)
            {
                return;
            }
        }
        self.add_checkpoint_with_state(name, manual, current);
    }

    /// A checkpoint's document state: the materialized copy when it has one
    /// (instant), else the legacy time-travel read (`state_at`, seconds on a
    /// long oplog — exists only until the background backfill lands).
    pub fn checkpoint_state(&self, cp: &Checkpoint) -> Option<(String, SpanSet, BlockMap)> {
        match &cp.state {
            Some(s) => Some((s.text.clone(), s.spans.clone(), s.blocks.clone())),
            None => self.state_at(&cp.frontiers),
        }
    }

    /// Do all checkpoints carry a materialized state? (True for every file
    /// this build has checkpointed; false until a legacy file's backfill.)
    pub fn checkpoints_materialized(&self) -> bool {
        self.checkpoints().iter().all(|cp| cp.state.is_some())
    }

    /// Attach a materialized state to a legacy checkpoint (the backfill's
    /// write-back). Refuses to overwrite an existing state — states are
    /// immutable once recorded.
    pub fn set_checkpoint_state(&self, ix: usize, state: CheckpointState) {
        let list = self.doc.get_list(CHECKPOINTS_CONTAINER);
        let Some(v) = list.get(ix) else { return };
        let Ok(LoroValue::String(json)) = v.into_value() else {
            return;
        };
        let Ok(mut cp) = serde_json::from_str::<Checkpoint>(&json) else {
            return;
        };
        if cp.state.is_some() {
            return;
        }
        cp.state = Some(state);
        match serde_json::to_string(&cp) {
            Ok(json) => {
                // Insert the updated copy BEFORE removing the old one: a failed
                // insert then leaves the original intact at `ix` and commits
                // nothing, so the checkpoint can never be lost outright.
                if let Err(e) = list.insert(ix, json) {
                    eprintln!("strop: backfill checkpoint state: {e}");
                    return;
                }
                let _ = list.delete(ix + 1, 1);
                self.doc.commit();
            }
            Err(e) => eprintln!("strop: encode checkpoint: {e}"),
        }
    }

    /// Current full snapshot as bytes — the input `materialize_checkpoint_states`
    /// chews through on a background thread (the live doc never blocks).
    pub fn export_bytes(&self) -> io::Result<Vec<u8>> {
        self.doc.export(ExportMode::Snapshot).map_err(io::Error::other)
    }

    /// Materialize the states of every checkpoint that lacks one, from a
    /// snapshot's BYTES — self-contained (its own private LoroDoc), so it can
    /// run on a background thread while the app keeps editing. Walks the
    /// checkpoints oldest→newest WITHOUT returning to the latest version
    /// between reads: each step is a short hop, where round-tripping from the
    /// tip cost 5–7 s per checkpoint on a long-lived file (the hang).
    /// Returns `(checkpoint index, state)` pairs for `set_checkpoint_state`.
    pub fn materialize_checkpoint_states(bytes: &[u8]) -> Vec<(usize, CheckpointState)> {
        let doc = LoroDoc::new();
        doc.config_text_style(style_config());
        if doc.import(bytes).is_err() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (ix, cp) in checkpoints_of(&doc).into_iter().enumerate() {
            if cp.state.is_some() {
                continue;
            }
            let Ok(frontiers) = Frontiers::decode(&cp.frontiers) else {
                continue;
            };
            if doc.checkout(&frontiers).is_err() {
                continue;
            }
            let (text, spans, blocks) = read_state_of(&doc);
            out.push((ix, CheckpointState { text, spans, blocks }));
        }
        out
    }

    pub fn checkpoints(&self) -> Vec<Checkpoint> {
        checkpoints_of(&self.doc)
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
    /// neither is mirrored per-edit — this avoids expand-rule drift. Every
    /// channel is guarded by its `SavedHashes` fingerprint: an unchanged
    /// piece writes NOTHING, because in an append-only CRDT each rewrite is
    /// permanent oplog growth (the 4.8 MB-file bug class).
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_save_with_state(
        &self,
        spans: &SpanSet,
        blocks: &BlockMap,
        history: &History,
        annotations: &Annotations,
        journal: &Journal,
        graveyard: &Graveyard,
        provenance: &Provenance,
    ) -> io::Result<PreparedSave> {
        // Journal: push only the tail past what the file already holds.
        // Callers settle the journal before saving, so every pushed item is
        // final — the containers stay strictly append-only (no rewrites, no
        // fingerprint needed).
        {
            let mut jsaved = self.journal_saved.borrow_mut();
            let runs = self.doc.get_list(JOURNAL_RUNS_CONTAINER);
            for run in &journal.runs[jsaved.0.min(journal.runs.len())..] {
                match serde_json::to_string(run) {
                    Ok(json) => {
                        if let Err(e) = runs.push(json) {
                            eprintln!("strop: persist journal run: {e}");
                        }
                    }
                    Err(e) => eprintln!("strop: encode journal run: {e}"),
                }
            }
            let events = self.doc.get_list(JOURNAL_EVENTS_CONTAINER);
            for ev in &journal.events[jsaved.1.min(journal.events.len())..] {
                match serde_json::to_string(ev) {
                    Ok(json) => {
                        if let Err(e) = events.push(json) {
                            eprintln!("strop: persist journal event: {e}");
                        }
                    }
                    Err(e) => eprintln!("strop: encode journal event: {e}"),
                }
            }
            *jsaved = (journal.runs.len(), journal.events.len());
        }
        let mut saved = self.saved.borrow_mut();
        match serde_json::to_string(annotations) {
            Ok(json) => {
                let h = fingerprint(&json);
                if h != saved.annotations {
                    if let Err(e) = self.doc.get_map(ANNOTATIONS_CONTAINER).insert("list", json) {
                        eprintln!("strop: persist annotations: {e}");
                    } else {
                        saved.annotations = h;
                    }
                }
            }
            Err(e) => eprintln!("strop: encode annotations: {e}"),
        }
        // Formatting persists as spans JSON. It used to persist as Peritext
        // marks, rebuilt by unmark-everything/remark-everything on every
        // save: each cycle left dead style anchors in the text state forever,
        // until reading the marks back (to_delta) cost 4.7 s on 5.7 KB of
        // prose — the slow-open/slow-checkout disease. Marks are now
        // read-only legacy (read_state_of falls back to them once, for files
        // saved before this).
        match serde_json::to_string(spans) {
            Ok(json) => {
                let h = fingerprint(&json);
                if h != saved.spans {
                    if let Err(e) = self.doc.get_map(BLOCKS_CONTAINER).insert("spans", json) {
                        eprintln!("strop: persist spans: {e}");
                    } else {
                        saved.spans = h;
                    }
                }
            }
            Err(e) => eprintln!("strop: encode spans: {e}"),
        }
        // Block kinds persist as JSON (like history/annotations/checkpoints in
        // this file), not a newline-joined token stream: a '\n' or '\r' inside
        // a CodeBlock.info / Image.src (reachable via .md import of an
        // entity-encoded URL) used to desync the token count and silently
        // collapse the whole BlockMap on reopen. JSON also carries Image
        // alt/caption, which the token format dropped.
        // Block kinds AND the out-of-band aside boundary share one fingerprint
        // channel (see `blocks_fingerprint`): the boundary is a separate key so
        // an older build that reads only "kinds" ignores it (compost folds into
        // the manuscript — text preserved, boundary dropped, documented).
        let h = blocks_fingerprint(blocks);
        if h != saved.blocks {
            let bmap = self.doc.get_map(BLOCKS_CONTAINER);
            let kinds = serde_json::to_string(blocks.kinds());
            let boundary = serde_json::to_string(&blocks.aside_boundary());
            let scrap_line = serde_json::to_string(&blocks.scrap_line());
            match (kinds, boundary, scrap_line) {
                (Ok(kinds), Ok(boundary), Ok(scrap_line)) => {
                    // A tail-era save writes the legacy `boundary` key as
                    // null (a top-era build then degrades to no-boundary,
                    // the documented safe path — never a misread) and the
                    // seam under its own `scrap_line` key.
                    if let Err(e) = bmap.insert("kinds", kinds) {
                        eprintln!("strop: persist blocks: {e}");
                    } else if let Err(e) = bmap.insert("boundary", boundary) {
                        eprintln!("strop: persist aside boundary: {e}");
                    } else if let Err(e) = bmap.insert("scrap_line", scrap_line) {
                        eprintln!("strop: persist scrap line: {e}");
                    } else {
                        saved.blocks = h;
                    }
                }
                (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                    eprintln!("strop: encode blocks: {e}")
                }
            }
        }
        match serde_json::to_string(graveyard) {
            Ok(json) => {
                let h = fingerprint(&json);
                if h != saved.graveyard {
                    if let Err(e) = self.doc.get_map(GRAVEYARD_CONTAINER).insert("list", json) {
                        eprintln!("strop: persist graveyard: {e}");
                    } else {
                        saved.graveyard = h;
                    }
                }
            }
            Err(e) => eprintln!("strop: encode graveyard: {e}"),
        }
        match serde_json::to_string(provenance) {
            Ok(json) => {
                let h = fingerprint(&json);
                if h != saved.provenance {
                    if let Err(e) = self.doc.get_map(PROVENANCE_CONTAINER).insert("list", json) {
                        eprintln!("strop: persist provenance: {e}");
                    } else {
                        saved.provenance = h;
                    }
                }
            }
            Err(e) => eprintln!("strop: encode provenance: {e}"),
        }
        match serde_json::to_string(history) {
            Ok(json) => {
                let h = fingerprint(&json);
                if h != saved.history {
                    if let Err(e) = self.doc.get_map(SESSION_CONTAINER).insert("history", json) {
                        eprintln!("strop: persist history: {e}");
                    } else {
                        saved.history = h;
                    }
                }
            }
            Err(e) => eprintln!("strop: encode history: {e}"),
        }
        drop(saved);
        self.collect_unreachable_assets(blocks, history, graveyard);
        self.doc.commit();
        self.prepare_save()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn save_with_state(
        &self,
        spans: &SpanSet,
        blocks: &BlockMap,
        history: &History,
        annotations: &Annotations,
        journal: &Journal,
        graveyard: &Graveyard,
        provenance: &Provenance,
    ) -> io::Result<()> {
        self.prepare_save_with_state(
            spans,
            blocks,
            history,
            annotations,
            journal,
            graveyard,
            provenance,
        )?
        .write()
    }

    /// Save-time asset GC: an asset survives if the current document, the
    /// live graveyard (Put back can re-stamp an Image block —
    /// graveyard-interplay 9), any persisted undo/redo state (its BlockMap
    /// AND its Graveyard element), or any checkpoint's document still
    /// references it. (Deleting an image block orphans its bytes only once
    /// every survivor path has rotated away.)
    fn collect_unreachable_assets(
        &self,
        blocks: &BlockMap,
        history: &History,
        graveyard: &Graveyard,
    ) {
        let assets = self.doc.get_map(ASSETS_CONTAINER);
        if assets.is_empty() {
            return;
        }
        let mut reachable: std::collections::HashSet<String> = blocks
            .asset_refs()
            .chain(history.asset_refs())
            .chain(graveyard.asset_refs())
            .map(str::to_owned)
            .collect();
        let stored: Vec<String> = assets.keys().map(|k| k.to_string()).collect();
        // Cheap gate — the fix for multi-second idle-save stalls. If every stored
        // asset is already referenced by the LIVE doc or undo history, nothing can
        // be orphaned, so the delete loop below would delete nothing regardless of
        // what the checkpoints reference. Skip the per-checkpoint historical
        // checkout (`state_at`) — it costs ~1s each on a large oplog and was
        // running on EVERY save just to re-confirm "still referenced". Only when a
        // stored asset is MISSING from the live set (an image was deleted) do we
        // pay to check whether some checkpoint still needs it before reclaiming.
        if stored.iter().all(|id| reachable.contains(id)) {
            return;
        }
        // Materialized checkpoint states read instantly; only a legacy
        // checkpoint (pre-backfill) still pays the historical checkout.
        for cp in self.checkpoints() {
            if let Some((_, _, cp_blocks)) = self.checkpoint_state(&cp) {
                reachable.extend(cp_blocks.asset_refs().map(str::to_owned));
            }
        }
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
        self.prepare_save()?.write()
    }

    pub fn prepare_save(&self) -> io::Result<PreparedSave> {
        stamp_schema_version(&self.doc)?;
        let bytes = self
            .doc
            .export(ExportMode::Snapshot)
            .map_err(io::Error::other)?;
        // These bytes ARE the doc at this frontier; once they land, disk
        // matches. Optimistic on write failure — but a failed write also
        // raises `save_error`, which every quit path checks before the
        // clean-close shortcut, so the LAW holds regardless.
        *self.saved_frontiers.borrow_mut() = self.doc.oplog_frontiers();
        Ok(PreparedSave {
            path: self.path.clone(),
            bytes,
        })
    }

    /// Has anything moved past the last PREPARED save (or the open read)?
    /// The frontiers baseline advances in `prepare_save`, before the bytes
    /// land — deliberately optimistic: a failed write raises `save_error`,
    /// and every quit path consults that alongside this predicate, so a
    /// lost write can never look clean (see `prepare_save`'s comment).
    /// Doc ops via the frontiers baseline; the journal's unpersisted tail by
    /// count (its items live editor-side until a save pushes them). The
    /// quit guard's clean predicate: false means a close may skip the final
    /// save entirely — nothing would be lost.
    pub fn needs_save(&self, journal: &Journal) -> bool {
        if *self.journal_saved.borrow() != (journal.runs.len(), journal.events.len()) {
            return true;
        }
        self.doc.oplog_frontiers() != *self.saved_frontiers.borrow()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Write a snapshot copy (full history) to another path; the open
    /// document keeps saving to its own.
    pub fn save_copy_to(&self, path: &Path) -> io::Result<()> {
        self.prepare_copy_to(path)?.write()
    }

    /// Prepare a full snapshot for another path without moving the open
    /// document. Export stays with the Loro owner; the immutable result may
    /// be written by a filesystem worker.
    pub fn prepare_copy_to(&self, path: &Path) -> io::Result<PreparedSave> {
        stamp_schema_version(&self.doc)?;
        let bytes = self.doc.export(ExportMode::Snapshot).map_err(io::Error::other)?;
        Ok(PreparedSave { path: path.to_owned(), bytes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::GraveRegion;
    use crate::Buffer;

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("strop-test-{}-{tag}.strop", std::process::id()))
    }

    #[test]
    fn atomic_save_replaces_an_existing_file() {
        let path = temp_path("atomic-replace");
        fs::write(&path, b"old").unwrap();
        atomic_write(&path, b"new snapshot").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new snapshot");
        assert!(!sidecar_path(&path, ".tmp").exists());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rename_survives_a_filesystem_boundary() {
        // temp_dir is usually tmpfs while target/ sits on the repo's disk —
        // when they differ this exercises the CrossesDevices copy fallback
        // (the portal-mount escape); when they don't, plain rename covers
        // the same contract. Either way: content moves, old path empties.
        let old = temp_path("rename-across");
        let _ = fs::remove_file(&old);
        let (mut store, loaded) = Store::open(&old).unwrap();
        assert!(loaded.is_none());
        fs::write(&old, b"the manuscript").unwrap();
        let far_dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/strop-test-rename");
        fs::create_dir_all(&far_dir).unwrap();
        let new = far_dir.join(format!("moved-{}.strop", std::process::id()));
        let _ = fs::remove_file(&new);

        store.rename_file(&new).unwrap();
        assert_eq!(store.path(), new);
        assert_eq!(fs::read(&new).unwrap(), b"the manuscript");
        assert!(!old.exists());
        let _ = fs::remove_file(&new);
    }

    #[test]
    fn sidecars_append_to_extensionless_dotted_names() {
        let path = Path::new("my-notes v0.2");
        assert_eq!(
            sidecar_path(path, ".tmp"),
            PathBuf::from("my-notes v0.2.tmp")
        );
        assert_eq!(
            sidecar_path(path, ".pre-compact.bak"),
            PathBuf::from("my-notes v0.2.pre-compact.bak")
        );
        assert_eq!(
            sidecar_path(Path::new("doc.strop"), ".tmp"),
            PathBuf::from("doc.strop.tmp")
        );
    }

    #[test]
    fn atomic_save_does_not_recreate_a_vanished_parent() {
        let dir = temp_path("vanished-parent");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("document.strop");
        fs::remove_dir(&dir).unwrap();

        assert!(atomic_write(&path, b"snapshot").is_err());
        assert!(!dir.exists());
        assert!(!path.exists());
        assert!(!sidecar_path(&path, ".tmp").exists());
    }

    #[test]
    fn save_worker_preserves_request_order() {
        let path = temp_path("worker-order");
        let mut worker = SaveWorker::new();
        let first = worker.request(PreparedSave {
            path: path.clone(),
            bytes: b"older".to_vec(),
        });
        let latest = worker.request(PreparedSave {
            path: path.clone(),
            bytes: b"newer".to_vec(),
        });
        assert!(first < latest);
        worker.wait_for(latest).result.unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"newer");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn failed_write_does_not_stop_later_generations() {
        let blocker = temp_path("worker-fault-parent");
        fs::write(&blocker, b"not a directory").unwrap();
        let good = temp_path("worker-after-fault");
        let mut worker = SaveWorker::new();
        let bad_generation = worker.request(PreparedSave {
            path: blocker.join("document.strop"),
            bytes: b"cannot land".to_vec(),
        });
        let good_generation = worker.request(PreparedSave {
            path: good.clone(),
            bytes: b"survived".to_vec(),
        });
        let bad = worker.completions.recv().unwrap();
        assert_eq!(bad.generation, bad_generation);
        assert!(bad.result.is_err());
        worker.wait_for(good_generation).result.unwrap();
        assert_eq!(fs::read(&good).unwrap(), b"survived");
        let _ = fs::remove_file(blocker);
        let _ = fs::remove_file(good);
    }

    #[cfg(windows)]
    #[test]
    fn windows_atomic_replace_accepts_unicode_paths() {
        let path = temp_path("atomic-作品");
        fs::write(&path, b"before").unwrap();
        atomic_write(&path, b"after").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"after");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn checkpoint_millisecond_timestamp_round_trips_with_legacy_fallback() {
        let legacy: Checkpoint = serde_json::from_str(
            r#"{"name":"old","created_unix":7,"frontiers":[],"manual":false}"#,
        )
        .unwrap();
        assert_eq!(legacy.timestamp_ms(), 7_000);

        let exact = Checkpoint {
            name: "new".into(),
            created_unix: 7,
            created_ms: Some(7_654),
            frontiers: Vec::new(),
            manual: false,
            state: None,
        };
        let back: Checkpoint = serde_json::from_str(&serde_json::to_string(&exact).unwrap()).unwrap();
        assert_eq!(back.timestamp_ms(), 7_654);
    }

    #[test]
    fn saves_stamp_the_schema_and_future_versions_fail_closed() {
        let path = temp_path("schema-version");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();
        store.seed("versioned");
        store.save().unwrap();
        let (reopened, loaded) = Store::open(&path).unwrap();
        assert_eq!(loaded.unwrap().text, "versioned");
        assert_eq!(schema_version_of(&reopened.doc).unwrap(), CURRENT_SCHEMA_VERSION);

        let future = LoroDoc::new();
        future
            .get_map(META_CONTAINER)
            .insert(SCHEMA_VERSION_KEY, CURRENT_SCHEMA_VERSION as i64 + 1)
            .unwrap();
        future.commit();
        fs::write(&path, future.export(ExportMode::Snapshot).unwrap()).unwrap();
        let err = Store::open(&path).err().expect("future schema is refused");
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
        assert!(err.to_string().contains("uses schema"));
        let _ = fs::remove_file(path);
    }

    /// Simulate a LEGACY file: strip the materialized state off every
    /// checkpoint record, leaving frontier-only checkpoints (what older
    /// builds wrote). Test-only — production states are immutable.
    fn strip_checkpoint_states(store: &Store) {
        let list = store.doc.get_list(CHECKPOINTS_CONTAINER);
        for ix in 0..list.len() {
            let Some(v) = list.get(ix) else { continue };
            let Ok(LoroValue::String(json)) = v.into_value() else {
                continue;
            };
            let Ok(mut cp) = serde_json::from_str::<Checkpoint>(&json) else {
                continue;
            };
            cp.state = None;
            let _ = list.delete(ix, 1);
            let _ = list.insert(ix, serde_json::to_string(&cp).unwrap());
        }
        store.doc.commit();
    }

    /// The oplog-bloat class: a save whose annotations/blocks/history/marks
    /// did not change must append NOTHING (in an append-only CRDT every
    /// rewrite is permanent growth — the 4.8 MB-file bug). And the guards
    /// must seed from the OPENED file, so a fresh session's first unchanged
    /// save is also a no-op. A real change still writes.
    #[test]
    fn unchanged_saves_append_nothing() {
        let path = temp_path("save-guard");
        let _ = fs::remove_file(&path);

        let (store, _) = Store::open(&path).unwrap();
        store.seed("текст с примечанием");
        let spans = SpanSet::default();
        let blocks = BlockMap::from_kinds(vec![BlockKind::Paragraph]);
        let history = History::default();
        let mut notes = Annotations::default();
        notes.add(0..5, "заметка".into(), 0);

        store.save_with_state(&spans, &blocks, &history, &notes, &Journal::default(), &Graveyard::default(), &Provenance::default()).unwrap();
        let first = fs::metadata(&path).unwrap().len();
        for _ in 0..5 {
            store.save_with_state(&spans, &blocks, &history, &notes, &Journal::default(), &Graveyard::default(), &Provenance::default()).unwrap();
        }
        let after_idle = fs::metadata(&path).unwrap().len();
        assert_eq!(after_idle, first, "unchanged saves must not grow the file");

        // Guards persist across a reopen: the next session's first
        // unchanged save is also a no-op.
        drop(store);
        let (store, loaded) = Store::open(&path).unwrap();
        let loaded = loaded.unwrap();
        store
            .save_with_state(
                &loaded.spans,
                &loaded.blocks,
                &loaded.history.clone().unwrap_or_default(),
                &loaded.annotations,
                &Journal::default(),
                &Graveyard::default(),
                &Provenance::default(),
            )
            .unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().len(),
            after_idle,
            "reopened session's unchanged save must not grow the file"
        );

        // A real change still writes (and is readable after reopen).
        let mut notes2 = loaded.annotations.clone();
        notes2.add(6..7, "ещё".into(), 1);
        store
            .save_with_state(
                &loaded.spans,
                &loaded.blocks,
                &loaded.history.unwrap_or_default(),
                &notes2,
                &Journal::default(),
                &Graveyard::default(),
                &Provenance::default(),
            )
            .unwrap();
        let (_, reloaded) = Store::open(&path).unwrap();
        assert_eq!(reloaded.unwrap().annotations.open().count(), 2);

        let _ = fs::remove_file(&path);
    }

    /// The journal roundtrips: settled runs and events survive reopen in
    /// order, an unchanged journal appends nothing (the containers are
    /// strictly append-only — same bloat class as the guards above), and a
    /// new tail appends only itself.
    #[test]
    fn journal_persists_appends_only_the_tail() {
        use crate::journal::{EditRun, JournalEvent};
        let path = temp_path("journal");
        let _ = fs::remove_file(&path);

        let (store, _) = Store::open(&path).unwrap();
        store.seed("ночной паром");
        let spans = SpanSet::default();
        let blocks = BlockMap::from_kinds(vec![BlockKind::Paragraph]);
        let history = History::default();
        let notes = Annotations::default();

        let mut journal = Journal::default();
        journal.record(
            &TextOp {
                pos: 0,
                delete: 0,
                insert: "паром".into(),
            },
            1_000,
        );
        journal.settle();
        journal.record_event(JournalEvent::Pass {
            t: 2_000,
            mode: "developmental".into(),
            cards: 3,
        });
        store
            .save_with_state(&spans, &blocks, &history, &notes, &journal, &Graveyard::default(), &Provenance::default())
            .unwrap();
        let first = fs::metadata(&path).unwrap().len();

        // Unchanged journal: append nothing.
        for _ in 0..4 {
            store
                .save_with_state(&spans, &blocks, &history, &notes, &journal, &Graveyard::default(), &Provenance::default())
                .unwrap();
        }
        assert_eq!(
            fs::metadata(&path).unwrap().len(),
            first,
            "an unchanged journal must not grow the file"
        );

        // Reopen: the journal survives, in order, tail closed.
        drop(store);
        let (store, loaded) = Store::open(&path).unwrap();
        let mut journal = loaded.unwrap().journal;
        assert_eq!(journal.runs.len(), 1);
        assert_eq!(journal.runs[0].ins, "паром");
        assert_eq!(journal.events.len(), 1);

        // A new tail appends only itself (counters seeded from the file).
        journal.runs.push(EditRun {
            t0: 3_000,
            t1: 3_100,
            pos: 5,
            del_chars: 0,
            del_words: None,
            ins: " идёт".into(),
        });
        store
            .save_with_state(&spans, &blocks, &history, &notes, &journal, &Graveyard::default(), &Provenance::default())
            .unwrap();
        let (_, reloaded) = Store::open(&path).unwrap();
        let journal = reloaded.unwrap().journal;
        assert_eq!(journal.runs.len(), 2, "the tail landed once, not twice");
        assert_eq!(journal.runs[1].ins, " идёт");

        let _ = fs::remove_file(&path);
    }

    /// The wave-1 review's biggest catch: checkpoint states are materialized
    /// through `read_state_of`, which used to build boundary-None BlockMaps —
    /// so ANY restore silently merged the compost into the manuscript
    /// (exported, counted, AI-scoped) and the next save persisted the loss.
    /// The boundary must ride every materialized state.
    #[test]
    fn checkpoint_states_carry_the_aside_boundary() {
        let path = temp_path("boundary-cp");
        let _ = fs::remove_file(&path);

        let (store, _) = Store::open(&path).unwrap();
        store.seed("compost line

manuscript opens here");
        let spans = SpanSet::default();
        let mut blocks =
            BlockMap::from_kinds(vec![BlockKind::Paragraph; 3]);
        blocks.set_aside_boundary(Some(1));
        store
            .save_with_state(
                &spans,
                &blocks,
                &History::default(),
                &Annotations::default(),
                &Journal::default(),
                &Graveyard::default(),
                &Provenance::default(),
            )
            .unwrap();

        store.add_checkpoint("with rail", true);
        let cp = store.checkpoints().pop().unwrap();
        let (_, _, cp_blocks) = store.checkpoint_state(&cp).unwrap();
        assert_eq!(
            cp_blocks.aside_boundary(),
            Some(1),
            "a restored version must reproduce its rail"
        );

        let _ = fs::remove_file(&path);
    }

    /// The NAMED regression fixture (08 §2; adjudications "the foundation"):
    /// a legacy top-boundary file with a sealed old checkpoint. After the
    /// migration flips the live doc, (a) the live state reads back TAIL-era,
    /// (b) the legacy `boundary` key is written as null so a top-era build
    /// degrades to no-boundary instead of misreading, and (c) the sealed old
    /// checkpoint keeps its OWN era — states are immutable once recorded,
    /// and restoring one goes through normalization, never a misread.
    #[test]
    fn legacy_top_boundary_file_migrates_and_old_checkpoint_keeps_its_era() {
        use crate::document::{BoundaryEra, Document};
        let path = temp_path("era-migration");
        let _ = fs::remove_file(&path);

        // The legacy file: compost at the TOP, one sealed checkpoint.
        let (store, _) = Store::open(&path).unwrap();
        store.seed("pile line\n\nmanuscript body");
        let mut top = BlockMap::from_kinds(vec![BlockKind::Paragraph; 3]);
        top.set_aside_boundary(Some(1));
        store
            .save_with_state(
                &SpanSet::default(),
                &top,
                &History::default(),
                &Annotations::default(),
                &Journal::default(),
                &Graveyard::default(),
                &Provenance::default(),
            )
            .unwrap();
        store.add_checkpoint("sealed old", true);
        store.save().unwrap();
        drop(store);

        // Reopen: the file decodes top-era; the migration flips the live doc
        // (the editor drives this at open; here the same mechanics run bare).
        let (store, loaded) = Store::open(&path).unwrap();
        let loaded = loaded.unwrap();
        assert_eq!(loaded.blocks.boundary(), Some((BoundaryEra::Top, 1)));
        let mut doc = Document::new(&loaded.text, loaded.spans.clone(), loaded.blocks.clone());
        assert!(doc.migrate_top_to_tail());
        store.apply(&doc.take_ops());
        let mut journal = doc.journal().clone();
        journal.settle();
        store
            .save_with_state(
                doc.spans(),
                doc.blocks(),
                &doc.export_history(100),
                doc.notes(),
                &journal,
                doc.graveyard(),
                doc.provenance(),
            )
            .unwrap();

        // (a) The live doc reads back tail-era, text flipped.
        let (store2, reloaded) = Store::open(&path).unwrap();
        let reloaded = reloaded.unwrap();
        assert_eq!(reloaded.text, "manuscript body\n\npile line");
        assert_eq!(
            reloaded.blocks.boundary().map(|(e, _)| e),
            Some(BoundaryEra::Tail)
        );
        // (b) The legacy key holds null: a top-era build reads no boundary —
        // the documented degrade path, never an inverted region.
        let legacy = store2
            .doc
            .get_map(BLOCKS_CONTAINER)
            .get("boundary")
            .and_then(|v| v.into_value().ok());
        assert!(
            matches!(legacy, Some(LoroValue::String(ref s)) if s.as_str() == "null"),
            "migrated saves write boundary: null, got {legacy:?}"
        );
        // (c) The sealed checkpoint keeps its own era, byte-identical.
        let cp = &store2.checkpoints()[0];
        assert_eq!(cp.name, "sealed old");
        let (cp_text, _, cp_blocks) = store2.checkpoint_state(cp).unwrap();
        assert_eq!(cp_text, "pile line\n\nmanuscript body");
        assert_eq!(
            cp_blocks.boundary(),
            Some((BoundaryEra::Top, 1)),
            "historical checkpoints keep their own era"
        );

        let _ = fs::remove_file(&path);
    }

    /// The compaction class: a bloated oplog shrinks at open once every
    /// checkpoint is self-contained — and nothing readable is lost: text,
    /// annotations, checkpoint states and the block kinds survive, and a
    /// one-time .bak keeps the original bytes.
    #[test]
    fn bloated_file_compacts_at_open_without_losing_state() {
        let path = temp_path("compact");
        let bak = sidecar_path(&path, ".pre-compact.bak");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&bak);

        let (store, _) = Store::open(&path).unwrap();
        store.seed("текст, который переживёт уплотнение");
        store.add_checkpoint("v1", true);
        // Inflate the oplog the way real sessions did — a fat JSON value
        // rewritten with different content each save. Hash-chained hex so
        // Loro's compression can't flatten the fixture.
        let spans = SpanSet::default();
        let blocks = BlockMap::from_kinds(vec![BlockKind::Paragraph]);
        let history = History::default();
        for i in 0..24u8 {
            let mut body = String::new();
            let mut h = blake3::hash(&[i]);
            for _ in 0..600 {
                body.push_str(&h.to_hex());
                h = blake3::hash(h.as_bytes());
            }
            let mut notes = Annotations::default();
            notes.add(0..5, body, 0);
            store.save_with_state(&spans, &blocks, &history, &notes, &Journal::default(), &Graveyard::default(), &Provenance::default()).unwrap();
        }
        let mut notes = Annotations::default();
        notes.add(0..5, "финальная заметка".into(), 0);
        store.save_with_state(&spans, &blocks, &history, &notes, &Journal::default(), &Graveyard::default(), &Provenance::default()).unwrap();
        let bloated = fs::metadata(&path).unwrap().len() as usize;
        assert!(bloated > COMPACT_MIN_BYTES, "fixture must be bloated (got {bloated})");
        drop(store);

        let (store2, loaded) = Store::open(&path).unwrap();
        let compacted = fs::metadata(&path).unwrap().len() as usize;
        assert!(
            compacted * 2 < bloated,
            "open should compact ({bloated} → {compacted})"
        );
        assert!(bak.exists(), "original bytes kept once");
        let loaded = loaded.unwrap();
        assert_eq!(loaded.text, "текст, который переживёт уплотнение");
        assert_eq!(loaded.annotations.open().count(), 1);
        assert!(store2.checkpoints_materialized());
        assert_eq!(
            store2
                .checkpoint_state(&store2.checkpoints()[0])
                .unwrap()
                .0,
            "текст, который переживёт уплотнение"
        );

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&bak);
    }

    /// The history-sidebar hang class: checkpoints must be readable WITHOUT
    /// per-checkpoint historical checkouts. New checkpoints materialize their
    /// state at creation; a legacy (stripped) file backfills from snapshot
    /// bytes — off the live doc — and the write-back persists across reopen.
    #[test]
    fn checkpoint_states_materialize_and_backfill() {
        let path = temp_path("materialize");
        let _ = fs::remove_file(&path);

        let (store, _) = Store::open(&path).unwrap();
        store.seed("первая версия");
        store.add_checkpoint("v1", true);
        store.apply(&[crate::buffer::TextOp {
            pos: 0,
            delete: 0,
            insert: "правка: ".into(),
        }]);
        store.add_checkpoint("v2", true);

        // New checkpoints carry their state from birth.
        assert!(store.checkpoints_materialized());
        let cps = store.checkpoints();
        assert_eq!(cps[0].state.as_ref().unwrap().text, "первая версия");
        assert_eq!(cps[1].state.as_ref().unwrap().text, "правка: первая версия");

        // A legacy file (no states) backfills from snapshot bytes alone…
        strip_checkpoint_states(&store);
        assert!(!store.checkpoints_materialized());
        let bytes = store.export_bytes().unwrap();
        let states = Store::materialize_checkpoint_states(&bytes);
        assert_eq!(states.len(), 2);
        for (ix, state) in states {
            store.set_checkpoint_state(ix, state);
        }
        assert!(store.checkpoints_materialized());
        let cps = store.checkpoints();
        assert_eq!(cps[0].state.as_ref().unwrap().text, "первая версия");
        assert_eq!(cps[1].state.as_ref().unwrap().text, "правка: первая версия");

        // …and the write-back persists: reopening still needs no checkout.
        store.save().unwrap();
        let (store2, _) = Store::open(&path).unwrap();
        assert!(store2.checkpoints_materialized());
        assert_eq!(
            store2
                .checkpoint_state(&store2.checkpoints()[0])
                .unwrap()
                .0,
            "первая версия"
        );

        let _ = fs::remove_file(&path);
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
        let ops: Vec<TextOp> = buf.take_ops().into_iter().map(|(op, _)| op).collect();
        store.apply(&ops);
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
                &Journal::default(),
                &Graveyard::default(),
                &Provenance::default(),
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
            },
            BlockKind::Paragraph,
        ]);
        store
            .save_with_state(
                &SpanSet::default(),
                &blocks,
                &History::default(),
                &Annotations::default(),
                &Journal::default(),
                &Graveyard::default(),
                &Provenance::default(),
            )
            .unwrap();
        assert!(store.get_asset(&kept_id).is_some(), "referenced asset kept");
        assert!(store.get_asset(&orphan_id).is_none(), "orphan collected");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn asset_gc_gate_keeps_checkpoint_referenced_asset() {
        use crate::document::BlockKind;
        // The save-time GC gate short-circuits (skips the costly per-checkpoint
        // `state_at` scan) only while EVERY stored asset is still referenced by
        // the live doc or undo history. The data-loss regression it must never
        // cause: once an image is deleted from the live doc, the gate has to fall
        // THROUGH to the checkpoint scan and KEEP an asset some checkpoint still
        // references (deleting it would corrupt that rewind) — while still
        // dropping a genuine orphan referenced by nothing. This guards both arms
        // of the fall-through, the half the keeps-reachable test never reaches.
        let path = temp_path("gc-gate");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();
        store.seed("картинка\n");
        let cp_only = store.put_asset(vec![1, 2, 3], "png");
        // Save with the image live, then checkpoint THAT state (so the only thing
        // referencing the asset, after the next edit, is this checkpoint).
        let live = BlockMap::from_kinds(vec![
            BlockKind::Image {
                src: cp_only.clone(),
                alt: String::new(),
            },
            BlockKind::Paragraph,
        ]);
        store
            .save_with_state(
                &SpanSet::default(),
                &live,
                &History::default(),
                &Annotations::default(),
                &Journal::default(),
                &Graveyard::default(),
                &Provenance::default(),
            )
            .unwrap();
        store.add_checkpoint("with image", true);
        // Delete the image from the LIVE doc and add a brand-new orphan; save with
        // EMPTY history, so `cp_only` is reachable ONLY via the checkpoint. The
        // gate must NOT short-circuit (an asset is missing from the live set), so
        // it scans the checkpoint, keeps `cp_only`, and reclaims `orphan`.
        let orphan = store.put_asset(vec![9, 9, 9], "png");
        let no_image = BlockMap::from_kinds(vec![BlockKind::Paragraph, BlockKind::Paragraph]);
        store
            .save_with_state(
                &SpanSet::default(),
                &no_image,
                &History::default(),
                &Annotations::default(),
                &Journal::default(),
                &Graveyard::default(),
                &Provenance::default(),
            )
            .unwrap();
        assert!(
            store.get_asset(&cp_only).is_some(),
            "checkpoint-referenced asset must survive the live delete (rewind integrity)"
        );
        assert!(
            store.get_asset(&orphan).is_none(),
            "a true orphan is still collected through the gate's fall-through"
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn block_kind_metadata_with_newline_survives_roundtrip() {
        use crate::document::BlockKind;
        // A '\n' inside CodeBlock.info used to become a token boundary,
        // desyncing the kind count and collapsing the BlockMap on reopen.
        let path = temp_path("kind-newline");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();
        store.seed("first\nsecond"); // two rope lines => two blocks
        let blocks = BlockMap::from_kinds(vec![
            BlockKind::CodeBlock { info: "ru\nst".into() },
            BlockKind::Heading(2),
        ]);
        store
            .save_with_state(
                &SpanSet::default(),
                &blocks,
                &History::default(),
                &Annotations::default(),
                &Journal::default(),
                &Graveyard::default(),
                &Provenance::default(),
            )
            .unwrap();
        let (_s2, loaded) = Store::open(&path).unwrap();
        let loaded = loaded.unwrap();
        assert_eq!(loaded.blocks.len(), 2, "kind count must match rope lines");
        assert_eq!(
            loaded.blocks.kinds()[0],
            BlockKind::CodeBlock { info: "ru\nst".into() },
            "metadata field must round-trip intact"
        );
        assert_eq!(loaded.blocks.kinds()[1], BlockKind::Heading(2));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn image_alt_survives_roundtrip() {
        use crate::document::BlockKind;
        // The token format dropped alt; JSON persistence keeps the
        // author-entered alt (a shipped editor writes it). The caption is
        // the block's own LINE now (docs/inline-images.md §10) — its
        // persistence is the text container's, not the kind's.
        let path = temp_path("img-alt");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();
        store.seed("img");
        let blocks = BlockMap::from_kinds(vec![BlockKind::Image {
            src: "asset:abc.png".into(),
            alt: "a kitten on a mat".into(),
        }]);
        store
            .save_with_state(
                &SpanSet::default(),
                &blocks,
                &History::default(),
                &Annotations::default(),
                &Journal::default(),
                &Graveyard::default(),
                &Provenance::default(),
            )
            .unwrap();
        let (_s2, loaded) = Store::open(&path).unwrap();
        assert_eq!(
            loaded.unwrap().blocks.kinds()[0],
            BlockKind::Image {
                src: "asset:abc.png".into(),
                alt: "a kitten on a mat".into(),
            }
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn legacy_token_block_kinds_still_load() {
        use crate::document::BlockKind;
        // Backward compat: the legacy newline-joined token format must still
        // parse via the read_state fallback.
        assert_eq!(kind_from_token("p"), BlockKind::Paragraph);
        assert_eq!(kind_from_token("h2"), BlockKind::Heading(2));
        assert_eq!(
            kind_from_token("img:asset:abc.png"),
            BlockKind::Image {
                src: "asset:abc.png".into(),
                alt: String::new(),
            }
        );
        assert_eq!(
            kind_from_token("code:rust"),
            BlockKind::CodeBlock { info: "rust".into() }
        );
        // Unknown tokens degrade to Paragraph (never panic).
        assert_eq!(kind_from_token("???"), BlockKind::Paragraph);
    }

    /// A pre-migration file, exactly as a released build saved it: raw
    /// old-shape kinds JSON in the blocks container (the runtime enum can
    /// no longer express a non-empty caption, which is the point).
    fn write_legacy_kinds(store: &Store, kinds_json: &str) {
        store
            .doc
            .get_map(BLOCKS_CONTAINER)
            .insert("kinds", kinds_json)
            .unwrap();
        store.doc.commit();
        store.save().unwrap();
    }

    #[test]
    fn legacy_caption_migrates_into_the_line_once() {
        use crate::document::BlockKind;
        // docs/inline-images.md §10: a non-empty stored caption on a block
        // whose line is empty moves INTO the line at open — one-way, and
        // the vestigial wire value is cleared so an emptied caption line
        // can never resurrect it on a later open.
        let path = temp_path("cap-mig");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();
        store.seed("before\n\nafter");
        // Persist a history so the migration's drop-the-stacks rule shows.
        store
            .save_with_state(
                &SpanSet::default(),
                &BlockMap::new(3),
                &History::default(),
                &Annotations::default(),
                &Journal::default(),
                &Graveyard::default(),
                &Provenance::default(),
            )
            .unwrap();
        write_legacy_kinds(
            &store,
            r#"["Paragraph",{"Image":{"src":"asset:x.png","alt":"","caption":"fig 1"}},"Paragraph"]"#,
        );
        let (s2, loaded) = Store::open(&path).unwrap();
        let loaded = loaded.unwrap();
        assert_eq!(loaded.text, "before\nfig 1\nafter", "the caption is the line now");
        assert_eq!(
            loaded.blocks.kinds()[1],
            BlockKind::Image { src: "asset:x.png".into(), alt: String::new() }
        );
        assert!(
            loaded.history.is_none(),
            "persisted undo stacks predate the inserted text — dropped"
        );
        // Persist the migrated state, reopen: once means once.
        s2.save_with_state(
            &loaded.spans,
            &loaded.blocks,
            &History::default(),
            &Annotations::default(),
            &loaded.journal,
            &loaded.graveyard,
            &loaded.provenance,
        )
        .unwrap();
        let (s3, reloaded) = Store::open(&path).unwrap();
        assert_eq!(reloaded.unwrap().text, "before\nfig 1\nafter", "no double insert");
        let raw = match s3.doc.get_map(BLOCKS_CONTAINER).get("kinds").unwrap().into_value() {
            Ok(LoroValue::String(j)) => j.to_string(),
            _ => panic!("kinds must be a string"),
        };
        assert!(
            !raw.contains("fig 1") && raw.contains(r#""caption":"""#),
            "the wire keeps the key but the migrated value is cleared: {raw}"
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn caption_migration_survives_brackets_quotes_newlines() {
        // The store.rs metadata-robustness precedent, extended: the exact
        // shapes the token format used to corrupt must migrate intact —
        // except line breaks, which flatten to spaces (any of ropey's
        // break set would split the image block and desync the map).
        let path = temp_path("cap-torture");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();
        store.seed("before\n\nafter");
        let cap = "fig ]1 \"q\" *s*\r\nsecond\nthird\u{2028}soft";
        let legacy = serde_json::json!([
            "Paragraph",
            {"Image": {"src": "asset:x.png", "alt": "a]b", "caption": cap}},
            "Paragraph"
        ])
        .to_string();
        write_legacy_kinds(&store, &legacy);
        let (_s2, loaded) = Store::open(&path).unwrap();
        assert_eq!(
            loaded.unwrap().text,
            "before\nfig ]1 \"q\" *s* second third soft\nafter"
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn caption_migration_shifts_spans_and_notes_past_the_insertion() {
        use crate::document::InlineAttr;
        // The inserted caption is a real text edit: every persisted anchor
        // family after it must shift, or formatting and margin notes land
        // on the wrong words (and would persist wrong on the next save).
        let path = temp_path("cap-shift");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();
        store.seed("before\n\nafter");
        let mut spans = SpanSet::default();
        spans.add(8..13, InlineAttr::Strong); // "after"
        store
            .doc
            .get_map(BLOCKS_CONTAINER)
            .insert("spans", serde_json::to_string(&spans).unwrap())
            .unwrap();
        let mut notes = Annotations::default();
        notes.add(8..13, "on after".into(), 0);
        store
            .doc
            .get_map(ANNOTATIONS_CONTAINER)
            .insert("list", serde_json::to_string(&notes).unwrap())
            .unwrap();
        write_legacy_kinds(
            &store,
            r#"["Paragraph",{"Image":{"src":"asset:x.png","alt":"","caption":"fig 1"}},"Paragraph"]"#,
        );
        let (_s2, loaded) = Store::open(&path).unwrap();
        let loaded = loaded.unwrap();
        assert_eq!(loaded.text, "before\nfig 1\nafter");
        assert!(
            loaded.spans.covers(13..18, &InlineAttr::Strong),
            "the span still covers \"after\": {:?}",
            loaded.spans.spans()
        );
        assert_eq!(loaded.annotations.notes()[0].range, 13..18);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn caption_migration_resolves_blocks_by_the_rope_metric_not_lf_count() {
        use crate::document::BlockKind;
        // Block indices are ropey lines — CR/VT/FF/NEL/U+2028/2029 all
        // split (pre_edit_info's own doc comment) — so a legacy file with
        // any non-LF separator ABOVE the picture must still land the
        // caption on the picture's own line. The hand-rolled '\n' counter
        // drifted one line down per earlier non-LF break: onto a full line
        // (silent skip, the caption never migrates) or onto the empty
        // paragraph BELOW the picture (permanent misplacement — the
        // one-way migration clears the wire value).
        let path = temp_path("cap-u2028");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();
        store.seed("be\u{2028}fore\n\nafter");
        write_legacy_kinds(
            &store,
            r#"["Paragraph","Paragraph",{"Image":{"src":"asset:x.png","alt":"","caption":"fig 1"}},"Paragraph"]"#,
        );
        let (_s2, loaded) = Store::open(&path).unwrap();
        let loaded = loaded.unwrap();
        assert_eq!(
            loaded.text, "be\u{2028}fore\nfig 1\nafter",
            "the caption lands on the picture's own line"
        );
        assert_eq!(
            loaded.blocks.kinds()[2],
            BlockKind::Image { src: "asset:x.png".into(), alt: String::new() }
        );
        let _ = fs::remove_file(&path);
        // The classic-Mac paste shape: a lone CR above, and TWO empty
        // lines — the drifted counter used to pass the empty-line guard on
        // the wrong one and caption the paragraph below the picture.
        let path = temp_path("cap-cr");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();
        store.seed("A\rB\n\n\nafter");
        write_legacy_kinds(
            &store,
            r#"["Paragraph","Paragraph",{"Image":{"src":"asset:x.png","alt":"","caption":"fig 1"}},"Paragraph","Paragraph"]"#,
        );
        let (_s2, loaded) = Store::open(&path).unwrap();
        assert_eq!(
            loaded.unwrap().text,
            "A\rB\nfig 1\n\nafter",
            "the caption must not slide onto the paragraph below"
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn caption_migration_skips_a_line_that_already_has_words() {
        use crate::document::BlockKind;
        // Spec: only "a non-empty stored caption with an empty block line"
        // migrates. Words already on the line (a wrecked file) win — the
        // migration never overwrites or appends to the writer's text.
        let path = temp_path("cap-skip");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();
        store.seed("before\nwords here\nafter");
        write_legacy_kinds(
            &store,
            r#"["Paragraph",{"Image":{"src":"asset:x.png","alt":"","caption":"fig 1"}},"Paragraph"]"#,
        );
        let (_s2, loaded) = Store::open(&path).unwrap();
        let loaded = loaded.unwrap();
        assert_eq!(loaded.text, "before\nwords here\nafter", "text untouched");
        assert_eq!(
            loaded.blocks.kinds()[1],
            BlockKind::Image { src: "asset:x.png".into(), alt: String::new() }
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn caption_migration_never_touches_checkpoint_states() {
        // Build plan, adjudicated pushback 3: checkpoint states are
        // read-only past. The live doc migrates; the stored state's text
        // gains nothing and its raw JSON keeps the legacy value verbatim.
        let path = temp_path("cap-ckpt");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();
        store.seed("before\n\nafter");
        let cp = serde_json::json!({
            "name": "old", "created_unix": 5, "frontiers": [], "manual": true,
            "state": {
                "text": "x\n\ny",
                "spans": {"spans": []},
                "blocks": {"kinds": [
                    "Paragraph",
                    {"Image": {"src": "asset:c.png", "alt": "", "caption": "old cap"}},
                    "Paragraph"
                ]}
            }
        })
        .to_string();
        store.doc.get_list(CHECKPOINTS_CONTAINER).push(cp).unwrap();
        write_legacy_kinds(
            &store,
            r#"["Paragraph",{"Image":{"src":"asset:x.png","alt":"","caption":"live cap"}},"Paragraph"]"#,
        );
        let (s2, loaded) = Store::open(&path).unwrap();
        assert_eq!(loaded.unwrap().text, "before\nlive cap\nafter", "the live doc migrates");
        let cps = s2.checkpoints();
        assert_eq!(cps[0].state.as_ref().unwrap().text, "x\n\ny", "no injection into the past");
        let raw = match s2.doc.get_list(CHECKPOINTS_CONTAINER).get(0).unwrap().into_value() {
            Ok(LoroValue::String(j)) => j.to_string(),
            _ => panic!("checkpoint must be a string"),
        };
        assert!(raw.contains("old cap"), "the stored state keeps its legacy value verbatim");
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
            .save_with_state(&spans, &BlockMap::new(1), &History::default(), &Annotations::default(), &Journal::default(), &Graveyard::default(), &Provenance::default())
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
                &Journal::default(),
                &Graveyard::default(),
                &Provenance::default(),
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
    fn note_draft_persists_without_commit_and_skips_undo() {
        use crate::document::Document;
        // The keystroke-durability fix: an open composer's draft is mirrored
        // onto the note (set_note_body_draft) by the idle heartbeat, with no
        // Enter-commit. It must reach disk, and it must NOT push its own undo
        // state (or every keystroke would become a ctrl-z stop).
        let path = temp_path("note-draft");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();

        let mut doc = Document::new("", SpanSet::default(), BlockMap::default());
        doc.edit_bytes_coalescing(0..0, "body");
        let id = doc.add_note(0..4, String::new(), 0);
        store.apply(&doc.take_ops());
        doc.set_note_body_draft(id, "half-typed thought".into());
        store
            .save_with_state(doc.spans(), doc.blocks(), &doc.export_history(200), doc.notes(), &Journal::default(), &Graveyard::default(), &Provenance::default())
            .unwrap();

        // The uncommitted draft is on disk after reload.
        let (_store2, loaded) = Store::open(&path).unwrap();
        let loaded = loaded.unwrap();
        assert_eq!(loaded.annotations.notes().len(), 1);
        assert_eq!(loaded.annotations.notes()[0].body, "half-typed thought");

        // The draft pushed no undo state: undoing once reverts the add_note
        // (removing the note), not a body edit that would leave it behind.
        let mut d = Document::new("", SpanSet::default(), BlockMap::default());
        let nid = d.add_note(0..0, String::new(), 0);
        d.set_note_body_draft(nid, "draft".into());
        d.undo().unwrap();
        assert!(
            d.notes().notes().is_empty(),
            "the draft path must not push its own undo state"
        );

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
        let ops: Vec<TextOp> = buf.take_ops().into_iter().map(|(op, _)| op).collect();
        store.apply(&ops);
        assert_eq!(store.text(), "so…");
        assert_eq!(store.text(), buf.text());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn graveyard_and_boundary_persist_and_unchanged_asides_save_appends_nothing() {
        let path = temp_path("asides");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();
        store.seed("comp\n\nmanuscript"); // 3 rope lines
        let mut blocks = BlockMap::from_kinds(vec![
            BlockKind::Paragraph,
            BlockKind::Paragraph,
            BlockKind::Paragraph,
        ]);
        blocks.set_aside_boundary(Some(1));
        let spans = SpanSet::default();
        let history = History::default();
        let notes = Annotations::default();
        let mut graveyard = Graveyard::default();
        graveyard.file("a cut sentence".into(), "origin".into(), 6, 111, SpanSet::default(), Vec::new(), GraveRegion::Manuscript, false, false);

        store
            .save_with_state(&spans, &blocks, &history, &notes, &Journal::default(), &graveyard, &Provenance::default())
            .unwrap();
        let first = fs::metadata(&path).unwrap().len();
        for _ in 0..4 {
            store
                .save_with_state(&spans, &blocks, &history, &notes, &Journal::default(), &graveyard, &Provenance::default())
                .unwrap();
        }
        assert_eq!(
            fs::metadata(&path).unwrap().len(),
            first,
            "unchanged asides save must not grow the file"
        );

        // Reopen: the boundary and the graveyard both survive.
        drop(store);
        let (store2, loaded) = Store::open(&path).unwrap();
        let loaded = loaded.unwrap();
        assert_eq!(loaded.blocks.aside_boundary(), Some(1));
        assert_eq!(loaded.graveyard.len(), 1);
        assert_eq!(loaded.graveyard.entries()[0].text, "a cut sentence");
        assert_eq!(loaded.graveyard.entries()[0].origin_pos, 6);

        // The reopened session's first unchanged save is also a no-op (both
        // the graveyard channel and the blocks+boundary channel seeded).
        store2
            .save_with_state(
                &loaded.spans,
                &loaded.blocks,
                &loaded.history.clone().unwrap_or_default(),
                &loaded.annotations,
                &Journal::default(),
                &loaded.graveyard,
                &loaded.provenance,
            )
            .unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().len(),
            first,
            "reopened unchanged asides save must not grow the file"
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn legacy_file_without_boundary_key_loads_with_no_rail() {
        // An older build never wrote the out-of-band "boundary" key; it reads
        // only "kinds". Such a file must load with text and kinds intact and no
        // rail (review B13/H42) — compost simply folds into the manuscript.
        let path = temp_path("legacy-boundary");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();
        store.seed("head\n\nbody");
        let mut blocks = BlockMap::from_kinds(vec![
            BlockKind::Heading(1),
            BlockKind::Paragraph,
            BlockKind::Paragraph,
        ]);
        blocks.set_aside_boundary(Some(1));
        store
            .save_with_state(
                &SpanSet::default(),
                &blocks,
                &History::default(),
                &Annotations::default(),
                &Journal::default(),
                &Graveyard::default(),
                &Provenance::default(),
            )
            .unwrap();
        // Drop the boundary key to mimic a file an older build produced.
        let _ = store.doc.get_map(BLOCKS_CONTAINER).delete("boundary");
        store.doc.commit();
        store.save().unwrap();
        drop(store);

        let (_s2, loaded) = Store::open(&path).unwrap();
        let loaded = loaded.unwrap();
        assert_eq!(loaded.text, "head\n\nbody");
        assert_eq!(
            loaded.blocks.aside_boundary(),
            None,
            "a missing boundary key degrades to no rail"
        );
        assert_eq!(loaded.blocks.kinds()[0], BlockKind::Heading(1), "kinds still load");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn names_an_arbitrary_moment_without_moving_the_live_document() {
        let path = temp_path("named-arbitrary-moment");
        let _ = fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();
        store.seed("live");
        let state = CheckpointState {
            text: "past".into(),
            spans: SpanSet::default(),
            blocks: BlockMap::new(1),
        };
        store.add_checkpoint_at("Draft", 1_234_567, state.clone());
        store.add_checkpoint_at("Draft", 1_234_567, state);
        assert_eq!(store.read_state().0, "live");
        let checkpoints = store.checkpoints();
        assert_eq!(checkpoints.len(), 2, "duplicate names are allowed");
        assert!(checkpoints.iter().all(|c| c.manual && c.created_ms == Some(1_234_567)));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn needs_save_sees_seals_and_journal_tail_but_not_a_clean_open() {
        let path = temp_path("needs-save");
        let _ = fs::remove_file(&path);
        let (store, existing) = Store::open(&path).unwrap();
        assert!(existing.is_none());
        let empty = Journal::default();
        assert!(!store.needs_save(&empty), "an empty doc over a missing file has nothing to flush");
        store.seed("imported prose");
        assert!(store.needs_save(&empty), "seeding moved the doc past the baseline");
        store
            .save_with_state(
                &SpanSet::default(),
                &BlockMap::default(),
                &History::default(),
                &Annotations::default(),
                &empty,
                &Graveyard::default(),
                &Provenance::default(),
            )
            .unwrap();
        assert!(!store.needs_save(&empty), "a save re-baselines the frontiers");
        // A checkpoint seal never routes through the editor's dirty flag —
        // the frontier baseline is what keeps LAW 2 honest about it.
        store.add_checkpoint("Session start", false);
        assert!(store.needs_save(&empty), "a seal must reach disk before a close");
        drop(store);

        let (store, loaded) = Store::open(&path).unwrap();
        let loaded = loaded.unwrap();
        assert!(
            !store.needs_save(&loaded.journal),
            "a read-only reopen starts clean — glancing at a file must not rewrite it"
        );
        let tail = Journal::from_parts(
            vec![EditRun { t0: 1, t1: 2, pos: 0, del_chars: 0, del_words: None, ins: "x".into() }],
            Vec::new(),
        );
        assert!(store.needs_save(&tail), "an unpersisted journal tail counts as dirty");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn save_worker_wakes_its_owner_on_completion() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::{Duration, Instant};
        let path = temp_path("worker-notify");
        let notified = std::sync::Arc::new(AtomicUsize::new(0));
        let seen = notified.clone();
        let mut worker = SaveWorker::with_notify(move || {
            seen.fetch_add(1, Ordering::SeqCst);
        });
        let generation = worker.request(PreparedSave {
            path: path.clone(),
            bytes: b"woken".to_vec(),
        });
        worker.wait_for(generation).result.unwrap();
        // The completion send happens-before the notify on the worker
        // thread, but `wait_for` returns on the send — give the trailing
        // notify a bounded moment rather than assert a race.
        let deadline = Instant::now() + Duration::from_secs(5);
        while notified.load(Ordering::SeqCst) == 0 {
            assert!(Instant::now() < deadline, "completion never notified");
            std::thread::sleep(Duration::from_millis(1));
        }
        let _ = fs::remove_file(&path);
    }
}
