//! Throwaway measurement harness for the history-sidebar hang (2026-07-03).
//! Run against a real sidecar:
//!   STROP_BENCH_FILE="/path/to/doc.strop" cargo test -p strop-core --release --test bench_history -- --nocapture
//! Ignored without the env var so CI never depends on a private file.
//! Validates the NEW flow: one background backfill, then instant reads —
//! and previews the shallow-snapshot compaction win.

use std::time::Instant;

#[test]
fn bench_history_new_flow() {
    let Ok(path) = std::env::var("STROP_BENCH_FILE") else {
        eprintln!("bench_history: STROP_BENCH_FILE not set; skipping");
        return;
    };
    let t0 = Instant::now();
    let (store, loaded) = strop_core::Store::open(&path).expect("open sidecar");
    eprintln!("open: {:?}", t0.elapsed());
    let loaded = loaded.expect("existing file");
    eprintln!("materialized already: {}", store.checkpoints_materialized());

    // The one-time backfill (runs in the background in the app).
    let t = Instant::now();
    let bytes = store.export_bytes().unwrap();
    eprintln!("export_bytes: {:?} ({} bytes)", t.elapsed(), bytes.len());
    let t = Instant::now();
    let states = strop_core::Store::materialize_checkpoint_states(&bytes);
    eprintln!("backfill walk: {:?} for {} states", t.elapsed(), states.len());
    let t = Instant::now();
    for (ix, state) in states {
        store.set_checkpoint_state(ix, state);
    }
    eprintln!("write-back: {:?}", t.elapsed());

    // What the sidebar does on every open from now on.
    let t = Instant::now();
    let mut total_chars = 0usize;
    for cp in store.checkpoints() {
        let (text, _, _) = store.checkpoint_state(&cp).expect("state");
        total_chars += text.chars().count();
    }
    eprintln!(
        "enter_history reads (all {}): {:?} ({} chars total)",
        store.checkpoints().len(),
        t.elapsed(),
        total_chars
    );

    // Save the way the app does (save_with_state) — this also writes the
    // spans JSON, upgrading the file off the legacy marks read path.
    store
        .save_with_state(
            &loaded.spans,
            &loaded.blocks,
            &loaded.history.clone().unwrap_or_default(),
            &loaded.annotations,
                &strop_core::journal::Journal::default(),
                &loaded.graveyard,
                &loaded.provenance,
            )
        .unwrap();
    let after = std::fs::metadata(&path).unwrap().len();
    eprintln!("file after backfill+save: {} bytes", after);

    // Reopen: with states materialized, open may now COMPACT the oplog.
    let t = Instant::now();
    let (store2, _) = strop_core::Store::open(&path).expect("reopen");
    let open2 = t.elapsed();
    let t = Instant::now();
    for cp in store2.checkpoints() {
        store2.checkpoint_state(&cp).expect("state");
    }
    eprintln!("reopen: {:?}; reads: {:?}", open2, t.elapsed());
    eprintln!(
        "file after reopen: {} bytes",
        std::fs::metadata(&path).unwrap().len()
    );
    drop(store2);

    // Third open: the compacted file.
    let t = Instant::now();
    let (store3, loaded) = strop_core::Store::open(&path).expect("open compacted");
    eprintln!(
        "compacted open: {:?} (text {} bytes, {} checkpoints readable)",
        t.elapsed(),
        loaded.map(|l| l.text.len()).unwrap_or(0),
        store3
            .checkpoints()
            .iter()
            .filter(|cp| store3.checkpoint_state(cp).is_some())
            .count()
    );
}
