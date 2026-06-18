# Keymap Baseline: Plain-Text Editing Idioms for a Desktop Prose Editor

Canonical reference for the "what every native text field does" layer across
Linux (GTK), Windows (Win32 edit/RichEdit), and macOS (Cocoa text system). No
modal (vim/emacs-mode) behavior — this is the substrate users expect before
any editor personality is added. Compiled 2026-06 from primary sources; each
section cites its sources. Strop's bindings live in
`crates/strop-app/src/editor.rs` (`bind_keys`); the appendix tracks our gap
status.

**Legend:** ★ = table stakes (users will file bugs if missing) · ◇ =
nice-to-have (expected by power users / prose-tool veterans) · `Cmd` = ⌘,
`Opt` = ⌥. "Mod" in CodeMirror terms = Ctrl on Linux/Windows, Cmd on macOS.

**Keyboard-layout independence:** chords name physical keys, so `Ctrl+Shift+P`
must work whatever the active layout (a writer in a Russian layout presses the
same key and gets the palette). Linux delivers this; Windows/macOS have a
gpui-level gap for letter keys under non-Latin layouts. Analysis and the
tester-verification ask live in `docs/UPSTREAM-gpui-windows-keyboard-layout.md`.

**Sources used throughout:**

- **macOS selectors:** [NSStandardKeyBindingResponding](https://developer.apple.com/documentation/appkit/nsstandardkeybindingresponding) (full selector vocabulary); [Text System Defaults and Key Bindings](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/TextDefaultsBindings/TextDefaultsBindings.html) (StandardKeyBinding.dict mechanism, `^~$#` modifier syntax); [StandardKeyBinding.dict dump](https://gist.github.com/zsimic/1367779); [Apple: Mac keyboard shortcuts](https://support.apple.com/en-us/102650).
- **Linux/GTK:** [GNOME/gtk `gtk/gtktextview.c` @ main](https://github.com/GNOME/gtk/blob/main/gtk/gtktextview.c) — binding installs in `gtk_text_view_class_init` (≈ lines 1689–1947), `add_move_binding` helper (≈ line 877, auto-installs Shift variants), click-gesture handler (≈ lines 5687–5836), `get_middle_click_paste` (≈ line 6205).
- **Windows:** [About Edit Controls (Win32)](https://learn.microsoft.com/en-us/windows/win32/controls/about-edit-controls); [Keyboard shortcuts in Windows](https://support.microsoft.com/en-us/windows/keyboard-shortcuts-in-windows-dcc61a57-8ff0-cffe-9796-cb9706c75eec).
- **Cross-platform synthesis:** [codemirror/commands `src/commands.ts`](https://github.com/codemirror/commands/blob/main/src/commands.ts) — `standardKeymap`, `defaultKeymap`, `emacsStyleKeymap` (MIT; mirrors exactly this baseline and is a good implementation crib).
- **Mouse/selection:** GTK source above; Win32 About Edit Controls; [freedesktop.org Clipboard spec](https://specifications.freedesktop.org/clipboard/latest/); [NSTextView selectionRange(forProposedRange:granularity:)](https://developer.apple.com/documentation/appkit/nstextview/1449188-selectionrange).
- **Prose-editor extras:** [Google Docs keyboard shortcuts](https://support.google.com/docs/answer/179738).

---

## 1. Caret motion ★

GTK auto-generates a Shift-extends-selection variant for every motion binding
(`add_move_binding` asserts no Shift in the mask, then installs both). Cocoa
pairs every `move*` selector with `move*AndModifySelection`. Windows documents
Shift variants explicitly. So §2 is mechanical: **every row here + Shift =
selection extension.**

| Semantics | Linux (GTK) | Windows | macOS | Notes |
|---|---|---|---|---|
| Char left/right ★ | `←`/`→` (`MOVEMENT_VISUAL_POSITIONS`) | `←`/`→` | `←`/`→` (`moveLeft:`/`moveRight:`) | Visual (bidi-aware) positions on GTK/mac. With a non-empty selection, plain `←`/`→` collapses to the left/right edge of the selection without moving — all three platforms and CodeMirror. ★ |
| Word left/right ★ | `Ctrl+←`/`Ctrl+→` (`MOVEMENT_WORDS`) | `Ctrl+←`/`Ctrl+→` | `Opt+←`/`Opt+→` (`moveWordLeft:`/`moveWordRight:`) | **Landing differs:** Windows "move the cursor forward to the beginning of the next word" (GTK matches); macOS "move the insertion point to the end of the next word". Backward lands at word start everywhere. Don't unify — match the platform. |
| Line up/down ★ | `↑`/`↓` (`MOVEMENT_DISPLAY_LINES`) | `↑`/`↓` | `↑`/`↓` (`moveUp:`/`moveDown:`) | Moves by **visual (wrapped) line** with a remembered goal column ★. macOS `moveUp:` on the first line moves to offset 0 (and `moveDown:` on last line to end); GTK and Win32 EDIT leave the caret in place. Mac-only behavior is fine to gate by platform. |
| Start/end of visual line ★ | `Home`/`End` (`MOVEMENT_DISPLAY_LINE_ENDS`) | `Home`/`End` | `Cmd+←`/`Cmd+→` (`moveToLeftEndOfLine:`/`moveToRightEndOfLine:`) | All three mean the **wrapped/display line**, not the paragraph. mac `Home`/`End` do **not** move the caret (see §10). |
| Start/end of paragraph (logical line) ★ for prose | `Ctrl+↑`/`Ctrl+↓` (`MOVEMENT_PARAGRAPHS`) | `Ctrl+↑`/`Ctrl+↓` ("beginning of the previous/next paragraph") | `Opt+↑`/`Opt+↓` (`moveToBeginningOfParagraph:`/`moveToEndOfParagraph:`); also `Ctrl+A`/`Ctrl+E` | Writers use this constantly. mac semantics: Opt+↑ goes to start of *current* paragraph (repeat for previous); Opt+↓ to end of current. |
| Document start/end ★ | `Ctrl+Home`/`Ctrl+End` (`MOVEMENT_BUFFER_ENDS`) | `Ctrl+Home`/`Ctrl+End` | `Cmd+↑`/`Cmd+↓` (`moveToBeginningOfDocument:`/`moveToEndOfDocument:`) | CodeMirror also keeps `Mod-Home`/`Mod-End` on mac. |
| Page up/down ★ | `PgUp`/`PgDn` (`MOVEMENT_PAGES`) | `PgUp`/`PgDn` | `Fn+↑`/`Fn+↓` → `scrollPageUp:`/`scrollPageDown:` | **mac scrolls without moving the caret**; GTK/Windows move the caret a page. `Opt+PgUp/PgDn` on mac moves the caret. |
| Horizontal page scroll | `Ctrl+PgUp`/`Ctrl+PgDn` (`MOVEMENT_HORIZONTAL_PAGES`) | — | — | GTK oddity; skip. |

## 2. Selection extension ★

Everything in §1 with `Shift` added, same granularity, anchor fixed at the
selection origin. Explicit table only where platforms diverge:

| Semantics | Linux (GTK) | Windows | macOS |
|---|---|---|---|
| Select all ★ | `Ctrl+A` (also legacy `Ctrl+/`) | `Ctrl+A` | `Cmd+A` (`selectAll:`) |
| Unselect all ◇ | `Ctrl+Shift+A` (also `Ctrl+\`) | `Esc` (app-level) | arrow key collapses |
| Select to paragraph boundary ◇ | `Ctrl+Shift+↑/↓` | `Ctrl+Shift+↑/↓` | `Opt+Shift+↑/↓` |
| Select to line start/end ★ | `Shift+Home`/`Shift+End` | `Shift+Home`/`Shift+End` | `Cmd+Shift+←/→` |
| Select to doc start/end ★ | `Ctrl+Shift+Home/End` | `Ctrl+Shift+Home/End` | `Cmd+Shift+↑/↓` |

Subtlety ★: shift-extension always moves the **head**, leaving the anchor;
mixing granularities (Shift+→ then Ctrl+Shift+→) keeps the same anchor. After
extension, a plain arrow collapses (see §1 row 1).

## 3. Deletion ★

| Semantics | Linux (GTK) | Windows | macOS | Notes |
|---|---|---|---|---|
| Delete char back ★ | `Backspace` (also `Shift+Backspace`, "to help with mis-typing") | `Backspace` | `Delete` (`deleteBackward:`) | With selection: deletes selection — all platforms; typing replaces selection ★. |
| Delete char forward ★ | `Delete` | `Del` | `Fn+Delete` / `⌦` (`deleteForward:`) | |
| Delete word back ★ | `Ctrl+Backspace` | `Ctrl+Backspace` | `Opt+Delete` (`deleteWordBackward:`) | |
| Delete word forward ★ | `Ctrl+Delete` | `Ctrl+Del` | `Opt+Fn+Delete` (`deleteWordForward:`) | |
| Delete to start of line ◇ (★ on mac) | `Ctrl+Shift+Backspace` | — | `Cmd+Delete` (`deleteToBeginningOfLine:`) | `Cmd+Delete` is deep mac muscle memory. |
| Delete to end of line/paragraph ◇ | `Ctrl+Shift+Delete` | — | `Ctrl+K` (`deleteToEndOfParagraph:`) | |

## 4. Clipboard ★

| Semantics | Linux (GTK) | Windows | macOS |
|---|---|---|---|
| Cut ★ | `Ctrl+X`; legacy `Shift+Delete`; `XF86Cut` | `Ctrl+X`; legacy `Shift+Delete` | `Cmd+X` |
| Copy ★ | `Ctrl+C`; legacy `Ctrl+Insert`; `XF86Copy` | `Ctrl+C`; legacy `Ctrl+Insert` | `Cmd+C` |
| Paste ★ | `Ctrl+V`; legacy `Shift+Insert`; `XF86Paste` | `Ctrl+V`; legacy `Shift+Insert` | `Cmd+V` |

The `Ctrl+Insert`/`Shift+Insert`/`Shift+Delete` trio is implemented by
**both** Win32 and GTK — bind on those platforms, never on mac.

## 5. History ★

| Semantics | Linux (GTK) | Windows | macOS |
|---|---|---|---|
| Undo ★ | `Ctrl+Z` (+ `XF86Undo`) | `Ctrl+Z`; legacy `Alt+Backspace` | `Cmd+Z` |
| Redo ★ | `Ctrl+Shift+Z` **and** `Ctrl+Y` (GTK binds both) | `Ctrl+Y` | `Cmd+Shift+Z` (never `Cmd+Y`) |

Undo granularity for prose: group consecutive typing into word/burst-sized
undo units (Cocoa/Word behavior). ★ for a writing tool. (Strop: done.)

## 6. Insertion & misc

| Semantics | Linux (GTK) | Windows | macOS | Priority |
|---|---|---|---|---|
| Newline | `Enter` | `Enter` | `Return` (`insertNewline:`) | ★ |
| Soft line break (same paragraph) | — | `Shift+Enter` (Word/Docs) | `Ctrl+Return`; apps use `Shift+Return` | ◇ — matters once blocks exist |
| Overwrite toggle | `Insert` | `Insert` | — | skip (accidental activation is a support burden) |
| Emoji picker | `Ctrl+.` / `Ctrl+;` (GTK) | `Win+.` (OS) | `Cmd+Ctrl+Space` (OS) | ◇ |
| Context menu key | `Menu` / `Shift+F10` | `Menu` / `Shift+F10` | — | ◇ |
| Cancel/escape | — | `Esc` | `Esc` → `cancelOperation:` | ◇ (clear selection) |

## 7. macOS hidden Ctrl layer (emacs-style) — ★ *on macOS only*

Mac users expect these in **every** text field; they come free from
StandardKeyBinding.dict in native apps, so a GPUI editor must reimplement
them. Shift variants = `AndModifySelection` (marked ⇧):

| Key | Selector | Behavior | ⇧ |
|---|---|---|---|
| `Ctrl+A` | `moveToBeginningOfParagraph:` | start of paragraph — **not** visual line | ✓ |
| `Ctrl+E` | `moveToEndOfParagraph:` | end of paragraph | ✓ |
| `Ctrl+F` / `Ctrl+B` | `moveForward:` / `moveBackward:` | char right/left | ✓ |
| `Ctrl+N` / `Ctrl+P` | `moveDown:` / `moveUp:` | line down/up | ✓ |
| `Ctrl+D` | `deleteForward:` | forward delete | |
| `Ctrl+H` | `deleteBackward:` | backspace | |
| `Ctrl+K` | `deleteToEndOfParagraph:` | kill to paragraph end; **fills the yank buffer**, not the clipboard | |
| `Ctrl+Y` | `yank:` | paste the kill buffer (separate from `Cmd+V`) | |
| `Ctrl+T` | `transpose:` | swap chars around caret | |
| `Ctrl+O` | open line below, caret stays | | |
| `Ctrl+V` | `pageDown:` | move caret a page down | ✓ |
| `Ctrl+L` | `centerSelectionInVisibleArea:` | scroll caret to center | |
| `Opt+F`/`Opt+B`, `Opt+D` | word fwd/back, delete word fwd | | ✓ |

CodeMirror ships exactly this set as `emacsStyleKeymap`, enabled only on mac
inside `standardKeymap` — copy that gating.

## 8. Mouse & selection idioms ★

| Idiom | Linux | Windows | macOS | Priority |
|---|---|---|---|---|
| Click places caret; drag selects by char | ✓ | ✓ | ✓ | ★ |
| Shift+click extends selection to click point | ✓ | ✓ | ✓ | ★ — Win/GTK keep the existing anchor; classic mac extends from the **nearest** selection edge. Matching Win/GTK everywhere is acceptable. |
| Double-click selects word; **drag while held extends by whole words** | ✓ (`SELECT_WORDS`) | ✓ | ✓ | ★ both halves — drag-by-word after double-click is the most commonly forgotten |
| Triple-click selects paragraph; drag extends by paragraph | ✓ (`SELECT_LINES`; GTK buffer "line" = paragraph) | RichEdit/Word: yes | ✓ | ★ — for prose, select the **paragraph** |
| Click-count resets if next click is on a different line | ✓ (explicit in GTK source) | — | — | detail |
| Click inside selection + drag = drag-and-drop text | ✓ | ✓ (RichEdit) | ✓ | ◇ — defer |
| Middle-click pastes PRIMARY selection at click position | ✓ X11 **and** Wayland (`wp_primary_selection`) | n/a | n/a | ★ on Linux. Per the freedesktop spec: selecting sets PRIMARY only (never CLIPBOARD); middle button pastes PRIMARY (never CLIPBOARD). Implement both halves: *publish* selection to PRIMARY and *paste* on middle-click. |
| Scroll wheel scrolls without moving caret; drag-select past edge autoscrolls | ✓ | ✓ | ✓ | ★ (autoscroll during drag-select is frequently missed) |

## 9. Prose-editor extras — ◇

- **Move paragraph up/down:** Google Docs `Ctrl+Shift+↑/↓`; Word `Alt+Shift+↑/↓`. Conflicts with the Win/GTK "select paragraphs" meaning of `Ctrl+Shift+↑/↓` — if added, prefer `Alt+↑/↓` (CodeMirror moveLineUp/Down norm).
- **Smart-Home** (first non-whitespace vs column 0): code-editor convention, **not** native-field or prose behavior. Skip.
- **Find:** `Ctrl/Cmd+F` ★ (app-level); `Ctrl+H` replace on Windows/GTK, `Cmd+Opt+F` on mac.
- Undo grouping by typing bursts (§5) and goal-column persistence (§1) are the two invisible behaviors writers feel immediately.

## 10. Semantic fine print (the bugs-waiting-to-happen list)

1. **Word-right landing** ★: Windows/GTK → *start of next word*; mac `Opt+→` → *end of next word*. Word-left → start of previous word on all platforms. Native-correct behavior requires per-platform word-boundary policy. (Strop currently lands at end-of-word — see appendix.)
2. **"Line" is visual; "paragraph" is logical** ★: Home/End and Cmd+←/→ operate on the wrapped display line on all three platforms; paragraph ops (Ctrl+↑/↓, mac Ctrl+A/E, Opt+↑/↓) operate on newline-delimited text. Both layers must exist.
3. **mac Home/End/PgUp/PgDn scroll, never move the caret** — porting Windows Home/End behavior to mac is a classic cross-platform tell. ★
4. **Up on first line / Down on last:** mac moves to offset 0 / end-of-doc; GTK and Win32 leave the caret. (Strop does the mac/CodeMirror flatten — acceptable.)
5. **Arrow with active selection collapses without moving**; Shift+arrow keeps the anchor. ★
6. **mac Ctrl+K/Ctrl+Y use a private kill buffer**, not the system clipboard.
7. **GTK Shift+Backspace = Backspace** — mis-typing forgiveness; cheap to copy.
8. **Clipboard legacy trio** exists on both Windows and GTK — bind there, never on mac.
9. **PRIMARY selection on Linux** is a two-way contract (publish on select, paste on middle-click), on Wayland too.

## Appendix: Strop status (Linux baseline, updated 2026-06-10)

Done: char/word/line/paragraph motion + all Shift variants, visual-line
Home/End (+Shift), doc start/end (+Shift), word & char deletion both
directions, clipboard + CUA legacy trio, undo/redo (Ctrl+Z, Ctrl+Shift+Z,
Ctrl+Y) with word-grouped typing, selection-collapse-on-arrow, goal-column,
double-click word / triple-click paragraph with drag-by-unit extension,
Shift+Backspace, PRIMARY selection (publish + middle-click paste),
Shift+click extend.

Missing (blocked on scrolling/viewport): PgUp/PgDn (+Shift), autoscroll
during drag-select, scroll-without-caret. Missing (deferred): drag-and-drop
of selected text, Esc-clears-selection, context-menu key, find. Per-platform
(when mac/Windows ports happen): the entire §7 mac Ctrl layer, Cmd-based
bindings, word-right landing policy switch (§10.1), mac Home/End scroll
semantics.
