# Critique — `design-tail` ("Scraps") · practice panel

Two voices. Read against the brief, `research-practice.md`, `research-tools.md`, and `docs/design-principles.md`.

## The Novelist

Concession first, because it's structural: the month-three test passes almost by construction. This isn't a new surface that must earn a habit; it's the pile I already keep, with the tool finally admitting it exists. Pressfield's CULLS outlives every app feature because the substrate is the manuscript file itself — this design keeps that substrate. It will still be in use in month three because *it was in use before the feature shipped*. And the emotional register is right: **Set aside** resting one row above **Exile** is exactly where the hand hovers when a cut hurts, and permission is consumed at cut-time, not retrieval-time (research-practice §2). The empty state being pixel-identical to nothing is the most writer-respecting sentence in any of these designs.

Now the misfires I will actually hit.

**I edit my scraps, and Esc will maul me.** Scraps are living text — I rewrite them, splice them, annotate them. The spec says "Esc from anywhere in the tail returns to the last manuscript caret." So the one region of my own editable text behaves modally: caret in a scrap, typing, reflexive Esc (a habit the product itself trained via find), and I'm teleported hundreds of lines away mid-edit. Nowhere else does Esc move a live caret out of editable text. Esc's shipped contract is *leave a transient context* — find, the parked past, cold read. Editing my own words is not a transient context. This is the brief's own "extend, never repurpose" broken by the design that quotes it.

**The jot quietly re-imposes the filing decision.** A stray thought mid-burst now has two chords: ⌃M if it's about *this passage*, ⌃⇧A if it's about *the piece*. Research-practice §5 is unambiguous: any capture-time classification kills the habit. The design brags "zero classification" while making me classify tense-of-thought before my fingers move. Worse, the misfire is silent: a stale selection I forgot exists + intended jot = a passage parked out of my prose with no motion but a footer number ticking — Spike-grade invisibility for exactly the case that needs visibility.

**The retrieval verb is aimed at the wrong case.** F4's requirement is re-entry *at the writer's insertion point*. The evidence (Pressfield ~1–2%, phrase-grained, memory-triggered) says restore-to-origin essentially never happens — the passage's old slot was rewritten; that's *why* it was cut. Yet the engineered inverse, with anchor-migration machinery, is Put-back-to-origin, while the actual retrieval I'll perform is a three-step manual dance: cut → Esc → paste. The scorecard's "F4 beats" stands on the leg nobody uses.

**Iterated skim loses my place.** The hunt is a comparison loop: read a scrap, jump home to the passage, jump back down. Esc remembers the manuscript end exactly; the chip always lands at the seam. Second round trip, I restart at the top of a 3,000-word pile. The design's honest confession (no adjacency, ever) I accept as the direction's price; restarting the skim I do not — that's an implementation choice, not a structural one.

Small dissonances: parked text *prepends* under the seam, but ten years of typing-under-a-divider is append muscle memory (the gradient argument is good; the surprise is still real). And the provenance chips: after sixty parks, my pile wears a column of machine receipts I cannot delete, in the one region that's supposed to be most mine. I write my own why-notes ("cut from ch. 2 — too much backstory"); the machine's when/where is welcome but should not columnize my tail.

## The Spartan

My default vote is shelve, and this is the only direction where I lose it cleanly: the empty state ships zero standing chrome, and a document with no scraps is the null baseline to the pixel. So I audit rent instead.

**Pays rent:** the seam (failures 1, 2, 6 — scope-exclusion is the *entire* documented value of every shipped success, research-tools lesson 1; Fountain proves writers themselves asked for a boundary the format knows). The verb (failure 3, and it's F8 — the invitation is one menu row in the selection path, which is where discovery demonstrably lives). The footer composer (failure 4, org-capture shape, transient). Three elements, three named failures. iA Writer would refuse all of this — but iA's own answer is *convention the tool knows* (hashtags, content blocks), which is precisely what the seam is. I concede the core.

**Dodges rent:** the provenance chip + Put-back-to-origin apparatus. It defends the rarest sub-case of a ~1–2% event with real machinery (anchor migration, orphan handling, sage returns) and a permanent widget per scrap. Grammar parity with the graveyard is worth one action row on an expanded chip — it is not worth a standing margin colony in the writer's living region. Cut the standing chips; keep put-back as the cheap parity verb.

**Actively costs:** the one-chip merge is a regression against a shipped contract. Today the graveyard chip lands me at the record; tomorrow "Scraps · 2,340" lands me at the seam and the record is a pile-length scroll away — and the pile is "always longer than the book." The design taxes an existing feature to subsidize a new one and relabels the dead record under the living pile's name.

**Unfinished:** undo. The spec's inverses are chip-borne, but the reflex inverse — Ctrl-Z the instant after ⌃⇧A — is never specified. Does the text return? Does a just-born seam evaporate? P13's whole point is that the writer infers the inverse; the inferred inverse here is undo, and the spec is silent. Until that paragraph exists, the "lossless and reversible" claim is folklore.

## Joint verdict

**Needs surgery — and the surgery is subtraction and spec-completion, not redesign.** The seam + verb + composer core genuinely beats the baseline where the brief said wins were possible (failures 1–4, 6, and F8); nothing else in the design may ride along on that.

**Kill-shots**

1. Ctrl-Z immediately after a park/jot is unspecified — the reflex-grade inverse P13 promises doesn't exist, and it's the only recovery from the silent stale-selection misfire. Test: park, undo; text must return verbatim and a first-scrap seam must evaporate.
2. Esc teleports a live caret out of editable scrap text — the only place in the product Esc acts on a non-transient surface. Test: type in a scrap, press Esc; any navigation is the bug.
3. The one-chip merge regresses graveyard access behind a pile "longer than the book" and mislabels the record. Test: 3,000-word pile, reach a graveyard entry; compare with the shipped chip.
4. F4 is engineered backwards: machinery serves restore-to-origin (~never happens), while insertion-point re-entry — the requirement — is a manual three-step. The scorecard line "F4 beats" is unearned as written.
5. Jot vs annotate re-imposes capture-time classification (⌃M "about here" vs ⌃⇧A "about the piece"). Test: corridor writers given a stray thought mid-draft; hesitation or wrong-chord picks kill the "zero filing" claim.

**Grafts**

- **Undo-first inverse:** park/jot join the edit history; immediate Ctrl-Z restores verbatim and evaporates a just-born seam.
- **Excursion-scoped Esc:** Esc-home only when the tail was entered via chip or find this excursion; a caret placed by scroll-and-click makes the tail plain text, Esc inert.
- **Descending chip:** repeated activation steps down the gradient — first press the seam, second the graveyard — one chip, both afterlives reachable, each named at its station.
- **Both-ends round trip:** the excursion remembers the tail position too; after Esc-home, the next chip press resumes where the skim stopped (session-scoped).
- **Quiet provenance:** the one-liner chip renders only while the caret rests in its block; otherwise the pile is clean text. Put back survives inside the expanded chip as graveyard-parity, minus the standing colony.
