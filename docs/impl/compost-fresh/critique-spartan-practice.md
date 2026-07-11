# Critique — design-spartan ("the piece ends here")

Panel: the Novelist (practice) and the Spartan (rent audit). Verdict at the end.

## The Novelist

This is my desk. I already keep the pile at the bottom of the file; this design keeps my gesture, my container, my ⌘F, and teaches the tool to stop counting my compost as chapter. The end-of-revision sweep — the one ritual that keeps a cuts file alive (the lawyer who "always reads back over the scrap") — is served by geography alone: the pile is the last thing before the dead. Month three looks fine, because there is no surface to abandon; the decay mode is pile-bloat, and Pressfield says the file being longer than the book is normal, not failure. Seeds cut per the digest's own gradient — I accept that, said aloud.

But three moments ring false against the desk.

**The park doesn't show me the catch.** F1's product is permission, and permission is *knowing it's kept*. What I see at the moment of the pang: the paragraph vanishes, the gap closes, a footer number goes DOWN. A decrement is the emotional sign of loss, not of safekeeping; it proves departure, never arrival. Highland's Bin is the loved park precisely because you watch the text land; the Spike is the dead one because feedback was absent. The design names this risk (loss 2) and then ships it anyway. The first three times, I will scroll down to check — and a verb you have to audit hasn't bought permission, it's sold anxiety with a receipt.

**The jot transits my prose.** Type the stray thought into the manuscript, then select it, then chord. That's three gestures where org-capture and Lamott's pocket card cost one; the selection step mid-burst (shift-home, or worse, mouse) is the tax. And the thought physically enters the piece: my sentence is broken by a grocery line for two seconds — and if a lull checkpoint fires inside those two seconds, the piece's history immortalizes the grocery line *as manuscript*. The design cites Altmann & Trafton for the unchanged screen; the screen isn't unchanged — my sentence had a foreign body in it and I watched it.

**Skim has no glance.** Owned as loss 3, and honestly. I'd add: it's the one moment a drawer rival wins outright, and "retrieval is 1–2%" is the correct reason not to care much. Accepted, not forgiven.

## The Spartan

My default vote is shelve, and this design was written to disarm me: it ships the null baseline plus the one thing the baseline cannot do for itself — a boundary the machine respects. Scope exclusion is the entire value (tools digest, lesson 1), the mark-as-content makes checkpoint geometry free while every containered rival buys per-state storage, and the empty state is genuinely nothing. F5 is a decisive, honest win. I concede the bet.

I do not concede the mechanism. **The soul of this design is a magic string, and the string is prose.** Fountain earned the in-document boundary by choosing delimiters prose cannot collide with (`/* */`); this design chose a natural-language sentence for register-poetry and bought collision semantics with it:

- A one-line paragraph of dialogue — `"The piece ends here."` — matches the recognition rule exactly (letters, ignoring case and surrounding punctuation). Typed mid-chapter, it silently *becomes* the boundary; being the FIRST match, it hijacks the boundary even when a real mark sits lower, and everything between becomes scraps. Export truncates; the AI goes blind below; the count lies. The baseline's failure 1, reintroduced by a legal line of fiction.
- The inverse is as cheap: delete or edit the mark line (or a replace-all that touches "piece") and 3,000 words of scraps silently rejoin the manuscript — announced only by a footer number jumping and a ground repaint that may be entirely off-screen. Next review pass, the caged machine diagnoses the scrap pile.
- Assumption 6 makes boundary validity depend on the app build's locale phrase table. A mark a build fails to recognize is not a degraded feature; it is a silently re-scoped manuscript.

Also count the conventions honestly. "No new UI" ships: a recognition grammar, a stamped sentence the writer didn't compose (the design's own loss 5 — P1's bright line, argued around, not through), a ground cutoff, a chip suffix, a chip jump, an Esc clause. Six conventions. Convention beats chrome, so I still prefer this to every drawer — but invisible syntax has Fountain's documented tax: it must be learned, and its near-misses fail without error ("peice", conceded in loss 1). A boundary this load-bearing must be an *element* the tool owns, wearing the sentence as its face — recognition off the element, not the letters. That is not chrome; it is the difference between a convention and a superstition.

## Kill-shots (testable)

1. **Phrase collision hijacks the boundary.** Type the one-line dialogue `"The piece ends here."` mid-chapter of an 80-line document with a real mark below: recognition rule makes it the first match → export truncates at line 40, AI scope ends there, count drops. No error possible, because nothing is wrong by the spec's own rules.
2. **One keystroke dissolves the boundary, quietly.** Delete the mark line while the viewport is mid-document: sole witnesses are a footer count jump and an off-screen ground change; run a review pass and the AI diagnoses the scrap pile. Baseline failure 1 reborn.
3. **Park compliance is a decrement, not an arrival** (Spike's cause of death, named in loss 2, unfixed). Corridor test: first-time parker strikes ⇧⌃A — does she scroll down to verify? If yes, F1's "compliance visible from where the writer sits" is failed as specced.
4. **The jot transits prose and history.** Jot mid-sentence via type-select-chord; let a lull checkpoint fire between type and park; open that state from the strip: the piece contains the grocery note. Also: three gestures where the digests' gold standard (org-capture) costs one.
5. **Locale-dependent structure.** Open the document in a build whose phrase table misses the mark's language: the yard silently rejoins the piece — scope, counts, and AI all wrong with zero signal.

## Grafts (named)

1. **The element wears the sentence** — the mark is a tool-known content line (inserted by the verb, deletable, movable, versioned as content — everything §7 keeps), rendered as *· · · the piece ends here · · ·*; recognition keys off the element, never off letters. Kills shots 1 and 5 outright; keeps time-travel-for-free.
2. **Arrival, not decrement** — the parked block visibly departs toward the foot using the shipped re-pack slide grammar (cross-fade under reduce-motion); the chip's tick is an event, not a new number.
3. **Selectionless jot** — ⇧⌃A with no selection parks the current line/paragraph: type the thought as its own line, one chord, no selection gymnastics, caret restored.
4. **Atomic jot in history** — jot-park is one history operation; the transit text can never land in a checkpoint state.
5. **Dissolution announced at the writer** — when the boundary dies, the chip swells with the count jump at the moment of change (P12), wherever the viewport is.

## Verdict

**Needs surgery; beats the baseline.** The place, the bet, and the scorecard survive both of us — this is the only shape the Spartan has ever voted to keep. The recognition mechanism as specced fails two testable cases that re-create the exact disease the design exists to cure, and the park's feedback repeats the Spike's documented death. All three are repairable without moving a single piece of furniture.
