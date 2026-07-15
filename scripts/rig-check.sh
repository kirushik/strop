#!/bin/bash
# Headless integration checks (docs/VISUAL-RIG.md): drive REAL GPUI through
# STROP_SMOKE and assert, against an actually-rendered frame, the margin
# invariants the packer proptests can only cover in the pure layer — no two
# visible cards overlap, the selected card is genuinely on-screen, the pill
# counts are honest — plus that the whole document surface scrolls. This is the
# integration half of Phase 6 (the height-measurement + culling paths need a
# Window, so they can't be unit-tested; gpui test-support is deliberately off).
#
# Run from the repo root after `cargo build -p strop-app`. Exits non-zero on the
# first failed assertion, so it slots into CI next to `cargo test`.
set -u
BIN="$PWD/target/debug/strop"
[ -x "$BIN" ] || { echo "rig-check: build target/debug/strop first"; exit 2; }

DOC=$(mktemp --suffix=.md)
{
  echo "The man who sold his shadow walked into the quiet thing at the edge of town."
  echo
  echo "By nightfall the dogs had begun to growl, and the children, who notice everything, first understood what had arrived."
  echo
  # A long tail so the document is taller than the viewport (something to scroll to).
  for i in $(seq 1 60); do echo "Ordinary padding paragraph number $i, plain and unremarked."; echo; done
} > "$DOC"

# A second fixture for the crowded-lane check: eight flaggable phrases, all in
# the first viewport, so seed:many overflows the full-size budget on screen.
DOC2=$(mktemp --suffix=.md)
{
  for i in $(seq 1 8); do
    echo "Here stands crowded margin phrase number $i, holding its place in the draft."
    echo
  done
  for i in $(seq 1 40); do echo "Ordinary padding paragraph number $i, plain and unremarked."; echo; done
} > "$DOC2"
# Fixtures three and four: the reveal-clock and entrance-fade checks (each
# its own file — sidecars accumulate, and both need a margin that starts empty).
DOC3=$(mktemp --suffix=.md)
head -4 "$DOC" > "$DOC3"
DOC4=$(mktemp --suffix=.md)
head -4 "$DOC" > "$DOC4"
# Five and six: the bucket-exclusivity and re-pack-motion checks (again their
# own files — both need sidecars unpolluted by the earlier resolves).
DOC5=$(mktemp --suffix=.md)
cp "$DOC" "$DOC5"
DOC6=$(mktemp --suffix=.md)
cp "$DOC2" "$DOC6"
trap 'rm -f "$DOC" "$DOC.strop" "$DOC2" "$DOC2.strop" "$DOC3" "$DOC3.strop" "$DOC4" "$DOC4.strop" "$DOC5" "$DOC5.strop" "$DOC6" "$DOC6.strop"' EXIT

fail=0
field() { echo "$1" | grep -oE "\"$2\":[^,}]*" | head -1 | cut -d: -f2; }
expect() { # label want got
  if [ "$2" = "$3" ]; then echo "  ok   $1"; else echo "  FAIL $1: want $2, got $3"; fail=1; fi
}

echo "rig-check: margin packing (seed:diag — 4 cards, the 3rd active)"
M=$(WRUN_TAIL=40 scripts/wrun.sh "$DOC" "seed:diag dump:ui" 2>/dev/null | grep -oE '"margin":\{[^}]*\}')
[ -n "$M" ] || { echo "  FAIL no margin dump (rig didn't render?)"; exit 1; }
expect "no two visible cards overlap" false "$(field "$M" overlap)"
expect "the active card is visible"   true  "$(field "$M" active_visible)"
expect "all four seeded cards fit"    4     "$(field "$M" visible)"

echo "rig-check: crowded lane recedes, never hides (seed:many — 8 cards, budget 5)"
M=$(WRUN_TAIL=40 scripts/wrun.sh "$DOC2" "seed:many dump:ui" 2>/dev/null | grep -oE '"margin":\{[^}]*\}')
[ -n "$M" ] || { echo "  FAIL no margin dump (rig didn't render?)"; exit 1; }
# The honesty invariant: every flagged passage in view keeps a card — the
# budget shrinks the oldest pass to one-line cards, it never hides them.
expect "every flagged passage keeps a card" 8     "$(field "$M" visible)"
expect "the oldest pass receded (8 - 5)"    3     "$(field "$M" collapsed)"
expect "receded cards still never overlap"  false "$(field "$M" overlap)"

# And the interaction half: clicking a receded card selects it, and the
# selected card is budget-exempt, so it expands in place (1274,104 is the
# first receded card in this fixture at the rig's stable 1600x1200 window).
M=$(WRUN_TAIL=40 scripts/wrun.sh "$DOC2" "seed:many click:1274,104 dump:ui" 2>/dev/null | grep -oE '"margin":\{[^}]*\}')
[ -n "$M" ] || { echo "  FAIL no margin dump (rig didn't render?)"; exit 1; }
expect "clicking a receded card selects it"  true "$(field "$M" active_visible)"
expect "the selected card expanded (3 - 1)"  2    "$(field "$M" collapsed)"
expect "expansion never overlaps"            false "$(field "$M" overlap)"

echo "rig-check: a pass arriving mid-burst waits for the lull (the reveal clock)"
# 'x' opens a typing burst; seed:deliver pushes the demo pass through the real
# arrival gate ~80ms later — it must PARK (no squiggles/cards mid-thought),
# then land by itself once the prose has been still past TYPING_LULL (1s).
OUT=$(WRUN_TAIL=60 scripts/wrun.sh "$DOC3" "x seed:deliver dump:ui wait:1600 dump:ui" 2>/dev/null | grep 'UI-DUMP')
D1=$(echo "$OUT" | head -1); D2=$(echo "$OUT" | tail -1)
[ -n "$D1" ] || { echo "  FAIL no dump (rig didn't render?)"; exit 1; }
expect "mid-burst the pass parks"          true  "$(field "$D1" ai_deferred)"
expect "nothing surfaces mid-burst"        0     "$(field "$D1" visible)"
expect "the lull lands the parked pass"    false "$(field "$D2" ai_deferred)"
expect "all four cards surface after it"   4     "$(field "$D2" visible)"

echo "rig-check: landed cards get ONE entrance fade, then the marks clear"
# In a lull the pass lands at once: the dump right after shows all four cards
# inside their fade window; ~700ms later the marks are gone (fade 250ms +
# clear timer), so nothing can ever re-fade on a later scroll-out/in.
OUT=$(WRUN_TAIL=60 scripts/wrun.sh "$DOC4" "seed:deliver dump:ui wait:700 dump:ui" 2>/dev/null | grep 'UI-DUMP')
D1=$(echo "$OUT" | head -1); D2=$(echo "$OUT" | tail -1)
[ -n "$D1" ] || { echo "  FAIL no dump (rig didn't render?)"; exit 1; }
expect "a lull lands the pass at once"     4 "$(field "$D1" visible)"
expect "landed cards are inside the fade"  4 "$(field "$D1" appearing)"
expect "the fade marks clear after it"     0 "$(field "$D2" appearing)"
expect "cards persist past the fade"       4 "$(field "$D2" visible)"

echo "rig-check: scroll works anywhere on the document surface"
for x in 60 800 1500; do
  S=$(WRUN_TAIL=40 scripts/wrun.sh "$DOC" "wheel:$x,600,-600 dump:ui" 2>/dev/null \
        | grep -oE '"scroll_y":[0-9.]+' | head -1 | cut -d: -f2)
  if [ "${S%.*}" -gt 0 ] 2>/dev/null; then echo "  ok   wheel at x=$x scrolled ($S)"; else
    echo "  FAIL wheel at x=$x did not scroll (got $S)"; fail=1; fi
done

echo "rig-check: a resolved card exits with a brief ghost fade, model commits at once"
# Re-seed on DOC (dedupes against its sidecar), resolve the first note through
# the real set_note_status path: the note resolves IMMEDIATELY (visible 4 → 3)
# while its ghost fades (departing 1 → 0 after the timer).
OUT=$(WRUN_TAIL=60 scripts/wrun.sh "$DOC" "seed:diag resolve:first dump:ui wait:300 dump:ui" 2>/dev/null | grep 'UI-DUMP')
D1=$(echo "$OUT" | head -1); D2=$(echo "$OUT" | tail -1)
[ -n "$D1" ] || { echo "  FAIL no dump (rig didn't render?)"; exit 1; }
expect "the model resolves instantly"      3 "$(field "$D1" visible)"
expect "its ghost lingers for the fade"    1 "$(field "$D1" departing)"
expect "the ghost is gone after the fade"  0 "$(field "$D2" departing)"
expect "the lane stands re-packed"         3 "$(field "$D2" visible)"

# C4 / LAW 2 — no dead zones: a click in the margin BESIDE a composing card
# resolves the composer (report 1: the lane lives outside the editor column's
# hitbox, so only the ROOT handler sees the click — light_dismiss carries the
# resolution). An empty draft is discarded (no stray blank card); a typed
# draft commits; both exits restore the caret/selection saved at open.
echo "rig-check: a click beside the card still lands — the composer resolves (C4)"
DOCC4=$(mktemp --suffix=.md); cp "$DOC2" "$DOCC4"
OUT=$(WRUN_TAIL=60 scripts/wrun.sh "$DOCC4" "select:para ctrl-m dump:ui click:1274,700 dump:ui" 2>/dev/null | grep 'UI-DUMP')
C1=$(echo "$OUT" | sed -n 1p); C2=$(echo "$OUT" | sed -n 2p)
[ -n "$C2" ] || { echo "  FAIL no dump (composer C4)"; exit 1; }
rm -f "$DOCC4" "$DOCC4.strop"
expect "ctrl-m opens the composer"            '"NoteComposer"' "$(field "$C1" focused)"
expect "a margin-blank click closes it"       '"Editor"' "$(field "$C2" focused)"
expect "the empty draft is discarded"         0 "$(field "$C2" open_notes)"
DOCC5=$(mktemp --suffix=.md); cp "$DOC2" "$DOCC5"
OUT=$(WRUN_TAIL=60 scripts/wrun.sh "$DOCC5" "select:para dump:ui ctrl-m h i click:1274,700 dump:ui" 2>/dev/null | grep 'UI-DUMP')
C1=$(echo "$OUT" | sed -n 1p); C2=$(echo "$OUT" | sed -n 2p)
[ -n "$C2" ] || { echo "  FAIL no dump (composer C4 commit)"; exit 1; }
rm -f "$DOCC5" "$DOCC5.strop"
expect "a typed draft commits on the click"   1 "$(field "$C2" open_notes)"
expect "focus returns to the prose"           '"Editor"' "$(field "$C2" focused)"
SEL1=$(echo "$C1" | grep -oE '"sel":\[[0-9]+,[0-9]+\]' | sed 's/"sel"://')
SEL2=$(echo "$C2" | grep -oE '"sel":\[[0-9]+,[0-9]+\]' | sed 's/"sel"://')
expect "the dead-zone exit restores the saved caret" "$SEL1" "$SEL2"

echo "rig-check: keyboard composer exits share the empty-discard law"
DOCC6=$(mktemp --suffix=.md); cp "$DOC2" "$DOCC6"
OUT=$(WRUN_TAIL=40 scripts/wrun.sh "$DOCC6" "select:para ctrl-m enter dump:ui" 2>/dev/null | grep 'UI-DUMP' | tail -1)
rm -f "$DOCC6" "$DOCC6.strop"
[ -n "$OUT" ] || { echo "  FAIL no dump (keyboard empty-discard)"; exit 1; }
expect "Enter discards an untouched composer"  0 "$(field "$OUT" open_notes)"
DOCC7=$(mktemp --suffix=.md); cp "$DOC2" "$DOCC7"
OUT=$(WRUN_TAIL=40 scripts/wrun.sh "$DOCC7" "select:para ctrl-m h i enter dump:ui" 2>/dev/null | grep 'UI-DUMP' | tail -1)
rm -f "$DOCC7" "$DOCC7.strop"
[ -n "$OUT" ] || { echo "  FAIL no dump (keyboard typed-commit)"; exit 1; }
expect "Enter commits a typed composer"         1 "$(field "$OUT" open_notes)"

echo "rig-check: off-screen cards land in exactly one honest bucket"
# Scroll the anchors far off the top: culled cards count as 'above' — EXCEPT
# the selected card, which is exempt from the cull (you're working it) and
# stays in the lane. Every seeded card is thus in exactly one bucket
# (visible=1 + above=3 = the 4 seeded), the count-grammar exclusivity rule.
M=$(WRUN_TAIL=40 scripts/wrun.sh "$DOC5" "seed:diag wheel:800,600,-800 dump:ui" 2>/dev/null | grep -oE '"margin":\{[^}]*\}')
[ -n "$M" ] || { echo "  FAIL no margin dump (rig didn't render?)"; exit 1; }
expect "culled cards count as above"        3    "$(field "$M" above)"
expect "only the exempt active card stays"  1    "$(field "$M" visible)"
expect "and it is genuinely visible"        true "$(field "$M" active_visible)"

echo "rig-check: a re-pack SLIDES the survivors; scroll snaps all motion"
# Resolving the bottom full-size card frees a budget slot: a receded card
# expands and the run below shifts — the survivors SLIDE to their new slots
# (200ms, staggered) instead of teleporting, then settle. Two rig traps
# encoded here: wait:1100 first, because a FRESH sidecar imports the .md at
# open, which stamps a text edit — and a live burst rightly snaps, never
# slides; and resolve:LAST, because the oldest card is already a one-liner
# at its own anchor whose departure legitimately moves nothing.
OUT=$(WRUN_TAIL=60 scripts/wrun.sh "$DOC6" "seed:many wait:1100 resolve:last wait:60 dump:ui wait:600 dump:ui" 2>/dev/null | grep 'UI-DUMP')
D1=$(echo "$OUT" | head -1); D2=$(echo "$OUT" | tail -1)
[ -n "$D1" ] || { echo "  FAIL no dump (rig didn't render?)"; exit 1; }
# moves_started is session-monotonic: it proves the slide happened even when
# a cold launch makes the dump miss the 200ms flight itself.
MV=$(field "$D1" moves_started)
if [ "${MV:-0}" -gt 0 ] 2>/dev/null; then echo "  ok   survivors slide (moves_started=$MV)"; else
  echo "  FAIL no re-pack motion (moves_started=$MV)"; fail=1; fi
expect "the lane settles after the slide"   0 "$(field "$D2" moving)"
# A scroll mid-slide clears ALL motion at once — the lane never animates
# against the writer's own navigation.
# wait:80 after the wheel: the snap happens in the render pre-pass, and under
# load the dump can beat the next frame — one frame's grace kills the flake.
OUT=$(WRUN_TAIL=60 scripts/wrun.sh "$DOC6" "seed:many wait:1100 resolve:last wheel:800,600,-200 wait:80 dump:ui" 2>/dev/null | grep 'UI-DUMP')
D1=$(echo "$OUT" | head -1)
[ -n "$D1" ] || { echo "  FAIL no dump (rig didn't render?)"; exit 1; }
MV=$(field "$D1" moves_started)
if [ "${MV:-0}" -gt 0 ] 2>/dev/null; then echo "  ok   the resolve did start motion (moves_started=$MV)"; else
  echo "  FAIL no motion for the scroll to snap (moves_started=$MV)"; fail=1; fi
expect "scroll snaps the motion instantly"  0 "$(field "$D1" moving)"

echo "rig-check: reduce_motion turns the same re-pack into a cross-fade"
# Same resolve, same clock — but the travel renders as an opacity cross-fade
# (reduce:motion flips the config switch for the run).
OUT=$(WRUN_TAIL=60 scripts/wrun.sh "$DOC6" "reduce:motion seed:many wait:1100 resolve:last wait:60 dump:ui" 2>/dev/null | grep 'UI-DUMP')
D1=$(echo "$OUT" | head -1)
[ -n "$D1" ] || { echo "  FAIL no dump (rig didn't render?)"; exit 1; }
expect "cross-fade mode is on"              true "$(field "$D1" reduce_motion)"
MV=$(field "$D1" moves_started)
if [ "${MV:-0}" -gt 0 ] 2>/dev/null; then echo "  ok   the move still registers (moves_started=$MV)"; else
  echo "  FAIL no cross-fade motion (moves_started=$MV)"; fail=1; fi

echo "rig-check: the editor button's face is a priority, and the door law holds"
# A pass delivered in a lull lands at once: the door opens, the face reads
# the glossary word (Reading), and the count is the open queries. Delivered
# mid-burst it parks: the face says a read is ready, and NOTHING surfaces.
# Fresh docs per run: the .strop SIDECAR persists across wrun launches, so a
# reused doc reopens the previous run's saved cards into the new assertion.
DOCB1=$(mktemp --suffix=.md); cp "$DOC" "$DOCB1"
DOCB2=$(mktemp --suffix=.md); cp "$DOC" "$DOCB2"
OUT=$(WRUN_TAIL=60 scripts/wrun.sh "$DOCB1" "wait:1100 seed:deliver wait:1400 dump:ui" 2>/dev/null | grep 'UI-DUMP' | tail -1)
B=$(echo "$OUT" | grep -oE '"editor_btn":\{[^}]*\}')
[ -n "$B" ] || { echo "  FAIL no editor_btn dump"; exit 1; }
expect "the landed pass reads as Reading"   '"reading"' "$(field "$B" face)"
expect "its queries are the open count"     4 "$(field "$B" open_count)"
OUT=$(WRUN_TAIL=60 scripts/wrun.sh "$DOCB2" "2 seed:deliver dump:ui" 2>/dev/null | grep 'UI-DUMP' | tail -1)
B=$(echo "$OUT" | grep -oE '"editor_btn":\{[^}]*\}')
M=$(echo "$OUT" | grep -oE '"margin":\{[^}]*\}')
expect "mid-burst the face says ready"      '"ready"' "$(field "$B" face)"
expect "and the door law holds (no cards)"  0 "$(field "$M" visible)"

echo "rig-check: zero-query acknowledgement and run recovery stay on their owners"
DOCB3=$(mktemp --suffix=.md); cp "$DOC" "$DOCB3"
OUT=$(WRUN_TAIL=80 scripts/wrun.sh "$DOCB3" "ai:empty dump:ui ebtn:open dump:ui ebtn:close dump:ui" 2>/dev/null | grep 'UI-DUMP')
E1=$(echo "$OUT" | sed -n 1p); E3=$(echo "$OUT" | sed -n 3p)
B1=$(echo "$E1" | grep -oE '"editor_btn":\{[^}]*\}')
B3=$(echo "$E3" | grep -oE '"editor_btn":\{[^}]*\}')
expect "an empty read marks the button"     '"empty"' "$(field "$B1" face)"
expect "the marker begins unacknowledged"   true      "$(field "$B1" empty_unack)"
expect "closing the menu acknowledges it"  false     "$(field "$B3" empty_unack)"
expect "the menu keeps the last result"     '"line/0"' "$(field "$B3" last_result)"
DOCB4=$(mktemp --suffix=.md); cp "$DOC" "$DOCB4"
OUT=$(WRUN_TAIL=60 scripts/wrun.sh "$DOCB4" "ai:running ebtn:open dump:ui" 2>/dev/null | grep 'UI-DUMP' | tail -1)
B=$(echo "$OUT" | grep -oE '"editor_btn":\{[^}]*\}')
expect "running stays on the editor control" '"cooking"' "$(field "$B" face)"
expect "the attached menu owns Cancel"       true        "$(field "$B" menu_cancel)"
DOCB5=$(mktemp --suffix=.md); cp "$DOC" "$DOCB5"
OUT=$(WRUN_TAIL=60 scripts/wrun.sh "$DOCB5" "seed:diag ai:error dump:ui" 2>/dev/null | grep 'UI-DUMP' | tail -1)
B=$(echo "$OUT" | grep -oE '"editor_btn":\{[^}]*\}')
M=$(echo "$OUT" | grep -oE '"margin":\{[^}]*\}')
expect "a failure retains recovery"          true  "$(field "$B" recovery)"
expect "recovery and cards do not overlap"   false "$(field "$M" overlap)"
TOP=$(field "$M" min_top); FLOOR=$(field "$M" floor)
if awk "BEGIN { exit !($TOP >= $FLOOR) }"; then
  echo "  ok   recovery owns the lane floor ($TOP >= $FLOOR)"
else
  echo "  FAIL card crossed recovery floor ($TOP < $FLOOR)"; fail=1
fi
rm -f "$DOCB1" "$DOCB1.strop" "$DOCB2" "$DOCB2.strop" \
  "$DOCB3" "$DOCB3.strop" "$DOCB4" "$DOCB4.strop" \
  "$DOCB5" "$DOCB5.strop"

echo "rig-check: the strip scrubs without re-baking, and hides the margin while parked"
# bakes is session-monotonic: open bakes once; TWO scrubs later it is STILL 1
# (the stability law — scrubbing may never rebuild the fabric). Parking hides
# the live margin (cards must not float over past text); Now brings it back.
DOCS1=$(mktemp --suffix=.md); cp "$DOC" "$DOCS1"
OUT=$(WRUN_TAIL=80 scripts/wrun.sh "$DOCS1" "seed:diag seed:journal strip:open dump:ui strip:scrub:0.5 dump:ui strip:scrub:0.8 dump:ui strip:now dump:ui" 2>/dev/null | grep 'UI-DUMP')
D1=$(echo "$OUT" | sed -n 1p); D2=$(echo "$OUT" | sed -n 2p); D3=$(echo "$OUT" | sed -n 3p); D4=$(echo "$OUT" | sed -n 4p)
S1=$(echo "$D1" | grep -oE '"strip":\{[^}]*\}'); S2=$(echo "$D2" | grep -oE '"strip":\{[^}]*\}'); S3=$(echo "$D3" | grep -oE '"strip":\{[^}]*\}')
[ -n "$S1" ] || { echo "  FAIL no strip dump"; exit 1; }
expect "the strip opens at now"             true  "$(field "$S1" open)"
expect "opening bakes exactly once"         1     "$(field "$S1" bakes)"
expect "a scrub parks in the past"          true  "$(field "$S2" parked)"
W=$(field "$S2" words_at)
if [ "${W:-0}" -gt 0 ] 2>/dev/null; then echo "  ok   the readout counts the past ($W words)"; else
  echo "  FAIL words_at=$W (reconstruction empty?)"; fail=1; fi
expect "two scrubs later it has NOT re-baked" 1   "$(field "$S3" bakes)"
# The margin dump reports the MODEL; the hide is a render gate. margin_hidden
# is that gate's own bit (review H36).
expect "parked hides the live margin"       true  "$(field "$D2" margin_hidden)"
expect "Now brings the margin back"         false "$(field "$D4" margin_hidden)"

echo "rig-check: legacy history renders a real axis + a visible parked banner (Bug A/B)"
# A legacy file — six materialized checkpoints across two weeks, EMPTY journal.
# Before the fix the axis read only the journal, so every tick landed at x=0 and
# the whole pre-journal history was invisible. Fresh doc per run (the .strop
# sidecar persists across wrun launches within one rig run).
DOCL1=$(mktemp --suffix=.md); cp "$DOC" "$DOCL1"
OUT=$(WRUN_TAIL=80 scripts/wrun.sh "$DOCL1" "seed:legacy strip:open dump:ui strip:scrub:0.5 dump:ui ctrl-b dump:ui strip:now dump:ui" 2>/dev/null | grep 'UI-DUMP')
D1=$(echo "$OUT" | sed -n 1p); D2=$(echo "$OUT" | sed -n 2p); D3=$(echo "$OUT" | sed -n 3p); D4=$(echo "$OUT" | sed -n 4p)
S1=$(echo "$D1" | grep -oE '"strip":\{[^}]*\}'); S2=$(echo "$D2" | grep -oE '"strip":\{[^}]*\}'); S3=$(echo "$D3" | grep -oE '"strip":\{[^}]*\}')
[ -n "$S1" ] || { echo "  FAIL no strip dump (legacy)"; exit 1; }
ST=$(field "$S1" stations)
if [ "${ST:-0}" -ge 6 ] 2>/dev/null; then echo "  ok   the checkpoint era has stations ($ST)"; else
  echo "  FAIL stations=$ST (checkpoints not on the axis?)"; fail=1; fi
WK=$(field "$S1" work); WKI=${WK%.*}
if [ "${WKI:-0}" -gt 0 ] 2>/dev/null; then echo "  ok   the axis is non-degenerate (work=$WK)"; else
  echo "  FAIL work=$WK (axis collapsed to zero?)"; fail=1; fi
expect "a scrub parks in the legacy past"   true  "$(field "$S2" parked)"
expect "the parked banner is up"            true  "$(field "$S2" banner)"
WL=$(field "$S2" words_at)
if [ "${WL:-0}" -gt 0 ] 2>/dev/null; then echo "  ok   words_at reports the era ($WL words)"; else
  echo "  FAIL words_at=$WL (checkpoint-only reconstruction empty?)"; fail=1; fi
expect "parked hides the live margin"       true  "$(field "$D2" margin_hidden)"
# An edit gesture (ctrl-b) while parked is REFUSED: the banner pulses, the doc
# is untouched, and we stay parked (no silent restore — the terror Bug B fixes).
expect "a refused edit pulses the banner"   true  "$(field "$S3" pulse)"
expect "the refusal keeps us parked"        true  "$(field "$S3" parked)"
expect "the past is untouched (same chars)" "$(field "$D2" doc_chars)" "$(field "$D3" doc_chars)"
expect "Now returns the margin"             false "$(field "$D4" margin_hidden)"

echo "rig-check: no-history guard — an empty doc's strip never parks"
# No checkpoints, no journal (truly nothing): scrubbing the strip must NOT park.
DOCL2=$(mktemp --suffix=.md); : > "$DOCL2"
OUT=$(WRUN_TAIL=40 scripts/wrun.sh "$DOCL2" "strip:open strip:scrub:0.5 dump:ui" 2>/dev/null | grep 'UI-DUMP')
DN=$(echo "$OUT" | tail -1); SN=$(echo "$DN" | grep -oE '"strip":\{[^}]*\}')
if [ -n "$SN" ]; then
  expect "an empty-history scrub does not park" false "$(field "$SN" parked)"
else
  echo "  ok   an empty-history strip has no bake to park (degraded, fine)"
fi

echo "rig-check: asides file honestly — compost is text, the graveyard gives back"
# seed:aside asides one paragraph (births the rail) and exiles another (over
# the cut threshold). putback:last is the graveyard's single verb: the entry
# leaves the record and the prose returns — the inverse in the same grammar.
DOCA=$(mktemp --suffix=.md); cp "$DOC" "$DOCA"
OUT=$(WRUN_TAIL=60 scripts/wrun.sh "$DOCA" "seed:aside dump:ui putback:last dump:ui" 2>/dev/null | grep 'UI-DUMP')
D1=$(echo "$OUT" | head -1); D2=$(echo "$OUT" | tail -1)
[ -n "$D1" ] || { echo "  FAIL no dump"; exit 1; }
CB=$(field "$D1" compost_blocks)
if [ "${CB:-0}" -gt 0 ] 2>/dev/null; then echo "  ok   the aside birthed the rail (compost_blocks=$CB)"; else
  echo "  FAIL compost_blocks=$CB"; fail=1; fi
expect "the exile filed one entry"          1 "$(field "$D1" grave_entries)"
expect "put back empties the graveyard"     0 "$(field "$D2" grave_entries)"
# The graveyard record renders at the document TAIL in the scroll flow (Bug B):
# with this short fixture the whole doc + section fit, so the section header is
# on screen and the sticky footer bar unsticks (hides) into it (asides.md §3).
expect "the footer bar unsticks when the tail is on screen" true "$(field "$D1" grave_bar_hidden)"
MW=$(field "$D1" manuscript_words)
if [ "${MW:-0}" -gt 0 ] 2>/dev/null; then echo "  ok   the count is manuscript-only ($MW words)"; else
  echo "  FAIL manuscript_words=$MW"; fail=1; fi

echo "rig-check: a cut annotated paragraph leaves no dangling note (Bug C)"
# seed:annotated selects a paragraph carrying BOTH a writer note and a machine
# diagnosis; exile:selection cuts it. The writer note must change address to the
# compost (its own words are never lost); the diagnosis must close (a machine
# card never lingers pointing at nothing).
DOCC=$(mktemp --suffix=.md); cp "$DOC" "$DOCC"
OUT=$(WRUN_TAIL=60 scripts/wrun.sh "$DOCC" "seed:annotated dump:ui exile:selection dump:ui" 2>/dev/null | grep 'UI-DUMP')
C1=$(echo "$OUT" | head -1); C2=$(echo "$OUT" | tail -1)
[ -n "$C1" ] || { echo "  FAIL no dump"; exit 1; }
rm -f "$DOCC" "$DOCC.strop"
expect "the writer note starts open"          1 "$(field "$C1" open_notes)"
expect "the diagnosis starts open"            1 "$(field "$C1" open_diagnoses)"
expect "no compost before the cut"            0 "$(field "$C1" compost_blocks)"
expect "the cut files one grave entry"        1 "$(field "$C2" grave_entries)"
expect "the writer note left the margin"      0 "$(field "$C2" open_notes)"
expect "the dead-anchored diagnosis closed"   0 "$(field "$C2" open_diagnoses)"
CB2=$(field "$C2" compost_blocks)
if [ "${CB2:-0}" -gt 0 ] 2>/dev/null; then echo "  ok   the note migrated to the compost (compost_blocks=$CB2)"; else
  echo "  FAIL compost_blocks=$CB2 — the note did not migrate"; fail=1; fi

echo "rig-check: the omnibar is a field, and Esc walks the selection home (06 §1)"
# Typing "abcdef", parking the caret at the end, then ctrl-f + "b": the find
# preview WALKS selected_range onto the match [1,2). Esc must restore the
# pre-find state (S3/P13) and hand focus back to the prose.
DOCO=$(mktemp --suffix=.md); : > "$DOCO"
OUT=$(WRUN_TAIL=60 scripts/wrun.sh "$DOCO" "abcdef ctrl-f wait:80 dump:ui b wait:80 dump:ui escape wait:80 dump:ui" 2>/dev/null | grep 'UI-DUMP')
O1=$(echo "$OUT" | sed -n 1p); O2=$(echo "$OUT" | sed -n 2p); O3=$(echo "$OUT" | sed -n 3p)
[ -n "$O3" ] || { echo "  FAIL no dump"; exit 1; }
rm -f "$DOCO" "$DOCO.strop"
expect "ctrl-f focuses the omnibar field"    '"PaletteInput"' "$(field "$O1" focused)"
expect "the preview walks the selection"     '[1,2]' "$(echo "$O2" | grep -oE '"sel":\[[0-9]+,[0-9]+\]' | sed 's/"sel"://')"
expect "Esc returns focus to the prose"      '"Editor"' "$(field "$O3" focused)"
expect "Esc walks the selection home"        '[6,6]' "$(echo "$O3" | grep -oE '"sel":\[[0-9]+,[0-9]+\]' | sed 's/"sel"://')"

echo "rig-check: keyboard nav past the fold scrolls the omni-list; wheel still works (palette-scroll)"
# f10 opens the omnibox straight into command mode (bare `>`) — the grouped
# Frequent/File/Edit/… browsing view, comfortably longer than the 420px
# list, so 20 downs walks the highlight well past the visible window. The
# bug: palette_up/down moved `palette_selected` but never scrolled, so the
# writer navigated blind. The fix nudges a tracked ScrollHandle on every
# selection change; a manual wheel scroll afterwards must still work AND
# must not get snapped back by an unrelated redraw (the wait:200 with no
# input in between).
DOCP=$(mktemp --suffix=.md); : > "$DOCP"
KEYS="f10"
for _ in $(seq 1 20); do KEYS="$KEYS down"; done
KEYS="$KEYS dump:ui wheel:800,300,300 dump:ui wait:200 dump:ui"
OUT=$(WRUN_TAIL=60 scripts/wrun.sh "$DOCP" "$KEYS" 2>/dev/null | grep -oE '"omni":\{[^}]*\}')
P1=$(echo "$OUT" | sed -n 1p); P2=$(echo "$OUT" | sed -n 2p); P3=$(echo "$OUT" | sed -n 3p)
[ -n "$P3" ] || { echo "  FAIL no omni dump"; exit 1; }
rm -f "$DOCP" "$DOCP.strop"
expect "20 downs select row 20" 20 "$(field "$P1" selected)"
S1=$(field "$P1" scroll_y); S2=$(field "$P2" scroll_y); S3=$(field "$P3" scroll_y)
if awk "BEGIN{exit !($S1 < -1)}" 2>/dev/null; then
  echo "  ok   the list scrolled to keep row 20 visible (scroll_y=$S1)"
else
  echo "  FAIL row 20 selected but scroll_y=$S1 — navigation walked off-screen blind"; fail=1
fi
if [ "$S2" != "$S1" ]; then echo "  ok   the wheel still moves the list ($S1 -> $S2)"; else
  echo "  FAIL wheel did not move scroll_y ($S1)"; fail=1; fi
expect "selection is untouched by the wheel"            20  "$(field "$P2" selected)"
expect "an unrelated redraw doesn't snap the wheel back" "$S2" "$S3"

echo "rig-check: set-aside births the region — the chip exists, the receipt fires"
# The pile lives at the TAIL (the Scraps flip): the first aside is the
# ADOPTION of the trailing paragraph (its blank divider becomes the seam);
# the second parks the manuscript's first paragraph. The rail is dead — the
# compliance signal is the chip's existence + the arrival pulse.
DOCR=$(mktemp --suffix=.md); : > "$DOCR"
OUT=$(WRUN_TAIL=60 scripts/wrun.sh "$DOCR" "onexx enter enter twoxx ctrl-end select:para aside:selection wait:80 dump:ui ctrl-home select:para aside:selection wait:80 dump:ui" 2>/dev/null | grep 'UI-DUMP')
R1=$(echo "$OUT" | sed -n 1p); R2=$(echo "$OUT" | sed -n 2p)
[ -n "$R2" ] || { echo "  FAIL no dump"; exit 1; }
rm -f "$DOCR" "$DOCR.strop"
expect "the first aside births the region"   true "$(field "$R1" scraps_chip_exists)"
expect "the second park keeps it"            true "$(field "$R2" scraps_chip_exists)"
expect "the park's chip receipt fired"       true "$(field "$R2" chip_pulse)"
CBR=$(field "$R2" compost_blocks)
if [ "${CBR:-0}" -gt 1 ] 2>/dev/null; then echo "  ok   both passages landed in the scraps (compost_blocks=$CBR)"; else
  echo "  FAIL compost_blocks=$CBR"; fail=1; fi

echo "rig-check: the excursion latch — travel latches, walked-in carets stay plain text"
# scraps:travel (the chip / ctrl-shift-o) arms the latch; Esc returns exactly
# home and writes pile_end; a later travel RESUMES pile_end (both ends). A
# caret the writer walked in herself (ctrl-end) never latches — Esc is inert.
DOCX=$(mktemp --suffix=.md); : > "$DOCX"
OUT=$(WRUN_TAIL=120 scripts/wrun.sh "$DOCX" "seed:demo ctrl-home dump:ui scraps:travel dump:ui x dump:ui escape dump:ui scraps:travel dump:ui escape ctrl-end dump:ui escape dump:ui" 2>/dev/null | grep 'UI-DUMP')
X1=$(echo "$OUT" | sed -n 1p); X2=$(echo "$OUT" | sed -n 2p); X3=$(echo "$OUT" | sed -n 3p)
X4=$(echo "$OUT" | sed -n 4p); X5=$(echo "$OUT" | sed -n 5p); X6=$(echo "$OUT" | sed -n 6p); X7=$(echo "$OUT" | sed -n 7p)
[ -n "$X7" ] || { echo "  FAIL no dump"; exit 1; }
rm -f "$DOCX" "$DOCX.strop"
expect "at rest: unlatched"                    false "$(field "$X1" latched)"
expect "travel arms the latch"                 true  "$(field "$X2" latched)"
expect "travel enters the pile"                true  "$(field "$X2" caret_scraps)"
# The chip counts the PIECE in every state (2026-07-11: the caret-scoped
# region label was retired — the Scraps header names the pile itself).
expect "the count control never flips"         'words"' "$(field "$X2" count_label | cut -d' ' -f2)"
expect "typing INSIDE keeps the latch"         true  "$(field "$X3" latched)"
expect "Esc travels home"                      false "$(field "$X4" caret_scraps)"
expect "Esc drops the latch"                   false "$(field "$X4" latched)"
PE=$(field "$X4" pile_end)
if [ "${PE:-null}" != "null" ]; then echo "  ok   Esc wrote pile_end ($PE)"; else
  echo "  FAIL pile_end=$PE — the skim stop was not remembered"; fail=1; fi
S5=$(echo "$X5" | grep -oE '"sel":\[[0-9]+,[0-9]+\]' | sed 's/"sel"://')
if [ "$S5" = "[$PE,$PE]" ]; then echo "  ok   the next travel resumes pile_end ($S5)"; else
  echo "  FAIL travel resumed at $S5, wanted [$PE,$PE]"; fail=1; fi
expect "ctrl-end walks in unlatched"           false "$(field "$X6" latched)"
expect "walked-in caret is in the pile"        true  "$(field "$X6" caret_scraps)"
expect "Esc on a walked-in caret is inert"     true  "$(field "$X7" caret_scraps)"

echo "rig-check: the chips exist with their sections and hide with them on screen"
# A LONG doc with a park at its head: the pile lands at the far tail, off
# screen, so the Scraps chip must SHOW; scrolled to the tail, the region is
# on screen and it must hide (the two-sided gate, surfaces-attention 1).
DOCY=$(mktemp --suffix=.md); cp "$DOC" "$DOCY"
OUT=$(WRUN_TAIL=120 scripts/wrun.sh "$DOCY" "ctrl-home select:para aside:selection wait:80 dump:ui ctrl-end wait:80 dump:ui" 2>/dev/null | grep 'UI-DUMP')
Y1=$(echo "$OUT" | sed -n 1p); Y2=$(echo "$OUT" | sed -n 2p)
[ -n "$Y2" ] || { echo "  FAIL no dump"; exit 1; }
rm -f "$DOCY" "$DOCY.strop"
expect "the chip exists once the region does"  true  "$(field "$Y1" scraps_chip_exists)"
expect "seam off-screen: the chip shows"       false "$(field "$Y1" scraps_chip_hidden)"
expect "the park's receipt pulsed the chip"    true  "$(field "$Y1" chip_pulse)"
expect "seam on screen: the chip hides"        true  "$(field "$Y2" scraps_chip_hidden)"

echo "rig-check: the park receipt — origin ghost now, gone after its fade"
DOCG=$(mktemp --suffix=.md); cp "$DOC" "$DOCG"
OUT=$(WRUN_TAIL=120 scripts/wrun.sh "$DOCG" "ctrl-home select:para aside:selection dump:ui wait:400 dump:ui" 2>/dev/null | grep 'UI-DUMP')
G1=$(echo "$OUT" | sed -n 1p); G2=$(echo "$OUT" | sed -n 2p)
[ -n "$G2" ] || { echo "  FAIL no dump"; exit 1; }
rm -f "$DOCG" "$DOCG.strop"
expect "the origin ghost is up at commit"      true  "$(field "$G1" park_ghost)"
expect "the ghost is gone after its fade"      false "$(field "$G2" park_ghost)"

echo "rig-check: the retype-race guard — a textless pile holds its seam until the caret leaves"
DOCE=$(mktemp --suffix=.md); : > "$DOCE"
OUT=$(WRUN_TAIL=120 scripts/wrun.sh "$DOCE" "seed:demo scraps:travel ctrl-a delete dump:ui ctrl-home dump:ui" 2>/dev/null | grep 'UI-DUMP')
E1=$(echo "$OUT" | sed -n 1p); E2=$(echo "$OUT" | sed -n 2p)
[ -n "$E2" ] || { echo "  FAIL no dump"; exit 1; }
rm -f "$DOCE" "$DOCE.strop"
if [ "$(field "$E1" scraps_words)" = "0" ] && [ "$(field "$E1" seam)" != "null" ]; then
  echo "  ok   the emptied pile holds its seam while the caret stays (count reads 0)"
else
  echo "  FAIL textless pile: seam=$(field "$E1" seam) words=$(field "$E1" scraps_words)"; fail=1
fi
expect "the caret leaving evaporates the seam" null "$(field "$E2" seam)"

echo "rig-check: Move to the manuscript arrives SELECTED at home"
DOCV=$(mktemp --suffix=.md); cp "$DOC" "$DOCV"
OUT=$(WRUN_TAIL=120 scripts/wrun.sh "$DOCV" "ctrl-home select:para aside:selection wait:80 scraps:travel move:manuscript dump:ui" 2>/dev/null | grep 'UI-DUMP')
V1=$(echo "$OUT" | sed -n 1p)
[ -n "$V1" ] || { echo "  FAIL no dump"; exit 1; }
rm -f "$DOCV" "$DOCV.strop"
SV=$(echo "$V1" | grep -oE '"sel":\[[0-9]+,[0-9]+\]' | sed 's/"sel"://')
SVS=$(echo "$SV" | tr -d '[]' | cut -d, -f1); SVE=$(echo "$SV" | tr -d '[]' | cut -d, -f2)
if [ "${SVE:-0}" -gt "${SVS:-0}" ] 2>/dev/null; then echo "  ok   the moved text arrived selected ($SV)"; else
  echo "  FAIL selection empty after the move ($SV)"; fail=1; fi
expect "it landed in the manuscript"           false "$(field "$V1" caret_scraps)"
expect "the excursion ended with the return"   false "$(field "$V1" latched)"

echo "rig-check: find announces its split — both region counts as data"
# The long fixture + one parked paragraph: 'paragraph' matches land on both
# sides of the seam, and the omni dump carries the split halves.
DOCF=$(mktemp --suffix=.md); cp "$DOC" "$DOCF"
# The query must match on BOTH sides: "in" hits the padding's "padding" and
# the parked opener's "into" — and stays far under the 500 cap.
OUT=$(WRUN_TAIL=120 scripts/wrun.sh "$DOCF" "ctrl-home select:para aside:selection wait:80 ctrl-f wait:80 i n wait:120 dump:ui" 2>/dev/null | grep 'UI-DUMP' | tail -1)
OM=$(echo "$OUT" | grep -oE '"omni":\{[^}]*\}')
[ -n "$OM" ] || { echo "  FAIL no omni dump"; exit 1; }
rm -f "$DOCF" "$DOCF.strop"
FP=$(field "$OM" find_piece); FS=$(field "$OM" find_scraps)
if [ "${FP:-0}" -gt 0 ] 2>/dev/null && [ "${FS:-0}" -gt 0 ] 2>/dev/null; then
  echo "  ok   the split names both regions ($FP in the piece · $FS in scraps)"
else
  echo "  FAIL find split piece=$FP scraps=$FS"; fail=1
fi

echo "rig-check: a shipped compost-at-top file migrates once, at open (Scraps Wave A)"
# Run 1 writes a REAL top-era .strop (seed:topera installs the legacy
# boundary and saves). Run 2 reopens it: the one-time migration flips the
# geometry before the first edit — the tail seam exists, the pile counts,
# and the manuscript-only word count is UNCHANGED (membership-preserving).
DOCM=$(mktemp --suffix=.md); : > "$DOCM"
OUT=$(WRUN_TAIL=60 scripts/wrun.sh "$DOCM" "seed:topera dump:ui" 2>/dev/null | grep 'UI-DUMP' | tail -1)
[ -n "$OUT" ] || { echo "  FAIL no dump (seed:topera)"; exit 1; }
expect "run 1 is top-era (no tail seam yet)"   null "$(field "$OUT" seam)"
MW1=$(field "$OUT" manuscript_words)
OUT=$(WRUN_TAIL=60 scripts/wrun.sh "$DOCM" "dump:ui" 2>/dev/null | grep 'UI-DUMP' | tail -1)
[ -n "$OUT" ] || { echo "  FAIL no dump (migration run)"; exit 1; }
rm -f "$DOCM" "$DOCM.strop"
SEAM=$(field "$OUT" seam)
if [ "${SEAM:-null}" != "null" ] && [ "${SEAM:-0}" -ge 1 ] 2>/dev/null; then
  echo "  ok   reopening migrated to the tail era (seam=$SEAM)"
else
  echo "  FAIL seam=$SEAM — the top-era file did not migrate"; fail=1
fi
expect "the pile survived the flip"            1 "$(field "$OUT" compost_blocks)"
expect "the count never teleported (07 N3)"    "$MW1" "$(field "$OUT" manuscript_words)"

# ---------------- The cold read (impl 05 Wave B) ----------------------------

echo "rig-check: cold read — entry/exit round-trip (scroll, checkpoints, focus, margin)"
DOCR=$(mktemp --suffix=.md); cp "$DOC" "$DOCR"
OUT=$(WRUN_TAIL=200 scripts/wrun.sh "$DOCR" "wheel:800,400,-240 dump:ui coldread:open dump:ui escape dump:ui coldread:open escape dump:ui" 2>/dev/null | grep 'UI-DUMP')
D1=$(echo "$OUT" | sed -n 1p); D2=$(echo "$OUT" | sed -n 2p); D3=$(echo "$OUT" | sed -n 3p); D4=$(echo "$OUT" | sed -n 4p)
[ -n "$D4" ] || { echo "  FAIL missing dumps"; exit 1; }
rm -f "$DOCR" "$DOCR.strop"
SY1=$(field "$D1" scroll_y); SY3=$(field "$D3" scroll_y)
CR2=$(echo "$D2" | grep -oE '"coldread":\{[^}]*\}')
expect "the room is up with a real book"      true "$(field "$CR2" open)"
expect "page 1 under the ritual threshold"    0 "$(field "$CR2" page)"
expect "the takeover holds focus (F8)"        '"ColdRead"' "$(field "$D2" focused)"
expect "the margin sleeps under the desk"     true "$(field "$D2" margin_hidden)"
expect "entry checkpointed once (L3)"         1 "$(field "$D2" checkpoints)"
expect "Esc drops the room"                   null "$(field "$D3" coldread)"
expect "the margin gate reopens"              false "$(field "$D3" margin_hidden)"
expect "scroll untouched by the round trip"   "$SY1" "$SY3"
expect "double-open dedupes the checkpoint"   1 "$(field "$D4" checkpoints)"

echo "rig-check: cold read — guard pulses (the pierce table, F4)"
DOCG=$(mktemp --suffix=.md); cp "$DOC" "$DOCG"
OUT=$(WRUN_TAIL=300 scripts/wrun.sh "$DOCG" "coldread:open ctrl-shift-p dump:ui ctrl-f dump:ui ctrl-alt-h dump:ui ctrl-shift-d dump:ui j dump:ui wait:1100 shift dump:ui" 2>/dev/null | grep 'UI-DUMP')
rm -f "$DOCG" "$DOCG.strop"
i=1
for chord in ctrl-shift-p ctrl-f ctrl-alt-h ctrl-shift-d; do
  D=$(echo "$OUT" | sed -n ${i}p); i=$((i+1))
  CR=$(echo "$D" | grep -oE '"coldread":\{[^}]*\}')
  expect "$chord raises nothing"              '[]' "$(echo "$D" | grep -oE '"overlays":\[[^]]*\]' | sed 's/"overlays"://')"
  expect "$chord pulses the banner"           true "$(field "$CR" pulse)"
done
D=$(echo "$OUT" | sed -n 5p); CR=$(echo "$D" | grep -oE '"coldread":\{[^}]*\}')
expect "a letter key pulses (typing rule)"    true "$(field "$CR" pulse)"
D=$(echo "$OUT" | sed -n 6p); CR=$(echo "$D" | grep -oE '"coldread":\{[^}]*\}')
expect "a lone shift does NOT pulse"          false "$(field "$CR" pulse)"

echo "rig-check: cold read — copy is source-honest across hyphenated breaks (F5)"
DOCC=$(mktemp --suffix=.md)
{
  echo "Типографика переносов проверяется длинными русскими словами: образовательными учреждениями, предположительными обстоятельствами, естественнонаучными представлениями."
  echo
  echo "Замечательное словосочетание переносится предсказуемо, и выделение через перенос обязано вернуть исходную подстроку без дефисов и без потерянных пробелов."
} > "$DOCC"
OUT=$(WRUN_TAIL=100 scripts/wrun.sh "$DOCC" "coldread:open dump:ui coldread:select:0,300 coldread:copycheck" 2>&1 | grep -E "UI-DUMP|COPY-GOLDEN")
rm -f "$DOCC" "$DOCC.strop"
CR=$(echo "$OUT" | grep -oE '"coldread":\{[^}]*\}' | head -1)
HY=$(field "$CR" hyphens)
if [ "${HY:-0}" -gt 0 ] 2>/dev/null; then echo "  ok   the RU page really hyphenates (hyphens=$HY)"; else
  echo "  FAIL no hyphen-ended lines on the RU page"; fail=1; fi
if echo "$OUT" | grep -q "COPY-GOLDEN: OK"; then echo "  ok   clipboard equals the source substring"; else
  echo "  FAIL $(echo "$OUT" | grep COPY-GOLDEN)"; fail=1; fi

echo "rig-check: cold read — the reveal clock holds in the room (F7)"
DOCD=$(mktemp --suffix=.md); cp "$DOC" "$DOCD"
OUT=$(WRUN_TAIL=200 scripts/wrun.sh "$DOCD" "coldread:open seed:deliver wait:1200 dump:ui wheel:800,500,-240 dump:ui escape wait:200 dump:ui" 2>/dev/null | grep 'UI-DUMP')
D1=$(echo "$OUT" | sed -n 1p); DW=$(echo "$OUT" | sed -n 2p); D2=$(echo "$OUT" | sed -n 3p)
rm -f "$DOCD" "$DOCD.strop"
expect "results park while the room is up"    true "$(field "$D1" ai_deferred)"
expect "no cards land mid-read"               0 "$(field "$D1" open_diagnoses)"
# F6: the wheel is eaten twice — nothing scrolls, nothing flushes, no flip.
CRW=$(echo "$DW" | grep -oE '"coldread":\{[^}]*\}')
expect "a wheel mid-read never scrolls (F6)"  "$(field "$D1" scroll_y)" "$(field "$DW" scroll_y)"
expect "…never flushes the parked pass"       true "$(field "$DW" ai_deferred)"
expect "…never flips the page"                0 "$(field "$CRW" page)"
expect "exit flushes the parked pass"         false "$(field "$D2" ai_deferred)"
OD=$(field "$D2" open_diagnoses)
if [ "${OD:-0}" -gt 0 ] 2>/dev/null; then echo "  ok   the cards landed after Esc ($OD)"; else
  echo "  FAIL nothing landed after exit"; fail=1; fi

echo "rig-check: cold read — reactions file as ordinary notes (spec 5.3)"
DOCN=$(mktemp --suffix=.md); cp "$DOC" "$DOCN"
OUT=$(WRUN_TAIL=200 scripts/wrun.sh "$DOCN" "coldread:open coldread:select:5,40 coldread:react:? dump:ui coldread:select:60,90 coldread:react:~ dump:ui escape dump:ui" 2>/dev/null | grep 'UI-DUMP')
D1=$(echo "$OUT" | sed -n 1p); D2=$(echo "$OUT" | sed -n 2p); D3=$(echo "$OUT" | sed -n 3p)
rm -f "$DOCN" "$DOCN.strop"
CR1=$(echo "$D1" | grep -oE '"coldread":\{[^}]*\}')
CR2=$(echo "$D2" | grep -oE '"coldread":\{[^}]*\}')
expect "the chip filed the glyph body (P3)"   '"? doubt"' "$(field "$CR1" last_body)"
expect "the lane carries the session card"    1 "$(field "$CR1" lane)"
expect "a second reaction joins the lane"     2 "$(field "$CR2" lane)"
expect "both are ordinary open notes"         2 "$(field "$D2" open_notes)"
expect "they live on after the room closes"   2 "$(field "$D3" open_notes)"

echo "rig-check: quit flushes an open cold-read reaction"
DOCQ=$(mktemp --suffix=.md); cp "$DOC" "$DOCQ"
WRUN_TAIL=80 scripts/wrun.sh "$DOCQ" "coldread:open coldread:select:5,40 coldread:raise h i" >/dev/null 2>&1
OUT=$(WRUN_TAIL=40 scripts/wrun.sh "$DOCQ" "dump:ui" 2>/dev/null | grep 'UI-DUMP' | tail -1)
rm -f "$DOCQ" "$DOCQ.strop"
[ -n "$OUT" ] || { echo "  FAIL no dump (quit-flush cold read)"; exit 1; }
expect "quit files the unfinished reaction"    1 "$(field "$OUT" open_notes)"

# D1 — the open note owns its keys. The original bug: a space mid-note flipped
# the page (which files the note first), so a two-word reaction became "one word
# per note". Typed through REAL key dispatch: `space` must land IN the field.
echo "rig-check: cold read — a multi-word note owns the keyboard (D1)"
DOCK=$(mktemp --suffix=.md); cp "$DOC" "$DOCK"
OUT=$(WRUN_TAIL=200 scripts/wrun.sh "$DOCK" "coldread:open coldread:select:5,40 coldread:raise h i space t h e r e enter dump:ui" 2>/dev/null | grep 'UI-DUMP')
D=$(echo "$OUT" | tail -1)
rm -f "$DOCK" "$DOCK.strop"
CR=$(echo "$D" | grep -oE '"coldread":\{[^}]*\}')
expect "the space typed INTO the note"        '"hi there"' "$(field "$CR" last_body)"
expect "one note carries the whole phrase"    1 "$(field "$CR" lane)"
expect "the page never flipped mid-note"      0 "$(field "$CR" page)"
expect "the input closed on commit"           false "$(field "$CR" input)"
# Report 2's sibling: the commit exit must hand the keys back to the DESK —
# focus stranded on the removed field left the room Esc-proof.
expect "commit hands the keys back to the desk" '"ColdRead"' "$(field "$D" focused)"

# Report 2 — two-level Esc with the pencil up: the first Esc discards the
# note (FieldCancel at the NoteComposer context) and refocuses the desk; the
# second leaves the room. Pre-fix, the field's exits left focus on the dead
# field, so the second Esc dispatched into a void and the room never closed.
echo "rig-check: cold read — Esc closes the note, Esc again leaves the room"
DOCK3=$(mktemp --suffix=.md); cp "$DOC" "$DOCK3"
OUT=$(WRUN_TAIL=200 scripts/wrun.sh "$DOCK3" "coldread:open coldread:select:5,40 coldread:raise h i escape dump:ui escape dump:ui" 2>/dev/null | grep 'UI-DUMP')
D1=$(echo "$OUT" | sed -n 1p); D2=$(echo "$OUT" | sed -n 2p)
[ -n "$D2" ] || { echo "  FAIL no dump (two-level Esc)"; exit 1; }
rm -f "$DOCK3" "$DOCK3.strop"
CR1=$(echo "$D1" | grep -oE '"coldread":\{[^}]*\}')
expect "the first Esc closes the note input"  false "$(field "$CR1" input)"
expect "…and discards the draft"              0 "$(field "$CR1" lane)"
expect "…and the desk holds the keys again"   '"ColdRead"' "$(field "$D1" focused)"
expect "the second Esc leaves the room"       null "$(field "$D2" coldread)"

# D1 mouse — a click that resolves an open note commits it but must NOT also
# flip the page (commit-only, the one carve-out in C4's commit-AND-act rule).
echo "rig-check: cold read — a resolving click never flips (D1 mouse)"
DOCK2=$(mktemp --suffix=.md); cp "$DOC" "$DOCK2"
OUT=$(WRUN_TAIL=200 scripts/wrun.sh "$DOCK2" "coldread:open coldread:select:5,40 coldread:raise h i dump:ui coldread:pageclick:1 dump:ui" 2>/dev/null | grep 'UI-DUMP')
D1=$(echo "$OUT" | sed -n 1p); D2=$(echo "$OUT" | sed -n 2p)
rm -f "$DOCK2" "$DOCK2.strop"
CR1=$(echo "$D1" | grep -oE '"coldread":\{[^}]*\}')
CR2=$(echo "$D2" | grep -oE '"coldread":\{[^}]*\}')
expect "the input is up before the click"     true "$(field "$CR1" input)"
expect "a right-zone click resolves the note" false "$(field "$CR2" input)"
expect "…files it as an ordinary note"        '"hi"' "$(field "$CR2" last_body)"
expect "…and does NOT flip the page"          0 "$(field "$CR2" page)"

echo "rig-check: cold read — Past-from-parked round trip (Time 7, regions 13)"
DOCL=$(mktemp --suffix=.md); echo "A live line for the legacy litmus." > "$DOCL"
# The md import writes a "Started" birth checkpoint (index 0), so the seeded
# legacy plan sits at indexes 1..7 — the Top-era tick is coldread:past:7.
OUT=$(WRUN_TAIL=300 scripts/wrun.sh "$DOCL" "seed:legacy dump:ui coldread:open dump:ui escape strip:open strip:scrub:0.5 coldread:past:7 dump:ui escape dump:ui" 2>/dev/null | grep 'UI-DUMP')
D1=$(echo "$OUT" | sed -n 1p); D2=$(echo "$OUT" | sed -n 2p); D3=$(echo "$OUT" | sed -n 3p); D4=$(echo "$OUT" | sed -n 4p)
rm -f "$DOCL" "$DOCL.strop"
CR2=$(echo "$D2" | grep -oE '"coldread":\{[^}]*\}')
CR3=$(echo "$D3" | grep -oE '"coldread":\{[^}]*\}')
expect "seed:legacy carries the Top-era tick" 8 "$(field "$D1" checkpoints)"
expect "a Live read of a legacy file opens"   true "$(field "$CR2" open)"
expect "the Past book reads the OLD state"    '"past"' "$(field "$CR3" source)"
P1=$(field "$CR3" p1)
case "$P1" in
  '"The manuscript opens on '*) echo "  ok   page 1 is the manuscript, never the pile (F1)";;
  *) echo "  FAIL Past page 1 = $P1 (the pile leaked in)"; fail=1;;
esac
POS3=$(field "$D3" pos_ms); POS4=$(field "$D4" pos_ms)
CP3=$(field "$D3" checkpoints); CP4=$(field "$D4" checkpoints)
expect "a Past read leaves NO entry checkpoint" "$CP3" "$CP4"
expect "the parked strip survives untouched"  true "$(field "$D4" parked)"
expect "…at the identical playhead"           "$POS3" "$POS4"
expect "the parked banner returns"            true "$(field "$D4" banner)"

echo "rig-check: cold read — the empty book and the reduce-motion flip (regions 4, S12)"
DOCE=$(mktemp --suffix=.md); : > "$DOCE"
OUT=$(WRUN_TAIL=100 scripts/wrun.sh "$DOCE" "coldread:open dump:ui" 2>/dev/null | grep 'UI-DUMP' | tail -1)
rm -f "$DOCE" "$DOCE.strop"
CR=$(echo "$OUT" | grep -oE '"coldread":\{[^}]*\}')
expect "an empty doc is one honest blank page" 1 "$(field "$CR" pages)"
expect "…counted honestly in the banner"       0 "$(field "$CR" words)"
DOCM2=$(mktemp --suffix=.md); cp "$DOC2" "$DOCM2"
OUT=$(WRUN_TAIL=200 scripts/wrun.sh "$DOCM2" "coldread:open right dump:ui escape reduce:motion coldread:open right dump:ui" 2>/dev/null | grep 'UI-DUMP')
D1=$(echo "$OUT" | sed -n 1p); D2=$(echo "$OUT" | sed -n 2p)
rm -f "$DOCM2" "$DOCM2.strop"
CR1=$(echo "$D1" | grep -oE '"coldread":\{[^}]*\}')
CR2=$(echo "$D2" | grep -oE '"coldread":\{[^}]*\}')
expect "a flip from rest fades in (S5)"        true "$(field "$CR1" fading)"
expect "reduce_motion flips instantly (S12)"   false "$(field "$CR2" fading)"

echo "rig-check: cold read — over the ritual threshold the read opens at the caret's chapter"
DOCT=$(mktemp --suffix=.md)
{
  for c in $(seq 1 12); do
    printf '## Chapter %s\n\n' "$c"
    for p in 1 2 3 4 5; do
      for r in $(seq 1 20); do printf 'again the ferry held its line against the dark water and the far shore '; done
      printf '\n\n'
    done
  done
} > "$DOCT"
OUT=$(WRUN_TAIL=200 scripts/wrun.sh "$DOCT" "ctrl-end coldread:open dump:ui escape ctrl-home coldread:open dump:ui" 2>/dev/null | grep 'UI-DUMP')
D1=$(echo "$OUT" | sed -n 1p); D2=$(echo "$OUT" | sed -n 2p)
[ -n "$D2" ] || { echo "  FAIL missing dumps (threshold fixture)"; exit 1; }
rm -f "$DOCT" "$DOCT.strop"
CR1=$(echo "$D1" | grep -oE '"coldread":\{[^}]*\}')
CR2=$(echo "$D2" | grep -oE '"coldread":\{[^}]*\}')
W=$(field "$CR1" words)
if [ "${W:-0}" -gt 10000 ] 2>/dev/null; then echo "  ok   the fixture crosses the ritual threshold ($W words)"; else
  echo "  FAIL fixture too small (words=$W)"; fail=1; fi
P1=$(field "$CR1" page)
if [ "${P1:-0}" -gt 0 ] 2>/dev/null; then echo "  ok   a caret in the last chapter opens the read mid-book (page $((P1+1)))"; else
  echo "  FAIL over-threshold entry stayed at page 1 (page=$P1)"; fail=1; fi
expect "a caret at the top reads from page 1"  0 "$(field "$CR2" page)"

# ---------------- Inline images (docs/inline-images.md §11) -----------------
# The §11 acceptance script: the round's field repro driven end-to-end through
# REAL key dispatch (stage / refuse / exile / put back), then the travel verbs
# (copy, cut, paste, replace-in-place, external drop). Every law that has a
# state channel is asserted programmatically — the SMOKE per-key lines carry
# `imgsel=<block> door=<caret> hold=<key>`, `img-geo` carries every picture's
# rect + caption line + asset id, `dump:ui` carries doc hash/chars and the
# graveyard census; pixels are judged by the stills at the end, never diffed.
# Two limits stand recorded, untestable under the rig: real compositor
# drag-and-drop (the rig synthesizes the FileDropEvent stream gpui's window
# would translate — the translation itself is the fork's, not ours) and
# cross-document/cross-app clipboard travel (one process per document; Linux
# offers text-only MIMEs cross-app — spec §9's recorded platform cost).

# Two tiny PNG fixtures (64x48 red, 48x64 blue), embedded so the check stays
# self-contained; content addressing gives each a distinct, stable asset id.
IMG_A=$(mktemp --suffix=.png)
IMG_B=$(mktemp --suffix=.png)
base64 -d > "$IMG_A" <<'PNG'
iVBORw0KGgoAAAANSUhEUgAAAEAAAAAwCAIAAAAuKetIAAAAZElEQVR4nO3PUQkAIBTAwBfHEPbH
WIbw4xAGC3Cbs9fXDRc0oAUNaEEDWtCAFjSgBQ1oQQNa0IAWNKAFDWhBA1rQgBY0oAUNaEEDWtCA
FjSgBQ1oQQNa0IAWNKAFDWhBA1rw2AUOKBDTbVK3WwAAAABJRU5ErkJggg==
PNG
base64 -d > "$IMG_B" <<'PNG'
iVBORw0KGgoAAAANSUhEUgAAADAAAABACAIAAADTQmMRAAAATUlEQVR4nO3OQQ0AIAwAsclBE4qR
hQuOR5MK6Kx9vjL5QEhISKgeCAkJCdUDISEhoXogJCQkVA+EhISE6oGQkJBQPRASEhKqB0JCQo9d
Elh44ohqjGcAAAAASUVORK5CYII=
PNG

# One img-geo line -> a named value. Format:
#   img IX @X,Y w=W h=H cap@CX,CY src=asset:...
imgv() { # line field(ix|x|y|w|h|capy|src)
  echo "$1" | awk -v want="$2" '{
    split($3, c, /[@,]/); split($6, k, /[@,]/);
    v["ix"]=$2; v["x"]=c[2]; v["y"]=c[3];
    v["w"]=substr($4,3); v["h"]=substr($5,3);
    v["capy"]=k[3]; v["src"]=substr($7,5);
    print v[want] }'
}
selof() { echo "$1" | grep -oE '"sel":\[[0-9]+,[0-9]+\]' | sed 's/"sel"://'; }

echo "rig-check: inline images — under never on, and the fit law (seed:image)"
# The gallery fixture: a captioned landscape and a tall uncaptioned portrait
# (300x1800). The caption's line-center must clear the picture's bottom edge
# on BOTH (the typover died this round), and the portrait must cap at roughly
# two-thirds of the viewport with proportional fit — never the natural 1800.
DI0=$(mktemp --suffix=.md)
GEO=$(WRUN_TAIL=40 scripts/wrun.sh "$DI0" "seed:image img-geo" 2>/dev/null | grep '^img ')
rm -f "$DI0" "$DI0.strop"
expect "both seeded pictures render"        2 "$(echo "$GEO" | grep -c '^img ')"
GW=$(echo "$GEO" | sed -n 1p); GT=$(echo "$GEO" | sed -n 2p)
for L in "$GW" "$GT"; do
  IX=$(imgv "$L" ix)
  if awk -v y="$(imgv "$L" y)" -v h="$(imgv "$L" h)" -v cy="$(imgv "$L" capy)" \
       'BEGIN{exit !(cy > y + h/2)}'; then
    echo "  ok   picture $IX: the caption line sits below the pixels"
  else
    echo "  FAIL picture $IX: caption at $(imgv "$L" capy) is ON the pixels (typover)"; fail=1
  fi
done
TH=$(imgv "$GT" h); TW=$(imgv "$GT" w)
if [ "${TH:-0}" -ge 400 ] 2>/dev/null && [ "${TH:-0}" -le 800 ] 2>/dev/null; then
  echo "  ok   the tall portrait caps at ~2/3 viewport (h=$TH)"
else
  echo "  FAIL tall portrait h=$TH — the fit law is broken"; fail=1
fi
if awk -v w="$TW" -v h="$TH" 'BEGIN{d = w*6 - h; exit !(d > -12 && d < 12)}'; then
  echo "  ok   …with proportional fit (w=$TW, 6w≈h)"
else
  echo "  FAIL tall portrait w=$TW h=$TH — the cap squashed the aspect"; fail=1
fi

echo "rig-check: the field repro — the Delete door (stage, refuse, exile, put back)"
# seed:imgrepro is the origin bug's exact shape: an EMPTY paragraph, a
# captioned picture, a prose paragraph below; caret parked in the prose.
# Arrow up climbs into the caption (a text line like any other); Delete at
# the empty block above STAGES (never fuses — the doc hash is the witness);
# the same key's next press (no key-up seen) is autorepeat and is refused;
# a key-up then a fresh press completes the exile; Put back restores the
# picture AND its caption whole, the same asset — never a twin.
DI1=$(mktemp --suffix=.md)
OUT=$(WRUN_TAIL=80 scripts/wrun.sh "$DI1" "seed:imgrepro img-geo dump:ui up up dump:ui ctrl-home delete dump:ui delete dump:ui keyup:delete delete dump:ui img-geo putback:last dump:ui img-geo" 2>/dev/null)
rm -f "$DI1" "$DI1.strop"
DU=$(echo "$OUT" | grep 'UI-DUMP')
D1=$(echo "$DU" | sed -n 1p); D3=$(echo "$DU" | sed -n 3p); D4=$(echo "$DU" | sed -n 4p)
D5=$(echo "$DU" | sed -n 5p); D6=$(echo "$DU" | sed -n 6p)
[ -n "$D6" ] || { echo "  FAIL missing dumps (field repro)"; exit 1; }
UP2=$(echo "$OUT" | grep '^SMOKE up' | sed -n 2p)
DL1=$(echo "$OUT" | grep '^SMOKE delete' | sed -n 1p)
DL2=$(echo "$OUT" | grep '^SMOKE delete' | sed -n 2p)
IMGS=$(echo "$OUT" | grep '^img ')
expect "arrow up lands in the caption"      1 "$(echo "$UP2" | grep -c 'kind=Image')"
expect "Delete at the empty block stages"   1 "$(echo "$DL1" | grep -c 'imgsel=1 door=0 hold=delete')"
expect "the stage parks the caret at the door" '[0,0]' "$(selof "$D3")"
expect "staging never fuses (doc untouched)" "$(field "$D1" doc_hash)" "$(field "$D3" doc_hash)"
expect "the held key's repeat is refused"   1 "$(echo "$DL2" | grep -c 'imgsel=1')"
expect "a refused press mutates nothing"    "$(field "$D1" doc_hash)" "$(field "$D4" doc_hash)"
expect "a fresh Delete after key-up exiles" 1 "$(field "$D5" grave_entries)"
expect "picture AND caption leave together" 132 "$(field "$D5" doc_chars)"
expect "Put back restores both, whole"      "$(field "$D1" doc_hash)" "$(field "$D6" doc_hash)"
expect "…and empties the graveyard"         0 "$(field "$D6" grave_entries)"
# Two img-geo calls flank the exile (1 picture each); the one between saw
# none — so 2 lines total, and one asset id across them (content addressing).
expect "the picture returns, never a twin"  2 "$(echo "$IMGS" | grep -c '^img ')"
expect "…under the same asset id"           "$(imgv "$(echo "$IMGS" | sed -n 1p)" src)" \
                                            "$(imgv "$(echo "$IMGS" | sed -n 2p)" src)"

echo "rig-check: the field repro — a HELD Backspace stops at the stage"
# 31 rights park the caret 5 chars into the prose; 8 backspaces with NO
# key-up between them are one held run: five walk the prose, the sixth
# stages at the wall, the last two are autorepeat and refuse — the picture
# survives. Esc then returns the door caret and drops the state; a fresh
# press re-stages, and press–release–press completes the exile (§5's
# freshness ladder, R5 — never trusting is_held).
DI2=$(mktemp --suffix=.md)
KEYS="seed:imgrepro ctrl-home"
for _ in $(seq 1 31); do KEYS="$KEYS right"; done
for _ in $(seq 1 8); do KEYS="$KEYS backspace"; done
KEYS="$KEYS dump:ui img-geo escape dump:ui backspace dump:ui keyup:backspace backspace dump:ui img-geo"
OUT=$(WRUN_TAIL=140 scripts/wrun.sh "$DI2" "$KEYS" 2>/dev/null)
rm -f "$DI2" "$DI2.strop"
DU=$(echo "$OUT" | grep 'UI-DUMP')
D1=$(echo "$DU" | sed -n 1p); D2=$(echo "$DU" | sed -n 2p); D4=$(echo "$DU" | sed -n 4p)
[ -n "$D4" ] || { echo "  FAIL missing dumps (held backspace)"; exit 1; }
BS6=$(echo "$OUT" | grep '^SMOKE backspace' | sed -n 6p)
BS9=$(echo "$OUT" | grep '^SMOKE backspace' | sed -n 9p)
ESC=$(echo "$OUT" | grep '^SMOKE escape' | sed -n 1p)
expect "five presses walk the prose, three refuse" 152 "$(field "$D1" doc_chars)"
expect "the sixth press stages at the wall" 1 "$(echo "$BS6" | grep -c 'imgsel=1 door=26 hold=backspace')"
expect "no refused press reaches the grave" 0 "$(field "$D1" grave_entries)"
expect "the picture survives the held run"  1 "$(echo "$OUT" | grep -c '^img 1 ')"
expect "Esc returns the door caret"         '[26,26]' "$(selof "$D2")"
expect "…and drops the selection"           0 "$(echo "$ESC" | grep -c 'imgsel')"
expect "a fresh press re-stages"            1 "$(echo "$BS9" | grep -c 'imgsel=1 door=26 hold=backspace')"
expect "press, release, press exiles"       1 "$(field "$D4" grave_entries)"
expect "the remnant is exact (caption gone with it)" 127 "$(field "$D4" doc_chars)"

echo "rig-check: the field repro — Enter never duplicates the picture"
# §6's one law, all three stances: at caption END the split opens below (the
# tail is born a Paragraph — R6, the model's split law); at the START of a
# non-empty caption the room opens ABOVE (the picture's block index rises);
# on the SELECTED picture Enter opens below and the selection decays. One
# picture stands at the end of all three.
DI3=$(mktemp --suffix=.md)
OUT=$(WRUN_TAIL=80 scripts/wrun.sh "$DI3" "seed:imgrepro up up enter dump:ui ctrl-home right enter dump:ui click:798,470 enter dump:ui img-geo" 2>/dev/null)
rm -f "$DI3" "$DI3.strop"
DU=$(echo "$OUT" | grep 'UI-DUMP')
D1=$(echo "$DU" | sed -n 1p); D2=$(echo "$DU" | sed -n 2p); D3=$(echo "$DU" | sed -n 3p)
[ -n "$D3" ] || { echo "  FAIL missing dumps (enter rules)"; exit 1; }
EN1=$(echo "$OUT" | grep '^SMOKE enter' | sed -n 1p)
EN2=$(echo "$OUT" | grep '^SMOKE enter' | sed -n 2p)
EN3=$(echo "$OUT" | grep '^SMOKE enter' | sed -n 3p)
CLK=$(echo "$OUT" | grep '^SMOKE click' | sed -n 1p)
expect "at caption end the split opens below"  1 "$(echo "$EN1" | grep -c 'kind=Paragraph')"
expect "at caption start the room opens above" 1 "$(echo "$EN2" | grep -c 'kind=Image')"
expect "a click on the pixels selects"         1 "$(echo "$CLK" | grep -c 'imgsel=2 door=- hold=-')"
expect "on the selection Enter opens below"    1 "$(echo "$EN3" | grep -c 'kind=Paragraph')"
expect "three Enters, one picture"             1 "$(echo "$OUT" | grep -c '^img ')"
expect "…each made room, none destroyed"       160 "$(field "$D3" doc_chars)"

echo "rig-check: travel — copy then paste resolves to the SAME asset id (§9)"
# Click-born selection, ctrl-c (the two-entry form rides the clipboard shim),
# Esc parks at the caption start (the click-born origin), paste at the end:
# the copy travels as `![alt](src "caption")`, so the pasted picture carries
# the caption AND resolves by asset hash — the ladder's first rung.
DI4=$(mktemp --suffix=.md)
OUT=$(WRUN_TAIL=80 scripts/wrun.sh "$DI4" "seed:imgrepro img-geo click:798,318 ctrl-c escape dump:ui ctrl-end ctrl-v wait:600 img-geo dump:ui" 2>/dev/null)
rm -f "$DI4" "$DI4.strop"
DU=$(echo "$OUT" | grep 'UI-DUMP')
D1=$(echo "$DU" | sed -n 1p); D2=$(echo "$DU" | sed -n 2p)
[ -n "$D2" ] || { echo "  FAIL missing dumps (copy/paste)"; exit 1; }
CLK=$(echo "$OUT" | grep '^SMOKE click' | sed -n 1p)
expect "the click selects, click-born"      1 "$(echo "$CLK" | grep -c 'imgsel=1 door=- hold=-')"
expect "Esc parks at the caption start"     '[1,1]' "$(selof "$D1")"
expect "the paste lands a second picture"   3 "$(echo "$OUT" | grep -c '^img ')"
expect "one asset id across the pair"       1 "$(echo "$OUT" | grep '^img ' | grep -oE 'src=[^ ]+' | sort -u | wc -l)"
expect "the caption travelled with it"      183 "$(field "$D2" doc_chars)"

echo "rig-check: travel — cut files the grave, paste puts the picture back"
# §9 cut = copy + leave-whole: the block exits by the §5 whole-block door
# (the graveyard records it), and the clipboard holds the travelling form —
# paste re-inserts the SAME asset. The grave record stays: cut filed a real
# departure, and paste is a copy, not an un-filing (Put back is the other door).
DI5=$(mktemp --suffix=.md)
OUT=$(WRUN_TAIL=80 scripts/wrun.sh "$DI5" "seed:imgrepro img-geo click:798,318 ctrl-x dump:ui ctrl-v wait:600 dump:ui img-geo" 2>/dev/null)
rm -f "$DI5" "$DI5.strop"
DU=$(echo "$OUT" | grep 'UI-DUMP')
D1=$(echo "$DU" | sed -n 1p); D2=$(echo "$DU" | sed -n 2p)
[ -n "$D2" ] || { echo "  FAIL missing dumps (cut/paste-back)"; exit 1; }
expect "cut exiles picture + caption whole" 132 "$(field "$D1" doc_chars)"
expect "…and files one grave entry"         1 "$(field "$D1" grave_entries)"
expect "paste-back lands the same asset"    1 "$(echo "$OUT" | grep '^img ' | grep -oE 'src=[^ ]+' | sort -u | wc -l)"
expect "two sightings flank the cut"        2 "$(echo "$OUT" | grep -c '^img ')"
expect "the grave record honestly stays"    1 "$(field "$D2" grave_entries)"

echo "rig-check: travel — replace-in-place is one undo step (§4)"
# A foreign bitmap on the clipboard (clipimg — no Strop line, §9's fallback),
# pasted onto the SELECTED picture: the src swaps in place, the caption and
# the block stand untouched — and ONE ctrl-z returns the old picture.
DI6=$(mktemp --suffix=.md)
OUT=$(WRUN_TAIL=80 scripts/wrun.sh "$DI6" "seed:imgrepro img-geo clipimg:$IMG_B click:798,318 ctrl-v wait:600 img-geo dump:ui ctrl-z wait:300 img-geo dump:ui" 2>/dev/null)
rm -f "$DI6" "$DI6.strop"
DU=$(echo "$OUT" | grep 'UI-DUMP')
D1=$(echo "$DU" | sed -n 1p); D2=$(echo "$DU" | sed -n 2p)
[ -n "$D2" ] || { echo "  FAIL missing dumps (replace-in-place)"; exit 1; }
L1=$(echo "$OUT" | grep '^img ' | sed -n 1p)
L2=$(echo "$OUT" | grep '^img ' | sed -n 2p)
L3=$(echo "$OUT" | grep '^img ' | sed -n 3p)
expect "one picture at every station"       3 "$(echo "$OUT" | grep -c '^img ')"
if [ -n "$L2" ] && [ "$(imgv "$L2" src)" != "$(imgv "$L1" src)" ]; then
  echo "  ok   the paste swapped the src in place"
else
  echo "  FAIL replace-in-place did not swap the src"; fail=1
fi
expect "the caption survives the swap"      157 "$(field "$D1" doc_chars)"
expect "ONE undo returns the old picture"   "$(imgv "$L1" src)" "$(imgv "$L3" src)"
expect "…and only the src had moved"        157 "$(field "$D2" doc_chars)"

echo "rig-check: travel — a drop lands at the pointer's gap, the caret stays (§7)"
# The synthesized compositor stream (Entered → Pending → Submit) drops a file
# at the gap ABOVE the picture while the caret sits at the prose end. The new
# picture is born at that gap (block 1, its own 64px width names it); the old
# one stands below; the caret never moves — and the newborn's EMPTY caption
# answers a click in the band under its pixels with a caret, no selection.
DI7=$(mktemp --suffix=.md)
OUT=$(WRUN_TAIL=80 scripts/wrun.sh "$DI7" "seed:imgrepro dragenter:798,100,$IMG_A dragmove:798,110 dragdrop:798,110 wait:600 img-geo dump:ui click:798,213 dump:ui" 2>/dev/null)
rm -f "$DI7" "$DI7.strop"
DU=$(echo "$OUT" | grep 'UI-DUMP')
D1=$(echo "$DU" | sed -n 1p)
[ -n "$D1" ] || { echo "  FAIL missing dumps (drop)"; exit 1; }
DRP=$(echo "$OUT" | grep '^SMOKE dragdrop' | sed -n 1p)
CLK=$(echo "$OUT" | grep '^SMOKE click' | sed -n 1p)
LN=$(echo "$OUT" | grep '^img ' | sed -n 1p)
LO=$(echo "$OUT" | grep '^img ' | sed -n 2p)
expect "the drop adds one picture"          2 "$(echo "$OUT" | grep -c '^img ')"
expect "the newborn stands at the gap"      1 "$(imgv "$LN" ix)"
expect "…at its own natural size"           64 "$(imgv "$LN" w)"
expect "the old picture stands below it"    604 "$(imgv "$LO" w)"
expect "the caret never moved for the drop" 1 "$(echo "$DRP" | grep -c 'tail="es the wall." kind=Paragraph')"
expect "the empty slot's band answers with a caret" 1 "$(echo "$CLK" | grep -c 'off=1 sel=1\.\.1')"
expect "…a caret in the caption, no selection" 0 "$(echo "$CLK" | grep -c 'imgsel')"

echo "rig-check: inline images — stills for the eyes"
# What only eyes can judge: the block-wide selection wash + the alt strip
# (image-wash), caption optics / the chrome-free empty slot / the capped
# portrait in one page (image-page), and the drop-gap rule painted mid-drag
# (image-drop-gap). P6's refused-press still-identity is covered above by
# the model (hash + caret unchanged); these frames are for the record.
SHOTS="${RIG_SHOTS:-target/rig-shots}"; mkdir -p "$SHOTS"
DIS=$(mktemp --suffix=.md)
scripts/wshot.sh "$SHOTS/image-wash.png" 1 "$DIS" "seed:imgrepro click:798,318" >/dev/null 2>&1
rm -f "$DIS" "$DIS.strop"; DIS=$(mktemp --suffix=.md)
scripts/wshot.sh "$SHOTS/image-page.png" 1 "$DIS" "seed:image" >/dev/null 2>&1
rm -f "$DIS" "$DIS.strop"; DIS=$(mktemp --suffix=.md)
scripts/wshot.sh "$SHOTS/image-drop-gap.png" 1 "$DIS" "seed:imgrepro dragenter:798,100,$IMG_A dragmove:798,110" >/dev/null 2>&1
rm -f "$DIS" "$DIS.strop"
for s in image-wash image-page image-drop-gap; do
  if [ -s "$SHOTS/$s.png" ]; then echo "  ok   still $SHOTS/$s.png"; else
    echo "  FAIL still $s.png did not render"; fail=1; fi
done
rm -f "$IMG_A" "$IMG_B"

[ "$fail" = 0 ] && echo "rig-check: PASS" || echo "rig-check: FAIL"
exit "$fail"
