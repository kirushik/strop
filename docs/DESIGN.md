# Strop Design Architecture

> Started 2026-06-13 after Kirill's diagnosis: "all those nits and
> papercuts are the result of our UX never being _designed_, only
> _evolving as it went_." This document is the cure: the cohesive
> UI/UX architecture every surface must answer to. PLAN.md tracks
> execution; this tracks intent. Research provenance: three agent
> reports (HCI fundamentals; surface conventions; AI-panel patterns),
> landing 2026-06-13.

## 0. Directives already decided (the user's law)

These came from use, not theory; they outrank any pattern below.

1. **Universal gestures stay universal.** Any chord that works on text
   anywhere works on text everywhere — ctrl-backspace in a query field,
   ctrl-arrows in a rename box. No surface gets a dumber text model
   because it's small. (Implication: NoteInput must grow into a real
   single-line editor — cursor, word motion, selection — or be replaced
   by a shared field widget.)
2. **Status never covers prose.** Cards, popups, toasts — nothing sits
   on top of the user's words. Margins, strips, panels: yes. Overlap:
   no. Everything transient must be dismissable.
3. **AI provider setup is the core onboarding task** and gets a real
   UI: form, async validation, live model list from the API, visible
   feedback. The config file remains the storage (UI writes through
   toml_edit, comments preserved; hand edits stay respected).
4. **No MS Visual Studio™.** Chrome stays minimal — but minimal is a
   budget to spend deliberately, not an excuse to spend it by accident
   (the current titlebar is "almost accidental").
5. **Enough support, not neutrality.** The target user includes an
   ADHD-minded aspiring fiction writer who needs the tool's help to
   *finish*. Strop should lean toward Bryce-school explorability —
   the software invites trying things — without costume-party UI.

## 1. Design principles (synthesis pending — HCI agent)

_To fill: ranked principles with provenance._

## 2. The capability map (what surfaces exist and why)

Current inventory: titlebar (title/rename, format buttons, history
ring, hamburger, window controls), command palette, keyboard map,
margin (notes/diagnoses/AI status), footnote zone, find/replace strip,
history dropdown + inline diff, bottom strips (narrow-window variants).

_To resolve against research:_
- Toolbar: what earns persistent chrome; selection-popover question;
  heading access; undo/redo buttons.
- History: dropdown → side panel? Anatomy, mode entry/exit, where
  voice-drift lives.
- Footnotes: complete the bottom-zone model (mark rendering, hover,
  bidirectional jumps, edit-in-zone).
- AI settings panel: form anatomy, /models picker, validation states.

## 3. The explorability layer (pending — HCI agent)

The Bryce question: which mechanisms make discovery feel like play in
a *text* editor. Candidates to evaluate: fearless-history framing
("nothing can be lost" as a first-class promise), first-run cards that
invite a pass, empty-state invitations, palette serendipity.

## 4. The finish-your-story layer (pending — HCI agent)

Evidence-backed support for completing drafts; dark patterns to refuse.

## 5. Component language

_To fill: the shared vocabulary — field widget, card, strip, panel,
popover; spacing/rhythm rules (28px), color roles, type roles — so new
surfaces are assembled, not invented._

## Open questions for Kirill

- Selection-popover formatting vs persistent format buttons (can't
  have both as primary — one demotes).
- History side panel: full-height right panel would displace the
  margin while open — acceptable?
