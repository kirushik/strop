//! The first-run document (PLAN.md E4): the onboarding IS a document.
//! Typora's Quick Start pattern — written in Strop's own format, literal
//! chords inline, short, and alive: reading it exercises the features,
//! and two pre-seeded margin cards show what a diagnosis feels like
//! before any API key exists.

use strop_core::document::{Annotation, Annotations, BlockMap, NoteKind, NoteStatus, SpanSet};

const TUTORIAL_MD: &str = r#"# Welcome to Strop

Strop is a **writer's editor with an editor inside**: it *diagnoses* prose the way a good human editor does — it names problems and asks questions. It never rewrites you, and it never averages your voice toward the mean.

This document is alive — edit anything. It is a real file in your documents folder; click its name in the titlebar (or press F2) to rename it, file and all.

## The two keys that matter

`ctrl-shift-p` opens the command palette: every command, searchable, with its shortcut on the row. `ctrl-?` shows the whole keyboard map. Everything mentioned below lives in both.

## Marks

Select a word and try `ctrl-b` bold, `ctrl-i` italic, `ctrl-shift-h` highlight. Headings are `ctrl-1`–`3`, and `ctrl-shift-o` opens an outline of them on the left — click one to jump. The typograph quietly sets quotes, dashes and spaces as you type — «так», “so”, 1941—1945 — and a single undo always returns exactly what you typed.

## The margin

The cards on the right are the point of this editor — and an editorial pass wrote the two diagnoses in this section, the same kind you can run on your own words in a minute. Each one names a problem and asks a question; deciding is your job, and a dismissed card never comes back. There is two of them anchored here, so you can feel how a diagnosis reads before you run one.

To run your own: press `ctrl-shift-d`, or click the little card in the title bar. If a model is already running on your computer (Ollama), Strop offers it on the spot — no key, no account, and your text never leaves your machine. Otherwise it walks you through a provider once, then answers the pass you asked for. A pass has depths — developmental (structure), line (clarity), copy (mechanics); switch them in the palette. `ctrl-shift-b` runs the believing pass instead: what already works and where the center of gravity is. `ctrl-m` adds your own margin note.

## When to look

Writing and editing are different jobs, and doing both at once is how sentences die — the inner editor strangles a line before it is fully born. So Strop keeps a door between them. While you draft, press `ctrl-shift-r` to close it: the margin goes quiet and its cards rest behind a thin rail (nothing is lost — one click on the rail, or `ctrl-shift-r` again, brings them back). When you are ready to read with an editor's eye, open the door; running a pass opens it for you. Your own notes never hide — the door quiets the editor, not you.

## Rewind

Every keystroke is kept, across sessions — `ctrl-z` works after a restart. Try it: delete this whole paragraph — really, all of it — then press `ctrl-alt-h` and step back through versions to bring it home, or just `ctrl-z`. Nothing in Strop can be lost. Restoring is itself undoable, and `ctrl-alt-s` names a checkpoint; this document already has one — open history and look.

## Files

Documents are real files in ~/Documents/Strop — `ctrl-n` new, `ctrl-o` open, `ctrl-shift-e` exports Markdown next to the file. Work saves continuously; there is no Save.

## When you're done today

Run “End Session…” from the palette and answer one question — *next session I will ___*. Strop quits, and the next open lands on that note with your cursor exactly where you left it. Quitting any other way is fine too; nothing is ever asked.

*The fix is yours. Always.*"#;

/// Build the welcome document: text, formatting, and the demo margin.
pub fn document() -> (String, SpanSet, BlockMap, Annotations) {
    let (text, spans, blocks) = strop_core::markdown::from_markdown(TUTORIAL_MD);
    let mut notes = Annotations::default();
    let char_range = |quote: &str| {
        text.find(quote).map(|pos| {
            let start = text[..pos].chars().count();
            start..start + quote.chars().count()
        })
    };
    // A copy-level diagnosis on a deliberate error: the demo teaches by
    // catching a real (planted) problem, not by lorem ipsum.
    if let Some(range) = char_range("There is two of them") {
        notes.push(Annotation {
            id: 0,
            range,
            body: "Subject and verb disagree. Deliberate, or should this be “there are”?"
                .to_owned(),
            status: NoteStatus::Open,
            created_unix: 0,
            kind: NoteKind::Diagnosis,
            title: "Agreement slip".to_owned(),
            level: "copy".to_owned(),
        });
    }
    if let Some(range) = char_range("it never averages your voice toward the mean") {
        notes.push(Annotation {
            id: 0,
            range,
            body: "Strong claim in the first breath. Where does the text earn it — or is the \
                   promise the point?"
                .to_owned(),
            status: NoteStatus::Open,
            created_unix: 0,
            kind: NoteKind::Diagnosis,
            title: "Unearned claim?".to_owned(),
            level: "line".to_owned(),
        });
    }
    if let Some(range) = char_range("deciding is your job") {
        notes.push(Annotation {
            id: 0,
            range,
            body: "Notes like this one are yours — ctrl-m on any selection. Cards follow \
                   their text as you edit."
                .to_owned(),
            status: NoteStatus::Open,
            created_unix: 0,
            kind: NoteKind::Note,
            title: String::new(),
            level: String::new(),
        });
    }
    (text, spans, blocks, notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tutorial_builds_with_all_demo_cards() {
        let (text, _, blocks, notes) = document();
        assert!(text.starts_with("Welcome to Strop"));
        assert_eq!(notes.notes().len(), 3, "all three demo anchors must match");
        assert!(
            blocks
                .kinds()
                .iter()
                .any(|k| matches!(k, strop_core::document::BlockKind::Heading(2))),
            "sections survived the markdown import"
        );
        // Anchors are char ranges into the text.
        for n in notes.notes() {
            assert!(text.chars().count() >= n.range.end);
        }
    }
}
