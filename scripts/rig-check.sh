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
MW=$(field "$D1" manuscript_words)
if [ "${MW:-0}" -gt 0 ] 2>/dev/null; then echo "  ok   the count is manuscript-only ($MW words)"; else
  echo "  FAIL manuscript_words=$MW"; fail=1; fi

[ "$fail" = 0 ] && echo "rig-check: PASS" || echo "rig-check: FAIL"
exit "$fail"
