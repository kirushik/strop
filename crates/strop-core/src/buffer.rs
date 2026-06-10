use std::ops::Range;

use ropey::Rope;

/// The hot-path text buffer. A thin wrapper over a rope for now; anchors and
/// transaction-grouped undo land here next, designed against `Snapshot` from
/// day one so AI edits can be rebased over interleaved user typing.
#[derive(Debug, Default)]
pub struct Buffer {
    rope: Rope,
    version: u64,
}

/// An immutable, O(1)-cloned view of the buffer at a point in time.
#[derive(Debug, Clone)]
pub struct Snapshot {
    rope: Rope,
    pub version: u64,
}

impl Buffer {
    pub fn new(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            version: 0,
        }
    }

    /// Replace `char_range` with `text`. The single mutation primitive;
    /// everything else (insert, delete, paste) lowers to this.
    pub fn edit(&mut self, char_range: Range<usize>, text: &str) {
        self.rope.remove(char_range.start..char_range.end);
        self.rope.insert(char_range.start, text);
        self.version += 1;
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            rope: self.rope.clone(),
            version: self.version,
        }
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }
}

impl Snapshot {
    pub fn text(&self) -> String {
        self.rope.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_and_snapshot_isolation() {
        let mut buf = Buffer::new("Hello world");
        let snap = buf.snapshot();
        buf.edit(5..11, ", Strop");
        assert_eq!(buf.text(), "Hello, Strop");
        // The snapshot is unaffected by later edits.
        assert_eq!(snap.text(), "Hello world");
        assert_eq!(snap.version, 0);
        assert_eq!(buf.snapshot().version, 1);
    }
}
