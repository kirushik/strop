# strop — tester feedback guide: attention & motion

*A ~2-week trial instrument for the attention/motion release. Goal: validate (or
overturn) the design bets we made from HCI research (`docs/attention-motion.md`)
with how the work actually FEELS to real writers. Use it as a self-administered
questionnaire OR a 30-min interview guide — the open prompts matter more than the
ratings.*

---

## 0. For the facilitator (internal — do NOT show testers)

We grounded the design in research rather than guessing, but several values are
**best-bets, not validated**. This trial is how we settle them. Don't tell
testers the numbers — it biases them. Ask about *experience*; we map experience
back to the knob.

The bets this release puts in front of testers, and what we're really asking.
**Build status as of this doc:** the **card budget** (older cards recede to
one-liners, never hidden) is shipped + tested; the
**reveal-timing / pip / eased-motion / reduced-motion** pieces are the next build
increment (designed + grounded in `attention-motion.md`, landing before the
release goes wide). Only run the sections below whose behaviour is actually in
the build the tester has — mark each row's status before handing this out.

| Knob | Behaviour under test | The question this trial answers |
|---|---|---|
| **Reveal timing** | AI cards from a finished pass *announce* immediately (a quiet cue) but *appear* only when you turn to review (scroll/hover the margin, open the door, or a long pause) — never mid-typing | Does it ever feel like the AI interrupts mid-thought? Does deferring ever feel like the feedback is "lost" or late? |
| **Pause threshold** | ~1.5 s of typing-stillness counts as a pause | Too eager (cards pop while still thinking) vs too sluggish (waiting after you've clearly stopped)? |
| **Completion pip** | a brief, soft glow near the cursor when a pass finishes | Noticed at all? Reassuring vs naggy? Missed it entirely? |
| **Card budget** | at most ~5 AI cards show *full-size* per screen; older ones shrink to a one-line card at their sentence (click to expand) — nothing is ever hidden | Does the margin ever feel cluttered/"wallpaper"? Do the small one-line cards read as "still there," or do they feel demoted/lost? |
| **Motion** | cards fade/slide in; nothing pops, loops, or bounces | Calm vs distracting? Anything feel laggy/janky? |
| **Reduced motion** | a setting that swaps movement for cross-fades | (For motion-sensitive testers) does the calm mode still feel coherent — nothing teleports? |

Recruit a spread: at least one heavy drafter, one heavy reviser, one
motion-sensitive person, and one who runs strop on a low-power machine (jank
shows there). Capture verbatim quotes for the open prompts.

Rating scale where used: **1 = strongly disagree … 5 = strongly agree**, plus a
"didn't notice / N/A". Always pair a rating with "why?".

---

## 1. For the tester — the ask

You've been using strop for a couple of weeks. Most of this release is invisible
on purpose: it's about *when* the editor's AI feedback reaches you and *how*
things move in the margin, so the tool stays calm while you write and useful when
you turn to review. Please answer from memory of real sessions — not by hunting
for the behaviors. If you never noticed something, that's a finding, say so.

One framing question before the rest: **over these two weeks, did strop mostly
feel like it stayed out of your way while drafting, and showed up when you wanted
it?** (1–5, then a sentence on the strongest moment either way.)

---

## 2. Flow while drafting — does the AI interrupt?

The thesis is that the editor's critical voice should never break your drafting
flow; you summon it.

- When you were writing and an AI pass finished, did anything pull your eye to the
  margin *before you were ready*? (Y/N — describe the moment.)
- Did AI cards ever **appear/move while you were mid-sentence or mid-thought**? How
  did that feel? (rate intrusiveness 1–5)
- When you *did* turn to the margin (scrolled, moused over, opened review), were
  the cards **already there waiting**, or did you watch them arrive? Which felt
  better?
- Be honest about the opposite failure: did feedback ever feel **late or hidden** —
  you'd stopped, wanted it, and it wasn't there yet? Roughly how long did that gap
  feel? (This is the pause-threshold question.)
- Open: describe the *best* and *worst* moment an AI card reached you this trial.

## 3. Knowing a pass finished — the completion cue

When you ask for a pass, it runs in the background and then signals it's done.

- After requesting a pass, **how did you know it had finished?** (the glow near the
  cursor / a count in the margin / the title bar / you just looked / didn't notice)
- The brief glow near your cursor on completion: **noticed it? once you knew it was
  there, was it reassuring or annoying?** (rate 1–5 each; "never saw it" is valid)
- Did the completion signal ever feel like it **demanded** attention rather than
  *offered* it?
- Open: if we removed the glow entirely and only updated a quiet count, would you
  miss it?

## 4. The margin when there's a lot — clutter & "nothing vanishes"

A long session can produce many notes and AI cards.

- Did the margin ever feel **cluttered, busy, or like wallpaper you stopped
  reading**? (rate 1–5; when?)
- When more than a handful of AI cards shared a screen, the older ones shrink to
  a **one-line card** next to their sentence (a click opens them full). Did you
  notice? Did a shrunken card ever feel **lost or demoted**, or did it read as
  "still there, just quieter"? Was expanding one back obvious?
- Your *own* margin notes never shrink or hide — did that distinction (your
  notes always full, AI cards budgeted) feel right, or did you not notice?
- Open: what's the right number of *full-size* AI cards to see at once before
  it's too many — did ~a handful feel right, too few, too many?

## 5. Motion — calm or distracting?

Cards fade and slide rather than appear instantly; the goal is motion you feel
without being pulled by it.

- Did margin movement ever **catch your eye when you didn't want it to**?
- Did anything feel **laggy, janky, or stuttery** (especially on a slower machine)?
- Did the motion ever help you **follow a card** as it moved/resettled, or was it
  just decoration?
- Pace check: did transitions feel **too fast (abrupt) / about right / too slow
  (sluggish)**?
- Open: one word for how the margin's motion *feels*. (We're aiming for "calm /
  paper-like," not "app-y / animated.")

## 6. Reduced motion / accessibility (motion-sensitive testers especially)

There's a setting to replace movement with gentle cross-fades.

- Did you find/enable it? Was it discoverable?
- With it on, did anything **teleport or jump** in a way that lost the thread (a
  card vanishing with no trace, a count snapping)? It should cross-fade, not snap.
- Did the reduced-motion mode still feel like a *finished* experience, or a
  degraded one?
- (If motion-sensitive) did the default motion cause any discomfort over a long
  session?

## 7. Overall

- Net: did this release make strop feel **calmer / the same / busier** than before?
- Did it change *when or whether* you run AI passes at all?
- If we could only fix ONE attention/motion thing before launch, what would you
  point at?
- Anything that surprised you, delighted you, or that you actively disliked.

---

## Notes for the team after collecting

- Map "interrupted mid-thought" reports → tighten reveal to coarser breakpoints /
  raise the pause threshold. Map "felt late/hidden" reports → lower it. (The
  research says the affective cost of unsolicited interrupts likely *overstates*
  ours, so don't over-defer on weak signal.)
- "Never noticed the pip" + "didn't miss it" from most testers → cut it (it's our
  least-supported bet). "Reassuring" → keep, tune amplitude per intrusiveness
  reports.
- "Felt lost" on a shrunken card → the one-line card isn't reading as "still
  here"; strengthen its affordance (hover, label) before touching the budget
  number.
- Any jank report on a low-power machine → revisit durations / frame budget before
  shipping wider.
