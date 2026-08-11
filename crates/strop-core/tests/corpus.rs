mod support;

use std::fs;
use std::path::{Path, PathBuf};

use strop_core::buffer::TextOp;
use strop_core::document::{
    Annotation, Annotations, BlockKind, BlockMap, GraveRegion, Graveyard, History, InlineAttr,
    NoteKind, NoteStatus, Provenance, SpanSet,
};
use strop_core::journal::{CardDisposition, CardRebaseEntry, EditRun, Journal, JournalEvent};
use strop_core::store::{CheckpointState, Store};

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

fn fixed_edit(store: &Store) {
    store.apply(&[TextOp {
        pos: store.text().chars().count(),
        delete: 0,
        insert: " Corpus edit.".into(),
    }]);
}

#[test]
fn released_corpus_preserves_semantics_and_editability() {
    let mut fixtures: Vec<_> = fs::read_dir(corpus_dir())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("strop"))
        .collect();
    fixtures.sort();
    assert!(!fixtures.is_empty(), "the released corpus must not be empty");
    for fixture in fixtures {
        let expected = fixture.with_extension("expected.json");
        let post_expected = fixture.with_extension("post-edit.expected.json");
        let scratch = std::env::temp_dir().join(format!(
            "strop-corpus-{}-{}",
            std::process::id(),
            fixture.file_name().unwrap().to_string_lossy()
        ));
        fs::copy(&fixture, &scratch).unwrap();
        let (store, loaded) = Store::open(&scratch).unwrap();
        let actual = serde_json::to_value(support::project(&store, loaded.unwrap())).unwrap();
        let wanted: serde_json::Value =
            serde_json::from_slice(&fs::read(expected).unwrap()).unwrap();
        assert_eq!(actual, wanted, "pre-edit projection for {}", fixture.display());
        fixed_edit(&store);
        store.save().unwrap();
        drop(store);
        let (store, loaded) = Store::open(&scratch).unwrap();
        let actual = serde_json::to_value(support::project(&store, loaded.unwrap())).unwrap();
        let wanted: serde_json::Value =
            serde_json::from_slice(&fs::read(post_expected).unwrap()).unwrap();
        assert_eq!(actual, wanted, "post-edit projection for {}", fixture.display());
        let _ = fs::remove_file(scratch);
    }
}

#[test]
#[ignore = "fixture writer: run deliberately before tagging a release"]
fn write_v0_2_0_fixture_deterministically() {
    let dir = corpus_dir();
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("v0.2.0.strop");
    let _ = fs::remove_file(&path);
    let (store, _) = Store::open(&path).unwrap();
    store.debug_set_peer_id(0x5354_524f_5002);
    let asset = store.put_asset(
        vec![
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
            0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
            0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
            0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41,
            0x54, 0x08, 0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
            0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99,
            0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
            0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ],
        "png",
    );
    let text = "Title\nQuoted\nList\nRule\nCode\nImage caption\nFootnote\nBody";
    store.seed(text);
    let mut spans = SpanSet::default();
    spans.add(0..5, InlineAttr::Strong);
    spans.add(6..12, InlineAttr::Emphasis);
    let blocks = BlockMap::from_kinds(vec![
        BlockKind::Heading(1),
        BlockKind::Blockquote,
        BlockKind::ListItem { ordered: true, depth: 1 },
        BlockKind::Divider,
        BlockKind::CodeBlock { info: "rust".into() },
        BlockKind::Image { src: asset.clone(), alt: "tiny".into() },
        BlockKind::FootnoteDef { id: "n1".into() },
        BlockKind::Paragraph,
    ]);
    let mut notes = Annotations::default();
    notes.add(0..5, "margin note".into(), 1_600_000_000);
    let journal = Journal::from_parts(
        vec![EditRun {
            t0: 1_600_000_000_000,
            t1: 1_600_000_000_250,
            pos: 0,
            del_chars: 0,
            del_words: Some(0),
            ins: "Title".into(),
        }],
        vec![JournalEvent::Export { t: 1_600_000_001_000 }],
    );
    let mut graveyard = Graveyard::default();
    graveyard.file(
        "cut image".into(),
        "from body".into(),
        3,
        1_600_000_002,
        SpanSet::default(),
        vec![BlockKind::Image { src: asset, alt: "grave".into() }],
        GraveRegion::Manuscript,
        true,
        false,
    );
    let mut provenance = Provenance::default();
    provenance.add(7..13, "from quoted".into(), 1_600_000_003);
    store
        .save_with_state(
            &spans,
            &blocks,
            &History::default(),
            &notes,
            &journal,
            &graveyard,
            &provenance,
        )
        .unwrap();
    store.add_checkpoint_at(
        "first",
        1_600_000_004_000,
        CheckpointState { text: text.into(), spans: spans.clone(), blocks: blocks.clone() },
    );
    store.add_checkpoint_at(
        "second",
        1_600_000_005_000,
        CheckpointState { text: text.into(), spans, blocks },
    );
    store.save().unwrap();
    drop(store);
    let pristine = fs::read(&path).unwrap();

    let (store, loaded) = Store::open(&path).unwrap();
    let before = support::project(&store, loaded.unwrap());
    fs::write(
        path.with_extension("expected.json"),
        serde_json::to_vec_pretty(&before).unwrap(),
    )
    .unwrap();
    fixed_edit(&store);
    store.save().unwrap();
    drop(store);
    let (store, loaded) = Store::open(&path).unwrap();
    let after = support::project(&store, loaded.unwrap());
    fs::write(
        path.with_extension("post-edit.expected.json"),
        serde_json::to_vec_pretty(&after).unwrap(),
    )
    .unwrap();
    fs::write(path, pristine).unwrap();
}

/// serde_json emits U+2028/U+2029 raw (valid JSON, but invisible in diffs
/// and prone to silent editor normalization); the fixture's hard break must
/// stay reviewable as an escape.
fn escape_line_separators(json: String) -> String {
    json.replace('\u{2028}', "\\u2028").replace('\u{2029}', "\\u2029")
}

/// Char range of `needle`'s first occurrence — span/anchor ranges are char
/// ranges, and the 0.3 fixture's second line is dense enough that
/// hand-counted offsets would be write-only.
fn char_range(text: &str, needle: &str) -> std::ops::Range<usize> {
    let byte = text.find(needle).unwrap();
    let start = text[..byte].chars().count();
    start..start + needle.chars().count()
}

/// The 0.3-minor fixture, frozen retroactively (2026-08) — the pre-tag
/// ritual of docs/releasing.md §6.3 was missed for v0.3.0/v0.3.1, and the
/// schema never moved off 1, so the bytes current code writes ARE what the
/// 0.3 line shipped. Beyond the v0.2.0 surface it freezes what the minor
/// added or first made durable: every inline attr, both list flavors, a
/// U+2028 hard break inside a paragraph, diagnosis cards (title/level/
/// pass_id/unverified/orphaned) beside writer notes, the full journal
/// event vocabulary plus a legacy `del_words: None` run, both graveyard
/// regions, and an asset kept alive by the graveyard alone.
#[test]
#[ignore = "fixture writer: run deliberately before tagging a release"]
fn write_v0_3_1_fixture_deterministically() {
    let dir = corpus_dir();
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("v0.3.1.strop");
    let _ = fs::remove_file(&path);
    let (store, _) = Store::open(&path).unwrap();
    store.debug_set_peer_id(0x5354_524f_5003);
    let figure = store.put_asset(
        vec![
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
            0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
            0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
            0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41,
            0x54, 0x08, 0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
            0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99,
            0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
            0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ],
        "png",
    );
    // Referenced by nothing but a graveyard entry — the GC gate must keep it.
    let grave_asset = store.put_asset(b"strop-corpus-0.3-grave-asset".to_vec(), "png");
    let text = "Corpus 0.3\n\
        Strong emph struck under mark code ref link\n\
        Quoted aside\n\
        First step\n\
        Nested bullet\n\
        Rule\n\
        fn main() {}\n\
        Figure caption\n\
        Footnote body\n\
        Hard\u{2028}break\n\
        Body tail";
    store.seed(text);
    let mut spans = SpanSet::default();
    spans.add(char_range(text, "Strong"), InlineAttr::Strong);
    spans.add(char_range(text, "emph"), InlineAttr::Emphasis);
    spans.add(char_range(text, "struck"), InlineAttr::Strikethrough);
    spans.add(char_range(text, "under"), InlineAttr::Underline);
    spans.add(char_range(text, "mark"), InlineAttr::Highlight);
    spans.add(char_range(text, "code"), InlineAttr::Code);
    spans.add(char_range(text, "ref"), InlineAttr::FootnoteRef("n1".into()));
    spans.add(char_range(text, "link"), InlineAttr::Link("https://example.org/corpus".into()));
    let blocks = BlockMap::from_kinds(vec![
        BlockKind::Heading(1),
        BlockKind::Paragraph,
        BlockKind::Blockquote,
        BlockKind::ListItem { ordered: true, depth: 1 },
        BlockKind::ListItem { ordered: false, depth: 2 },
        BlockKind::Divider,
        BlockKind::CodeBlock { info: "rust".into() },
        BlockKind::Image { src: figure, alt: "figure".into() },
        BlockKind::FootnoteDef { id: "n1".into() },
        BlockKind::Paragraph,
        BlockKind::Paragraph,
    ]);
    let mut notes = Annotations::default();
    notes.add(char_range(text, "Corpus 0.3"), "title could carry the era".into(), 1_690_000_010);
    notes.push(Annotation {
        id: 0,
        range: char_range(text, "Quoted aside"),
        body: "Quote leans on its source.".into(),
        status: NoteStatus::Open,
        created_unix: 1_690_000_020,
        kind: NoteKind::Diagnosis,
        title: "Loose quotation".into(),
        level: "line".into(),
        orphaned: false,
        pass_id: 2,
        unverified: true,
    });
    notes.push(Annotation {
        id: 0,
        range: char_range(text, "Hard\u{2028}break"),
        body: "hard break holds".into(),
        status: NoteStatus::Done,
        created_unix: 1_690_000_030,
        kind: NoteKind::Note,
        title: String::new(),
        level: String::new(),
        orphaned: false,
        pass_id: 0,
        unverified: false,
    });
    notes.push(Annotation {
        id: 0,
        range: char_range(text, "Nested bullet"),
        body: "does the list need depth?".into(),
        status: NoteStatus::Dismissed,
        created_unix: 1_690_000_040,
        kind: NoteKind::Diagnosis,
        title: "Deep nesting".into(),
        level: "developmental".into(),
        orphaned: true,
        pass_id: 1,
        unverified: false,
    });
    let journal = Journal::from_parts(
        vec![
            EditRun {
                t0: 1_690_000_000_000,
                t1: 1_690_000_000_400,
                pos: 0,
                del_chars: 0,
                del_words: Some(0),
                ins: "Corpus 0.3".into(),
            },
            // A legacy run: `del_words` was never recorded, and None must
            // survive round-trips distinguishable from exact zero.
            EditRun {
                t0: 1_690_000_010_000,
                t1: 1_690_000_010_500,
                pos: 11,
                del_chars: 4,
                del_words: None,
                ins: "Strong".into(),
            },
        ],
        vec![
            JournalEvent::Pass { t: 1_690_000_020_000, mode: "line".into(), cards: 2 },
            JournalEvent::CardRaised {
                t: 1_690_000_020_100,
                id: 2,
                card_kind: NoteKind::Diagnosis,
                range: char_range(text, "Quoted aside"),
                body: "Quote leans on its source.".into(),
                title: "Loose quotation".into(),
                level: "line".into(),
                pass_id: 2,
                status: NoteStatus::Open,
                orphaned: false,
                unverified: false,
            },
            JournalEvent::CardEdited {
                t: 1_690_000_021_000,
                id: 2,
                body: "Quote leans on its source.".into(),
                title: "Loose quotation".into(),
                level: "line".into(),
                pass_id: 2,
                status: NoteStatus::Open,
                orphaned: false,
                unverified: true,
            },
            JournalEvent::CardClosed { t: 1_690_000_022_000, id: 3, resolved: true },
            JournalEvent::Seam { t: 1_690_000_023_000, at: Some(9) },
            JournalEvent::Restore {
                t: 1_690_000_024_000,
                from_unix: 1_690_000_100,
                len_chars: text.chars().count(),
            },
            JournalEvent::CardsRebased {
                t: 1_690_000_024_500,
                entries: vec![
                    CardRebaseEntry {
                        id: 2,
                        range: char_range(text, "Quoted aside"),
                        status: NoteStatus::Open,
                        title: "Loose quotation".into(),
                        level: "line".into(),
                        pass_id: 2,
                        orphaned: false,
                        unverified: true,
                        disposition: CardDisposition::Anchored,
                    },
                    CardRebaseEntry {
                        id: 3,
                        range: char_range(text, "Hard\u{2028}break"),
                        status: NoteStatus::Done,
                        title: String::new(),
                        level: String::new(),
                        pass_id: 0,
                        orphaned: false,
                        unverified: false,
                        disposition: CardDisposition::Migrated,
                    },
                    CardRebaseEntry {
                        id: 4,
                        range: char_range(text, "Nested bullet"),
                        status: NoteStatus::Dismissed,
                        title: "Deep nesting".into(),
                        level: "developmental".into(),
                        pass_id: 1,
                        orphaned: true,
                        unverified: false,
                        disposition: CardDisposition::Orphaned,
                    },
                ],
            },
            JournalEvent::Seam { t: 1_690_000_025_000, at: None },
            JournalEvent::Export { t: 1_690_000_026_000 },
        ],
    );
    let mut graveyard = Graveyard::default();
    let mut grave_spans = SpanSet::default();
    grave_spans.add(0..3, InlineAttr::Strong);
    graveyard.file(
        "cut figure".into(),
        "after the rule".into(),
        char_range(text, "Figure caption").start,
        1_690_000_050,
        grave_spans,
        vec![BlockKind::Image { src: grave_asset, alt: "grave".into() }],
        GraveRegion::Manuscript,
        true,
        false,
    );
    graveyard.file(
        "compost line one\ncompost line two".into(),
        "from the tail".into(),
        char_range(text, "Body tail").start,
        1_690_000_060,
        SpanSet::default(),
        vec![BlockKind::Paragraph, BlockKind::Paragraph],
        GraveRegion::Scraps,
        false,
        true,
    );
    let mut provenance = Provenance::default();
    provenance.add(char_range(text, "Quoted aside"), "Quoted aside".into(), 1_690_000_070);
    provenance.add(char_range(text, "Footnote body"), "the note's source".into(), 1_690_000_080);
    store
        .save_with_state(
            &spans,
            &blocks,
            &History::default(),
            &notes,
            &journal,
            &graveyard,
            &provenance,
        )
        .unwrap();
    let mut seed_spans = SpanSet::default();
    seed_spans.add(0..6, InlineAttr::Strong);
    store.add_checkpoint_at(
        "seed",
        1_690_000_100_000,
        CheckpointState {
            text: "Corpus 0.3\nBody tail".into(),
            spans: seed_spans,
            blocks: BlockMap::from_kinds(vec![BlockKind::Heading(1), BlockKind::Paragraph]),
        },
    );
    store.add_checkpoint_at(
        "figure landed",
        1_690_000_200_000,
        CheckpointState { text: text.into(), spans, blocks },
    );
    store.save().unwrap();
    drop(store);
    let pristine = fs::read(&path).unwrap();

    let (store, loaded) = Store::open(&path).unwrap();
    let before = support::project(&store, loaded.unwrap());
    fs::write(
        path.with_extension("expected.json"),
        escape_line_separators(serde_json::to_string_pretty(&before).unwrap()),
    )
    .unwrap();
    fixed_edit(&store);
    store.save().unwrap();
    drop(store);
    let (store, loaded) = Store::open(&path).unwrap();
    let after = support::project(&store, loaded.unwrap());
    fs::write(
        path.with_extension("post-edit.expected.json"),
        escape_line_separators(serde_json::to_string_pretty(&after).unwrap()),
    )
    .unwrap();
    fs::write(path, pristine).unwrap();
}
