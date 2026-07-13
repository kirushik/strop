# Cold read — code recon (2026-07-06)

Branch `cold-read` @ bb13058. Read-only survey of everything the cold-read
build touches, with file:line anchors. Spec: `docs/impl/05-cold-read.md`;
design: `docs/golden-path.md` D3 + §9.2–9.3; hyphenation language survey:
`docs/ux-glossary.md` §Appendix (lines 51–82).

Layout of the codebase: two crates. `crates/strop-app/src/editor.rs`
(17,968 lines) is the app monolith — Editor entity, EditorElement,
StripElement, all render fns. `crates/strop-core/src/document.rs` (4,069)
and `store.rs` (2,053) hold the model. All line anchors below are exact on
this commit.

---

## 1. The existing read-only takeover: history preview / "parked"

This is the pattern the cold read imitates (and the history-preview
variant literally reuses).

**State.**
- `Editor.history_preview: Option<PreviewDoc>` — editor.rs:958. The single
  read-only-takeover channel; both the history panel and the strip scrub
  write it; paint reads it.
- `PreviewDoc` — editor.rs:13924–13936: `text: String`,
  `inserts`/`deletes: Vec<Range<usize>>` (diff tints), `spans_bytes:
  Vec<(Range<usize>, InlineAttr)>`, `kinds: Vec<BlockKind>`, `boundary:
  Option<(BoundaryEra, usize)>` — the *previewed state's own* scraps seam,
  drawn read-only.
- `Strip` — strip.rs:912–940: `open: bool` (:913), `parked: bool` (:922),
  `scrubbing`, `pos_ms`, `pin_ms`, `bake: Option<StripBake>`, `scratch:
  Option<ScrubDoc>`, `words_at`. Gate: `Strip::is_parked()` —
  strip.rs:963–965 (`open && parked`).

**Enter / exit.**
- Enter parked: `strip_park_at_x(x, pin, cx)` — editor.rs:2596 (sets
  `parked=true; scrubbing=true`, then `strip_scrub_to`); wired from the
  rail mousedown at 16949–16955. Rig entry `debug_strip_scrub` —
  editor.rs:9166.
- Exit to now (strip stays): `strip_return_to_now` — editor.rs:2753–2766
  (clears parked/scrubbing/pin/scratch and `history_preview=None`).
- Close strip: `close_strip` — editor.rs:2452–2464 (also clears preview).
- Panel variant: `enter_history` — editor.rs:1977–2027; `exit_history` —
  editor.rs:2075.

**Render switch.** All inside EditorElement prepaint:
- Master switch: `let preview = editor.history_preview.clone(); let
  in_history = preview.is_some();` — editor.rs:10301–10302.
- Text/spans/kinds swap — editor.rs:10433–10436. Selection forced `0..0`
  (10437–10441), marked `None`, cursor blink off (10449). Find matches
  reuse the sage tint for diff inserts (10500–10501).
- Scraps seam swap: `boundary = if in_history { preview.boundary } else {
  doc.boundary() }` — editor.rs:10546–10553; graveyard tail emptied at
  10555–10556. This is how a *past* state's seam draws read-only.
- Layout-cache interaction: the preview **vetoes** the reuse fast-path
  (`can_reuse = !in_history && …` — editor.rs:10346–10355); a parked frame
  always full-rebuilds. There is no preview-revision key — just the veto.
- Mouse hit-testing clamps to preview text: `index_for_mouse` —
  editor.rs:8029–8060 (max = preview len at 8039–8042).

**Edit refusal (Bug B pulse).** Central sink `pulse_strip` —
editor.rs:2775–2800 (consts STRIP_PULSE_MS/FRAMES at 229–230). Every
mutation entry checks `if self.strip.is_parked() { self.pulse_strip(cx);
return; }`: `apply_replace` 9350–9359, `toggle_span` 5354,
`open_link_input` 5384, `toggle_block` 7665, `cut` 7786, `undo` 7886,
`redo` 7910, `scraps_travel` 6008. **Copy is deliberately allowed** —
`copy` at 7760–7781 slices from `history_preview.text`. Navigation and
selection are unguarded (they act on the preview text).

**Banner.** `render_strip_banner` — editor.rs:12850–12932. Returns None
unless parked. DOM: `absolute().top(px(BAR_HEIGHT)).left_0().right_0()
.h(px(30.))`, bg 0xEFEEEA, border_b RULE — a full-width row flush under
the titlebar (mounted in the root at 16617–16620). Strings, left→right:
bold moment label in a seltint chip (station name if playhead within 5px
of a labeled tick, else `strip::format_moment` — 12859–12868) · `"· {N}
words"` (from `strip.words_at`) · dark `Restore` chip (12877–12896) · `·`
· `"Esc returns"` (12930). Refusal pulse tints the moment chip
(12872–12875). The panel sibling `render_history_banner` —
editor.rs:12780–12840: `"Viewing:"` · `"{name} · {stamp}"` · `"Restore
this version"` · `"Esc to exit"`.

**Restore.**
- `restore_selected(cx)` — editor.rs:2341–2355 (panel).
- `strip_restore(cx)` — editor.rs:2856–2883 (strip; guards `is_parked`).
- Shared mechanism `restore_to_state(text: String, spans: SpanSet, blocks:
  BlockMap, from_unix: i64, cx)` — editor.rs:2369–2419: save_now →
  `add_checkpoint("Before restore")` (2385) → `doc.restore_state` (2386)
  → `exit_history` (drops both previews, 2395) →
  `migrate_orphans_after_restore` (2399) → journal `Restore` event
  (2406–2412) → save + `add_checkpoint("Restored")` (2413–2416). A cold
  history-preview Restore chip calls exactly this.

**Esc/find/scroll inside it.** `escape_mode` — editor.rs:6940–7008; the
strip branch at 6997–7004 (parked → return-to-now; at-now → close). Find
(`fn find` 4404) is not parked-guarded, but layout forces `find_query =
None` while `in_history` (10331–10332). Scroll: single root
`on_scroll_wheel` at 16399, unguarded — the preview scrolls normally.

**Surface suppression while previewing** (predicate is uniformly
`history_view.is_some() || strip.is_parked()`):
- Margin lane + rail skipped — root render editor.rs:16575–16585.
- Narrow-notes pill / panel — 16035 / 16109.
- Editor-menu rows inert — `inert = history_preview.is_some() || running`
  at 15868 (H33: never diagnose a document the screen isn't showing).
- Flanks — `history_up = history_view || strip.open ||
  history_preview.is_some()` at 11636 (note: `strip.open`, stricter).
- Footer chips — hidden on `history_view` only (12623); **not** on
  `is_parked` — an existing gap the cold-read predicate should not copy.
- Rig gate: `"margin_hidden"` at 8632; `"banner": strip.parked` at 8626.

**Past-state materialization.**
- Panel diff path: `rebuild_preview` — editor.rs:2079–2211 (diff via
  `prose_diff_blocks` 2151; writes PreviewDoc at 2201–2210 with the newer
  side's boundary).
- Strip scrub path: `strip_reconstruct(pos_ms)` — editor.rs:2656–2748:
  anchors on the latest materialized checkpoint ≤ t
  (`checkpoint_state_at_ms` 2679), replays the journal through a
  `ReplayDoc` scratch (`ScrubDoc` strip.rs:899–908, advanced
  incrementally), emits a PreviewDoc with **empty** inserts/deletes ("the
  past as it was, not a diff") at 2737–2746. This no-diff projection is
  the natural feed for the paged "read this version" renderer.
- Legacy backfill: `backfill_checkpoint_states` — editor.rs:2034–2071
  (off-thread `Store::materialize_checkpoint_states`, write-back via
  `set_checkpoint_state`).

---

## 2. Text shaping machinery & the layout-reuse keystone

**Custom Element precedents.** Two full `impl Element` in the app:
`EditorElement` (prepaint ~editor.rs:10240–10990, paint ~11000–11309) and
`StripElement` (editor.rs:17042 on; prepaint 17068). A ColdRead paged
Element is a third sibling. Both wrap phases in `DrawGuard::enter()`
(editor.rs:17061, 17077; guard in `draw_guard.rs`) and use
`update_in_draw` (`EntityUpdateExt`, draw_guard.rs) for mid-draw entity
writes.

**shape_line at arbitrary origin — the footnote-superscript proof.**
- Shaped in prepaint: fn_marks built at editor.rs:10739–10773 —
  `TextRun { len, font: gpui::font("PT Serif"), color, … }`, then
  `window.text_system().shape_line(label, bstyle.size * 0.65, &[run],
  None)` with the comment "Shaped HERE, in prepaint: never shape during
  paint."
- Painted at arbitrary origin with manual baseline math:
  editor.rs:11168–11191 (`shaped.paint(bounds.origin + point(x, line_y +
  dy), …)`; dy computed from ascent/descent to raise the small baseline
  35% of the font size). This is exactly the paint-at-pen-position
  mechanic a justified line of per-word ShapedLines needs.
- Other shape sites: word-shape helper returning `gpui::ShapedLine` at
  10028–10050; body paragraphs via `shape_text` at 10164–10174 and
  10837–10847; seam row 10606; strip labels 17093–17104; card height
  measurement `shape_text_height_w` 14391–14410; TextField 13441–13456.
  A standalone shaping playground exists:
  `crates/strop-app/examples/shape_audit.rs`.

**The layout-reuse keystone (perf work — do not regress).**
- `LayoutKey` — editor.rs:9624–9650: `revision` (=`Document::revision()`,
  document.rs:1249/1283 — bumped by every model mutation, e.g.
  document.rs:1455), `width_bits`, `font_scale_bits`, `selection`,
  `marked`, `find_query`, `active_note`, `scrap_flash`, `scrap_words`,
  `grave_fingerprint`.
- Fast path: editor.rs:10304–10430. `can_reuse` (10346–10355) requires
  `!in_history`, no Image blocks, and `last_frame.layout_key == key`;
  paragraphs are `std::mem::take`n out of `Editor.last_frame`
  (10364–10377) and moved back at paint end (11296–11308). Geometry
  changes request one follow-up frame (11287–11295).
- Per-block reuse inside a full rebuild: `ParagraphLayout.runs:
  Vec<TextRun>` — editor.rs:1349–1352 — compared to skip re-shaping
  unchanged blocks.
- `ParagraphLayout` — editor.rs:1308–1353; `PrepaintState` — 9652–9662.

**Invariants a NEW paged layout cache must respect:**
1. Shape only in prepaint, never in paint (the 2026-06-12 sprite/atlas
   corruption rule — editor.rs:1339–1343, 17024–17026). Never
   `cx.notify()` mid-draw; use `update_in_draw` + deferred notify.
2. Never touch `Editor.last_frame` or its LayoutKey — the paged view keeps
   its own cache (spec 05 §0: "the per-block `shape_text` cache doesn't
   apply").
3. Key the paged cache on at least: source identity (live
   `Document::revision()` for the live cold read; checkpoint index /
   `created_unix` for the history variant), page box size, and
   `font_scale` (`config.font_size` /20 clamped 0.6–2.0 —
   editor.rs:10317–10321) — plus the book face size if independently
   configurable.
4. Read-only means `Document::revision` never bumps during the takeover,
   so a revision-keyed cache is stable for the whole session; filing a
   reaction note DOES bump revision (document.rs:2308 path) — the page
   layout must not depend on note ranges or must tolerate the bump.
5. Block metrics live in `block_style` — editor.rs:9715 (body: PT Serif
   20px / 28px line height, defaults at 9688–9702; headings PT Sans bold);
   `block_style_scaled` 9704–9713 keeps the 2px rhythm.

**What gpui gives / doesn't give** (verified in the fork the pinned rev
matches; see §3 for provenance):
- `WindowTextSystem::shape_line(text: SharedString, font_size: Pixels,
  runs: &[TextRun], force_width: Option<Pixels>) -> ShapedLine` —
  crates/gpui/src/text_system.rs:397 (panics on `\n`; `force_width` is
  the only width knob — total-advance forcing, not tracking).
- `ShapedLine` (derefs to `Arc<LineLayout>`) —
  text_system/line.rs:43: `paint(origin, line_height, align, align_width,
  window, cx)` :83, `paint_background` :108, `split_at(byte)` :141;
  `LineLayout` — line_layout.rs:16: `width`, `ascent`, `descent`,
  `index_for_x` :58, `x_for_index` :105.
- Line breaking: `LineWrapper::wrap_line(fragments: &[LineFragment],
  wrap_width: Pixels) -> impl Iterator<Item = Boundary>` —
  text_system/line_wrapper.rs:40 (`Boundary { ix, next_indent }` :665;
  `LineFragment::Text | Element{width, len_utf8}` :605); per-char width
  cache `width_for_char` :484. Pooled via `TextSystem::line_wrapper`
  (text_system.rs:850).
- **No justify** (`TextAlign` = Left/Center/Right only; the paint match at
  text_system/line.rs:745–749 has no Justify arm), **no hyphenation**,
  **no letter-/word-spacing** (`TextRun` — text_system.rs:987 — has no
  spacing fields; `FontFeatures` is OpenType tags only). Confirms spec 05
  §0: justification = per-word `shape_line` + slack distributed across
  gaps + paint each word at its own pen position.

---

## 3. Fonts

**What the editor uses today.** PT superfamily (ParaType, OFL), bundled in
`assets/fonts/` (PTSerif/PTSans 4 styles each + PTMono-Regular, plus the
OFL texts). Embedded via `include_bytes!` — main.rs:53–61 — and registered
at startup: `cx.text_system().add_fonts(vec![…]).expect(…)` —
main.rs:119–131. Body prose = **PT Serif 20px / 28px line height**
(block_style default, editor.rs:9689–9691); headings PT Sans Bold
(9715–9724); UI chrome PT Sans (e.g. 15963); mono PT Mono. User size
override: `config.font_size: Option<f32>` — config.rs:19. History note:
main.rs:46–52 — Literata was replaced after variable-font-derived statics
corrupted in gpui's shaping/atlas path; **prefer static-instance TTFs for
any new face**.

**gpui registration API** (fork provenance updated 2026-07-13: Strop's root
Cargo.toml pins `gpui`/`gpui_platform` to `github.com/kirushik/zed` rev
`994cdfd1`, branch `strop-patches-on-main`, based on stable `v1.10.2` plus the
four patches documented in `docs/gpui-fork.md`):
- `TextSystem::add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) ->
  Result<()>` — crates/gpui/src/text_system.rs:102; trait method
  platform.rs:813. Platform impls: Linux cosmic-text
  gpui_wgpu/src/cosmic_text_system.rs:93→209 (`fontdb::load_font_data`),
  macOS gpui_macos/src/text_system.rs:98→256 (font-kit MemSource),
  Windows gpui_windows/src/direct_write.rs:227→350. Registers face data
  only; resolution stays by family name.

**Fallback when a family is missing.** `TextSystem::resolve_font` —
text_system.rs:148–159: tries the family, then walks
`fallback_font_stack` (built at :71–83: `.ZedMono`/`.ZedSans` aliases →
IBM Plex Sans / Lilex, then Helvetica / Segoe UI / Ubuntu / Noto Sans /
DejaVu Sans / Arial…), **panics only if everything fails**. So an
unbundled "Bookman" **silently degrades** to a sans fallback — no error
surfaces. Conclusion for a Bookman-class page face: bundling (the PT
pattern above) is the only reliable cross-platform route; system
resolution is a silent-wrong-face trap. (Also: the doc corpus is
bilingual — the face must carry full Cyrillic, which PT Serif already
does; most free Bookman-alikes, e.g. TeX Gyre Bonum, do not. See risks.)

---

## 4. Margin notes (the "reaction" target)

**Creation path (ctrl-m).** Binding: commands.rs:270–276 ("Add Margin
Note", editor-scoped per `Command::global` — commands.rs:50). Action
wiring `.on_action(cx.listener(Self::add_note))` — editor.rs:16480.
Handler `fn add_note(&mut self, _: &AddNote, window, cx)` —
editor.rs:3209–3229: range = `selected_range` if non-empty else
`word_range_at(cursor_offset())`; empty → return. Byte→char via
`rope.byte_to_char`; `self.doc.add_note(char_range, String::new(), now)`;
then `open_composer(id, …)`. Second entry: selection-flank `SelVerb::Note`
→ same handler (editor.rs:11923, 11977).

**Model** (strop-core/src/document.rs):
- `NoteStatus` — :676–680 (`Open`, `Done`, `Dismissed`); the doc comment
  at :665–674 warns a new enum **variant** breaks old builds' whole
  annotations parse.
- `NoteKind` — :685–689 (`#[default] Note`, `Diagnosis`).
- `Annotation` — :701–724: `id: u64`, `range: Range<usize>` (**char
  offsets, whole-doc coords**), `body`, `status`, `created_unix`, plus
  `#[serde(default)]`: `kind`, `title`, `level`, `orphaned`, `pass_id`,
  `unverified`. No quote/excerpt field — covered text derives from
  `range`.
- `Annotations` — :730–733; `add(range, body, created_unix) -> u64` :736
  (hardcodes `NoteKind::Note`); `push(annotation) -> u64` :796 (assigns
  id; the diagnosis path); `set_status` :764; `set_body` :777 (writer
  notes only); `apply_op` :831; `reanchor` :892.

**Programmatic API a reaction calls.**
`Document::add_note(&mut self, range: Range<usize>, body: String,
created_unix: i64) -> u64` — document.rs:2308 (bumps revision, own undo
step). To file with a non-default kind you'd add a Document method
wrapping `Annotations::push` (precedent: `Document::add_diagnoses` —
document.rs:2352). Related: `set_note_body` :2326, `set_note_body_draft`
:2339, `set_note_status` :2317.

**Anchoring by content.**
- Incremental edits: `Annotations::apply_op` — document.rs:831 —
  non-expanding span shift; a deleted anchor collapses to a zero-width
  point; writer notes never decay (diagnoses get `unverified`).
- Wholesale swap (restore): `Annotations::reanchor(old_text, new_text)` —
  document.rs:892–912 — slices each note's covered substring from the old
  text and calls `diagnose::anchor(new_text, &covered, hint)`
  (diagnose.rs:134–150: substring search from the former offset, fallback
  from zero). Miss → `range = p..p`, `orphaned = true`. Runs inside
  `Document::restore_state` (document.rs:2552, installed :2609). Not run
  at open. Editor follow-up moves doomed notes' text to Scraps:
  `note_is_doomed` editor.rs:6597, `perform_orphan_migrations` :6627.
- The cold read's "reaction anchored by content" gets this behavior for
  free by storing a normal char-range note: content re-anchoring only
  engages at restores; ordinary edits shift the range like any span.

**Coordinate interplay (paged view works in manuscript space).** Notes are
stored whole-doc; the manuscript slice rebases to 0. The established
convention is the diagnosis anchor code — editor.rs:3665–3689: rebase in
by subtracting `manuscript_base_char()`, anchor within `mtext`, add the
base back to every produced range (:3688–3689). A reaction filed from a
page selection must do the same (+base; base is 0 in the tail era).

**Margin-lane display.** `MarginCard` — editor.rs:13967–13997; built in
`margin_cards(cull)` — :14474 (construction :14562–14577). Header label
`note_card_label(is_diagnosis, level, orphaned)` — :14001 ("Note" /
level, "· detached" when orphaned). Tint: notes warm
`NOTE_CARD_BG=0xFAF4E2`, diagnoses cool `DIAGNOSIS_CARD_BG=0xEEF3FB`,
stale `STALE_BG` (theme.rs:73/78/82; applied editor.rs:14310–14316 and
15517–15523; corner radius 9 vs 3 at 14309/15510). Composer:
`CardFocus::Composing { id, input: Entity<TextField> }` (enum
:641–652); `open_composer(id, body, window, cx)` — :3277
(`TextField::multiline`, Enter/Esc subscriptions, focus);
single commit exit `resolve_composer` — :3236 (`doc.set_note_body`);
`finish_composing` — :3257; draft heartbeat :1773.

**One-glyph reaction prefix (? ! ~) — both options, no decision.**
Persistence site: the annotations JSON channel —
store.rs:27 (`ANNOTATIONS_CONTAINER`), save :837–841
(`serde_json::to_string(annotations)` under key `"list"`,
fingerprint-gated), load :463–470 — **a failed parse silently drops all
annotations** (`_ => Annotations::default()`), which is why every added
field uses `#[serde(default)]`.
- (a) Schema change. A new `NoteKind` **variant** is the documented
  anti-pattern (old builds fail the whole parse → all notes vanish on
  downgrade). A new `#[serde(default)]` **field** (e.g. `reaction:
  Option<char>` or `String`) is the sanctioned pattern — old files load
  with the default; old builds ignore unknown fields
  (no `deny_unknown_fields`). Touch points: `Annotations::add`
  (document.rs:739–751) or a new add-variant, `card_height_key`
  (editor.rs:14017) if the glyph affects height, the card render sites.
- (b) Body convention (body starts with the glyph). Zero schema change;
  special-casing at the body render arms (editor.rs:15628–15636 and
  :14340) and, if the glyph should drive the label instead of appearing
  inline, `note_card_label` (:14001). Fragile against the writer editing
  the body's first char; survives round-trips trivially.

---

## 5. Checkpoints

**The struct** (this branch): `Checkpoint` — store.rs:60–77: `name:
String`, `created_unix: i64` (seconds), `frontiers` (Loro),
`manual: bool`, `state: Option<CheckpointState>`. `CheckpointState` —
:82 (`text, spans, blocks` — the materialized copy). **There is no
kind/label enum on this branch** — "kinds" are name conventions ranked by
`station_rank(name, manual)` — strip.rs:681–695 ("Before restore" /
"Restored" / "Exported" / seal / "Session start" / "Checkpoint N" =
reflex tier / manual = writer-named).

**Fingerprint-guarded (unchanged → no checkpoint):**
- `add_checkpoint(name, manual)` — store.rs:559 (state materialized NOW).
- `add_checkpoint_if_changed(name, manual)` — store.rs:658 → delegates to
  `seal_session_with` — store.rs:668–688: full `(text, spans, blocks)`
  equality against the LAST checkpoint's materialized state; equal →
  no-op; a legacy checkpoint mid-migration defers. **This is the
  cold-read-entry call**: `store.add_checkpoint_if_changed("<name>",
  false)` after a `save_now()` (the save-first pattern is
  `restore_to_state` — editor.rs:2377–2385).
- Save-channel fingerprints (a different guard — unchanged save = 0
  bytes): `fingerprint(&str)` store.rs:366, `blocks_fingerprint` :378,
  `SavedHashes` around :355, used throughout `save_with_state` :796+.

**Materialized states:** `checkpoint_state(cp)` — store.rs:693 (instant
when materialized; legacy `state_at` fallback); `checkpoints_materialized`
:702; backfill `materialize_checkpoint_states(bytes)` :751 +
`set_checkpoint_state` :709 (insert-before-delete, immutable once set);
rig backdater `debug_push_checkpoint` :600; `rename_checkpoint` :629
(renaming makes it manual).

**Existing callers:** open-time seal `add_checkpoint_if_changed("Session",
false)` — editor.rs:1683; manual `AddCheckpoint` (ctrl-alt-s) → `name =
"Checkpoint {n}"` — editor.rs:1908–1915; restore brackets "Before
restore"/"Restored" — editor.rs:2385/2416.

**Design-only (NOT in code on this branch):** `CheckpointMeta`
(intent/goal/cards/passes), seal/submitted **kinds**, the Ctrl+S reflex
binding, Export auto-checkpoint with filename-derived labels — all
golden-path D8/D9 (docs/golden-path.md:441–466). The commit that built
the checkpoint-flow redesign (69da501) is **not an ancestor** of
`cold-read` — it lives on the golden-path lineage. Note there is no
`ctrl-s` binding anywhere in the app today (grep: zero hits); saves are
continuous (`save_now` — editor.rs:5284). A cold-read entry checkpoint on
this branch = a name convention (e.g. "Cold read", manual=false, ranked
like reflex unless station_rank learns the name).

---

## 6. App modes/state — where ColdRead sits

**The Editor's mode-ish fields** (all on `Editor`, editor.rs):
`focus: CardFocus` :973 (FSM at :641–652: Idle / Selected(id) /
Composing{id, input}); `drafting: bool` :1041–1050 (the door; `true` =
door closed / Away, GENERATE; `false` = Reading, EVALUATE — ctrl-m notes
are never hidden by it); `history_view: Option<…>` :956 +
`history_preview` :958; `strip: Strip` :1232; `palette_input`
(omnibar), `ai_settings`, `shortcuts_open`, `editor_menu_open` :1112,
`selection_popover`, `link_input`, `excursion` (Scraps latch),
`deferred_pass` :180 (a completed pass parked until the lull).

**The door.** `toggle_review` (ctrl-shift-r) — editor.rs:3395 →
`toggle_door` :3403 (flush-and-flip: a parked pass lands NOW, then
`drafting = !drafting`). Reaching for a resting diagnosis opens it
(:8317–8322); running a pass opens it (:3548, :9016). Face:
`EditorFace` :759–772 + `face_for` :801 (priority: NeedsSetup > Error >
Cooking > Ready > Reading·N > Idle); the door word is the *presence pair*
Reading/Away (menu footer :15908–15944).

**Esc precedence stack** (`escape_mode` — editor.rs:6940–7008, binding
"escape"→EscapeMode :561): ai_settings → palette → shortcuts →
editor_menu → link_input → selection_popover → Scraps excursion latch →
strip (parked→now, open→close) → history_view. A ColdRead takeover slots
its exit at the very top of this fn (nothing else should be reachable
inside the takeover anyway) — or, cleaner, in its own key context (see
§9) so `escape_mode` never fires.

**Where ColdRead sits.** The precedent is *state-on-Editor + render
switch*, not a routed enum: `Option<ColdReadState>` beside
`history_preview`/`strip`, checked by the root render
(editor.rs:16380–16660 region) the way the strip overlay and banners are.
Both existing takeovers keep the titlebar and swap/suppress everything
under it.

**Surfaces to suppress** (the parked precedent, §1): margin lane + rail
(16575–16585), narrow pill/panel (16035/16109), flanks (11636), footer
chips (12623 — currently history_view-only; a ColdRead predicate must
include itself), scraps seam + graveyard (the paged view consumes
`manuscript_slice`, so the pile never enters at all), history strip
(mutually exclusive — decide: entering cold read closes the strip or is
refused while parked), selection popover, editor-menu (rows inert at
15868 or menu closed). The omnibar/palette: global App-context chords
(ctrl-shift-p, ctrl-f) fire from any focus — a ColdRead focus context
does NOT stop App-context bindings, so either close the takeover on
palette open, or guard those handlers (recon flag, see risks).

**reduce_motion.** `Config.reduce_motion` — config.rs:20–25 (writer's own
switch; gpui exposes no OS query). Rig flip `reduce:motion` —
smoke.rs:386–395; exposed in dump at editor.rs:8598. Page-flip animation
must honor it (cross-fade, never teleport — the established law,
editor.rs:14253–14282).

---

## 7. The region API (the paged view's input)

All strop-core/src/document.rs, char offsets:
- `manuscript_base_char() -> usize` — :1469 (= range start; 0 in tail
  era, >0 only in the legacy top era).
- `manuscript_end_char()` — :1474; `manuscript_char_range() ->
  Range<usize>` — :1481 (`None` boundary → whole doc).
- `manuscript_slice() -> (String, SpanSet, BlockMap)` — :1517–1550. The
  triple is **rebased to 0** — directly consumable by a paged layout.
  Era match: no boundary → clones the whole doc; Tail → rope slice +
  `SpanSet::slice(range)` + kinds truncated at the boundary; Top → rebase
  spans by `base`. "Add `manuscript_base_char()` back to any range that
  must return to full-document coordinates" (doc comment :1510–1516) —
  i.e. the reaction-filing path (§4).
- Also available: `scraps_char_range()` :1486, `region_of_char(ch)` :1491,
  `scraps_textless()` :1498. Word-count usage example: editor.rs:1710
  (`count_words(rope.slice(manuscript_char_range()).chunks())`).
- The history variant feeds the same renderer from
  `store.checkpoint_state(cp)` (store.rs:693) or the strip's
  `strip_reconstruct` PreviewDoc (§1) — note a checkpoint state's own
  boundary must be respected (manuscript = up to ITS boundary, cf.
  `strip_reconstruct`'s own-range word count editor.rs:2713–2722).

---

## 8. The visual rig — where `coldread:open` plugs in

**Driver.** `STROP_SMOKE` token scripts, replayed by
`smoke::maybe_run(window, cx)` — smoke.rs:30–554. Grammar: one
`if key == "…"` / `strip_prefix` branch per token, each calling a
`pub fn debug_*` on Editor inside `window.update`, then an 80–150ms
timer + an eprintln marker. Existing seeds: `seed:diag` :110
(`debug_seed_notes` editor.rs:8691), `seed:many` :258, `seed:deliver`
:271 (`debug_deliver_pass` — the real arrival gate), `seed:journal` :309,
`seed:legacy` :322 (backdated checkpoints), `seed:demo`/`seed:aside`/
`seed:topera`/`seed:annotated`/`seed:mockup1|2|3`. Generic tokens:
keystrokes (`Keystroke::parse` :530), `click:X,Y[,N]` :436, `drag:` :476,
`wheel:X,Y,DY` :67, `wait:MS` :419, `reduce:motion` :386, `dump:ui` :427.
`STROP_SMOKE_HOLD=1` keeps the window for screenshots (:546–551).

**A `coldread:open` hook** = one smoke.rs branch → `editor.debug_…` (or
dispatching the real verb action, the `scraps:travel` pattern :98–109) →
new keys in `debug_ui_dump`.

**dump:ui.** `debug_ui_dump(window, cx) -> String` — editor.rs:8456–8660:
a `serde_json::json!` with `overlays` (ai_settings/palette/shortcuts/
popover/replace/history/strip — :8457–8478), `focused`, `scroll_y`,
`doc_chars`, `doc_hash`, `margin{visible,above,below,overlap,…}`
(:8536–8553), `omni{…}`, `editor_btn{face,open:!drafting,…}` :8599–8604,
`strip{…, banner: parked, pulse}` :8612–8627, `margin_hidden` :8632,
scraps latch/chip keys :8520–8535. NOTE the macro-recursion comment at
:8520–8521 — hoist a new `coldread` sub-object into a `let` like
`margin`. Page-count assertion = `"coldread": {"open": true, "pages": N,
"page": i}`.

**Assertions.** `scripts/rig-check.sh` (473 lines): fixtures via `mktemp`
+ here-docs (:16–48), helpers `field()` (:51 — greps `"key":value` out of
the UI-DUMP JSON) and `expect()` (:52–54), sections = `echo` + `WRUN_TAIL=N
scripts/wrun.sh "$DOC" "<tokens>" | grep UI-DUMP` + expect lines; exits
non-zero on first failure. `scripts/wrun.sh`: headless sway
(WLR_BACKENDS=headless, pixman) + `timeout 60` + tail. Screenshots:
`scripts/wshot.sh out.png scale [doc] [tokens…]` — headless sway + grim;
`WSHOT_MODE=WxH` sets window width, `WSHOT_FLOAT` for CSD. There is **no
automated pixel-diff golden harness** — "wshot golden of a rendered page"
(spec 05 §2) means a committed reference PNG reviewed by eye/agent
(docs/VISUAL-RIG.md; note its "capture twice, keep the second" and
pixel-diff gotchas).

---

## 9. Keybindings & verbs — where "Read it cold" registers

**One table drives keymap + palette + cheatsheet:** `commands.rs`.
`Command { label, aliases (incl. Russian), section, keys, make }` —
commands.rs:21–30; the table `all()` :70–376; App-vs-Editor scoping
`Command::global` :42–54 (View/History/Session/Help/File → App context =
fires from any focus). Fuzzy scoring :381–425, frequency boost :433–471.
**The registration recipe** (the "Scraps" travel verb, freshest example —
commands.rs:196–205): (1) add the action name to the `actions!` block —
editor.rs:455–474; (2) a `cmd!` row with label + aliases (bilingual) +
optional chord; (3) `.on_action(cx.listener(Self::…))` in the root render
— editor.rs:16470–16501; (4) the handler. Section for a "Read it cold"
verb: "View" (like Scraps) — recon note, not a decision.

**Keymap mechanics.** `bind_keys(cx)` — editor.rs:476–509: registry
commands bind via `KeyBinding::load` with an "App" or "Editor" context
predicate; raw movement/selection/clipboard bindings :509–561 (incl.
`pageup`/`pagedown` → PageUp/PageDown actions :541–544, arrows :516–519,
`escape` → EscapeMode :561). **Context-scoped bindings are the precedent
for a ColdRead context**: `Some("NoteInput")` / `Some("NoteComposer")`
rows at :564–580 — a `Some("ColdRead")` context would give the takeover
its own Esc/arrows/PageUp bindings without touching the editor's, keyed
off the takeover's own `key_context`.

**ctrl-shift-<letter> chords taken** (commands.rs unless noted):
`s` SaveCopyAs :98 · `e` ExportMarkdown :105 · `z` Redo :118 · `x`
Strikethrough :137 · `h` Highlight :144 · `8`/`7` lists :178/:185 · `o`
ScrapsTravel :202 · `a` SetAside :212 · `g` SendToGraveyard :235 · `d`
RunDiagnosis :249 · `b` RunBelieving :256 · `r` ToggleReview :266 · `p`
TogglePalette :356; movement (editor.rs): ctrl-shift-up/down/left/right
:524–531, ctrl-shift-home/end :539–540. **Free letters: c f i j k l m n q
t u v w y** (c/v carry terminal copy/paste muscle memory; k/l/t/w are
clean).

**Omnibar.** The palette IS the titlebar omnibar (`palette_input`,
`omni_mode` — find `/`, commands `>`, headings `@`); rows from
`commands::ranked_with_freq`. A verb needs nothing omnibar-specific —
the table row is the registration.

---

## 10. Titlebar & banner

`render_titlebar(window, cx)` — editor.rs:12028–12534. `BAR_HEIGHT` tall,
three `flex_1` thirds:
- Left (:12054–12183): macOS traffic-light spacer; **doc name** chip
  `"doc-title"` (rename on click/F2) :12072–12111; **word-count pill**
  `"word-count"` (`count_label()` + session-goal delta, sage dot at goal)
  :12114–12182.
- Center (:12184–12284): the **omnibar** — live TextField + find-match
  counter, or the rest-state Search pill `"omni-pill"` → `find`.
- Right (:12287–12533): **editor button** `"editor-btn"` :12299–12417 —
  label from `EditorFace`; `Reading` renders "Reading · N open · Ask the
  editor" :12311–12314 — the existing precedent for a mode word living in
  the titlebar; palette hamburger :12421–12449; **history clock**
  `"history-toggle"` → `toggle_strip`, lit while `strip.open`
  :12450–12497; window controls off-macOS :12503–12532.

**Banner-row-under-titlebar pattern exists twice** and is the exact model
for the cold-read banner (lab v4: data-only strings, Reading chip pulses
on typing): `render_history_banner` — :12780–12840 ("Viewing: {name} ·
{stamp} · Restore this version · Esc to exit") and `render_strip_banner`
— :12850–12932 (§1). Both `absolute().top(px(BAR_HEIGHT))`, h 30,
border_b RULE, mounted after the titlebar in the root (16553,
16617–16620). A "— reading" label could live either as a third banner row
(consistent) or in the doc-name third (:12072 area) — recon only.

---

## 11. The editor-button pass menu ("Ask the editor for…")

`render_editor_menu(window, cx)` — editor.rs:15855–16020; row helper
`editor_menu_row` :15739. Header "Ask the editor for…" :15975. Entries
:15977–16016: **"A believing read"** (PassKind::Believing) :15977–15984 ·
"A developmental read" · "A line read" · "A copy read" (data-driven gate
`copy_gated()` :15873–15875) · "A doubting read" (PassKind::Doubting).
`PassKind` enum :707–750 (`Believing` :719–720, journal mode string
"believing" :733). Rows go **inert while the past is on screen**:
`inert = self.history_preview.is_some() || running` — :15868 (review H33)
— the cold read must extend or inherit this gate. Footer :15893–15945:
"{n} queries open · {m} resolved" + the Reading·Away door toggle
(→ `toggle_door`). Menu geometry: glued under the button's painted right
edge (`editor_btn_right` :1113–1116, est fallback :15883–15886), 430px
one-line law :15887–15891.

**"Offer" mechanics:** none exist in code. The believing-read offer after
a seal ("The draft is resting — sealed yesterday, 4,120 words. Read it
cold?") is golden-path **D8 design** (docs/golden-path.md:441–455), three
CheckpointMeta-driven sentences in the pass-menu header — blocked on the
CheckpointMeta layer that isn't on this branch (§5). The menu's `when`
qualifier line on the copy row (:16001–16008, `copy_gate`) is the closest
existing mechanic for a contextual sentence in the menu.

---

## Risks & unknowns

1. **The checkpoint-kind layer is on another branch.** CheckpointMeta,
   seal/submitted kinds, label stamps, the Ctrl+S reflex (69da501,
   golden-path lineage) are NOT ancestors of `cold-read`. Entry
   checkpoint here = name-convention via `add_checkpoint_if_changed`;
   `station_rank` (strip.rs:681) won't recognize a "Cold read" name
   (falls to writer-named rank if manual, or needs a new arm). Rebase or
   convention — unresolved.
2. **Fingerprint-guard semantics**: `add_checkpoint_if_changed` compares
   against the **last** checkpoint only (store.rs:674–686). Repeated
   enter/exit of the cold read with edits between produces a checkpoint
   each time (correct), but entering twice with NO edits after some other
   auto-checkpoint still dedupes only against the newest — acceptable,
   but verify the intended UX.
3. **App-context chords pierce a focus-scoped takeover.** Global bindings
   (ctrl-shift-p palette, ctrl-f find, ctrl-alt-h strip, ctrl-shift-d
   pass…) bind to the root "App" context (editor.rs:488–508) and will
   fire inside a ColdRead key context. Each needs an explicit
   guard-or-exit decision (the parked mode leaves them live and only
   guards mutations).
4. **Footer-chips suppression gap**: `render_footer_chips` hides on
   `history_view` only (12623), not `is_parked` — don't copy the
   predicate; possibly fix in passing.
5. **Font choice constraints**: no Bookman-class face bundled; gpui
   silently falls back on a missing family (resolve_font), so bundling is
   mandatory. The corpus is bilingual — full Cyrillic required; TeX Gyre
   Bonum (the usual free Bookman) has no Cyrillic; PT Serif does but is
   the everyday desk face (defeats "this is a different room"). Candidate
   search (OFL + Cyrillic + bookish) is open. Also: static TTFs only
   (the Literata corruption history, main.rs:46–52).
6. **hyphenation crate not yet a dependency**; deny.toml/licensing pass
   pending (`hyphenation_commons` license to verify — spec 05 §0), and
   the runtime-loaded ru dictionary needs a packaging home (assets/ vs
   config dir) — the LPPL posture requires it NOT be compiled in
   (ux-glossary appendix). Where runtime data files ship cross-platform
   is unsolved (no precedent; fonts are include_bytes!).
7. **Selection on a justified page is greenfield.** No precedent for
   hit-testing across per-word ShapedLines painted at manual pen
   positions; word-box selection (spec 05 §0.3) implies a custom hit map
   (the GraveLine `hits: Vec<(Bounds, Action)>` pattern —
   editor.rs:1286–1306 — is the nearest precedent).
8. **Reaction filing bumps `Document::revision`** while the takeover
   shows a static page; the paged cache must not invalidate on note-only
   revisions (or must tolerate a re-layout that changes nothing
   visually). Key the cache on text/spans/blocks identity, not raw
   revision — or accept the rebuild.
9. **History-variant scope**: bind the paged preview to materialized
   checkpoints only (store.checkpoint_state — instant), or also to
   arbitrary strip moments (strip_reconstruct — replay cost, ScrubDoc
   cache)? Unresolved.
10. **Deferred-pass interplay**: entering the takeover while
    `deferred_pass` is parked — the door/scroll flush rules
    (editor.rs:3749–3782) will land cards into a hidden margin.
    Harmless (they show on exit) but the reveal-clock semantics during a
    cold read are undesigned.
11. **Page texture / "bookish" rendering**: only paint_quad fills and
    RenderImage exist; no texture/asset pipeline for a paper grain.
    Probably solid warm fills + hairlines (cheap), else an embedded
    image asset — undecided.
12. **Footnotes on pages** = research item G1 (spec 05 §1): v1 renders
    refs only (the superscript machinery §2 carries over), definitions
    stay off-page.
13. **wshot goldens are eyeball-verified** — no pixel-diff harness; the
    "golden of a rendered page" acceptance is a committed reference
    PNG + human/agent review, and text rendering differs across scale
    factors (docs/VISUAL-RIG.md gotchas).
14. **`escape_mode` vs own key context**: two viable Esc architectures
    (top-of-stack branch in escape_mode, or a "ColdRead" KeyBinding
    context). The NoteInput precedent favors the context; the strip
    precedent favors the branch. Undecided — affects how arrows/PageUp
    become page-flips.
