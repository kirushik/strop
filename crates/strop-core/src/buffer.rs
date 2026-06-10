use std::ops::Range;

use ropey::Rope;

/// The hot-path text buffer: a rope plus a transaction-grouped undo history.
/// Anchors land here next, designed against `Snapshot` from day one so AI
/// edits can be rebased over interleaved user typing.
#[derive(Debug, Default)]
pub struct Buffer {
    rope: Rope,
    version: u64,
    undo_stack: Vec<Transaction>,
    redo_stack: Vec<Transaction>,
    /// Whether the top of `undo_stack` may still absorb coalesced edits.
    group_open: bool,
}

/// An immutable, O(1)-cloned view of the buffer at a point in time.
#[derive(Debug, Clone)]
pub struct Snapshot {
    rope: Rope,
    pub version: u64,
}

/// One undoable user-visible step; may group several primitive edits
/// (e.g. the keystrokes of a word).
#[derive(Debug)]
struct Transaction {
    edits: Vec<Edit>,
}

/// A primitive edit, recorded in the char coordinates that were valid at the
/// moment it was applied (so a transaction undoes cleanly in reverse order).
#[derive(Debug)]
struct Edit {
    start: usize,
    old: String,
    new: String,
}

#[derive(Debug, PartialEq, Eq)]
enum EditKind {
    Insert,
    Delete,
    Replace,
}

impl Edit {
    fn kind(&self) -> EditKind {
        match (self.old.is_empty(), self.new.is_empty()) {
            (true, _) => EditKind::Insert,
            (false, true) => EditKind::Delete,
            (false, false) => EditKind::Replace,
        }
    }

    fn new_chars(&self) -> usize {
        self.new.chars().count()
    }

    fn old_chars(&self) -> usize {
        self.old.chars().count()
    }

    /// Can `next` continue the run this edit ends? Typing extends forward;
    /// backspace eats backward; forward-delete stays in place.
    fn continues_into(&self, next: &Edit) -> bool {
        if self.kind() != next.kind() {
            return false;
        }
        match next.kind() {
            EditKind::Insert => next.start == self.start + self.new_chars(),
            EditKind::Delete => {
                next.start + next.old_chars() == self.start || next.start == self.start
            }
            EditKind::Replace => false,
        }
    }
}

impl Buffer {
    pub fn new(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            ..Default::default()
        }
    }

    /// Replace `char_range` with `text` as its own transaction.
    pub fn edit(&mut self, char_range: Range<usize>, text: &str) {
        self.apply_edit(char_range, text, false);
    }

    /// Replace `byte_range` (UTF-8 offsets) with `text`. UI layers work in
    /// byte offsets (text layout) and UTF-16 (IME); chars are internal.
    pub fn edit_bytes(&mut self, byte_range: Range<usize>, text: &str) {
        let range = self.byte_range_to_chars(byte_range);
        self.apply_edit(range, text, false);
    }

    /// Like `edit_bytes`, but coalesces with the previous edit when it
    /// continues the same typing/deleting run. Word-granularity: an inserted
    /// whitespace closes the group.
    pub fn edit_bytes_coalescing(&mut self, byte_range: Range<usize>, text: &str) {
        let range = self.byte_range_to_chars(byte_range);
        self.apply_edit(range, text, true);
    }

    fn byte_range_to_chars(&self, byte_range: Range<usize>) -> Range<usize> {
        self.rope.byte_to_char(byte_range.start)..self.rope.byte_to_char(byte_range.end)
    }

    fn apply_edit(&mut self, char_range: Range<usize>, text: &str, coalesce: bool) {
        let edit = Edit {
            start: char_range.start,
            old: self.rope.slice(char_range.clone()).to_string(),
            new: text.to_owned(),
        };
        self.rope.remove(char_range.clone());
        self.rope.insert(char_range.start, text);
        self.version += 1;
        self.redo_stack.clear();

        let extend = coalesce
            && self.group_open
            && self
                .undo_stack
                .last()
                .and_then(|tx| tx.edits.last())
                .is_some_and(|last| last.continues_into(&edit));
        // A whitespace insert finishes the word: record it, then close.
        self.group_open = coalesce && !edit.new.chars().any(char::is_whitespace);
        if extend {
            self.undo_stack.last_mut().unwrap().edits.push(edit);
        } else {
            self.undo_stack.push(Transaction { edits: vec![edit] });
        }
    }

    /// Undo the last transaction. Returns a char offset for the cursor.
    pub fn undo(&mut self) -> Option<usize> {
        self.group_open = false;
        let tx = self.undo_stack.pop()?;
        let mut cursor = 0;
        for edit in tx.edits.iter().rev() {
            let end = edit.start + edit.new_chars();
            self.rope.remove(edit.start..end);
            self.rope.insert(edit.start, &edit.old);
            cursor = edit.start + edit.old_chars();
        }
        self.version += 1;
        self.redo_stack.push(tx);
        Some(cursor)
    }

    /// Redo the last undone transaction. Returns a char offset for the cursor.
    pub fn redo(&mut self) -> Option<usize> {
        self.group_open = false;
        let tx = self.redo_stack.pop()?;
        let mut cursor = 0;
        for edit in &tx.edits {
            let end = edit.start + edit.old_chars();
            self.rope.remove(edit.start..end);
            self.rope.insert(edit.start, &edit.new);
            cursor = edit.start + edit.new_chars();
        }
        self.version += 1;
        self.undo_stack.push(tx);
        Some(cursor)
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

    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        self.rope.char_to_byte(char_idx)
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

    #[test]
    fn undo_redo_roundtrip() {
        let mut buf = Buffer::new("abc");
        buf.edit(3..3, "def");
        buf.edit(0..1, "X");
        assert_eq!(buf.text(), "Xbcdef");
        assert_eq!(buf.undo(), Some(1));
        assert_eq!(buf.text(), "abcdef");
        buf.undo();
        assert_eq!(buf.text(), "abc");
        assert!(buf.undo().is_none());
        buf.redo();
        buf.redo();
        assert_eq!(buf.text(), "Xbcdef");
        assert!(buf.redo().is_none());
    }

    #[test]
    fn typing_coalesces_by_word() {
        let mut buf = Buffer::new("");
        for (i, ch) in "то самое".chars().enumerate() {
            let byte = buf.char_to_byte(i);
            buf.edit_bytes_coalescing(byte..byte, &ch.to_string());
        }
        assert_eq!(buf.text(), "то самое");
        // One undo removes "самое", the next removes "то" + the space.
        buf.undo();
        assert_eq!(buf.text(), "то ");
        buf.undo();
        assert_eq!(buf.text(), "");
    }

    #[test]
    fn backspace_run_coalesces_and_breaks_on_discontinuity() {
        let mut buf = Buffer::new("парус");
        // Backspace twice from the end: one transaction.
        buf.edit_bytes_coalescing(8..10, "");
        buf.edit_bytes_coalescing(6..8, "");
        // Jump elsewhere and delete: separate transaction.
        buf.edit_bytes_coalescing(0..2, "");
        assert_eq!(buf.text(), "ар");
        // First undo restores only the discontinuous deletion...
        buf.undo();
        assert_eq!(buf.text(), "пар");
        // ...the second restores the whole backspace run.
        buf.undo();
        assert_eq!(buf.text(), "парус");
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut buf = Buffer::new("a");
        buf.edit(1..1, "b");
        buf.undo();
        buf.edit(1..1, "c");
        assert!(buf.redo().is_none());
        assert_eq!(buf.text(), "ac");
    }
}
