//! The durable layer: a Loro document mirroring the hot-path buffer.
//!
//! Architecture rule (docs/DECISIONS.md D3): Loro is never on the keystroke
//! path. The rope edits first; committed ops are mirrored here, and the
//! `.strop` file is a Loro snapshot — full edit history plus current state.
//! That history is what later buys checkpoints, time travel, the author's
//! own voice corpus, and (eventually) sync.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use loro::{ExportMode, LoroDoc};

use crate::buffer::TextOp;

const TEXT_CONTAINER: &str = "content";

pub struct Store {
    doc: LoroDoc,
    path: PathBuf,
}

impl Store {
    /// Open a `.strop` file. Returns the store and, when the file already
    /// existed, its current text (None means a brand-new document).
    pub fn open(path: impl Into<PathBuf>) -> io::Result<(Self, Option<String>)> {
        let path = path.into();
        let doc = LoroDoc::new();
        let existing = match fs::read(&path) {
            Ok(bytes) => {
                doc.import(&bytes).map_err(io::Error::other)?;
                Some(doc.get_text(TEXT_CONTAINER).to_string())
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };
        Ok((Self { doc, path }, existing))
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
        assert_eq!(existing.as_deref(), Some("привет, мир"));

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
