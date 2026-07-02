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
trap 'rm -f "$DOC" "$DOC.strop" "$DOC2" "$DOC2.strop" "$DOC3" "$DOC3.strop" "$DOC4" "$DOC4.strop"' EXIT

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

[ "$fail" = 0 ] && echo "rig-check: PASS" || echo "rig-check: FAIL"
exit "$fail"
