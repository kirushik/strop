mod support;

use std::fs;
use std::path::{Path, PathBuf};

use strop_core::buffer::TextOp;
use strop_core::document::{
    Annotations, BlockKind, BlockMap, GraveRegion, Graveyard, History, InlineAttr, Provenance,
    SpanSet,
};
use strop_core::journal::{EditRun, Journal, JournalEvent};
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
