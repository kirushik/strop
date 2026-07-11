# Compost extraction (code-only, commit e5fa3e1) — condensed findings

Ground truth: compost = rope text blocks 0..b (TOP of doc), separator block b, manuscript after. manuscript_base_char rebases counts/export/AI-scope/diagnosis anchors. Graveyard = side record (not rope) at tail, read-only, lossless put back.

Flows as implemented:
1. Park: ctrl-shift-a / menu / caret-para -> moves to bottom of compost pile (just above separator); caret stays; first birth opens rail; 3 flash surfaces (region, rail row, toggle).
2. Jot a NEW thought: NO PATH. No verb/button/affordance; boundary only born by moving manuscript text. Only route: type in manuscript, set aside, click back in, edit. UI teaches none of it.
3. Skim: two homes at once (in-flow top region + rail panel rows). Esc returns to last_manuscript_caret BUT that inits to 0, so on fresh open Esc returns INTO compost.
4. Retrieve: NO VERB. Right verb menu suppressed for compost selections (flank_gate !in_compost). Manual ctrl-x/ctrl-v only, plain-text lossy (spans/kinds lost). Graveyard meanwhile has lossless put back + show origin + delete.
5. Open a doc with compost: caret at byte 0 = INSIDE first compost item; scroll top; first screen = scraps; ctrl-home lands in compost.
6. Edit inside compost: typing/formatting works; boundary edges protected (backspace/forward-delete no-ops); set-aside refused there; ctrl-m note chord UNGATED (menu gated) -> live margin card can anchor to compost; heading chords ungated -> tiny grey headings; find/replace pierces the boundary (replace-all silently rewrites the scrap box) though counts/export/AI all exclude it; select-all is region-scoped.

Manifestations: 3 persistent (in-flow region, rail panel, titlebar dot) + 3 synchronized flashes. 

Papercuts ranked: (1) no from-scratch entry; (2) no pull-back verb + menu suppressed, automatic pile richer than deliberate pile; (3) two homes; (4) caret-trap on open + defeated Esc; (5) Find/Replace pierces the one boundary everything else respects; (6) chord-vs-menu gating inconsistency; (7) presence idiom asymmetry (dot vs counted footer).
