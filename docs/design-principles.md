# Strop design principles

*(The constitution. Each principle was earned — born from a specific
design failure caught in review, not deduced in the abstract. The
born-from stories stay attached because a principle without its wound
drifts into slogan. Companion docs apply these: `golden-path.md` (the
arc), `color-language.md` (provenance), `attention-motion.md` (timing),
`history-strip.md`, `asides.md`. When a new design contradicts one of
these, the design is wrong or the principle gets amended here — never
silently.)*

## The ethic

### P1. The text is sovereign

The author's words are never raw material for the interface. Nothing is
drawn ON the prose; the machine never writes into it. Chrome may sit
*beside* the text (margins, rails, footers, strips) — never on top of
it, never inside it. The boundary is mechanical, not a matter of
intent: the software may **record** the writer's text verbatim (the
graveyard, history) and may **relocate** it as still-editable text (an
aside, an orphaned note); it may never **decorate** it, **quote it
rhetorically**, or **wear it as chrome**.

*Born from:* re-entry v3, which took the writer's own last line and
returned it the next morning dressed as a UX element. Kirill: "Someone
changed my text, and also drew their pesky UX popup on top of it…
My text matters more than anything you'd ever be able to tell me from
your UX."

*Forbids:* overlays on prose; the software quoting the writer back at
her; any feature whose UI surface is the writer's own words wearing
chrome.

### P2. The tool never wants anything from you

Strop is a tool — a well-built, dignified one. A high-end impact drill
has a sculpted grip, safety mechanisms, and a notch on the handle whose
purpose you discover in week two, mid-problem, with a *but of course*.
It does not have a popup reminding you which bit you meant to use next.
If the builder wants a reminder, she puts the bit in the box beside it.
"My microwave never wants something from me."

We are not the creative-writing-methods police. The golden path is a
map of what writers actually do, and Strop's methods wait at intuitive
locations for the writer who reaches for them — the software never
jumps out of its way to impose the correct pattern. **No amount of
p-confidence that a nudge helps excuses being annoying or mentoring.**
This is why everyone hates app-enabled appliances: they are not
respectful, they don't know their place, they want something from you.

*Born from:* the re-entry feature's third failure (round 4, shelved on
this principle) — every mechanism for inviting a note-to-future-self
was the software wanting something.

*Forbids:* software-initiated prompts, reminders, invitations,
congratulations; onboarding that withholds the tool until the lesson is
absorbed; any UI whose purpose is to make the writer do writing
"correctly."

*Demands:* that every capability have a calm, findable resting place —
the drill notch — and that discovery happen in use, not in dialogue.

## The ontology

### P3. Everything the writer owns is text

Writer-side material — prose, margin notes, compost, graveyard entries,
checkpoint names — is text, with text mechanics: click gives a caret,
typing types, selection selects, the same formatting works everywhere
the writer's words live. Widgets (cards, buttons, toggles, threads) are
the machine's shape; the writer's things never become widgets. The
warm/cool axis (`color-language.md`) marks the same boundary in color:
warm things are text you own, cool things are machinery you operate.

*Born from:* the compost rail's card phase (round 4) — "this all feels
that things on the left are not the cards; probably just paragraphs."
They are paragraphs. Earlier: the checkbox refusal (no clickable
checkboxes in prose, ever — a checkbox is a widget squatting in text).

*Demands:* wherever the writer's text is, the writer's tools are — the
formatting flank works in compost and margin notes exactly as in prose.

## The pedagogy

### P4. Show, don't explain

Interface text is either **data** (names, dates, counts, the writer's
own labels) or an **actionable label** (the verb on a button, the
carrier sentence of a menu row). It is never a description of an
affordance. A legend is a design failure with a caption; if an element
needs prose to be understood, redesign the element. The moment the UI
starts explaining itself it has become a manual — "a cryptic D&D manual
which requires you to read it, and only in the right sequence."

The test: delete every string that is neither data nor an actionable
label. If the surface stops working, the *surface* was broken.

*Born from:* round 4's chattiness audit — "read-only — put back or
delete" printed on a graveyard entry that is visibly read-only and
already carries Put back and Delete buttons; "reactions become margin
notes"; "a mark or a note — never an edit"; the strip's legend-as-axis-
labelling. A corridor tester refused the strip outright: too much to
read before anything could be done.

*Forbids:* legends; axis labels that teach; captions that justify;
tooltips that define; any string whose audience is "a user who hasn't
understood yet."

*Budget:* the carrier sentence (a menu row that teaches a term by
carrying it — "Ask the editor for a **line read**") is the one
sanctioned channel for craft vocabulary, and it is capped: carried
terms appear only in *action* rows (menu items, button phrases), never
on passive surfaces, one carried term per sentence. Without the cap,
every explanation smuggles itself in as a carrier.

*Demands:* affordances that carry their meaning in form (a slider looks
scrubbable; a button looks pressable); words spent only where words are
the data.

### P5. The corridor floor and the notch gradient

Every surface must be operable at first sight, by a stranger in a
corridor, at its basic level — because it *looks like something the
stranger already knows* (a seek bar, a footer, a menu, a page).
"Intuitive" means familiar; familiarity is borrowed, not taught. Depth
arrives the way the drill notch does: capabilities rest where a curious
hand falls, and each reveals itself in use, one at a time, never
announced. The floor is for everyone on day one; the notches repay
weeks; nothing about the notches may tax the floor.

*Born from:* the history strip's corridor failure (round 4) — a
sample-of-one tester "would never even try to understand and learn
this"; the same tester would happily use "a simple slider like a time
control in a media player." The strip's depth was real; its floor was
missing.

*Forbids:* surfaces that must be read before they can be touched;
features that advertise themselves; a "simple mode" that amputates
depth (the floor and the depth are one surface at two levels of
attention, not two modes).

## The mechanics

### P6. The screenshot test

Every frame of every transition must make sense as a still image. If a
mid-animation screenshot shows a state that never logically existed,
the animation is lying. Corollary for time-varying views: position and
progress get *static* encodings (a dimmed region, a stepped line), so
any paused instant reads correctly.

*Born from:* Kirill's graveyard-footer design (round 3), stated as a
general principle there: "every element of in-transition animation
makes sense, if say made into a screenshot."

### P7. Widget contracts: extend, never subvert

A control that borrows a common widget's face must honor that widget's
contract. Enrich the content; never fake the nature. A button that is
secretly a text field, a menu detached from its control, a scrubber
that snaps when it looks continuous — each spends the user's trust in
every real widget on the screen. When our behavior is *safer* than the
ancestor's (restore that appends instead of destroying), that is a
lawful extension: the familiar word and face, a contract honored plus.

*Born from:* the titlebar palette (a fake button pretending to be a
focusable text field — logged as a shipped bug, round 3) and the lab's
detached editor-button menu, same sin at small scale.

### P8. UI is grammar

Parallel meanings get parallel forms; one action gets one verb
everywhere (it is "Put back" in the graveyard section, on the footer,
and in the undo affordance — never "undo" here and "put back" there).
Menu rows read as completions of their carrier sentence. And **system
templates never swallow writer text**: composing "after ⟨station
name⟩" produces "after Before the line read" the day a station is
named "Before the line read." Variable writer-owned strings are
displayed as data — set off, highlighted, quoted by *typography*, never
inlined into system prose.

*Born from:* round 2 (the re-entry banner mixing the writer's grammar
with the software's; adopted then as a fundamental) and round 4's
"after Before the line read" readout bug, which promoted the
template-composition ban from style advice to law.

### P9. Hover only expands

Hover may enlarge or complete what is already visible ("Tue 12 Jul" →
"Tue 12 Jul, 19:02–23:14"); it may never carry sole meaning. No
orientation may require a mouse; no two hover-values can be compared.

*Born from:* round 3's history-strip takes, where hover-gated labels
died panel-wide.

### P10. Color speaks once

Color carries provenance and state (warm = writer, cool = machine,
sage = returned/resolved, drained = stale, red = errors only —
`color-language.md`), and what color says, words do not repeat. A sage
endcap needs no "(answered)"; a cool veil needs no "(the editor)".
The converse holds: anything color says must ALSO be recoverable
without color (value contrast, position, form) — color speaks once,
but it is never the only speaker for the colorblind.

*Born from:* round 4 — a strip legend explaining "sage when answered"
in words while the whole product already color-codes exactly that.

### P11. One anchor object per surface

Every surface has exactly one anchor object — the thing the returning
eye lands on — and the contrast budget is spent on it; everything else
subordinates. A surface where everything is equally quiet is not calm,
it is mush; a surface where everything is equally loud is not rich, it
is noise.

*Born from:* the round-4 Birman audit — the strip already obeyed it by
instinct (the thumb and rail carry the highest contrast, the fabric
sits low-mid) while the first compost-rail draft shipped uniform grey
with nowhere to land. Its anchor is the tail — the live end, where
arrivals blink and the caret slot sits.

### P12. The control is the indicator

State is shown by the control that changes it — never displayed in one
place and changed in another. The door state lives on the editor
control as its label; "Now" dims when you are at now. Where a control
can carry its state, a separate status display is forbidden; grow one
and you have built a dashboard about your own chrome.

*Born from:* the same audit, naming what the door pair and the Now
chip already did ad hoc — one law before a status bar accretes.

### P13. Every verb has an inverse in the same grammar

Nothing destroys silently, and nothing is rescued through a different
door than it left by: an exile is undone by "Put back" where the exile
happened; a restore appends and is itself one Restore away from
undone; typing into the past first writes the way back. Reversibility
is not a feature list — it is a grammar rule, so the writer can infer
the inverse without being told (which is what lets P4 delete the
warnings).

*Born from:* the graveyard and the strip, retroactively — the
product's strongest trust claim was folklore distributed across
artifacts until this round wrote it down.

## Applying the constitution

Reviews cite principles by number. A mockup or feature that fails one
gets the principle named in the verdict, the way `golden-path.md` §9
records them. The list is short on purpose: a principle earns its place
by having killed at least one real design.
