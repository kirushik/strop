//! The single source of truth for user-facing commands (PLAN.md E1).
//! One table drives the keymap, the command palette, and (E4) the
//! shortcuts overlay — bindings can never drift from what the UI claims.
//!
//! Movement/selection/clipboard keys are deliberately NOT commands: the
//! palette is the menu, and menus don't list "move cursor left".

use gpui::Action;

use crate::editor::{
    AddCheckpoint, AddNote, CancelAiRun, CopyDocumentPath, ExportMarkdown, Find, Heading1,
    Heading2, Heading3, InsertFootnote, NewDocument, OpenFile, OpenWelcome, ReadItCold, Redo,
    OpenAiSettings, RenameDocument, Replace, RevealInFiles, RunBelieving, RunDiagnosis, SaveCopyAs,
    MoveToManuscript, PutBackScrap, SendToGraveyard, SetAside, SetSessionGoal, ShowShortcuts, TestAiConnection, ToggleBulletList,
    ToggleCode, ToggleCodeBlock, ToggleEmphasis, ToggleGraveyard, ToggleHighlight, ToggleHistory,
    ToggleOrderedList, TogglePalette, TogglePopover, ToggleQuoteBlock, ToggleReview, ToggleStrip,
    ToggleStrikethrough, ToggleStrong, ToggleUnderline, ScrapsTravel, Undo,
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

impl Command {
    /// Should this command's chord fire from ANY in-app focus, not just when
    /// the document has keyboard focus? The menu's navigation and app verbs
    /// (File, View, Margin & AI, History, Session, Help, plus Find/Replace)
    /// qualify — a writer expects Ctrl+Shift+P to open the palette while a
    /// field overlay is focused. Text mutations (Format, Structure, Undo/Redo,
    /// and Add Margin Note, which all act on the document selection) stay
    /// editor-scoped, so a chord typed into the palette or a note field can
    /// never reach the document behind it. `bind_keys` maps the two groups to
    /// the "App" and "Editor" key contexts respectively.
    pub fn global(&self) -> bool {
        match self.section {
            // Scraps' verbs (Set aside / Exile / Move to the manuscript /
            // Put back) act on the document selection or caret, so they stay
            // editor-scoped like Format/Structure — a chord typed into a
            // field never reaches the document behind it.
            "Format" | "Structure" | "Scraps" => false,
            "Edit" => matches!(self.label, "Find in Document" | "Find and Replace"),
            "Margin & AI" => self.label != "Add Margin Note",
            // File, View, History, Session, Help.
            _ => true,
        }
    }
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
        cmd!("New Document", "File", Some("ctrl-n"), NewDocument, ["create", "новый документ"]),
        cmd!("Open Document…", "File", Some("ctrl-o"), OpenFile, ["file", "открыть"]),
        cmd!(
            "Rename Document…",
            "File",
            Some("f2"),
            RenameDocument,
            ["title", "переименовать"]
        ),
        cmd!(
            "Reveal in Files",
            "File",
            None,
            RevealInFiles,
            ["show in file manager", "folder", "где файл", "папка"]
        ),
        cmd!(
            "Copy Document Path",
            "File",
            None,
            CopyDocumentPath,
            ["location", "путь к файлу"]
        ),
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
        cmd!(
            "Format Selection…",
            "Format",
            Some("ctrl-."),
            TogglePopover,
            [
                "popover",
                "toolbar",
                "формат",
                "formatting",
                "flank",
                // Format-word aliases so a "strikethrough"-shaped search SURFACES
                // the flank — prefixed so the direct Toggle command still wins the
                // tie (table order) and this row appears alongside, not above it.
                "format bold",
                "format italic",
                "format underline",
                "format strikethrough",
                "format highlight",
                "format code",
                "format link",
                "format heading"
            ]
        ),
        // ctrl-1..3 is the promoted chord (iA/Bear/Typora convention);
        // ctrl-alt-1..3 stay bound as silent aliases in bind_keys.
        cmd!("Heading 1", "Structure", Some("ctrl-1"), Heading1, ["title", "заголовок"]),
        cmd!("Heading 2", "Structure", Some("ctrl-2"), Heading2, ["заголовок"]),
        cmd!("Heading 3", "Structure", Some("ctrl-3"), Heading3, ["заголовок"]),
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
        // "Scraps" is the TRAVEL verb (08 §2; the rail it replaced is gone):
        // it arms the excursion latch and lands at the seam — the next press
        // resumes where the last visit stopped; Esc returns exactly home.
        cmd!(
            "Scraps",
            "View",
            Some("ctrl-shift-o"),
            ScrapsTravel,
            ["compost", "asides", "компост", "отложенное"]
        ),
        // Scraps (08 §2): the writer's deliberate pile at the tail; the
        // graveyard is the automatic record of cuts. "Scraps live; the
        // graveyard remembers."
        cmd!(
            "Set Aside",
            "Scraps",
            Some("ctrl-shift-a"),
            SetAside,
            ["scraps", "park", "shelf", "отложить", "компост"]
        ),
        // The pile-return verbs: chordless (the flank and the provenance
        // line are their homes; the palette is the narrow-width fallback).
        cmd!(
            "Move to the Manuscript",
            "Scraps",
            None,
            MoveToManuscript,
            ["retrieve", "bring back", "в рукопись", "вернуть в текст"]
        ),
        cmd!(
            "Put Back",
            "Scraps",
            None,
            PutBackScrap,
            ["return to origin", "unpark", "вернуть на место"]
        ),
        cmd!(
            "Send to the Graveyard",
            "Scraps",
            Some("ctrl-shift-g"),
            SendToGraveyard,
            ["cut", "exile", "delete to graveyard", "в могилу", "вырезать"]
        ),
        cmd!(
            "Toggle Graveyard",
            "View",
            Some("ctrl-alt-g"),
            ToggleGraveyard,
            ["cuts", "restore cut", "put back", "кладбище", "могила"]
        ),
        // The cold read (impl 05 §4.6, arbitration O7): the estrangement
        // ritual's one entry verb. "Cold read" is a carried term — the
        // Russian aliases translate the function, never the metaphor
        // (ux-glossary). Inside the room the same chord is the toggle-exit.
        cmd!(
            "Read it cold",
            "View",
            Some("ctrl-shift-l"),
            ReadItCold,
            ["cold read", "reading", "book", "свежим взглядом", "перечитать"]
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
        // The door (DESIGN §4.4): drafting quiets the editorial margin so a
        // burst is never pulled into evaluation; reviewing opens it. The
        // deliberate register change between GENERATE and EVALUATE.
        cmd!(
            "Drafting / Reviewing",
            "Margin & AI",
            Some("ctrl-shift-r"),
            ToggleReview,
            ["focus", "quiet margin", "review mode", "draft mode", "тишина", "правка", "режим правки", "черновик"]
        ),
        cmd!(
            "Add Margin Note",
            "Margin & AI",
            Some("ctrl-m"),
            AddNote,
            ["comment", "заметка на полях"]
        ),
        cmd!(
            "Set Up AI Provider…",
            "Margin & AI",
            None,
            OpenAiSettings,
            ["api key", "config", "settings", "настроить ии", "ключ"]
        ),
        cmd!(
            "Test AI Connection",
            "Margin & AI",
            None,
            TestAiConnection,
            ["verify", "ping", "проверить подключение"]
        ),
        cmd!(
            "Cancel AI Run",
            "Margin & AI",
            None,
            CancelAiRun,
            ["stop", "отменить"]
        ),
        // ctrl-alt-h and the titlebar clock open the STRIP (P1, the new first
        // history surface). The right-side panel lives on as its own palette
        // verb; the strip and the panel never open together.
        cmd!(
            "History",
            "History",
            Some("ctrl-alt-h"),
            ToggleStrip,
            ["timeline", "strip", "rewind", "scrub", "история", "лента"]
        ),
        cmd!(
            "History panel",
            "History",
            None,
            ToggleHistory,
            ["versions", "rewind", "sidebar", "history", "версии", "панель истории"]
        ),
        cmd!(
            "Name this version",
            "History",
            Some("ctrl-alt-s"),
            AddCheckpoint,
            ["snapshot", "version", "name", "версия"]
        ),
        // The finish-your-story layer (DESIGN §4): per-session progress.
        // Scaffolds prompt at CLOSE, never at open (§4b tension 6) — pull-only.
        // (The re-entry intent question / End Session was retired: impl 04 §1.)
        cmd!(
            "Set Session Goal…",
            "Session",
            None,
            SetSessionGoal,
            ["words", "target", "progress", "цель", "норма слов"]
        ),
        cmd!(
            "Open Command Palette",
            "Help",
            Some("ctrl-shift-p"),
            TogglePalette,
            ["commands", "menu", "палитра команд", "меню"]
        ),
        cmd!(
            "Keyboard Map",
            "Help",
            Some("ctrl-?"),
            ShowShortcuts,
            ["shortcuts", "cheatsheet", "горячие клавиши"]
        ),
        cmd!(
            "Open Welcome Guide",
            "Help",
            None,
            OpenWelcome,
            ["tutorial", "help", "учебник", "справка"]
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

/// Score arbitrary text (recents rows etc.) with the same rules commands
/// use; query is lowercased here, like in `score`.
pub fn score_text(query: &str, target: &str) -> Option<i32> {
    score_one(&query.to_lowercase(), &target.to_lowercase())
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
/// otherwise (stable across equal scores, so sections stay grouped) —
/// plus hit-frequency (DESIGN §3.3): a typed query gets a small boost
/// per past execution, capped so frequency can tilt ties but never
/// outshout a plainly better match. The empty state stays table order —
/// the Frequent section (see `frequent`) carries frequency there.
pub fn ranked_with_freq(
    query: &str,
    freq: &std::collections::HashMap<String, u32>,
) -> Vec<&'static Command> {
    let all = all();
    if query.trim().is_empty() {
        return all.iter().collect();
    }
    let mut scored: Vec<(i32, &Command)> = all
        .iter()
        .filter_map(|c| {
            score(query.trim(), c).map(|s| {
                let boost = freq.get(c.label).map_or(0, |&n| (n as i32).min(20));
                (s + boost, c)
            })
        })
        .collect();
    scored.sort_by_key(|&(score, _)| std::cmp::Reverse(score));
    scored.into_iter().map(|(_, c)| c).collect()
}

/// The empty-query palette's "Frequent" section (DESIGN §3.3): the top 5
/// commands by execution count, counts below 2 don't qualify (a single
/// use is noise, not habit), ties broken by table order. The rows repeat
/// in their home sections below — Obsidian does the same.
pub fn frequent(freq: &std::collections::HashMap<String, u32>) -> Vec<&'static Command> {
    let mut hits: Vec<(u32, &Command)> = all()
        .iter()
        .filter_map(|c| {
            freq.get(c.label)
                .copied()
                .filter(|&n| n >= 2)
                .map(|n| (n, c))
        })
        .collect();
    hits.sort_by_key(|&(count, _)| std::cmp::Reverse(count)); // stable: table order breaks ties
    hits.truncate(5);
    hits.into_iter().map(|(_, c)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(query: &str) -> Vec<&'static str> {
        let no_freq = std::collections::HashMap::new();
        ranked_with_freq(query, &no_freq).iter().map(|c| c.label).collect()
    }

    #[test]
    fn empty_query_lists_everything_in_table_order() {
        assert_eq!(labels("").len(), all().len());
        assert_eq!(labels("")[0], "New Document");
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
    fn naming_uses_the_version_vocabulary_and_existing_action_route() {
        let command = all().iter().find(|c| c.label == "Name this version").unwrap();
        assert_eq!(command.keys, Some("ctrl-alt-s"));
        assert!(all().iter().all(|c| {
            !c.label.to_ascii_lowercase().contains("checkpoint")
                && !c.section.to_ascii_lowercase().contains("checkpoint")
                && c.aliases
                    .iter()
                    .all(|a| !a.to_ascii_lowercase().contains("checkpoint"))
        }));
    }

    #[test]
    fn scattered_subsequence_still_matches() {
        assert!(labels("tgbld").contains(&"Toggle Bold"));
        assert!(labels("xyzzy").is_empty());
    }

    #[test]
    fn frequency_boost_tilts_ranking_but_is_capped() {
        // "toggle" matches many commands equally (word-start substring);
        // frequency breaks the tie toward the habitual one.
        let mut freq = std::collections::HashMap::new();
        freq.insert("Toggle Highlight".to_owned(), 3u32);
        let boosted = ranked_with_freq("toggle", &freq);
        assert_eq!(boosted[0].label, "Toggle Highlight");
        // Without history, table order holds.
        assert_eq!(labels("toggle")[0], "Toggle Bold");
        // The boost is capped at 20: "do" hits "New Document" at a word
        // start (~1096) and "Undo" mid-word (~998); a thousand Undo
        // executions close the tie-breaking gap, never the quality gap.
        let mut freq = std::collections::HashMap::new();
        freq.insert("Undo".to_owned(), 1000u32);
        let l: Vec<_> = ranked_with_freq("do", &freq).iter().map(|c| c.label).collect();
        assert_eq!(l[0], "New Document", "capped boost can't beat a better match");
        assert!(
            l.iter().position(|&c| c == "Undo").unwrap()
                < l.iter().position(|&c| c == "Redo").unwrap(),
            "but it does break the Undo/Redo tie"
        );
        // Empty query ignores frequency entirely (the Frequent section
        // carries it there); table order is untouched.
        assert_eq!(ranked_with_freq("", &freq)[0].label, "New Document");
    }

    #[test]
    fn frequent_needs_two_uses_and_caps_at_five() {
        let mut freq = std::collections::HashMap::new();
        assert!(frequent(&freq).is_empty());
        freq.insert("Undo".to_owned(), 1u32);
        assert!(frequent(&freq).is_empty(), "a single use is noise");
        freq.insert("Undo".to_owned(), 2);
        freq.insert("Toggle Bold".to_owned(), 9);
        assert_eq!(
            frequent(&freq)
                .iter()
                .map(|c| c.label)
                .collect::<Vec<_>>(),
            vec!["Toggle Bold", "Undo"],
            "count order, most-used first"
        );
        for c in ["Find in Document", "Heading 1", "Heading 2", "Heading 3"] {
            freq.insert(c.to_owned(), 5);
        }
        let top = frequent(&freq);
        assert_eq!(top.len(), 5, "capped at five");
        assert_eq!(top[0].label, "Toggle Bold");
        // Stale labels (renamed/removed commands) are simply skipped.
        freq.insert("Gone Command".to_owned(), 50);
        assert_eq!(frequent(&freq).len(), 5);
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

    #[test]
    fn read_depths_have_one_control_grammar() {
        assert!(
            all().iter().all(|command| !command.label.starts_with("Diagnosis Mode:")),
            "read depths belong to the editor menu, not sticky palette state"
        );
    }

    #[test]
    fn global_commands_are_the_app_verbs_not_text_mutations() {
        let by_label = |l: &str| all().iter().find(|c| c.label == l).unwrap();
        // App verbs must fire from any focus (palette, note field, settings).
        for l in [
            "Open Command Palette",
            "Find in Document",
            "Find and Replace",
            "Run Editorial Diagnosis",
            "History",
            "History panel",
            "Scraps",
            "Set Up AI Provider…",
            "New Document",
            "Set Session Goal…",
        ] {
            assert!(by_label(l).global(), "{l} should be global");
        }
        // Text mutations stay editor-scoped so a chord typed into a field can
        // never reach the document behind it.
        for l in [
            "Toggle Bold",
            "Heading 1",
            "Undo",
            "Redo",
            "Add Margin Note",
            "Insert Footnote",
        ] {
            assert!(!by_label(l).global(), "{l} should be editor-scoped");
        }
    }
}
