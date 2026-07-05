# Critique — `design-fold.md` (practice panel)

Two voices: the Novelist (a working writer with a real cuts file) and the Spartan (default vote: shelve). Verdict at the end.

---

## The Novelist

**The jot is right. I want to say that first because everything else I say is a knife.** Chord, type, Esc, caret back to the letter, screen never moved — that is org-capture's round trip without leaving the page, and it is exactly what a ferry-timetable thought costs me on paper: nothing. The aside tense is genuinely won here. TK-style notes *want* to sit at the text they concern, and the fold gives them that with scope-truth the inline TK never had.

**But the design has quietly given every tense the aside's home, and the research it cites says that is backwards.** research-practice §3 draws one fault line: *asides inline, cuts to a pile ordered by cut-time, seeds upstream.* Not one of the fifteen-plus writers inventoried keeps cuts *in place at the cut site*. The single precedent for inline cuts — Fountain's boneyard — carries exactly one documented complaint: parked text "stays visually in your way." The design quotes that complaint, claims folding fixes it, and then generalizes the mechanic. A pleat is quieter than a `/* */` block, yes. It is not gone. **Kill-shot 1, the tense inversion:** the design's F8 invitation asks writers to adopt a cut-keeping geometry that zero writers in the practice inventory perform, on the strength of the one format whose lone complaint is this geometry.

**Then run the Pressfield ratio.** His culls file is *longer than the book* — 263 pages against 194. At that documented ratio a 5,000-word chapter mid-revision carries dozens of pleats. The design names this in §6, honestly, and offers its cure: Exile the dead ones to the graveyard. But research-practice §5 is unambiguous that curation-maintenance is precisely the behavior that kills these systems — the collector's fallacy, the morgue dying with its librarians, "elaborate funnels with no drain." **Kill-shot 2, testable:** seed a draft at Pressfield ratio (~40 folds / 5,000 words), time a revision skim against the baseline's clean-body-plus-bottom-pile. The fold's only clean view is cold read — which is read-only. The baseline always has an editable clean body; the fold *never* does. And here is the emotional miss: parking works because it "feels like moving, not killing" (research-tools §0). I park to get the corpse *out of my face* while keeping it warm. A pleat is a scar I re-read on every pass through the scene — permission-to-cut delivered as a permanent reminder of the wound, at the wound.

**Month three:** I stop folding cuts around week five, when my chapter looks striated, and go back to the bottom pile (which still works, and which I can fold once for scope-truth — the design's own concession). Folds survive as my aside mechanism. That is a good feature. It is not the feature the brief asked for; parking is "the whole game" (research-practice §6.1), and parking is where this loses me.

**One more, mechanical:** the end-of-revision sweep — the one ritual research-practice §5c says keeps scrap files alive — lives behind typing `folds` into the omnibar. No resting affordance anywhere. NN/g (research-theory §4): hiding halves use, and that was for users who knew the feature existed. **Kill-shot 3:** corridor-test the lens; no stranger finds it, so the sweep never happens, so dead folds accrete, so kill-shot 2 compounds.

## The Spartan

My default is shelve, and I concede the fold clears my first bar: resting footprint zero, empty state indistinguishable from today, no drawer, no panel. iA Writer would not refuse the *silhouette*. It would refuse the mechanics.

**A closed fold is a widget wearing text's clothes.** Arrow keys step over it; selection treats it as an atom; deleting a span containing it routes its text to the graveyard; clipboard flattens it outside the app. That is four exceptions to "typing types" installed at the exact center of the writing surface (P3, P7). The design admits this in §6 as a risk "if it ever surprises." It will surprise, and here is the frame: **Kill-shot 4, the retype gesture.** Select three paragraphs to rewrite a scene — an ordinary, daily gesture — and one of them contains a closed fold. Per spec, the fold's text routes to the graveyard: living warm text demoted to the dead record as a *side effect* of typing, no verb spoken, temperature change announced nowhere at the point of action. P13's letter is technically kept (it's recorded), but its spirit — nothing destroys silently — is violated where it matters most. Rig-testable today: rewrite-over-fold → assert the fold survives or something visibly announces.

**Kill-shot 5, the find spring-load:** a match inside a closed fold "springs it open while current, re-pleating as you move past." A 200-word fold opening mid-cycle shifts every line below it, then shifts back on `n`. That is a viewport jump per fold-match — measurable, disorienting, and beneath the calm this product sells.

**Now the rent audit.** What does *in-place residence* — the bet itself — uniquely buy? Unfold-at-origin (the ~1–2% flow, by the design's own citation) and structural provenance (which decays: after two restructuring passes the fold sits between paragraphs that no longer resemble what it was cut from). Everything else the design wins — one-chord park, Esc-jot, scope exclusion from export/counts/AI/cold-read — is *separable from the bet* and available to a gathered design at the same price. The bet's unique yield is the rarest flow plus a decaying property; its unique cost is striation, atom mechanics, and the stump emotion. That is a bad trade dressed as a principled stand. I don't vote shelve — the scope-truth and jot wins over the null baseline are real and cheap — but I vote against the bet carrying the parking tense.

---

## Grafts (named)

1. **Split the tenses along the research fault line.** Folds keep asides and jots (inline-wanting, short). Parking a substantial selection takes the same chord but lands in a gathered cut-ordered tail region — or the crease gains a quiet second verb, "send to tail." Cuts get a pile; asides stay home.
2. **Folds survive surrounding deletion.** A fold is not manuscript; the delete verb targets manuscript. Deleting a span containing a closed fold leaves the fold in place — safer-than-ancestor, P7-lawful. Only Exile or editing inside the fold touches its text.
3. **Give the lens a resting edge.** A fold-count chip in the graveyard-footer grammar (F6 parity), present only when folds exist, opening the lens. Zero chrome at zero folds; the sweep ritual gets a visible home.
4. **The crease accepts an optional writer label as data.** Caret in the crease row types a reason ("too much backstory"), shown in the one-liner. Never demanded — schema deferred, Bernstein-compliant — but the hand-note the practice literature documents gets a channel.
5. **Find opens folds only on explicit entry.** Tally reads `3 in folds`; Enter on a fold-match opens it; cycling past never reflows the page.

## Verdict

**Needs surgery; beats the baseline.** The jot/aside half is the best-grounded capture design on the table and closes baseline failures 1–4 and 6 at near-zero resting cost. The parking half universalizes the wrong geometry, fails at documented cut volume, relies on curation the research says won't happen, and installs atom mechanics that break the product's central promise. Graft 1 is not optional.

*(~1,150 words)*
