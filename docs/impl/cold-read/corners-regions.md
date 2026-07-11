# Corner cases — regions & the manuscript slice (cold read)

Lens: everything about WHAT TEXT the book actually shows. Code read at
`cold-read` @ e6f8bb3 — all anchors exact on this commit. Spec:
`docs/impl/05-cold-read.md`; law register: `cold-read/design-law.md`
(L-numbers below); Scraps law: `docs/impl/08-compost-fresh.md` +
`compost-fresh/adjudications.md`. 2026-07-06.

Ground facts the cases lean on, verified in code:

- The slice machinery is centralized and era-aware:
  `manuscript_range_of(rope, blocks)` — document.rs:76 (a FREE function,
  deliberately, "so replay/strip states can rebase their own counts
  against their own boundary"); `Document::manuscript_slice()` —
  document.rs:1517–1551 (method-bound; Tail arm clips via
  `SpanSet::slice` document.rs:615–625, Top arm rebases spans by hand
  and takes `kinds[b+1..]`).
- Boundary indices are BLOCK indices, never char offsets
  (BlockMap docs, document.rs:294–321); Top era = pile at the text's
  HEAD (blocks `0..b`), Tail era = pile at the text's tail
  (document.rs:269–276).
- A live Top-era document is reachable on this branch by two deliberate
  degrade-loudly paths: the open-time migration refuses a mismatched
  map (`migrate_top_to_tail`, document.rs:2672–2673) and a
  length-mismatched restore CARRIES the Top boundary through the clamp
  instead of discarding it (document.rs:2596–2607, time-persistence 4).
- Every migrated file carries a Top-era "Before migration" checkpoint
  (editor.rs:1888–1903), so Top-era *states* are the common case in
  history, not an exotic one.
- Boundary clamps: a Tail seam needs `b >= 1` and `b + 1 < len`
  (document.rs:424–432) — never at block 0, never the last line; a Top
  separator needs `b >= 1 && b < len` (document.rs:408–416) — a Top
  state CAN have zero manuscript blocks (everything-compost).

---

## 1 · Past read of a Top-era checkpoint: the book must slice the STATE'S OWN manuscript — and no helper exists (BLOCKER)

**Scenario.** Any migrated document; the writer opens the strip, parks
on the "Before migration" tick (or any pre-migration checkpoint), and
presses the Past-read `Read` verb. The state's blocks carry
`aside_boundary` — Top era: the compost pile IS the head of the text.

**What goes wrong.** Spec §4.7 names the requirement in one
parenthetical ("its OWN manuscript; the state's own boundary") but no
mechanism exists: `Document::manuscript_slice` is method-bound to the
live doc; the natural feeds hand the builder the WHOLE text —
`checkpoint_state(cp)` returns a raw `(text, spans, blocks)` triple
(store.rs:693–698) and `PreviewDoc` carries the full text plus
`boundary` as a side field the *editor renderer* interprets
(editor.rs:13924–13936). Three naive builds, three lies: (a) feed
`state.text` whole → the book's page 1 is the writer's compost pile,
paginated in Bookman with a folio, presented as her piece; (b) treat
the boundary `usize` as a char offset → a near-empty book (it is a
block index); (c) slice "text before the boundary" uniformly → correct
for Tail states, EXACTLY INVERTED for Top states (manuscript is the
text AFTER the separator). Every one is a silent
wrong-text-teleport-class lie, and (a) is the path of least
resistance. The same trap arms the live entry too if the builder
bypasses `manuscript_slice` and reads `doc.text()`.

**Resolution.** Extract the era-branch into a free function beside
`manuscript_range_of` — `manuscript_slice_of(text: &str, spans:
&SpanSet, blocks: &BlockMap) -> (String, SpanSet, BlockMap)` — by
moving Document::manuscript_slice's body (document.rs:1517–1551) onto
it and delegating the method. The book consumes ONLY this function,
for both sources (live doc / checkpoint state). The Past banner's word
count and any folio-adjacent counting go through
`manuscript_range_of` on the state's own blocks — the strip's
`words_at` precedent, editor.rs:2713–2722 (adjudications Scopes 6).
The book NEVER feeds from the panel's diff projection
(`rebuild_preview` writes the newer side's boundary and diff tints,
editor.rs:2079–2211) — checkpoint triples only. Tests: a Top-era
state's book contains zero pile text and its first page is the
manuscript's true first words; a boundary-less (pre-Scraps) state
paginates whole; rig: `seed:topera` → relaunch (migration runs) →
Past-read "Before migration" → assert page-1 text. Law: L27
("the past state's own geometry"), adjudications Time §1 case 5
("preview draws the state's own seam read-only" — the book's analogue
is the state's own *region*), Scopes 6.

**Severity: BLOCKER** — ships the pile as the piece on every
pre-migration checkpoint of every migrated file.

## 2 · The spec's "NFC guaranteed at the document layer" is false — and the tempting fix misanchors reactions (BLOCKER)

**Scenario.** Spec §2.4 states "NFC guaranteed at the document
layer." It is not: the workspace has no normalization dependency
(`unicode-segmentation` only — Cargo.toml; zero `nfc`/`normalize` hits
in strop-core), markdown import inserts pulldown-cmark's raw text,
paste normalizes only CRLF (text_field.rs:169), and Loro stores what
it is given. A pasted NFD «ё» (е + U+0308) or a macOS-sourced
decomposed string sits in the document as-is.

**What goes wrong.** Two failure directions, one per reading of the
false sentence. (a) Wave A "restores" the guarantee by NFC-normalizing
the slice at entry: normalization CHANGES CHAR COUNTS, so every
offset computed against the book string — the reaction range that
files `+ manuscript_base_char()` (spec §5.3), the lane anchor flip,
the arrival flash — drifts against the real document for any NFD
text. Reactions anchor to the wrong words and PERSIST there
(document.rs:2308): data damage of exactly the family the compost
round's blockers caught. (b) Wave A trusts the guarantee: the
hyphenation lookup and the prefix re-shaping assume composed chars;
a decomposed word's break offsets can land inside a combining
sequence — the §2.8 "every break a char AND grapheme boundary" law
violated by a word the tests never contain.

**Resolution.** Strike the sentence from spec §2.4 and write the
inverse as law: **the book never transforms the slice string** — the
painted string may differ (soft-hyphen stripping, inserted "-"), but
every hit box and selection range is kept in SLICE space via the
explicit offset realignment §2.8 already mandates; identity between
slice offsets and document offsets (modulo the one `+base`) is the
invariant reactions rest on. For hyphenation: normalize a COPY of the
single word for dictionary lookup only, and either map break
positions back through the normalization or — the honest v1 — skip
hyphenation for any word whose NFC form differs from its raw form
(the same degradation posture as a missing dictionary, spec §2.4).
Add an NFD word to the §2.8 offset-realignment fixtures. Law: §2.8;
review-ledger H19 (visual→rope anchoring); P1 (the tool records the
writer's text verbatim).

**Severity: BLOCKER** — the false premise invites the
normalize-at-entry implementation, which persists misanchored notes.

## 3 · The parked-strip `Read` verb at a non-checkpoint moment: the book would show a different text than the preview behind it (DECIDE)

**Scenario.** The parked banner exists at ANY scrubbed `pos_ms`
(editor.rs:12850–12932; the moment label falls back to
`format_moment` when no station tick is within 5px,
editor.rs:12860–12868). Spec §4.7 puts the `Read` verb in that banner
AND binds the book to checkpoints only ("strip-scrub arbitrary
moments stay in the flat parked preview").

**What goes wrong.** Those two sentences jointly produce a
bait-and-switch: parked between checkpoints, the writer presses
`Read` and the book must open SOME checkpoint's state — text that
differs from the flat preview she was just looking at. The banner's
name-led grammar (L15) would announce the checkpoint it actually
shows, so it is discoverable — but the switch is silent at the moment
of the click, and "what text the book shows" stops matching "what the
screen showed."

**Resolution.** The verb renders only when the playhead is snapped to
a checkpoint tick — the same ±5px test the station label already
uses, widened from labeled ticks to all checkpoint ticks
(editor.rs:12860–12867); elsewhere the banner simply has no `Read`
(P12: the control appears only where it works; the strip's own
Restore precedent shows a verb-bearing banner is not obligated to
carry every verb at every moment). Named alternative for v2: feed the
book from the ScrubDoc replay triple (`strip_reconstruct`'s
`(text, spans, blocks)` scratch, editor.rs:2677–2693) through case
1's free function — it is already a state with its own blocks — and
the restriction dissolves. Law: spec §4.7 (v1 binds to checkpoints),
L15, P12.

**Severity: DECIDE** — one rendering rule, needed before Wave B.

## 4 · The empty book family: empty doc, textless manuscript over a full pile, everything-compost Top state (DECIDE)

**Scenario.** Four real shapes reach entry with nothing (or only
whitespace) in the manuscript region: (a) a truly empty document;
(b) a Tail doc whose manuscript is one blank block while the pile
holds 3,000 words — reachable because the seam clamp only demands
`b >= 1` (document.rs:424–432) and the blank block satisfies it (park
everything, then park the leftovers); (c) a Top-era STATE with zero
manuscript blocks — `b = len-1` is legal (document.rs:408–416),
`manuscript_range_of` yields `len..len` (document.rs:78–81), and
`flip_state` documents the shape ("everything is compost",
document.rs:160–163); (d) whitespace-only manuscripts. In (b/c) the
slice is `("", default 1-Paragraph BlockMap)` —
`BlockMap::from_kinds(vec![])` returns the one-Paragraph default
(document.rs:342–352), consistent with `""` being one rope line.

**What goes wrong.** The spec never says what a zero-content book IS.
A paginator that emits 0 pages makes the folio a lie ("— 1 of 0 —"),
`page: usize` an out-of-range index, and Home/End undefined. Refusing
entry needs a refusal surface that doesn't exist pre-entry (the pulse
target is the banner's "Reading", which only exists inside) and makes
the tool editorialize ("nothing worth reading") against P2. And in
(b), the writer whose text is all in Scraps may read the empty page
as data loss.

**Resolution.** Entry always succeeds; the paginator's page count is
`max(1, computed)`; the empty book is ONE blank page — real paper,
real grain, running head, folio `— 1 of 1 —`, banner `0 words`. That
page IS the honest cold look at the manuscript (L26: the read ends at
the piece's true last line — here its first). No hint that Scraps
holds text — the pile never enters the read's vocabulary (I1; a hint
is a caption, P4), and the banner's `0 words` states the fact. Blank
paragraphs INSIDE the slice paginate as blank lines (they are the
text); no trailing-blank trimming — the folio must never disagree
with what a re-entry recomputes. Unit tests: the four shapes above
each yield 1 page, 0 assertions failed on `page` clamping. Law: L26,
I1, P2, P4; corridor floor L30.

**Severity: DECIDE** — one paginator floor rule + the recorded
refusal-of-a-refusal.

## 5 · The banner station quotes the entry's own reflex checkpoint back at the writer (DECIDE)

**Scenario.** Spec §1 defines `station: Option<String>` as "newest
checkpoint name at entry" and §4.6 orders entry as save →
`add_checkpoint_if_changed("Cold read")` → snapshot. Whenever the
entry actually checkpoints (any edit since the last one), the newest
checkpoint is now named "Cold read".

**What goes wrong.** The banner reads **Reading · "Cold read" ·
4,120 words** — chrome quoting chrome. L14's station is "the
writer's own words in typographic quotes"; "Cold read" (and equally
"Session", "Checkpoint 7", "Migrated", "Before restore" — all
reachable as the newest name when entry doesn't checkpoint) is the
record's word, not hers. I7 explicitly reserves the quoted-station
grammar for the writer's own station name.

**Resolution.** The station segment reads the newest checkpoint whose
`station_rank` (strip.rs:681–695) is writer-tier (manual/renamed) or
seal-tier — never reflex-tier; none exists → segment omitted (already
specced). Read it from the checkpoint list at entry, order-independent
of the entry checkpoint's creation. The time lens owns the rank
table; flagged here because it is banner *data* riding the same entry
snapshot as the word count. Law: L14, I7, P8 (the station name is
data, never system prose).

**Severity: DECIDE** — a one-line filter, but strings are law.

## 6 · The inverse rebase: doc-space → slice-space for lane clicks and the arrival flash (NOTE)

**Scenario.** Spec §5.3 fixes the filing direction (slice range
`+ manuscript_base_char()`); §5.4's lane cards flip "to the page
containing its anchor" and flash the anchored words. If the lane
re-reads ranges from `doc.notes()` (whole-doc space), the flip needs
the subtraction the spec never mentions. `base > 0` on a LIVE doc is
real: the open-time migration refuses a mismatched map
(document.rs:2672–2673) and a length-mismatched restore carries the
Top boundary through the clamp (document.rs:2596–2607) — both
deliberate degrade paths that leave a live Top-era geometry.

**What goes wrong.** Tail-era docs have base 0, so every test passes;
the Top-era live doc ships a wrong-page flip and a flash on the wrong
words — the silent-on-the-happy-path corner.

**Resolution.** One conversion pair, stated as law and unit-tested as
a round-trip: file = slice range + base; display = doc range − base
(saturating, with an out-of-range guard that drops the link rather
than flipping to page 1). Simplest build: the ColdRead session lane
keeps the SLICE-space ranges it created (it never needs the document
ranges for display); only the filed note carries doc space. The
convention's home is the diagnosis anchor code (editor.rs:3665–3689,
"+base back to every produced range"). Law: spec §5.3/§5.4; recon §7
(TRAP 14); H40.

**Severity: NOTE** — one signed offset, tested both directions.

## 7 · One accounting: the banner's words, the chip's "piece", the Past banner (NOTE)

**Scenario.** L14's count and I8's law ("the banner's word count is
manuscript-only accounting — the same number the count chip calls
'piece'").

**What goes wrong / is undefined.** Two code paths counting "the
manuscript" drift under maintenance: the chip's number is
`self.word_count = manuscript_word_count()` —
`count_words(rope.slice(manuscript_char_range()))`
(editor.rs:1709–1711, 1728–1737); a book that counts its own slice
STRING (or its fragment tokens — note U+00A0 is `char::is_whitespace`
so "a\u{a0}b" is two words to `count_words` but ONE fragment to the
breaker) invents a second accounting. Formatting too: the chip uses
`format_thousands` (editor.rs:1467); "4,120 words" must come from the
same formatter.

**Resolution.** `ColdRead.words` is `manuscript_word_count()` taken at
entry — the function, not a re-derivation; the banner formats with
`format_thousands`. The Past banner counts through
`manuscript_range_of` on the STATE'S own rope+blocks — exactly the
strip's `words_at` (editor.rs:2713–2722; adjudications Scopes 6). The
running head carries no region data (doc name + fixed suffix, L12/O11)
— nothing to scope there. Rig: assert the dump's `coldread.words`
equals the pre-entry `word-count` pill number on a seamed fixture.
Law: I8, Scopes 5/6.

**Severity: NOTE.**

## 8 · One giant paragraph (NOTE)

**Scenario.** A 5,000-word document with no paragraph breaks — one
block, many pages.

**What goes wrong.** Pagination rules are written in paragraph terms:
widows/orphans "≥ 2 lines each side" (spec §2.7) reads as a
per-paragraph-edge rule; a paragraph taller than a page means EVERY
page break is mid-paragraph and the keep rules must not manufacture a
page that satisfies nothing (worst case: a keep rule that always
rejects the only available break point loops or pushes an
unbreakable残 to a phantom page). The hyphen-on-page's-last-line
avoidance ("one-line movement fixes it") must terminate when every
line ends in a candidate hyphen.

**Resolution.** State the precedence: a page must always make
progress (≥ 1 line placed); widow/orphan and hyphen-avoidance are
preferences that yield, in order, when the paragraph fills whole
pages. The §2.6 microbench gains a one-paragraph fixture (same word
budget); the §2.8 unit suite gains "single paragraph spanning ≥ 3
pages" with stable page-top-char resume. Law: spec §2.6–2.8.

**Severity: NOTE** — engine care, one precedence sentence.

## 9 · Heading as the slice's last block — keep-with-next has no next (NOTE)

**Scenario.** The manuscript's final block is a heading; its
"following paragraph" either doesn't exist or exists ONLY below the
seam (the writer parked the section body and left the heading).

**What goes wrong.** §2.7's "a heading is never the last block on a
page and keeps ≥ 2 lines of its following paragraph" is unsatisfiable
— and a paginator that peeks at the document beyond the slice to find
the "next paragraph" would leak pile text into the keep computation
(and its geometry into the book).

**Resolution.** Keep rules are evaluated on the SLICE's block
sequence only — below-seam blocks do not exist (L26/I1). A heading
with no following block in the slice sets as the last line of the
last page, keep rule vacuous. Same degradation for heading→heading
and heading→image tails. Unit test: heading-final fixture paginates
with the heading on the last page, no phantom page. Law: L26, I1,
spec §2.7.

**Severity: NOTE.**

## 10 · The slice's clipped formatting spans (NOTE)

**Scenario.** Bold running across the last manuscript block into the
pile (spans may cross blocks; the boundary is block-grained), or any
span touching the excluded joining break —
`manuscript_range_of` Tail excludes the break via `line_break_before`
(document.rs:82–85, CRLF-aware at 53–65).

**What goes wrong.** Nothing, if the slice is trusted:
`SpanSet::slice` clips both ends and rebases (document.rs:615–625);
the Top arm clips by hand (document.rs:1534–1544). The book's last
word carries a truncated span — correct. The risk is a book renderer
that indexes `text` by an UNCLIPPED span end, or re-derives spans
from the document instead of the slice triple.

**Resolution.** The slice triple is the book's ONLY formatting
source; add the invariant test the clip already guarantees (no span
end > slice char len, both eras, spans straddling the boundary and
covering the joining break). A `FootnoteRef` span can be clipped like
any other — its painted superscript derives from the clipped slice
span (see case 11). Law: I1; spec §6.10 (inline formatting is the
text).

**Severity: NOTE** — test coverage pinning existing correctness.

## 11 · Footnotes: definitions default INTO the pile; numbering across the seam (NOTE, plus a flagged pre-existing bug)

**Scenario.** `insert_footnote` appends the definition block at the
ABSOLUTE document end (`len..len`, editor.rs:2988–2992). In any
document with Scraps, every new footnote definition therefore lands
BELOW the seam — inside the pile: uncounted, unexported, outside AI
scope, and (for the book) off-slice. My agenda's "definitions live
below the seam" is the DEFAULT, not an edge.

**What goes wrong / is undefined.** For the book itself, v1 law
already answers: refs paint as superscripts, definitions stay
off-page (spec §2.7, I10, O1) — no page lies. Three loose ends:
(a) numbering — the editor numbers by ref order over the WHOLE doc
(`footnote_numbers`, editor.rs:9777–9792, fed whole-doc spans at
3044–3056); the book must number by ref order WITHIN THE SLICE. Tail
era these agree (manuscript refs precede pile refs). In a Top-era
STATE, pile refs precede manuscript refs in doc order — the editor's
"³" can be the book's "¹". Accepted: the painted number is
presentation, the stored id is identity (insert_footnote's own
comment, editor.rs:2960–2962); the book is self-consistent.
(b) `FootnoteDef` blocks ABOVE the seam are in the slice — the
paginator must SKIP them (definitions stay off-page even when they
live in the manuscript region); spec §2.7's per-kind table should say
so explicitly. (c) The banner counts def text that sits above the
seam while the pages omit it — accepted: the banner's number is the
chip's number (I8 outranks page-visible-words consistency).

**Resolution.** As above; plus file the region-blind
`insert_footnote` append as its own bug OUTSIDE this build (def
insertion belongs at `manuscript_end_char`, riding the same region
law as every other machine-placed artifact) — the cold read must not
quietly depend on defs being in the pile OR in the manuscript. Law:
I10, O1, I8, spec §2.7.

**Severity: NOTE** (the book); the insert_footnote placement is a
flagged pre-existing defect for a separate fix.

## 12 · Images: the only content, missing assets, and the Past read (NOTE)

**Scenario.** (a) A manuscript that is only Image blocks; (b) an
Image whose asset fails to load at book time; (c) a Past read of a
checkpoint whose image was long deleted from the live doc.

**What goes wrong / is undefined.** (a) word count 0 → banner
"0 words" — honest; pages are images scaled per §2.7; there are no
words to select, so reactions are unreachable — correct, nothing to
quote. (b) unspecced: a silently skipped image changes pagination
between two sessions if the asset later reappears — the folio must
not depend on load success. (c) verified NON-problem: the GC's
reachable set extends to checkpoint-state blocks when needed
(`collect_unreachable_assets`, store.rs:958–996) — a Past read's
images survive; no new hole to plug.

**Resolution.** (b) a missing asset reserves its box per the same
metrics a loaded one would get ONLY if dimensions are recorded;
strop records none — so v1 law: a missing asset renders as the
editor's existing missing-image degradation at measure width's
placeholder height, deterministically (same input → same pagination),
and logs. Both sources (live slice, checkpoint state) load through
the same asset-store path. Law: spec §2.7 (images keep whole), L10
(the folio count must be true — determinism included).

**Severity: NOTE.**

## 13 · Mid-migration legacy files: entry records nothing; unmaterialized Past reads stall (DECIDE)

**Scenario.** A long-lived legacy file on its first launch under this
build: the background backfill (`backfill_checkpoint_states`,
editor.rs:2034–2071) hasn't landed. (a) Cold-read entry calls
`add_checkpoint_if_changed` → `seal_session_with` returns EARLY when
the last checkpoint has no materialized state (store.rs:674–677) — no
checkpoint. (b) The writer parks the strip and presses `Read` on an
unmaterialized checkpoint: `checkpoint_state` falls back to
`state_at` — the multi-second historical checkout the materialization
work existed to kill (store.rs:690–698).

**What goes wrong.** (a) quietly violates L3's "every cold read
starts from a recorded state" for exactly one session per legacy
file. (b) blows the <100ms entry budget by orders of magnitude with
no feedback.

**Resolution.** (a) Accept — the deferral is precedented and
self-healing ("seal next launch"); record the exception in the spec's
§4.6 so the rig doesn't assert a checkpoint on the legacy fixture's
first launch. (b) Gate the Past-read affordance on
`cp.state.is_some()` (per-checkpoint, not the global
`checkpoints_materialized`) — the verb is absent until the backfill
reaches that checkpoint (P12; loud degrade beats a 7-second stall);
the flat parked preview remains available as today. Law: L3 with the
store's own deferral comment; the time lens owns the final call.

**Severity: DECIDE** — two small gates, both needing a ruling.

## 14 · Scraps edited between two cold reads: the fingerprint is whole-doc, deliberately (NOTE)

**Scenario.** Read 1 checkpoints state A. The writer edits ONLY the
pile (below the seam), touches no manuscript text, and enters again.

**What goes wrong / is undefined.** Nothing wrong — but the semantics
deserve recording: `seal_session_with` compares the FULL
(text, spans, blocks) including the pile (store.rs:681–685), so a
second "Cold read" checkpoint is created whose *manuscript* is
identical to A's. Two ticks, one book. The tempting "improvement" —
fingerprint the manuscript slice only, since the book only shows the
manuscript — is wrong: checkpoints are whole-document records (I7),
and a manuscript-scoped guard would silently refuse to record real
pile work, breaking L3's "every cold read starts from a recorded
state" for the changed document. Converse micro-case (recon risk 2):
edit-then-revert between reads still creates a duplicate-state
checkpoint because the guard compares against the LAST checkpoint
only — accepted, harmless.

**Resolution.** Keep the whole-doc fingerprint; record the two-ticks/
one-book outcome as intended. Law: L3, I7; store.rs:668–688.

**Severity: NOTE.**

## 15 · Boundary clamps: the seam is never at 0 or EOF, but every region can be blank (NOTE)

**Scenario/facts.** By construction (document.rs:408–432): no Tail
seam at block 0, no seam as the final line, no Top separator at
block 0. But: a blank one-block manuscript over a full pile (case 4b),
a textless pile held alive by the retype-race guard
(`scraps_textless`, document.rs:1498–1508 — the writer enters the
read with her caret parked in an empty pile), and a Top state with
zero manuscript blocks (case 4c) are all real. The slice code already
degrades correctly: `from_kinds(vec![])` → one default Paragraph
(document.rs:342–352), matching `""` = one rope line;
`line_break_before(_, 0)` returns 0 (document.rs:53–56).

**What goes wrong.** Only assumptions: book code that presumes a
non-empty slice, ≥1 non-blank line, or `blocks.len() > 0` off the raw
kinds slice before `from_kinds`'s floor.

**Resolution.** Pin with unit tests on `manuscript_slice_of` (case
1's function): blank-manuscript Tail doc, everything-compost Top
state, textless-pile doc with caret inside the pile at entry (exit
must restore that caret untouched — the takeover never moved it,
spec §4.6). Law: the clamps' own doc comments; seam-mechanics 6
(retype-race guard).

**Severity: NOTE.**

---

**Summary.** Fifteen cases: 2 BLOCKER, 4 DECIDE, 9 NOTE. The spine of
the round: the slice, not the document, is the book — and the two
blockers are both places where that identity silently breaks. The most
dangerous is case 1: the Past read of a Top-era checkpoint (which
every migrated file carries as "Before migration") has no existing
slicing mechanism for a raw checkpoint triple, and all three natural
implementations show the writer's compost pile — or an inverted slice
— typeset as her piece; the fix is one shared free function
(`manuscript_slice_of`) that both the live and Past books must be
REQUIRED to consume. Case 2 kills a false premise in the spec itself
(no NFC guarantee exists in the document layer; normalizing the slice
would misanchor persisted reactions). The DECIDEs are one rendering
gate for the parked `Read` verb (book ≠ preview at non-checkpoint
moments), the empty-book floor (max(1, pages), one honest blank
page), the station-name tier filter (the banner must not quote its
own reflex checkpoint), and the legacy-file gates. The NOTEs pin the
inverse `−base` rebase for lane clicks, one word-count accounting
(I8), slice-only pagination inputs (keep rules never peek below the
seam), and the discovery that `insert_footnote` already appends
definitions region-blind into the pile — a pre-existing defect
flagged for its own fix.
