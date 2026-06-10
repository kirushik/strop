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

    /// Replace `byte_range` (UTF-8 offsets) with `text`. UI layers work in
    /// byte offsets (text layout) and UTF-16 (IME); chars are internal.
    pub fn edit_bytes(&mut self, byte_range: Range<usize>, text: &str) {
        let start = self.rope.byte_to_char(byte_range.start);
        let end = self.rope.byte_to_char(byte_range.end);
        self.edit(start..end, text);
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            rope: self.rope.clone(),
            version: self.version,
        }
    }

    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    pub fn slice_bytes(&self, byte_range: Range<usize>) -> String {
        self.rope.byte_slice(byte_range).to_string()
    }

    pub fn byte_to_utf16(&self, byte: usize) -> usize {
        self.rope.char_to_utf16_cu(self.rope.byte_to_char(byte))
    }

    pub fn utf16_to_byte(&self, utf16: usize) -> usize {
        self.rope.char_to_byte(self.rope.utf16_cu_to_char(utf16))
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
    fn byte_and_utf16_conversions() {
        // "д" is 2 bytes / 1 utf16 unit; "🙂" is 4 bytes / 2 utf16 units.
        let buf = Buffer::new("aд🙂b");
        assert_eq!(buf.len_bytes(), 8);
        assert_eq!(buf.byte_to_utf16(3), 2); // after 'a' + 'д'
        assert_eq!(buf.utf16_to_byte(4), 7); // after 'a' + 'д' + '🙂'
        assert_eq!(buf.slice_bytes(1..3), "д");
    }

    #[test]
    fn edit_bytes() {
        let mut buf = Buffer::new("ёлки");
        buf.edit_bytes(0..2, "П");
        assert_eq!(buf.text(), "Плки");
    }

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
