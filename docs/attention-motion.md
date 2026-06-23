# strop — Attention & Motion Design Brief

*Synthesis of five research lenses (response-time thresholds, interruption science, calm/peripheral/pre-attentive perception, flow & creative-tool feedback, motion accessibility & object constancy). Numbers below are meant to be implemented as-is; every value is sourced or flagged as a judgment call.*

> Provenance: produced 2026-06-23 by a multi-agent deep-research pass (5 parallel
> web-research lenses → one synthesis), commissioned to ground strop's deferred
> attention/motion decisions (margin-card-dynamics.md Phases 4–5) in SotA HCI
> rather than tuning the feel blind. The **timing/motion/attention** companion to
> `color-language.md`. Values flagged "judgment call"/"A/B" are to be settled
> empirically; the rest derive from cited thresholds and design-system tokens.

---

## 1. Load-bearing principles

1. **The writer moves information center↔periphery; the system never pushes it to center.** Calm tech is defined by exactly this control direction — "the individual, not the environment, must be in charge of moving things from center to periphery and back" (Weiser & Brown 1995/96; Case 2015). This *is* the GENERATE/EVALUATE door, and it makes "AI never speaks first" a structural rule, not a slogan.

2. **A mistimed interruption is expensively asymmetric; deferring to a breakpoint is cheap — so reveal-on-pause is the correct policy.** Full task resumption after a badly-timed alert runs ~10–16 min (Iqbal & Horvitz 2007); a derailed thread ~25 min (Mark et al. 2005); deferring to a breakpoint costs only ~1.5 min and *reduces* annoyance up to 56% and is rated markedly more "respectful" (Iqbal & Bailey 2008; Adamczyk & Bailey 2004). For writing the breakpoint is concrete: writers rush to finish the current sentence/paragraph before they accept a switch (paragraph-completion rate 0.78→12.8/min in the pre-switch window, Iqbal & Horvitz 2007). *(Correction: never cite "23 min 15 s" — it is press folklore, not in any paper; use ~16 min or ~25 min.)*

3. **Abrupt onset/motion captures attention involuntarily; gradual onset does not — but gradual *change* is missed entirely.** Sudden appearance or movement is detected pre-attentively in ~200–250 ms and yanks the eye bottom-up (Yantis & Jonides 1984/90; Healey & Enns 2012). Conversely, a change with no transient is not perceived at all (change blindness — Rensink et al. 1997; Simons et al. 2000). The operational consequence is two-sided: **during drafting, anything that appears must *ramp*, never pop**; but **an honest indicator that eases too slowly will not be *felt*** — it needs one brief discrete transient at the moment of change, then a static rest.

4. **Motion earns its place only as object constancy, and only when congruent + apprehensible; otherwise it is the single strongest distractor.** Animating an element's *move* significantly lowers tracking/estimation error vs an instant jump (Heer & Robertson 2007), but most animation fails to help and only "transitions between states" survive the Apprehension/Congruence test (Tversky et al. 2002). Peripheral motion — and the margin lane *is* the writer's periphery — is involuntarily attention-grabbing (Bartram et al. 2003; NN/g). Therefore: animate moves, keep them short, quiet, discrete, never looping; reserve any salient beat for the once-per-pass announce.

---

## 2. Timing table

Frame budget for *all* of these: hold ≤16 ms/frame (60 fps; RAIL) or motion reads as cheap; run reveal/packing in ≤50 ms idle slices so a keystroke preempts within the 100 ms response window.

Easing tokens (hand these to the engineer directly):
- **decelerate / enter** = `cubic-bezier(0, 0, 0.2, 1)` (M2 decelerate)
- **standard / in-place move** = `cubic-bezier(0.2, 0, 0, 1)` (M3 standard)
- **accelerate / exit** = `cubic-bezier(0.4, 0, 1, 1)` (M2 accelerate)
- **No spring/overshoot/bounce** anywhere ("make all movement meaningful"; agency/playfulness is wrong for paper-calm — Zongker & Salesin via Heer & Robertson 2007; Apple HIG).

| Interaction | Duration | Easing | Source / threshold |
|---|---|---|---|
| **Card appear** (reveal into lane, at a breakpoint) | **250 ms** | decelerate (enter) — arrives and *settles* | M3 standard-incoming 250 ms; Carbon small-expansion band; "enter slower than exit" |
| **Card move / re-pack** (stays in lane) | **200 ms**, stagger **40–60 ms** between cards | standard (in-place) | Carbon "leaves but stays nearby → standard, not exit"; Heer & Robertson staging/staggering lowers tracking error |
| **Card resolve / dismiss** | **150 ms** | accelerate (exit) | M3 outgoing 200 ms / exits faster, "require less attention" |
| **Card demote behind rail** | **240 ms** | standard (it *travels* into the rail; never a delete) | Carbon moderate-02 240 ms "system comms"; object constancy = "nothing vanishes" enforced as a visible move |
| **Edge-count pill update (N above / N below)** | **120 ms** opacity cross-fade **+ a 1-frame luminance tick**; **no positional/odometer slide** | decelerate on the incoming digit | Carbon fast-02 110 ms / micro band 90–120 ms; change blindness ⇒ a transient is required for it to be *felt*; sliding a number is gratuitous peripheral motion |
| **Rail (held-back) state change** | **240 ms** | standard | Carbon moderate-02; structural, slower than the count tick |
| **Completion pip** | **fade-in 150 ms / fade-out ~400 ms, ONCE** | decelerate in, gentle out | Carbon micro band; WCAG 2.3.1 (single pulse, not a blink); opacity/soft-glow only — *no* scale/translate (vestibular) |
| **Hover / press feedback** | **70–100 ms** (first frame <100 ms) | standard | Carbon fast-01 70 ms; Material switch 100 ms; Nielsen 0.1 s = direct manipulation |
| **"Announce-loud-once" beat** | up to **~500 ms** entrance, then visible **~2–3.5 s** before it quiets | emphasized decelerate | M3 emphasized 500 ms "hero moment"; Material snackbar LENGTH_LONG 3.5 s (it is a transient announce, *not* persistent state) |

Whole reveal gesture (announce → first card visible) must resolve **≤400 ms** once triggered, so it reads as a door opening, not a load (Doherty 1982).

---

## 3. Auto-reveal policy

**Two-clock model.** The *announce* fires immediately; the *cards* wait for a breakpoint. This is the negotiated-interruption pattern — signal availability, let the user pick the moment — which outperformed and was preferred over immediate interruption (McFarlane 2002).

- **t = 0 (pass completes):** fire the pip + increment the pills **within ~100 ms** (Nielsen instant limit). This closes the action-feedback loop the writer opened by requesting the pass — so the deferred card reveal does *not* violate responsiveness. **No cards move yet.**

- **Hold cards until a breakpoint, preferring the coarsest available** (coarser breakpoints = larger cost reductions — Iqbal & Bailey 2007/2010). strop has it easier than the literature's ~55% breakpoint detectors because sentence/paragraph boundaries and keystroke-idle are *directly observable* in the editor.

  - **STRONG (reveal eagerly, ~immediately):** caret/pointer/scroll enters the margin lane, **or** the writer explicitly opens the door, **or** ≥**15 s** of idle (the dwell-as-engagement signal — Iqbal & Horvitz 2007). These mean "the writer has turned to review"; motion is now *congruent* with their attention shift, so this is exactly where object-constancy animation helps.
  - **MEDIUM (the default mid-session trigger):** a sentence/paragraph terminator (`.`/newline) **followed by** keystroke-idle ≥ the pause threshold.
  - **Pause threshold:** **~1.5–2 s of keystroke silence.** *(Judgment call — triangulated, not validated: 1 s is Nielsen's flow-of-thought limit; keystroke-logging puts higher-order planning pauses at >~2 s at clause/sentence boundaries — DiVA. A/B this; it is the single most likely value to need tuning.)*
  - **NEVER:** reveal during sub-2 s inter-key gaps or mid-word. An abrupt onset in the periphery mid-burst is an involuntary interrupt regardless of how small the card is (Yantis & Jonides; principle 3).

- **In EVALUATE (door already open):** reveal is eager and may animate fully — the writer is attending, so motion aids comprehension rather than distracting.

**Verdict: reveal-on-pause is SUPPORTED.** Directly by breakpoint-deferral results (Adamczyk & Bailey 2004; Iqbal & Bailey 2008/2010) and by the writing-specific sentence/paragraph-completion finding (Iqbal & Horvitz 2007). Note the relevant payoff is *affective* (less stress/annoyance, more "respect"), not throughput — Mark et al. (2008) found interrupted work is finished as fast or faster but at a stress cost, which is precisely strop's thesis, so "no measured slowdown" would never vindicate a pushy reveal.

---

## 4. Motion: when yes / when no

**Animate (YES) when both can hold:**
- the element keeps its identity through a layout change — re-pack, demotion to rail (object constancy lowers real tracking error — Heer & Robertson 2007); **and**
- it passes Congruence + Apprehension: the motion maps to the actual change (position→translate) and is slow/simple enough to perceive (Tversky et al. 2002).

**Instant / sub-threshold / nothing (NO) when:**
- the writer is in an active typing burst in GENERATE — **batch all re-packs to the next breakpoint so margin motion never occurs mid-burst at all** (the safest reading of the distraction literature);
- it would be a *count/number* change — use the 120 ms cross-fade + luminance tick, never a positional slide (NN/g; §2);
- it would loop/breathe/idle — banned outright (chronic peripheral motion is the worst case for a calm editor; also WCAG 2.2.2). Only **discrete** motion, active on the event, is calm (Bartram et al. 2003);
- it would scale/spin/parallax/spring — banned for vestibular safety and because it signals agency (Apple HIG).

**prefers-reduced-motion fallback — this is the part that protects "nothing vanishes":** replace every *translate/scale* with an **opacity cross-fade of the same duration**, not an instant snap. "Reduced motion is not *no* motion" (Apple HIG; Val Head 2015) — a teleport breaks felt continuity as badly as a violent slide. Keep all count/rail/card *state* and semantics intact; only the *travel* becomes a fade. The pip degrades to a static low-contrast dot or an instant count tick. **GPUI has no CSS media query — read the OS Reduce-Motion setting directly and branch the easing.** Treat instant/cross-fade as a first-class supported mode, not a degraded one.

---

## 5. The completion pip

**Verdict: DO it — conditionally — but A/B it, because it is the single most contested element in the whole brief.** *(Judgment call beyond the evidence: no study tests a one-shot peripheral flash in a writing tool; this is a synthesis that minimizes documented risk, not a proven win.)*

Why it's defensible: a brief luminance/motion onset is the *pre-attentively correct* completion cue (motion beats color/shape in the periphery — Bartram et al. 2003), and it is conveniently self-limiting — under high perceptual load (deep typing burst) capture is *reduced* (Lavie/onset-load work via the 2024 review), so it nags least exactly when flow matters most and is seen between thoughts. That is the perceptual justification for "announce once, then go quiet."

Why it's risky: peripheral motion is involuntarily grabbing (the feature *is* the bug), and the margin sits in **near** periphery (a few degrees off the caret), which is *more* motion-sensitive than the far-periphery the moticon studies used — so it may nag more than the literature predicts. Contingent capture (Folk et al. 1992) also means a focused writer may simply ignore it — acceptable as "calm," but it means **the pip can never be the only completion signal.**

Concrete spec if shipped:
- **Single gentle opacity fade: ~150 ms in (decelerate), ~400 ms out, once.** Opacity/soft-glow — *not* scale or translate.
- Fire it at **t=0 within ~100 ms** of completion (so it reads as caused-by-the-pass), in the caret's margin; if mid-keystroke, hold to the same micro-pause used for reveal.
- **Tune amplitude DOWN** for near-periphery; non-saturated (never red); WCAG 2.3.1 safe by construction (one pulse).
- **Pair with persistent state** (the pill increment + rail), which is what actually lowers return cost (Iqbal & Horvitz H4). reduced-motion → static dot.
- A/B: pip vs no-pip + static pill change, measuring *self-reported intrusiveness* alongside detection.

---

## 6. Per-decision verdicts

1. **Auto-reveal timing → announce-once at t=0 (<100 ms), reveal cards only at a coarse breakpoint** (margin gaze / scroll / door / ≥15 s idle, or sentence-end + ~1.5–2 s idle). **Supported** by interruption science; the win is reduced stress/annoyance, not speed.

2. **Card motion → ANIMATE moves, never pop, never during a burst.** Appear 250 ms ease-out; re-pack 200 ms standard staggered 40–60 ms; resolve 150 ms ease-in; demote 240 ms standard. No loop, no spring, no scale/spin. Justified by object constancy (Heer & Robertson) — but keep it at UI scale (200–300 ms), *not* the data-viz ~1 s figure. reduced-motion → cross-fade.

3. **Honest indicators → brief discrete pre-attentive transient, then static.** 120 ms opacity cross-fade + 1-frame luminance tick on the count; *not* a slow ease (change blindness would hide it — defeating "must FEEL it"), *not* a positional slide. **Encode the rail's held-back debt as ambient density** (stack thickness / tick-marks proportional to count), because exact digits are reliable glance-reads only up to ~4 (subitizing); pair the digit with the density cue past that. Persistent visibility is doing real cognitive work — it lowers the cost of returning to deferred review (Iqbal & Horvitz H4), so keep deferred cards represented, never fully hidden.

4. **The pip → YES, gentle single opacity fade, paired with persistent state, reduced-motion fallback, and A/B-gated.** This is the brief's genuine design bet; ship it behind a flag and measure intrusiveness.

5. **Visible cap → cap, yes; the number 7 is *not* grounded.** Capping concurrent demands and resting the rest behind a visible-but-quiet rail is sound (interruption-overload; visible-debt lowers return cost). But "7" is Miller's *recall* span, not a limit on persistent on-screen items — NN/g calls that application a myth, and Cowan's ~4 applies to un-chunkable items. **Recommend a comfortable resting count of ~5 (test 4–7), flexing with viewport height; chunk cards into themed groups if you want to push toward 7.** What matters more than the number: the demotion is a *visible travel into the rail*, not a delete.

---

## 7. Tensions & gaps (settle by testing)

- **No study targets strop's exact case** — self-*requested*, *silent* cards beside *creative composition*. The breakpoint/sentence-completion findings transfer well (measured during document editing), but the affect magnitudes (e.g. +31–106% annoyance) come from *unsolicited* interruptions and **likely overstate** the cost of a reveal the writer asked for. Don't over-defer.
- **The ~1.5–2 s pause threshold is uncalibrated** for a UI reveal trigger — triangulated from Nielsen's 1 s and keystroke-logging's ~2 s, not validated. Prefer coarse boundaries (paragraph end, scroll, margin gaze) over the raw idle timer wherever possible; tune the timer empirically.
- **Honesty-vs-calm (the change-blindness paradox)** — a slow eased count update is exactly what gets *missed*, a loud one distracts. The resolution here (brief discrete transient at the moment of change, then static rest) is reasoned from combining two literatures, **not directly tested** for retirement/debt cues. A/B detection vs intrusiveness.
- **The pip** is the weakest-supported element (see §5). Near-periphery sensitivity may make it nag harder than predicted.
- **Object-constancy duration is borrowed, not measured for cards** — the ~1 s figure is whole-chart morphs; 200–300 ms is an engineering inference. The *direction* (animate, don't teleport) is solid; the exact ms is interpolated. Consider that batching all re-packs to breakpoints may make mid-burst margin motion a non-issue entirely.
- **The cap number is numerology, not law** — pick 4–7 by testing, justified as a clutter/attention budget, never by citing 7±2.
- **Apple publishes no hard ms tokens** — treat Material/Carbon as the numeric authority, Apple/Fluent as principles.

---

## Sources

- Weiser, M. & Brown, J.S. (1995/96). *Designing Calm Technology / The Coming Age of Calm Technology.* Xerox PARC. http://www.ubiq.com/weiser/calmtech/calmtech.htm
- Case, A. (2015). *Calm Technology: Principles and Patterns for Non-Intrusive Design.* O'Reilly. https://www.calmtech.institute/calm-tech-principles
- Matthews, T. et al. (2004). *A Toolkit for Managing User Attention in Peripheral Displays.* UIST 2004. http://www.madpickle.net/scott/pubs/p321-matthews.pdf
- McCrickard, D.S. et al. (2003). *A model for notification systems evaluation.* ACM TOCHI 10(4).
- Nielsen, J. (1993/2014). *Response Times: The 3 Important Limits.* NN/g. https://www.nngroup.com/articles/response-times-3-important-limits/ (Miller 1968; Card, Robertson & Mackinlay 1991; Card, Moran & Newell 1983)
- Doherty, W.J. & Thadani, A.J. (1982). *The Economic Value of Rapid Response Time.* IBM. https://lawsofux.com/doherty-threshold/
- Google / web.dev. *Measure performance with the RAIL model.* https://web.dev/articles/rail
- Iqbal, S.T. & Horvitz, E. (2007). *Disruption and Recovery of Computing Tasks.* CHI 2007. http://erichorvitz.com/CHI_2007_Iqbal_Horvitz.pdf
- Iqbal, S.T. & Bailey, B.P. (2008). *Effects of Intelligent Notification Management on Users and Their Tasks.* CHI 2008. https://www.interruptions.net/literature/Iqbal-CHI08.pdf
- Iqbal, S.T. & Bailey, B.P. (2010). *Oasis: Linking Notification Delivery to the Perceptual Structure of Goal-Directed Tasks.* ACM TOCHI 17(4). https://www.interruptions.net/literature/Iqbal-TOCHI10.pdf
- Adamczyk, P.D. & Bailey, B.P. (2004). *If Not Now, When?* CHI 2004. https://interruptions.net/literature/Adamczyk-CHI04-p271-adamczyk.pdf
- Bailey, B.P. & Konstan, J.A. (2006). *On the need for attention-aware systems.* Computers in Human Behavior 22(4). https://interruptions.net/literature/Bailey-CHB06_1.pdf
- Mark, G., Gonzalez, V.M. & Harris, J. (2005). *No Task Left Behind?* CHI 2005. https://ics.uci.edu/~gmark/CHI2005.pdf
- Mark, G., Gudith, D. & Klocke, U. (2008). *The Cost of Interrupted Work: More Speed and Stress.* CHI 2008. https://ics.uci.edu/~gmark/chi08-mark.pdf
- Leroy, S. (2009). *Attention residue when switching between work tasks.* OBHDP 109(2).
- McFarlane, D.C. (2002). *Comparison of Four Primary Methods for Coordinating the Interruption of People in HCI.* HCI 17(1). https://www.interruptions.net/literature/McFarlane-HCI02_2.pdf
- Yantis, S. & Jonides, J. (1984/1990). *Abrupt Visual Onsets and Selective Attention.* J. Exp. Psychol. HPP. https://pubmed.ncbi.nlm.nih.gov/2137514/ ; review (2024): https://pmc.ncbi.nlm.nih.gov/articles/PMC11908675/
- Folk, C., Remington, R. & Johnston, J. (1992). *Contingent attentional capture.* (contingent-capture qualifier)
- Healey, C.G. & Enns, J.T. (2012). *Attention and Visual Memory in Visualization and Computer Graphics.* IEEE TVCG 18(7). https://www.csc2.ncsu.edu/faculty/healey/PP/
- Treisman, A. & Gelade, G. (1980). *A feature-integration theory of attention.* Cognitive Psychology 12(1).
- Bartram, L., Ware, C. & Calvert, T. (2003). *Moticons: detection, distraction and task.* IJHCS 58(5). https://www.sciencedirect.com/science/article/abs/pii/S1071581903000211
- Rensink, R., O'Regan, J.K. & Clark, J.J. (1997). *To See or Not to See.* Psychological Science 8(5); Simons, D., Franconeri, S. & Reimer, R. (2000). *Change Blindness in the Absence of a Visual Disruption.* Perception 29.
- Heer, J. & Robertson, G. (2007). *Animated Transitions in Statistical Data Graphics.* IEEE TVCG/InfoVis. https://idl.cs.washington.edu/files/2007-AnimatedTransitions-InfoVis.pdf (cites Robertson et al. 2002 ~1 s; Zongker & Salesin "make all movement meaningful")
- Tversky, B., Morrison, J.B. & Bétrancourt, M. (2002). *Animation: can it facilitate?* IJHCS 57(4). https://hci.stanford.edu/courses/cs448b/papers/Tversky_AnimationFacilitate_IJHCS02.pdf
- Lasseter, J. (1987). *Principles of Traditional Animation Applied to 3D Computer Animation* (staging, slow-in/slow-out). SIGGRAPH 1987.
- NN/g — Laubheimer, *Animation for Attention and Comprehension* https://www.nngroup.com/articles/animation-usability/ ; Harley, *Executing UX Animations: Duration and Motion Characteristics* https://www.nngroup.com/articles/animation-duration/ ; *Short-Term Memory and Web Usability* (7±2 myth) https://www.nngroup.com/articles/short-term-memory-and-web-usability/
- Material Design 2 *Speed* https://m2.material.io/design/motion/speed.html ; Material Design 3 *Easing & duration* https://m3.material.io/styles/motion/easing-and-duration/tokens-specs ; *Snackbar guidelines* https://m3.material.io/components/snackbar/guidelines
- IBM Carbon — *Motion* https://carbondesignsystem.com/elements/motion/overview/
- Apple HIG — *Motion* https://developer.apple.com/design/human-interface-guidelines/motion ; Microsoft *Fluent 2 Motion* https://fluent2.microsoft.design/motion
- Head, V. (2015). *Designing Safer Web Animation for Motion Sensitivity.* A List Apart. https://alistapart.com/article/designing-safer-web-animation-for-motion-sensitivity/ ; MDN *prefers-reduced-motion*.
- W3C WCAG 2.2 — SC 2.3.1 *Three Flashes* https://www.w3.org/WAI/WCAG22/Understanding/three-flashes-or-below-threshold.html ; SC 2.2.2 *Pause, Stop, Hide* https://www.w3.org/WAI/WCAG22/Understanding/pause-stop-hide.html ; SC 2.3.3 *Animation from Interactions* + Technique C39.
- Csikszentmihalyi, M. (1990). *Flow.* https://open.ncl.ac.uk/academic-theories/8/flow-theory/ ; Osborn, A. (1953). *Applied Imagination* (deferred judgment).
- Flower, L. & Hayes, J. (1981). *A Cognitive Process Theory of Writing.* CCC 32(4); Kellogg (working-memory writing model).
- iA — *Separate Writing and Formatting* https://ia.net/topics/separate-writing-and-formatting ; *Focus Mode*.
- DiVA. *Developing pause thresholds for keystroke logging analysis* (~2 s planning-pause cutoff). https://www.diva-portal.org/smash/get/diva2:834468/FULLTEXT01.pdf
- Miller, G.A. (1956). *The Magical Number Seven, Plus or Minus Two.* Psych. Review 63(2). https://psychclassics.yorku.ca/Miller/ ; Cowan, N. (2001). *The magical number 4 in short-term memory.* BBS 24(1).
- Ishii, H. et al. (1998). *ambientROOM.* CHI 1998 (ambient density). ; Trick & Pylyshyn (1994) subitizing ~4.
- oberien (2023). *Interruptions cost 23 minutes 15 seconds, right?* (debunk). https://blog.oberien.de/2023/11/05/23-minutes-15-seconds.html