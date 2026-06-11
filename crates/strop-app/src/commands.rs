//! The single source of truth for user-facing commands (PLAN.md E1).
//! One table drives the keymap, the command palette, and (E4) the
//! shortcuts overlay — bindings can never drift from what the UI claims.
//!
//! Movement/selection/clipboard keys are deliberately NOT commands: the
//! palette is the menu, and menus don't list "move cursor left".

use gpui::Action;

use crate::editor::{
    AddCheckpoint, AddNote, ExportMarkdown, Find, Heading1, Heading2, Heading3, InsertFootnote,
    OpenFile, Redo, Replace, RunBelieving, RunDiagnosis, SaveCopyAs, ToggleBulletList, ToggleCode,
    ToggleCodeBlock, ToggleEmphasis, ToggleHighlight, ToggleHistory, ToggleOrderedList,
    TogglePalette, ToggleQuoteBlock, ToggleStrikethrough, ToggleStrong, ToggleUnderline, Undo,
};

pub struct Command {
    /// Verb-first, the way a menu item would read.
    pub label: &'static str,
    /// Extra match targets: synonyms and Russian terms (bilingual user).
    pub aliases: &'static [&'static str],
    pub section: &'static str,
    /// GPUI keystroke chord; shown in the palette and bound in the keymap.
    pub keys: Option<&'static str>,
    pub make: fn() -> Box<dyn Action>,
}

macro_rules! cmd {
    ($label:literal, $section:literal, $keys:expr, $action:ty, [$($alias:literal),*]) => {
        Command {
            label: $label,
            aliases: &[$($alias),*],
            section: $section,
            keys: $keys,
            make: || Box::new(<$action>::default()),
        }
    };
}

/// Table order is presentation order (palette empty state, cheatsheet).
pub fn all() -> &'static [Command] {
    static COMMANDS: &[Command] = &[
        cmd!("Open Document…", "File", Some("ctrl-o"), OpenFile, ["file", "открыть"]),
        cmd!(
            "Save a Copy As…",
            "File",
            Some("ctrl-shift-s"),
            SaveCopyAs,
            ["duplicate", "сохранить копию"]
        ),
        cmd!(
            "Export as Markdown",
            "File",
            Some("ctrl-shift-e"),
            ExportMarkdown,
            ["save as", "md", "экспорт"]
        ),
        cmd!("Find in Document", "Edit", Some("ctrl-f"), Find, ["search", "поиск"]),
        cmd!(
            "Find and Replace",
            "Edit",
            Some("ctrl-h"),
            Replace,
            ["substitute", "замена"]
        ),
        cmd!("Undo", "Edit", Some("ctrl-z"), Undo, ["отмена"]),
        cmd!("Redo", "Edit", Some("ctrl-shift-z"), Redo, ["повтор"]),
        cmd!("Toggle Bold", "Format", Some("ctrl-b"), ToggleStrong, ["strong", "жирный"]),
        cmd!(
            "Toggle Italic",
            "Format",
            Some("ctrl-i"),
            ToggleEmphasis,
            ["emphasis", "курсив"]
        ),
        cmd!(
            "Toggle Underline",
            "Format",
            Some("ctrl-u"),
            ToggleUnderline,
            ["подчёркнутый"]
        ),
        cmd!(
            "Toggle Strikethrough",
            "Format",
            Some("ctrl-shift-x"),
            ToggleStrikethrough,
            ["зачёркнутый"]
        ),
        cmd!(
            "Toggle Highlight",
            "Format",
            Some("ctrl-shift-h"),
            ToggleHighlight,
            ["marker", "выделить маркером"]
        ),
        cmd!("Toggle Inline Code", "Format", Some("ctrl-e"), ToggleCode, ["mono", "код"]),
        cmd!("Heading 1", "Structure", Some("ctrl-alt-1"), Heading1, ["title", "заголовок"]),
        cmd!("Heading 2", "Structure", Some("ctrl-alt-2"), Heading2, ["заголовок"]),
        cmd!("Heading 3", "Structure", Some("ctrl-alt-3"), Heading3, ["заголовок"]),
        cmd!(
            "Toggle Blockquote",
            "Structure",
            Some("ctrl-alt-q"),
            ToggleQuoteBlock,
            ["quote", "цитата"]
        ),
        cmd!(
            "Toggle Code Block",
            "Structure",
            Some("ctrl-alt-c"),
            ToggleCodeBlock,
            ["блок кода"]
        ),
        cmd!(
            "Toggle Bullet List",
            "Structure",
            Some("ctrl-shift-8"),
            ToggleBulletList,
            ["unordered", "список"]
        ),
        cmd!(
            "Toggle Numbered List",
            "Structure",
            Some("ctrl-shift-7"),
            ToggleOrderedList,
            ["ordered", "нумерованный список"]
        ),
        cmd!(
            "Insert Footnote",
            "Structure",
            Some("ctrl-alt-f"),
            InsertFootnote,
            ["сноска"]
        ),
        cmd!(
            "Run Editorial Diagnosis",
            "Margin & AI",
            Some("ctrl-shift-d"),
            RunDiagnosis,
            ["ai", "check", "диагноз", "редактура"]
        ),
        cmd!(
            "Run Believing Pass",
            "Margin & AI",
            Some("ctrl-shift-b"),
            RunBelieving,
            ["strengths", "believe", "что работает"]
        ),
        cmd!(
            "Add Margin Note",
            "Margin & AI",
            Some("ctrl-m"),
            AddNote,
            ["comment", "заметка на полях"]
        ),
        cmd!(
            "Toggle History & Rewind",
            "History",
            Some("ctrl-alt-h"),
            ToggleHistory,
            ["versions", "rewind", "история", "версии"]
        ),
        cmd!(
            "Name a Checkpoint",
            "History",
            Some("ctrl-alt-s"),
            AddCheckpoint,
            ["snapshot", "version", "чекпоинт"]
        ),
        cmd!(
            "Open Command Palette",
            "Help",
            Some("ctrl-shift-p"),
            TogglePalette,
            ["commands", "menu", "палитра команд", "меню"]
        ),
    ];
    COMMANDS
}

/// Subsequence fuzzy match. Higher is better; None = no match.
/// Word-boundary hits and consecutive runs rank above scattered letters;
/// the best score across label and aliases wins.
pub fn score(query: &str, command: &Command) -> Option<i32> {
    let q = query.to_lowercase();
    std::iter::once(command.label)
        .chain(command.aliases.iter().copied())
        .filter_map(|target| score_one(&q, &target.to_lowercase()))
        .max()
}

fn score_one(query: &str, target: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    // Substring match dominates everything subsequence could produce.
    if let Some(pos) = target.find(query) {
        let at_word_start = pos == 0
            || target[..pos]
                .chars()
                .next_back()
                .is_some_and(|c| !c.is_alphanumeric());
        return Some(1000 + if at_word_start { 100 } else { 0 } - pos as i32);
    }
    let mut score = 0i32;
    let mut qchars = query.chars().peekable();
    let mut prev_matched = false;
    let mut prev_char: Option<char> = None;
    for tc in target.chars() {
        match qchars.peek() {
            Some(&qc) if qc == tc => {
                qchars.next();
                let word_start = prev_char.is_none_or(|p| !p.is_alphanumeric());
                score += 1 + if prev_matched { 3 } else { 0 } + if word_start { 5 } else { 0 };
                prev_matched = true;
            }
            _ => prev_matched = false,
        }
        prev_char = Some(tc);
    }
    qchars.next().is_none().then_some(score)
}

/// Commands ranked for a query: table order when empty, score order
/// otherwise (stable across equal scores, so sections stay grouped).
pub fn ranked(query: &str) -> Vec<&'static Command> {
    let all = all();
    if query.trim().is_empty() {
        return all.iter().collect();
    }
    let mut scored: Vec<(i32, &Command)> = all
        .iter()
        .filter_map(|c| score(query.trim(), c).map(|s| (s, c)))
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().map(|(_, c)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(query: &str) -> Vec<&'static str> {
        ranked(query).iter().map(|c| c.label).collect()
    }

    #[test]
    fn empty_query_lists_everything_in_table_order() {
        assert_eq!(ranked("").len(), all().len());
        assert_eq!(labels("")[0], "Open Document…");
    }

    #[test]
    fn substring_beats_subsequence() {
        let l = labels("bold");
        assert_eq!(l[0], "Toggle Bold");
    }

    #[test]
    fn aliases_match_including_russian() {
        assert_eq!(labels("диагноз")[0], "Run Editorial Diagnosis");
        assert_eq!(labels("ai")[0], "Run Editorial Diagnosis");
        assert_eq!(labels("экспорт")[0], "Export as Markdown");
    }

    #[test]
    fn scattered_subsequence_still_matches() {
        assert!(labels("tgbld").contains(&"Toggle Bold"));
        assert!(labels("xyzzy").is_empty());
    }

    #[test]
    fn every_binding_is_unique() {
        let mut seen = std::collections::HashSet::new();
        for c in all() {
            if let Some(k) = c.keys {
                assert!(seen.insert(k), "duplicate binding {k}");
            }
        }
    }
}
