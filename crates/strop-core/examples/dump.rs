//! Debug dump of a .strop file: text, spans (with quoted slices), blocks.
//! `cargo run -p strop-core --example dump -- path/to/file.strop`

fn main() {
    let path = std::env::args().nth(1).expect("usage: dump <file.strop>");
    let (_store, loaded) = strop_core::Store::open(path).expect("open failed");
    let Some(loaded) = loaded else {
        println!("empty store (no document)");
        return;
    };
    let text = &loaded.text;
    println!("=== text: {} bytes, {} chars ===", text.len(), text.chars().count());
    for (i, line) in text.split('\n').enumerate() {
        println!("line {i}: {:?}", line.chars().take(60).collect::<String>());
    }
    println!("\n=== blocks ===");
    println!("{:?}", loaded.blocks);
    println!("\n=== spans ({}) ===", loaded.spans.spans().len());
    for s in loaded.spans.spans() {
        let as_bytes = text.get(s.range.clone()).map(|t| t.to_owned());
        let as_chars: String = text
            .chars()
            .skip(s.range.start)
            .take(s.range.end - s.range.start)
            .collect();
        println!(
            "{:?} {}..{}  byte-slice={:?}  char-slice={:?}",
            s.attr,
            s.range.start,
            s.range.end,
            as_bytes.unwrap_or_else(|| "NOT-UTF8-ALIGNED".into()),
            as_chars,
        );
    }
    // Overlap / ordering sanity: flag identical-attr overlaps and reversed ranges.
    let spans = loaded.spans.spans();
    for s in spans {
        if s.range.start >= s.range.end {
            println!("DEGENERATE span: {s:?}");
        }
    }
    for (i, a) in spans.iter().enumerate() {
        for b in &spans[i + 1..] {
            if a.attr == b.attr && a.range.start < b.range.end && b.range.start < a.range.end {
                println!("OVERLAP same-attr: {a:?} vs {b:?}");
            }
        }
    }
}
