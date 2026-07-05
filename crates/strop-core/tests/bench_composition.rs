//! Throwaway: what is the 4.8MB made of, and what does one save append?
//! STROP_BENCH_FILE=... cargo test -p strop-core --release --test bench_composition -- --nocapture

#[test]
fn bench_file_composition() {
    let Ok(path) = std::env::var("STROP_BENCH_FILE") else {
        eprintln!("bench_composition: STROP_BENCH_FILE not set; skipping");
        return;
    };
    let disk = std::fs::metadata(&path).unwrap().len();
    let (store, loaded) = strop_core::Store::open(&path).expect("open");
    let loaded = loaded.expect("existing file");
    eprintln!("disk file:        {:>9} bytes", disk);
    eprintln!("text:             {:>9} bytes", loaded.text.len());
    eprintln!("spans:            {:>9}", loaded.spans.spans().len());
    eprintln!("blocks json:      {:>9} bytes",
        serde_json::to_string(loaded.blocks.kinds()).unwrap().len());
    eprintln!("annotations json: {:>9} bytes ({} notes)",
        serde_json::to_string(&loaded.annotations).unwrap().len(),
        loaded.annotations.open().count());
    let hist_json = loaded.history.as_ref()
        .map(|h| serde_json::to_string(h).unwrap().len()).unwrap_or(0);
    eprintln!("history json:     {:>9} bytes", hist_json);
    eprintln!("checkpoints:      {:>9}", store.checkpoints().len());

    // What does one unchanged-state save append?
    let hist = loaded.history.clone().unwrap_or_default();
    for i in 0..3 {
        store
            .save_with_state(&loaded.spans, &loaded.blocks, &hist, &loaded.annotations, &strop_core::journal::Journal::default())
            .unwrap();
        let now = std::fs::metadata(&path).unwrap().len();
        eprintln!("after no-change save {i}: {:>9} bytes (+{})", now, now as i64 - disk as i64);
    }
}
