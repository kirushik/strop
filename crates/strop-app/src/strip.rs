//! The history strip's view model (P1 — docs/impl/01-history-strip.md,
//! docs/history-strip.md v2). A seek bar that happens to be true: a rail with
//! a thumb over a band of amber "fabric" that IS the writing, scrubbable at
//! frame rate.
//!
//! This module is the PURE half — no gpui. It owns the immutable BAKE (fleck
//! quads, envelope, veils, threads, stations, date lane, the working-time
//! axis) and the mutable scrub STATE, kept apart because the stability law
//! (design §3, review B7) is exactly that separation: the bake never changes
//! while the strip is open (the one lawful in-session re-bake is an explicit
//! Restore — review H35); only the view offset, playhead, readout and label
//! brightness may vary as the thumb moves. Keeping the bake gpui-free lets the
//! geometry, label omission, anchor selection (the seconds/ms law) and the
//! readout be unit-tested against a real `Journal` without a window; the
//! painter (editor.rs `StripElement`) reads these vecs and draws them.

use strop_core::journal::{EditRun, Journal, JournalEvent};

// ---- Layout ----------------------------------------------------------------
// The band, top → bottom (design §1, restored to the lab-mockup composition):
// a control row for the chips alone (readout/Restore … Now/×), a label lane
// for the two rows of station names, then THE RAIL — the document's own top
// edge, which the thumb rides and the cream page hangs from — the fabric
// below it, and a thin date lane. The rows sum to STRIP_H.
pub const STRIP_H: f32 = 196.;
pub const TOP_ROW_H: f32 = 26.;
pub const LABEL_LANE_H: f32 = 30.;
pub const FABRIC_H: f32 = 126.;
pub const DATE_LANE_H: f32 = 14.;
/// The rail's y from the strip's top edge. The rail IS the page's top edge:
/// the envelope hangs from it, so the thumb literally rides along the top of
/// the manuscript. (The v1 build parked the rail up in the control row, where
/// the chips occluded it — and the thumb with it — on every short history.)
pub const RAIL_Y: f32 = TOP_ROW_H + LABEL_LANE_H;
/// Fabric top == the rail: the page hangs from the seek bar.
pub const FAB_Y0: f32 = RAIL_Y;
/// Horizontal breathing room at each end of the band (the readout/Now chips
/// live inside this margin at the far ends; the rail and fabric span between).
pub const SIDE_PAD: f32 = 28.;

/// The fixed product-wide quant: one pixel of fabric ≈ 30 s of WORKING time
/// (design §1 — "one fixed scale, never per-document"). Working time folds the
/// long idle gaps out, so the amber density always means the same amount of
/// work. Expressed here as px per millisecond for the timeline math.
pub const PX_PER_MS: f32 = 1. / 30_000.;
/// A gap longer than this (15 min) is not working time — it folds to a thin
/// seam instead of stretching the axis with dead air (design §1, spec §1).
pub const GAP_FOLD_MS: i64 = 15 * 60 * 1000;
/// The working-time width a folded gap collapses to — a WELL: a recessed
/// full-height column, the visible presence of time away (the lab mockup had
/// these; v1 shrank them to invisible hairlines). Two fixed tiers, not a
/// gap-proportional scale: the axis spends x on WORK, so absence is
/// punctuation, never a bar chart — an overnight break and a long
/// interruption just read as two different marks.
pub const SEAM_PX: f32 = 10.;
pub const SEAM_WIDE_PX: f32 = 20.;
/// A gap at or past this (2 days) earns the wide well.
pub const SEAM_WIDE_MS: i64 = 2 * 86_400_000;
/// Every run gets at least this much x, so even a one-op run has somewhere to
/// hang its flecks (a run is seconds long — below a pixel at the fixed scale —
/// so the density that reads as flow/deliberation is emergent from run
/// ADJACENCY along x, not intra-run spread; the review's fleck-cap concern is
/// moot here because the run's own x-extent is already sub-pixel).
pub const MIN_RUN_PX: f32 = 0.6;

/// Working px per word of |Δwords| across a checkpoint-only span (Bug A). The
/// legacy era has no keystroke record — no working time to scale by — so its
/// width is derived from the word delta between materialized states. Chosen so
/// a multi-thousand-word fortnight of checkpoints reads as a scannable
/// landscape (~one to two fabric widths), not a run-era-style solid stroke:
/// checkpoints are sparse anchors, so each word of delta earns more x than a
/// keystroke run's word (whose density comes from adjacency, not per-word px).
pub const CKPT_WORD_PX: f32 = 0.28;
/// A checkpoint-only span never narrower than this, so a zero-Δwords span (a
/// formatting-only or same-second twin checkpoint) still separates its ticks
/// instead of overprinting — the label lane needs the gap to place two names.
pub const CKPT_MIN_PX: f32 = 16.;

/// Fleck edge (a 2 px amber grain, design §1). Ins amber / del burnt amber, the
/// value-contrast pair that stays legible colorblind (docs/color-language.md).
pub const FLECK: f32 = 2.;
pub const FLECK_INS: u32 = 0xC8A951; // amber — words the writer added
pub const FLECK_DEL: u32 = 0x8A6D35; // burnt amber — words the writer cut
pub const FLECK_INS_ALPHA: f32 = 0.5;
pub const FLECK_DEL_ALPHA: f32 = 0.62;
/// At most this many flecks per run (spec §1). A cap the batching does not
/// need (thousands of quads are one instanced batch) but which bounds a
/// pathological single paste; at the fixed scale a run is sub-pixel wide so 70
/// grains already saturate its column.
pub const FLECK_CAP: usize = 70;

/// The machine-room dark ground and the readout chip (spec §0 — new inline
/// values in that family; NOT theme tokens, chrome fills stay at use sites).
pub const GROUND: u32 = 0x26251F;
/// The desk beyond the manuscript's recorded extent (spec v3 §1a).
pub const DESK: u32 = 0x211F1A;
pub const READOUT_CHIP: u32 = 0x111009;
/// The cream page-fill under the envelope and the envelope stroke itself
/// (design §1 — the corridor fix filled the rail→envelope band faint so it
/// reads as a page, not a floating line).
pub const CREAM: u32 = 0xE9E2D0;
pub const CREAM_FILL_ALPHA: f32 = 0.13;
pub const ENVELOPE_ALPHA: f32 = 0.9;
/// Cool veil for an AI pass (the machine read everything → a translucent
/// column over the page, rail to envelope) and the cool thread for a card's
/// open life. Bounded by the page, the veil affords a little more presence
/// than the v1 full-band wash did.
pub const VEIL: u32 = 0x86B0E6;
pub const VEIL_ALPHA: f32 = 0.16;
pub const THREAD: u32 = 0x86B0E6;
/// Sage terminal dot for a resolved card / a restore tick — the one theme
/// token, re-exported under the strip's short name (token audit A6: no local
/// re-declaration to drift from `theme::SAGE_COLOR`).
pub use crate::theme::SAGE_COLOR as SAGE;
pub const GREY: u32 = 0x8A8678;

// ---- The working-time axis -------------------------------------------------

/// One contiguous stretch of the x-axis. Active stretches map wall time to
/// working px 1:1 (`PX_PER_MS`); folded gaps collapse a long wall span into
/// `SEAM_PX`. Contiguous and non-overlapping by construction, so the two maps
/// (`work_at`/`wall_at`) are simple piecewise-linear lookups.
#[derive(Clone, Copy, Debug)]
struct Seg {
    wall0: i64,
    wall1: i64,
    work0: f32,
    work1: f32,
    folded: bool,
}

/// The x-axis: wall-clock ms ⇄ working px. Built once at bake from the
/// journal's runs (extended to `now` so the rail reaches the present).
#[derive(Clone, Debug, Default)]
pub struct Timeline {
    segs: Vec<Seg>,
    pub total_work: f32,
    pub start_ms: i64,
    pub end_ms: i64,
}

/// A folded gap's well width — the two-tier rule.
fn seam_width(gap_ms: i64) -> f32 {
    if gap_ms >= SEAM_WIDE_MS { SEAM_WIDE_PX } else { SEAM_PX }
}

impl Timeline {
    /// Walk the merged ACTIVITY in order — runs, event instants, AND checkpoint
    /// instants — folding >15 min gaps, extending to `now_ms`. Events count as
    /// activity because a pass typically lands a lull AFTER the last keystroke:
    /// built from runs alone, its veil would fall inside the folded gap and
    /// paint collapsed onto the seam (found on the first real screenshot).
    ///
    /// Checkpoints (Bug A) are the axis for a LEGACY era — a document whose
    /// journal is empty or sparse but whose history lives in the checkpoint
    /// states. A span bracketed by two checkpoints with no runs between them
    /// (`Ckpt → Ckpt`) sizes itself from |Δwords|, not wall time: the legacy
    /// era has no keystroke record to scale by, and folding its multi-day
    /// wall-clock gap to a seam would collapse every tick onto the left edge
    /// (the reported bug). |Δwords| is the era's only honest measure of work,
    /// and keeps the word-quant law honest. Any OTHER long gap — checkpoint→run
    /// (the seam between the legacy era and today's session), run→run overnight,
    /// or a pass falling between two checkpoints — folds exactly as before.
    pub fn build(journal: &Journal, stations: &[StationSnap], now_ms: i64) -> Self {
        // `Ckpt(words)` carries the state's word count so a checkpoint-only
        // span can size itself; `Run` has a real extent; `Instant` (pass,
        // restore, card-close) only keeps its neighborhood unfolded.
        #[derive(Clone, Copy)]
        enum Kind {
            Run,
            Instant,
            Ckpt(usize),
        }
        let mut activity: Vec<(i64, i64, Kind)> = journal
            .runs
            .iter()
            .map(|r| (r.t0, r.t1.max(r.t0 + 1), Kind::Run))
            .collect();
        activity.extend(journal.events.iter().map(|e| (e.t(), e.t(), Kind::Instant)));
        activity.extend(
            stations
                .iter()
                .map(|s| (s.created_ms, s.created_ms, Kind::Ckpt(s.words))),
        );
        activity.sort_by_key(|a| a.0);
        let Some(first) = activity.first().copied() else {
            return Self {
                segs: Vec::new(),
                total_work: 0.,
                start_ms: now_ms,
                end_ms: now_ms,
            };
        };
        let mut segs: Vec<Seg> = Vec::with_capacity(activity.len() * 2);
        let start_ms = first.0;
        let mut work = 0.;
        let mut prev = start_ms;
        // `push` owns the only mutable borrow of `segs`, so every segment —
        // gap, run, checkpoint span, tail — goes through it; the span is
        // computed at the call site (time-, seam-, or word-derived).
        let mut push = |wall0: i64, wall1: i64, work_start: &mut f32, span: f32, folded: bool| {
            if span <= 0. {
                return;
            }
            segs.push(Seg {
                wall0,
                wall1,
                work0: *work_start,
                work1: *work_start + span,
                folded,
            });
            *work_start += span;
        };
        // The immediately-preceding item's checkpoint words, or None if it was
        // a run/instant — a `Ckpt → Ckpt` adjacency is a checkpoint-only span.
        // The first item has no gap before it (t0 == prev), so its initial
        // value is immaterial to any span; it is set as each item is consumed.
        let mut prev_ckpt: Option<usize> = None;
        for (t0, t1, kind) in activity {
            if t0 > prev {
                match (prev_ckpt, kind) {
                    (Some(pw), Kind::Ckpt(cw)) => {
                        // Checkpoint-only span: |Δwords| working px, floored so
                        // a zero-Δ twin still separates its ticks. Active (not
                        // folded) — it carries the envelope step and real shape.
                        let dw = (pw as i64 - cw as i64).unsigned_abs() as f32;
                        push(prev, t0, &mut work, (dw * CKPT_WORD_PX).max(CKPT_MIN_PX), false);
                    }
                    _ => {
                        let folded = t0 - prev > GAP_FOLD_MS;
                        let span = if folded {
                            seam_width(t0 - prev)
                        } else {
                            (t0 - prev) as f32 * PX_PER_MS
                        };
                        push(prev, t0, &mut work, span, folded);
                    }
                }
            }
            // A run's own span is floored so even a one-op run has an x-home;
            // a checkpoint/event instant contributes no width of its own.
            if let Kind::Run = kind {
                let end = t1.max(prev);
                let start = t0.max(prev);
                push(start, end.max(start), &mut work, ((end - start) as f32 * PX_PER_MS).max(MIN_RUN_PX), false);
                prev = end.max(prev);
            } else {
                prev = prev.max(t0);
            }
            prev_ckpt = match kind {
                Kind::Ckpt(w) => Some(w),
                _ => None,
            };
        }
        // Extend to the present so the rail's right end is "now".
        let end_ms = now_ms.max(prev);
        if end_ms > prev {
            let folded = end_ms - prev > GAP_FOLD_MS;
            let span = if folded {
                seam_width(end_ms - prev)
            } else {
                (end_ms - prev) as f32 * PX_PER_MS
            };
            push(prev, end_ms, &mut work, span, folded);
        }
        Self {
            segs,
            total_work: work,
            start_ms,
            end_ms,
        }
    }

    /// Working-px for a wall-clock instant (clamped to the timeline's extent).
    /// Binary search: `segs` is sorted, and the bake calls this once per run —
    /// a linear scan made the whole bake O(runs²), measurable at a year of
    /// history (wave-1 review, perf/high).
    pub fn work_at(&self, wall: i64) -> f32 {
        if self.segs.is_empty() {
            return 0.;
        }
        let wall = wall.clamp(self.start_ms, self.end_ms);
        let ix = self.segs.partition_point(|s| s.wall1 < wall);
        match self.segs.get(ix) {
            Some(s) => {
                let span = (s.wall1 - s.wall0).max(1) as f32;
                s.work0 + (wall - s.wall0).max(0) as f32 / span * (s.work1 - s.work0)
            }
            None => self.total_work,
        }
    }

    /// The inverse: wall-clock instant for a working-px position (clamped).
    /// Scrubbing across a folded seam jumps wall time fast — the folded gap by
    /// design. Binary search, same reasoning as `work_at`.
    pub fn wall_at(&self, work: f32) -> i64 {
        if self.segs.is_empty() {
            return self.start_ms;
        }
        let work = work.clamp(0., self.total_work);
        let ix = self.segs.partition_point(|s| s.work1 < work);
        match self.segs.get(ix) {
            Some(s) => {
                let span = (s.work1 - s.work0).max(f32::EPSILON);
                let frac = ((work - s.work0) / span).clamp(0., 1.);
                s.wall0 + (frac * (s.wall1 - s.wall0) as f32) as i64
            }
            None => self.end_ms,
        }
    }
}

// ---- Bake inputs (built by the editor, kept store/document free here) -------

/// A checkpoint reduced to what the strip draws (built once at open; the strip
/// never re-reads the store while scrubbing). `created_ms` is `created_unix ×
/// 1000` — the seconds→ms conversion done ONCE, at the boundary (the unit law,
/// review B11).
#[derive(Clone, Debug)]
pub struct StationSnap {
    pub created_ms: i64,
    pub name: String,
    pub manual: bool,
    /// A materialized state exists (so reconstruction may ANCHOR here). Every
    /// checkpoint this build writes is materialized; a legacy file's are
    /// backfilled. A station still DRAWS without one — it just can't be an
    /// anchor (avoids the truncation-to-empty lie of anchoring on a stateless
    /// checkpoint; review mid).
    pub has_state: bool,
    /// The materialized state's word count (0 when stateless). A checkpoint-only
    /// span's axis EXTENT is the |Δwords| between its two ends — the legacy era
    /// (empty journal, no keystroke record) still gets a real, work-proportional
    /// width instead of collapsing every tick onto the left edge (Bug A). Keeps
    /// the word-quant law honest: width is words, as the fabric's flecks are.
    pub words: usize,
    /// The materialized state's CHAR count (0 when stateless). The envelope's
    /// y-height is char-based (its scale, and the run era, are chars); feeding
    /// checkpoint char counts in keeps ONE continuous envelope across the merged
    /// axis — a step at each checkpoint, no discontinuity at the era seam.
    pub chars: usize,
}

/// A margin card's lifespan, for the thread it draws (design §1: a cool thread
/// from raised to resolved/dismissed; its length is how long the question
/// stayed open). Times in ms; `raised_ms` is the note's `created_unix × 1000`.
#[derive(Clone, Copy, Debug)]
pub struct CardSnap {
    pub raised_ms: i64,
    pub closed_ms: Option<i64>,
    /// The anchor's position in chars (today's text) — mapped through the
    /// envelope's own y-scale at bake, so the thread sits inside the page.
    pub anchor: usize,
    pub resolved: bool,
}

// ---- Baked geometry --------------------------------------------------------

/// A 2×2 amber grain. `x`/`y` are content-space (fabric-local: x is working px
/// from the fabric's left edge, y from the strip's top); the painter applies
/// the view offset and clamps to the visible window.
#[derive(Clone, Copy, Debug)]
pub struct Fleck {
    pub x: f32,
    pub y: f32,
    pub del: bool,
}

/// A cool column: the machine read here (a `Pass` event). Spans the page —
/// rail down to `y1`, the envelope's level at that moment — not the void.
#[derive(Clone, Copy, Debug)]
pub struct Veil {
    pub x: f32,
    pub y1: f32,
}

/// A card's open life as a 1-px cool thread; `sage` terminal when resolved,
/// grey when dismissed, `open` runs to the right edge (still open).
#[derive(Clone, Copy, Debug)]
pub struct Thread {
    pub x0: f32,
    pub x1: f32,
    pub y: f32,
    pub resolved: bool,
    pub open: bool,
}

/// A checkpoint tick + its (possibly omitted) label. `restore` draws it sage
/// with a dashed arc back to `arc_to` (the source station's x).
#[derive(Clone, Debug)]
pub struct Station {
    pub x: f32,
    pub label: String,
    pub rank: u8,
    pub restore: bool,
    pub arc_to: Option<f32>,
    /// Label placement, computed at bake (the stability law: never re-flowed
    /// mid-scrub). `row` stacks colliding labels; `flip_left` sets a
    /// near-right-edge label to the tick's left; `show` is the ranked-omission
    /// verdict (a dropped label keeps its tick).
    pub row: u8,
    pub flip_left: bool,
    pub show: bool,
    /// Wall time of the tick — the painter brightens it near the playhead.
    pub at_ms: i64,
}

/// A stepwise envelope vertex (document length over time): x = a run's right
/// edge, y hangs from the fabric top and steps down as the story grows.
#[derive(Clone, Copy, Debug)]
pub struct EnvPoint {
    pub x: f32,
    pub y: f32,
}

/// A folded-gap WELL (a >15 min break): a recessed full-height column over the
/// folded span — time away, given the presence a hairline never had.
#[derive(Clone, Copy, Debug)]
pub struct Seam {
    pub x: f32,
    pub w: f32,
}

/// A quiet date in the bottom lane ("Today" / "Tue 1 Jul").
#[derive(Clone, Debug)]
pub struct DateTick {
    pub x: f32,
    pub label: String,
    /// The sitting's first recorded moment: the date control seeks here.
    pub at_ms: i64,
    /// First/last run time, used only by the visible hover expansion.
    pub span_start_ms: i64,
    pub span_end_ms: i64,
}

/// The immutable view model (spec §1). Built once per open/Restore from
/// `(journal, checkpoints, cards)`; scrubbing NEVER rebuilds it — the rig
/// asserts the `bakes` counter, not this geometry (review B7/H35).
#[derive(Clone, Debug, Default)]
pub struct StripBake {
    pub timeline: Timeline,
    pub flecks: Vec<Fleck>,
    pub veils: Vec<Veil>,
    pub threads: Vec<Thread>,
    pub stations: Vec<Station>,
    pub envelope: Vec<EnvPoint>,
    pub seams: Vec<Seam>,
    pub dates: Vec<DateTick>,
    /// Materialized-checkpoint anchor times (ms), sorted — the reconstruction
    /// anchor is the latest ≤ pos_ms (the editor holds the states themselves).
    pub anchor_ms: Vec<i64>,
    /// Step destinations (ms), sorted+deduped: every station, both shoulders
    /// of every big cut/paste (the envelope's corners — "just before the
    /// damage" is where a rescue lands), the start, and now. Arrow keys walk
    /// this list while parked.
    pub snap_ms: Vec<i64>,
    pub now_ms: i64,
}

impl StripBake {
    /// Build the whole model. `seed_len` is the document's char length at
    /// journal start (from the earliest checkpoint ≤ the first run, else 0) so
    /// the envelope is seeded correctly for a doc that already had content when
    /// journaling began.
    pub fn build(
        journal: &Journal,
        stations: &[StationSnap],
        cards: &[CardSnap],
        seed_len: usize,
        now_ms: i64,
    ) -> Self {
        let timeline = Timeline::build(journal, stations, now_ms);

        // --- One rebased walk: runs and materialized checkpoints, merged in
        // time order. Checkpoint STATES are ground truth wherever they exist —
        // a restore's wholesale swap is journal-suppressed, so run deltas
        // alone drift after one, and the v1 build (which kept two independent
        // bookkeepings and merge-sorted their points) drew a one-px sawtooth
        // spike wherever they disagreed. Here the walk REBASES at each state
        // and accumulates run deltas between them: one envelope, one truth.
        enum Step<'a> {
            Run(usize, &'a EditRun),
            St(&'a StationSnap),
        }
        let mut walk: Vec<(i64, Step)> = journal
            .runs
            .iter()
            .enumerate()
            .map(|(i, r)| (r.t0, Step::Run(i, r)))
            .collect();
        walk.extend(
            stations
                .iter()
                .filter(|s| s.has_state)
                .map(|s| (s.created_ms, Step::St(s))),
        );
        walk.sort_by_key(|w| w.0);

        // --- Envelope y-scale, fixed ONCE (design §1), from the same walk ----
        let mut len: i64 = seed_len as i64;
        let mut max_len: i64 = len.max(1);
        for (_, step) in &walk {
            match step {
                Step::Run(_, r) => len = (len + r.delta_chars()).max(0),
                Step::St(s) => len = s.chars as i64,
            }
            max_len = max_len.max(len);
        }
        let scale = max_len as f32 * 1.1; // headroom for a restore past now-length
        let depth_y = |chars: i64| -> f32 { FAB_Y0 + (chars as f32 / scale) * FABRIC_H };

        // --- Flecks + envelope ------------------------------------------------
        let mut flecks: Vec<Fleck> = Vec::new();
        let mut envelope: Vec<EnvPoint> = Vec::new();
        len = seed_len as i64;
        for (_, step) in &walk {
            match step {
                Step::St(s) => {
                    // The rebase: a step to the state's own length. The legacy
                    // era's envelope is nothing but these (no keystroke record
                    // → no flecks fabricated); in a run era the step also
                    // corrects any suppressed-swap drift.
                    len = s.chars as i64;
                    envelope.push(EnvPoint {
                        x: timeline.work_at(s.created_ms),
                        y: depth_y(len),
                    });
                }
                Step::Run(i, run) => {
                    let x0 = timeline.work_at(run.t0);
                    let x1 = timeline.work_at(run.t1.max(run.t0 + 1)).max(x0 + MIN_RUN_PX);
                    let after = (len + run.delta_chars()).max(0);
                    // The run's text extent on the SAME chars axis the envelope
                    // uses — a grain lives inside the page it was typed into,
                    // and an append rides the growing edge. (v1 normalized by
                    // the instantaneous doc length over the full band height:
                    // every append painted at the band FLOOR, a dirt band the
                    // envelope never touched.)
                    let ins_chars = run.ins.chars().count();
                    let page_bot = depth_y(len.max(after)).max(FAB_Y0 + FLECK);
                    let pos = run.pos.min(len.max(after) as usize);
                    let y0 = depth_y(pos as i64).min(page_bot - FLECK);
                    let y1 = depth_y((pos + run.del_chars.max(ins_chars)) as i64).min(page_bot);
                    let ins = run.ins_words();
                    let (del, _exact) = run.deleted_words();
                    let n = (ins + del).min(FLECK_CAP);
                    for k in 0..n {
                        let (jx, jy) = jitter(*i as u64, k);
                        let (jb, _) = jitter((*i as u64) ^ 0x9E37_79B9, k);
                        // ±1.2 px of x-bleed across run adjacency: sub-pixel
                        // runs at metronome spacing otherwise alias into a
                        // picket fence that reads as machine output. ±1.2 px
                        // is ±36 s — texture moves, testimony doesn't (the
                        // count stays exact: one fleck, one word).
                        let x = (x0 + jx * (x1 - x0) + (jb - 0.5) * 2.4).max(0.);
                        let y = (y0 + jy * (y1 - y0).max(3.)).clamp(FAB_Y0, page_bot - FLECK);
                        flecks.push(Fleck {
                            x,
                            y,
                            del: k >= ins,
                        });
                    }
                    len = after;
                    envelope.push(EnvPoint {
                        x: x1,
                        y: depth_y(len),
                    });
                }
            }
        }
        // The x-bleed can nudge a grain past a neighbour; re-sort so the
        // painter's window (a partition_point + early break) stays honest.
        flecks.sort_by(|a, b| a.x.total_cmp(&b.x));

        // --- Veils (Pass events) & wells --------------------------------------
        // A veil spans the PAGE, not the void: rail down to the envelope as it
        // stood — the machine read the whole text, and only the text.
        let env_at = |x: f32| -> f32 {
            match envelope.partition_point(|p| p.x <= x).checked_sub(1) {
                Some(i) => envelope[i].y,
                None => envelope.first().map_or(FAB_Y0, |p| p.y),
            }
        };
        let mut veils: Vec<Veil> = Vec::new();
        for ev in &journal.events {
            if let JournalEvent::Pass { t, .. } = ev {
                let x = timeline.work_at(*t);
                veils.push(Veil {
                    x,
                    y1: env_at(x).max(FAB_Y0 + 8.),
                });
            }
        }
        let seams: Vec<Seam> = timeline
            .segs
            .iter()
            .filter(|s| s.folded)
            .map(|s| Seam {
                x: s.work0,
                w: s.work1 - s.work0,
            })
            .collect();

        // --- Threads (card lifespans) ----------------------------------------
        // Anchor depth on the chars axis (inside the page, like everything
        // else). `anchor` is the note's position in TODAY'S text — the
        // historical offset isn't recorded — an honest approximation that
        // stays within the page because pos ≤ len ≤ max.
        let threads: Vec<Thread> = cards
            .iter()
            .map(|c| {
                let x0 = timeline.work_at(c.raised_ms);
                let (x1, open) = match c.closed_ms {
                    Some(t) => (timeline.work_at(t).max(x0 + 1.), false),
                    None => (timeline.total_work, true),
                };
                Thread {
                    x0,
                    x1,
                    y: depth_y(c.anchor as i64).clamp(FAB_Y0 + 2., FAB_Y0 + FABRIC_H),
                    resolved: c.resolved,
                    open,
                }
            })
            .collect();

        // --- Stations (checkpoints) + Restore/Export ticks -------------------
        // Sitting seals remain durable replay anchors and arrow-step targets,
        // but are not marks: neither their tick nor their internal name may
        // reach the strip (v3 §1b). The birth station alone reads "Started"
        // once the record has distinguishable extent (v3 §1a).
        let first_ms = stations.iter().map(|s| s.created_ms).min();
        let mut baked: Vec<Station> = Vec::new();
        for st in stations {
            let session = st.name == "Session start" || st.name == "Session";
            let birth = Some(st.created_ms) == first_ms
                && (st.name == "Session start"
                    || st.name == "Started"
                    || st.name == "Fresh tutorial");
            if (session && !birth) || (birth && timeline.total_work <= 0.) {
                continue;
            }
            let label = if birth {
                "Started".into()
            } else {
                station_display(&st.name)
            };
            baked.push(Station {
                x: timeline.work_at(st.created_ms),
                label,
                rank: station_rank(&st.name, st.manual),
                restore: st.name == "Restored",
                arc_to: None,
                row: 0,
                flip_left: false,
                show: true,
                at_ms: st.created_ms,
            });
        }
        // Restore events → sage tick + dashed arc back to the source station
        // (the checkpoint the restore copied from, matched by `from_unix`).
        for ev in &journal.events {
            match ev {
                JournalEvent::Restore { t, from_unix, .. } => {
                    let src = stations
                        .iter()
                        .find(|s| s.created_ms == *from_unix * 1000)
                        .map(|s| timeline.work_at(s.created_ms));
                    baked.push(Station {
                        x: timeline.work_at(*t),
                        label: String::new(),
                        rank: RANK_RESTORE,
                        restore: true,
                        arc_to: src,
                        row: 0,
                        flip_left: false,
                        show: false, // the "Restored" checkpoint carries the label
                        at_ms: *t,
                    });
                }
                JournalEvent::Export { t } => {
                    baked.push(Station {
                        x: timeline.work_at(*t),
                        label: "Exported".into(),
                        rank: RANK_EXPORT,
                        restore: false,
                        arc_to: None,
                        row: 0,
                        flip_left: false,
                        show: true,
                        at_ms: *t,
                    });
                }
                _ => {}
            }
        }
        layout_labels(&mut baked, timeline.total_work);

        // --- Date lane (session-start days, thinned once) --------------------
        let dates = build_dates(&journal.runs, stations, &timeline, now_ms);

        let anchor_ms: Vec<i64> = {
            // ONLY materialized checkpoints are anchors — a stateless one can't
            // base a reconstruction (it would truncate to the empty doc).
            let mut v: Vec<i64> = stations
                .iter()
                .filter(|s| s.has_state)
                .map(|s| s.created_ms)
                .collect();
            v.sort_unstable();
            v
        };

        // Step destinations: stations, the shoulders of every ≥150-word run
        // (a big cut's corners), the two ends.
        let snap_ms: Vec<i64> = {
            let mut v: Vec<i64> = stations.iter().map(|s| s.created_ms).collect();
            for run in &journal.runs {
                if run.delta_chars().unsigned_abs() >= 800 {
                    v.push(run.t0);
                    v.push(run.t1.max(run.t0 + 1));
                }
            }
            v.push(timeline.start_ms);
            v.push(now_ms);
            v.sort_unstable();
            v.dedup();
            v
        };

        Self {
            timeline,
            flecks,
            veils,
            threads,
            stations: baked,
            envelope,
            seams,
            dates,
            anchor_ms,
            snap_ms,
            now_ms,
        }
    }
}

/// Dates for the bottom lane, one control per sitting. Its position and target
/// are the sitting's first run; its hover span is the first/last run.
fn build_dates(
    runs: &[EditRun],
    stations: &[StationSnap],
    timeline: &Timeline,
    now_ms: i64,
) -> Vec<DateTick> {
    let mut spans: Vec<(i64, i64)> = Vec::new();
    let mut prev_t1 = i64::MIN;
    for run in runs {
        if prev_t1 == i64::MIN || run.t0 - prev_t1 > GAP_FOLD_MS {
            spans.push((run.t0, run.t1));
        } else if let Some(span) = spans.last_mut() {
            span.1 = run.t1.max(span.1);
        }
        prev_t1 = run.t1;
    }
    // A legacy checkpoint-only era has no runs from which to prove a sitting
    // span. Its materialized moments remain usable date controls, without a
    // fabricated duration.
    if spans.is_empty() {
        spans.extend(stations.iter().map(|s| (s.created_ms, s.created_ms)));
    }
    spans
        .into_iter()
        .map(|(start, end)| DateTick {
            x: timeline.work_at(start),
            label: date_label(start / 1000, now_ms / 1000),
            at_ms: start,
            span_start_ms: start,
            span_end_ms: end,
        })
        .collect()
}

// ---- Ranked omission -------------------------------------------------------
// Lower rank wins a label collision (design §2): writer-named > seal >
// before-restore/restore > export > session-start > reflex. (A "manual" tier
// between export and session isn't needed — a manual checkpoint always carries
// a writer's own name, so it ranks as writer-named.)
const RANK_WRITER: u8 = 0;
const RANK_SEAL: u8 = 1;
const RANK_BEFORE_RESTORE: u8 = 2;
const RANK_RESTORE: u8 = 2;
const RANK_EXPORT: u8 = 3;
const RANK_SESSION: u8 = 5;
const RANK_REFLEX: u8 = 6;

/// The automatic checkpoint names Strop writes — everything else that is
/// `manual` is a writer's own title (the highest-ranked label).
fn station_rank(name: &str, manual: bool) -> u8 {
    if name == "Before restore" || name.starts_with("Before restoring") {
        RANK_BEFORE_RESTORE
    } else if name == "Restored" {
        RANK_RESTORE
    } else if name == "Exported" {
        RANK_EXPORT
    } else if name.contains("seal") || name == "Draft complete" {
        RANK_SEAL
    } else if name == "Session start"
        || name == "Session"
        || name == "Started"
        || name == "Fresh tutorial"
    {
        RANK_SESSION
    } else if name.starts_with("Checkpoint ") {
        RANK_REFLEX
    } else if manual {
        RANK_WRITER
    } else {
        RANK_REFLEX
    }
}

/// Reflex checkpoints are deliberately unnamed on the strip (bare ticks,
/// lowest rank — design §2), and so are session starts: the date lane already
/// says when a sitting began, and a lane full of "Session start" echoes was
/// the doubled-print smear. Everything else shows its own name.
fn station_display(name: &str) -> String {
    if name.starts_with("Checkpoint ") || name == "Session start" || name == "Session" {
        String::new()
    } else {
        name.to_owned()
    }
}

/// Approx label width in px at the strip's ~11px label font (monospaced
/// estimate — the exact shape is the painter's; the bake only needs collision
/// geometry, and a slight over-estimate errs toward omission, which is safe).
fn label_width(label: &str) -> f32 {
    label.chars().count() as f32 * 6.2 + 8.
}

/// Assign label rows and omit colliding lower-ranked labels — computed ONCE at
/// bake (the stability law: labels never re-rank or re-flow mid-scrub). Two
/// rows; a near-right-edge label flips to sit left of its tick.
fn layout_labels(stations: &mut [Station], total_work: f32) {
    // Higher-ranked labels claim their row first (so a writer's own name
    // evicts a colliding reflex tick's label). Ties break left→right.
    let mut order: Vec<usize> = (0..stations.len())
        .filter(|&i| stations[i].show && !stations[i].label.is_empty())
        .collect();
    // Drop the labels that are empty-by-name up front.
    for st in stations.iter_mut() {
        if st.label.is_empty() {
            st.show = false;
        }
    }
    order.sort_by(|&a, &b| {
        stations[a]
            .rank
            .cmp(&stations[b].rank)
            .then(stations[a].x.total_cmp(&stations[b].x))
    });
    // Each row records the occupied x-intervals already claimed on it;
    // `claimed` remembers who claimed them, for the same-name rule.
    let mut rows: [Vec<(f32, f32)>; 2] = [Vec::new(), Vec::new()];
    let mut claimed: Vec<(usize, f32, f32)> = Vec::new();
    for &i in &order {
        let w = label_width(&stations[i].label);
        // Near the right edge a label would overflow — flip it left of its
        // tick, but only when there IS a left to flip into: on a minutes-old
        // history every x "overflows" a 2px axis, and flipping threw the
        // young document's own "Started" off the world's left edge.
        let flip = stations[i].x + w > total_work + 4. && stations[i].x >= w + 8.;
        let (start, end) = if flip {
            (stations[i].x - w, stations[i].x)
        } else {
            (stations[i].x, stations[i].x + w)
        };
        // A same-named twin overlapping an already-placed label adds nothing:
        // omit it whole instead of stacking the lane with an echo (the
        // doubled-print smear). Different names still compete for row two.
        if claimed.iter().any(|&(j, s, e)| {
            stations[j].label == stations[i].label && start < e + 4. && s < end + 4.
        }) {
            stations[i].show = false;
            continue;
        }
        let mut placed = false;
        for (r, occ) in rows.iter_mut().enumerate() {
            let clear = occ.iter().all(|&(s, e)| end + 4. <= s || start >= e + 4.);
            if clear {
                occ.push((start, end));
                claimed.push((i, start, end));
                stations[i].row = r as u8;
                stations[i].flip_left = flip;
                placed = true;
                break;
            }
        }
        if !placed {
            // Both rows blocked at this x: omit the (lower-ranked) label; the
            // tick stays.
            stations[i].show = false;
        }
    }
}

// ---- Reconstruction anchor (the seconds/ms law) ----------------------------

/// Index into `anchor_ms` of the latest anchor with `t ≤ pos_ms` — the
/// reconstruction base (spec §1, review B11). `anchor_ms` is already in ms
/// (converted from `created_unix` seconds at the boundary), so this comparison
/// is ms-vs-ms; mixing units would silently anchor every scrub at the newest
/// checkpoint. `None` = no anchor ≤ t → the editor replays from the empty doc
/// at journal start (spec §3).
pub fn anchor_index(anchor_ms: &[i64], pos_ms: i64) -> Option<usize> {
    let i = anchor_ms.partition_point(|&t| t <= pos_ms);
    // `checked_sub`, not `(i>0).then_some(i-1)`: the latter EAGERLY evaluates
    // `i - 1` and underflows usize when nothing is ≤ t.
    i.checked_sub(1)
}

// ---- Readout & dates -------------------------------------------------------

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Civil (y, month 1-12, day, hour, minute, weekday 0=Mon) from unix seconds,
/// UTC (Howard Hinnant's algorithm — rough UI, matching editor::format_unix).
fn civil(secs: i64) -> (i64, u32, u32, u32, u32, usize) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    // 1970-01-01 was a Thursday (index 3 with Mon=0).
    let weekday = (days.rem_euclid(7) + 3).rem_euclid(7) as usize;
    (
        y,
        m as u32,
        d as u32,
        (rem / 3600) as u32,
        ((rem % 3600) / 60) as u32,
        weekday,
    )
}

/// Just the date/time half of the readout (spec §2): "Sun 5 Jul, 10:41" — the
/// year only when it isn't the current one (histories never expire). The
/// parked banner (Bug B) shows this as the moment label between stations.
pub fn format_moment(wall_ms: i64, now_ms: i64) -> String {
    let (y, m, d, hh, mm, wd) = civil(wall_ms / 1000);
    let (cur_y, ..) = civil(now_ms / 1000);
    let mon = MONTHS[(m - 1) as usize];
    if y == cur_y {
        format!("{} {d} {mon}, {hh:02}:{mm:02}", WEEKDAYS[wd])
    } else {
        format!("{} {d} {mon} {y}, {hh:02}:{mm:02}", WEEKDAYS[wd])
    }
}

/// The readout chip's text (spec §2): `{date} · {n} words`, tabular, NEVER a
/// sentence and NEVER a station name (P8's template ban).
pub fn format_readout(wall_ms: i64, words: usize, now_ms: i64) -> String {
    format!(
        "{} · {} words",
        format_moment(wall_ms, now_ms),
        group_thousands(words)
    )
}

/// A session-start date for the bottom lane (spec §1): "Today" / "Yesterday" /
/// "Tue 1 Jul" — real dates, never "day 12"; year when it isn't the current.
pub fn date_label(day_secs: i64, now_secs: i64) -> String {
    let day = day_secs.div_euclid(86_400);
    let today = now_secs.div_euclid(86_400);
    if day == today {
        return "Today".into();
    }
    if day == today - 1 {
        return "Yesterday".into();
    }
    let (y, m, d, _, _, wd) = civil(day_secs);
    let (cur_y, ..) = civil(now_secs);
    let mon = MONTHS[(m - 1) as usize];
    if y == cur_y {
        format!("{} {d} {mon}", WEEKDAYS[wd])
    } else {
        format!("{} {d} {mon} {y}", WEEKDAYS[wd])
    }
}

/// The date control's hover face: expand the existing visible label, never
/// add a second annotation (P9).
pub fn date_span_label(date: &DateTick) -> String {
    let (_, _, _, sh, sm, _) = civil(date.span_start_ms / 1000);
    let (_, _, _, eh, em, _) = civil(date.span_end_ms / 1000);
    format!("{}, {sh:02}:{sm:02}–{eh:02}:{em:02}", date.label)
}

/// 1234 → "1,234" (the titlebar's convention, mirrored so the readout at now
/// reads identically to the titlebar count).
fn group_thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Two deterministic floats in [0, 1) from a run index and a fleck index — the
/// fabric's jitter is a pure function of position, so it is byte-identical
/// across bakes (the rig can rely on it; nothing random ever moves).
fn jitter(seed: u64, i: usize) -> (f32, f32) {
    let mut h = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((i as u64).wrapping_mul(0xD1B5_4A32_D192_ED03))
        .wrapping_add(0x1234_5678);
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^= h >> 33;
    let a = ((h & 0xFFFF_FFFF) as f32) / (u32::MAX as f32);
    let b = ((h >> 32) as f32) / (u32::MAX as f32);
    (a, b)
}

// ---- Scrub state -----------------------------------------------------------

/// A reconstructed past document, cached across scrub frames. Rightward drags
/// advance `replay` incrementally; a leftward jump or a new anchor rebuilds it
/// (spec §2). Holds no `Store` — reconstruction NEVER journals (the scratch is
/// a bare `ReplayDoc`, so a scrub can never record phantom runs; review low).
pub struct ScrubDoc {
    pub replay: strop_core::journal::ReplayDoc,
    /// The anchor this replay is based on (ms) — a change re-anchors.
    pub anchor_ms: i64,
    /// The anchor's state, kept so a LEFTWARD drag within the same anchor
    /// rebuilds from this clone instead of re-reading (and re-parsing) the
    /// store's whole checkpoint list per mouse-move (wave-1 review,
    /// perf/high).
    pub anchor_state: (String, strop_core::document::SpanSet, strop_core::document::BlockMap),
}

/// The mutable half (spec §2). Separate from the bake by the stability law: a
/// scrub touches only these fields; the bake is frozen while the strip is open.
pub struct Strip {
    pub open: bool,
    /// Playhead position, wall-clock ms. Meaningful only while `open`.
    pub pos_ms: i64,
    /// A pinned second playhead (shift-click Compare); its delta folds into the
    /// readout's single line. A second pin-click clears it (review: every state
    /// needs an exit).
    pub pin_ms: Option<i64>,
    /// True once the writer has grabbed the thumb — the past is previewed, Now
    /// brightens and Restore appears. At now this is false.
    pub parked: bool,
    /// A drag is in flight (mousedown-hold): moves update the playhead.
    pub scrubbing: bool,
    /// The in-flight drag started in the FABRIC (direct touch on the cloth:
    /// moment-under-cursor mapping, view never yanks) rather than on the rail
    /// (fraction-of-the-whole seek, view locked to the thumb).
    pub scrub_fabric: bool,
    /// Sitting whose already-visible date is under the pointer.
    pub hover_date_ms: Option<i64>,
    /// Fabric horizontal pan (px) — auto-scroll keeps the playhead in view at
    /// novel scale; wheel pans it. NOT part of the bake (review B7).
    pub view_offset: f32,
    /// Session-monotonic bake counter — the stability-law assertion. Scrubbing
    /// must never bump it; only open and Restore do.
    pub bakes: u64,
    pub bake: Option<StripBake>,
    pub scratch: Option<ScrubDoc>,
    /// The readout's word count at `pos_ms` — tokenized from the reconstructed
    /// rope once per park (exact, cheap; review H30), not summed from run
    /// deltas.
    pub words_at: usize,
    /// The Compare pin's word count, computed once when the pin is set, so the
    /// readout's folded delta needs no per-frame reconstruction.
    pub pin_words: usize,
    /// The live document's selection, saved at park and restored at Now/close
    /// (its byte offsets mean nothing against a preview's text, and Esc must
    /// return the identical frame). A Restore drops it instead — the text
    /// changed for real.
    pub saved_sel: Option<std::ops::Range<usize>>,
}

impl Default for Strip {
    fn default() -> Self {
        Self {
            open: false,
            pos_ms: 0,
            pin_ms: None,
            parked: false,
            scrubbing: false,
            scrub_fabric: false,
            hover_date_ms: None,
            view_offset: 0.,
            bakes: 0,
            bake: None,
            scratch: None,
            words_at: 0,
            pin_words: 0,
            saved_sel: None,
        }
    }
}

impl Strip {
    /// Parked in the past (previewing) — the gate for Restore/Now styling, the
    /// banner, and the uniform read-only refusal (Bug B).
    pub fn is_parked(&self) -> bool {
        self.open && self.parked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strop_core::buffer::TextOp;
    use strop_core::journal::Journal;

    fn ins(pos: usize, text: &str) -> TextOp {
        TextOp {
            pos,
            delete: 0,
            insert: text.into(),
        }
    }

    // A fortnight fixture: three sittings a day apart, each a burst of runs.
    fn fixture() -> Journal {
        let mut j = Journal::default();
        let day = 86_400_000i64;
        let base = 1_700_000_000_000i64; // ms
        let mut pos = 0usize;
        for s in 0..3 {
            let t0 = base + s * day;
            for k in 0..20 {
                let t = t0 + k * 400; // <2s apart → coalesces within pauses
                let word = "word ";
                j.record(&ins(pos, word), t);
                pos += word.len();
            }
            j.settle();
        }
        j
    }

    #[test]
    fn timeline_folds_long_gaps_into_seams() {
        let j = fixture();
        let now = j.runs.last().unwrap().t1 + 1000;
        let tl = Timeline::build(&j, &[], now);
        // Three day-apart sittings ⇒ at least two folded seams.
        let seams = tl.segs.iter().filter(|s| s.folded).count();
        assert!(seams >= 2, "day gaps fold to seams, got {seams}");
        // Working time is a tiny fraction of the ~2-day wall span.
        let wall_ms = (now - tl.start_ms) as f32;
        assert!(
            tl.total_work < wall_ms * PX_PER_MS,
            "folding removes idle time from the axis"
        );
    }

    #[test]
    fn work_and_wall_maps_are_inverse_within_active_segments() {
        let j = fixture();
        let now = j.runs.last().unwrap().t1 + 1000;
        let tl = Timeline::build(&j, &[], now);
        // A point inside a run round-trips (folding makes it non-exact across
        // seams, but an active point is stable).
        let t = j.runs[1].t0;
        let w = tl.work_at(t);
        let back = tl.wall_at(w);
        assert!((back - t).abs() <= 2, "active point round-trips: {t} vs {back}");
    }

    #[test]
    fn anchor_selection_obeys_the_ms_law() {
        // created_unix in SECONDS ×1000 at the boundary; a naive seconds-vs-ms
        // compare would always pick the newest. Anchors at 1000s and 2000s.
        let anchors = vec![1_000_000i64, 2_000_000i64]; // already ms
        assert_eq!(anchor_index(&anchors, 1_500_000), Some(0));
        assert_eq!(anchor_index(&anchors, 2_500_000), Some(1));
        // Before any anchor → None (replay from empty doc).
        assert_eq!(anchor_index(&anchors, 500_000), None);
    }

    #[test]
    fn bake_is_deterministic_and_reproduces_flecks() {
        let j = fixture();
        let now = j.runs.last().unwrap().t1 + 1000;
        let a = StripBake::build(&j, &[], &[], 0, now);
        let b = StripBake::build(&j, &[], &[], 0, now);
        assert_eq!(a.flecks.len(), b.flecks.len());
        assert!(!a.flecks.is_empty(), "a fortnight of typing lays down flecks");
        // Byte-identical jitter across bakes (the stability rig relies on it).
        assert_eq!(a.flecks[0].x.to_bits(), b.flecks[0].x.to_bits());
        assert_eq!(a.flecks[0].y.to_bits(), b.flecks[0].y.to_bits());
    }

    #[test]
    fn envelope_grows_downward_with_the_story() {
        let j = fixture();
        let now = j.runs.last().unwrap().t1 + 1000;
        let bake = StripBake::build(&j, &[], &[], 0, now);
        // The story only grew here, so the envelope steps monotonically down.
        let ys: Vec<f32> = bake.envelope.iter().map(|p| p.y).collect();
        assert!(ys.windows(2).all(|w| w[1] >= w[0] - 0.01), "envelope hangs downward");
        assert!(*ys.last().unwrap() > ys[0], "it ends deeper than it began");
    }

    #[test]
    fn label_omission_drops_lower_rank_on_collision() {
        // Three stations clustered at the same x: a writer-named one and two
        // reflex ones. Two rows fit two labels; the third (lowest rank) drops.
        let mk = |x: f32, label: &str, rank: u8| Station {
            x,
            label: label.into(),
            rank,
            restore: false,
            arc_to: None,
            row: 0,
            flip_left: false,
            show: true,
            at_ms: 0,
        };
        let mut sts = vec![
            mk(100., "Draft two", RANK_WRITER),
            mk(101., "unnamed a", RANK_REFLEX),
            mk(102., "unnamed b", RANK_REFLEX),
        ];
        layout_labels(&mut sts, 1000.);
        assert!(sts[0].show, "the writer's own name survives");
        let shown = sts.iter().filter(|s| s.show).count();
        assert_eq!(shown, 2, "two rows fit two labels; the third is omitted");
    }

    #[test]
    fn readout_never_forms_a_sentence_or_names_a_station() {
        let now = 1_700_000_000_000i64;
        let s = format_readout(now, 3412, now);
        assert!(s.contains(" · "), "date · words, tabular");
        assert!(s.ends_with("words"));
        assert!(s.contains("3,412"));
        // No station name, no "after", no verb — the template ban.
        assert!(!s.to_lowercase().contains("after"));
    }

    #[test]
    fn date_label_speaks_relative_then_absolute() {
        let now = 1_700_000_000i64; // secs
        assert_eq!(date_label(now, now), "Today");
        assert_eq!(date_label(now - 86_400, now), "Yesterday");
        let older = date_label(now - 5 * 86_400, now);
        assert!(!older.contains("day"), "real dates, never 'day 12': {older}");
    }

    #[test]
    fn flecks_ride_the_envelope_not_the_floor() {
        // Pure drafting (appends at the end): the grains hug the growing edge.
        // The v1 mapping normalized by the instantaneous doc length over the
        // full band height, so every append painted at the band FLOOR — a
        // dirt band the page never touched.
        let j = fixture();
        let now = j.runs.last().unwrap().t1 + 1000;
        let bake = StripBake::build(&j, &[], &[], 0, now);
        let page_bottom = bake.envelope.last().unwrap().y;
        assert!(
            bake.flecks.iter().all(|f| f.y <= page_bottom + 0.01),
            "no grain below the deepest page edge"
        );
        // The stroke follows the envelope down as the story grows.
        let n = bake.flecks.len();
        assert!(n >= 20, "enough grains to compare ends: {n}");
        let avg = |fs: &[Fleck]| fs.iter().map(|f| f.y).sum::<f32>() / fs.len() as f32;
        assert!(
            avg(&bake.flecks[..n / 5]) + 20. < avg(&bake.flecks[n - n / 5..]),
            "early grains sit high, late grains ride deep"
        );
    }

    #[test]
    fn a_checkpoint_rebases_the_envelope_instead_of_spiking() {
        // A materialized checkpoint mid-run-era claiming a length the run
        // deltas never saw — the shape a journal-suppressed restore swap
        // leaves behind. v1 merge-sorted two bookkeepings into one polyline:
        // a one-px spike to the state's length and straight back. The rebased
        // walk steps to the truth and CONTINUES from it.
        let j = fixture();
        let mid = (j.runs[j.runs.len() / 2 - 1].t1 + j.runs[j.runs.len() / 2].t0) / 2;
        let st = vec![station(mid, "Restored", 700, 4000)];
        let now = j.runs.last().unwrap().t1 + 1000;
        let bake = StripBake::build(&j, &st, &[], 0, now);
        let sx = bake.timeline.work_at(mid);
        let step_y = bake.envelope.iter().find(|p| p.x >= sx).unwrap().y;
        for p in bake.envelope.iter().filter(|p| p.x > sx) {
            assert!(
                p.y >= step_y - 0.01,
                "no snap back to the stale sum after the rebase: {} vs {step_y}",
                p.y
            );
        }
    }

    #[test]
    fn wells_have_two_tiers() {
        let mut j = Journal::default();
        let day = 86_400_000i64;
        let base = 1_700_000_000_000i64;
        let mut pos = 0usize;
        for t0 in [base, base + day, base + 5 * day] {
            for k in 0..5i64 {
                j.record(&ins(pos, "word "), t0 + k * 400);
                pos += 5;
            }
            j.settle();
        }
        let tl = Timeline::build(&j, &[], base + 5 * day + 10_000);
        let widths: Vec<f32> = tl
            .segs
            .iter()
            .filter(|s| s.folded)
            .map(|s| s.work1 - s.work0)
            .collect();
        assert_eq!(widths.len(), 2, "two folded gaps: {widths:?}");
        assert_eq!(widths[0], SEAM_PX, "an overnight break is the thin well");
        assert_eq!(widths[1], SEAM_WIDE_PX, "days away earn the wide well");
    }

    #[test]
    fn session_marks_die_but_sitting_shoulders_remain_steps() {
        let day = 86_400_000i64;
        let base = 1_700_000_000_000i64;
        let stations = vec![
            station(base, "Session start", 10, 60),
            station(base + day, "Session start", 200, 1200),
            station(base + 2 * day, "Session start", 400, 2400),
        ];
        let j = Journal::default();
        let bake = StripBake::build(&j, &stations, &[], 0, base + 3 * day);
        assert_eq!(
            bake.stations.len(),
            1,
            "only the distinguishable birth remains"
        );
        assert_eq!(bake.stations[0].label, "Started", "the birth keeps a name");
        assert!(
            stations.iter().all(|st| bake.snap_ms.contains(&st.created_ms)),
            "invisible sitting boundaries remain arrow steps"
        );
        assert!(
            bake.stations.iter().all(|st| !st.label.to_lowercase().contains("session"))
        );
    }

    #[test]
    fn idle_session_name_never_reaches_the_bake() {
        let base = 1_700_000_000_000i64;
        let stations = vec![
            station(base, "Started", 10, 60),
            station(base + 20_000, "Session", 20, 120),
        ];
        let bake = StripBake::build(&Journal::default(), &stations, &[], 0, base + 30_000);
        assert!(bake.stations.iter().all(|st| st.at_ms != base + 20_000));
        assert!(
            bake.stations.iter().all(|st| !st.label.to_lowercase().contains("session"))
        );
    }

    #[test]
    fn duplicate_adjacent_labels_collapse_to_one() {
        let mk = |x: f32, label: &str| Station {
            x,
            label: label.into(),
            rank: RANK_WRITER,
            restore: false,
            arc_to: None,
            row: 0,
            flip_left: false,
            show: true,
            at_ms: 0,
        };
        let mut sts = vec![mk(100., "Draft complete"), mk(103., "Draft complete")];
        layout_labels(&mut sts, 1000.);
        let shown = sts.iter().filter(|s| s.show).count();
        assert_eq!(shown, 1, "the echo is omitted, not stacked into row two");
        // Distinct names at the same x still get the second row.
        let mut sts = vec![mk(100., "Draft complete"), mk(103., "Line pass done")];
        layout_labels(&mut sts, 1000.);
        assert_eq!(sts.iter().filter(|s| s.show).count(), 2);
    }

    #[test]
    fn veils_span_the_page_not_the_void() {
        use strop_core::journal::JournalEvent;
        let j = fixture();
        let t = j.runs.last().unwrap().t1 + 30_000;
        let j = Journal::from_parts(
            j.runs.clone(),
            vec![JournalEvent::Pass {
                t,
                mode: "developmental".into(),
                cards: 3,
            }],
        );
        let bake = StripBake::build(&j, &[], &[], 0, t + 1000);
        assert_eq!(bake.veils.len(), 1);
        let page = bake.envelope.last().unwrap().y;
        assert!(
            (bake.veils[0].y1 - page).abs() < 0.5,
            "the veil ends at the envelope, not the band floor: {} vs {page}",
            bake.veils[0].y1
        );
    }

    #[test]
    fn snap_points_cover_stations_and_big_cut_shoulders() {
        let day = 86_400_000i64;
        let base = 1_700_000_000_000i64;
        let mut j = Journal::default();
        let mut pos = 0usize;
        for k in 0..40i64 {
            j.record(&ins(pos, "word "), base + k * 400);
            pos += 5;
        }
        j.settle();
        // One big cut (≥ 800 chars) a day later — its shoulders become steps.
        let cut_t = base + day;
        j.record(
            &TextOp {
                pos: 20,
                delete: 900,
                insert: String::new(),
            },
            cut_t,
        );
        j.settle();
        let st = vec![station(base - day, "Started", 5, 30)];
        let now = cut_t + 100_000;
        let bake = StripBake::build(&j, &st, &[], 1000, now);
        assert!(bake.snap_ms.contains(&(base - day)), "stations are steps");
        assert!(
            bake.snap_ms.iter().any(|&t| (t - cut_t).abs() < 2),
            "the big cut's shoulder is a step: {:?}",
            bake.snap_ms
        );
        assert!(bake.snap_ms.contains(&now), "now is the last step");
        assert!(bake.snap_ms.windows(2).all(|w| w[0] < w[1]), "sorted, deduped");
    }

    #[test]
    fn empty_journal_bakes_without_panicking() {
        let j = Journal::default();
        let bake = StripBake::build(&j, &[], &[], 0, 1_700_000_000_000);
        assert!(bake.flecks.is_empty());
        assert!(bake.envelope.is_empty());
        assert_eq!(bake.timeline.total_work, 0.);
        assert!(bake.dates.is_empty(), "minute one has an empty date lane");
    }

    #[test]
    fn zero_extent_birth_station_lays_no_mark() {
        let now = 1_700_000_000_000;
        let birth = vec![station(now, "Session start", 0, 0)];
        let bake = StripBake::build(&Journal::default(), &birth, &[], 0, now);
        assert!(bake.stations.is_empty());
    }

    #[test]
    fn date_control_targets_the_sittings_first_recorded_moment() {
        let j = fixture();
        let now = j.runs.last().unwrap().t1 + 1000;
        let bake = StripBake::build(&j, &[], &[], 0, now);
        assert_eq!(bake.dates.len(), 3, "the fixture has three sittings");
        let starts: Vec<i64> = j
            .runs
            .iter()
            .enumerate()
            .filter_map(|(i, run)| {
                (i == 0 || run.t0 - j.runs[i - 1].t1 > GAP_FOLD_MS).then_some(run.t0)
            })
            .collect();
        for (date, first) in bake.dates.iter().zip(starts) {
            assert_eq!(date.at_ms, first);
            assert!(date.span_end_ms >= date.span_start_ms);
        }
        assert!(date_span_label(&bake.dates[0]).contains('–'));
    }

    // A materialized checkpoint snapshot for the merged-axis tests (Bug A).
    fn station(created_ms: i64, name: &str, words: usize, chars: usize) -> StationSnap {
        StationSnap {
            created_ms,
            name: name.into(),
            manual: true,
            has_state: true,
            words,
            chars,
        }
    }

    #[test]
    fn checkpoint_only_history_builds_a_real_axis() {
        // Six checkpoints two days apart, EMPTY journal — the legacy shape.
        // Before Bug A `total_work` was 0 and every tick landed at x=0.
        let day = 86_400_000i64;
        let base = 1_700_000_000_000i64;
        // Growing, with a dip at index 3 (a mid-arc cut).
        let counts = [90usize, 340, 720, 610, 1500, 2100];
        let stations: Vec<StationSnap> = counts
            .iter()
            .enumerate()
            .map(|(i, &w)| station(base + i as i64 * 2 * day, &format!("cp {i}"), w, w * 6))
            .collect();
        let j = Journal::default();
        let now = base + 13 * day;
        let bake = StripBake::build(&j, &stations, &[], 0, now);

        // A real axis, sized from |Δwords| — not the degenerate zero.
        assert!(bake.timeline.total_work > 0., "the checkpoint era has width");
        // No flecks: there is no keystroke record to fabricate quanta from.
        assert!(bake.flecks.is_empty(), "the legacy era lays no flecks");
        // Ticks spread monotonically along x, not stacked at the left edge.
        let xs: Vec<f32> = bake.stations.iter().map(|s| s.x).collect();
        assert!(xs.windows(2).all(|w| w[1] >= w[0]), "ticks spread in order: {xs:?}");
        assert!(*xs.last().unwrap() > 1., "the last tick is well right of x=0");
        assert!(
            xs.iter().filter(|&&x| x > 1.).count() >= 4,
            "ticks are genuinely spread, not overprinted: {xs:?}"
        );
        // The wider |Δwords| span earns more x than the narrow one — width is
        // work-proportional. |1500-610|=890 words vs |610-720|=110.
        let span_big = xs[4] - xs[3];
        let span_small = xs[3] - xs[2];
        assert!(span_big > span_small, "more words → more x: {span_big} vs {span_small}");

        // One envelope step per checkpoint, ordered along x, and its depth
        // tracks the state's length: the deepest step is the largest state,
        // and the mid-arc cut steps the envelope back up (shallower).
        assert_eq!(bake.envelope.len(), stations.len(), "one step per checkpoint");
        let ys: Vec<f32> = bake.envelope.iter().map(|p| p.y).collect();
        assert!(ys[2] > ys[0], "the story grew (deeper)");
        assert!(ys[3] < ys[2], "the mid-arc cut steps the envelope back up");
        assert_eq!(
            ys.last().copied(),
            ys.iter().copied().reduce(f32::max),
            "the final, largest draft is the deepest step"
        );
    }

    #[test]
    fn mixed_era_keeps_both_spans_and_runs_unchanged() {
        // A legacy checkpoint era, THEN today's journal runs (the fortnight
        // fixture sits around `base`; the checkpoints are a week earlier).
        let j = fixture();
        let day = 86_400_000i64;
        let base = 1_700_000_000_000i64;
        let stations = vec![
            station(base - 10 * day, "start", 100, 600),
            station(base - 6 * day, "half", 500, 3000),
            station(base - 3 * day, "most", 900, 5400),
        ];
        let now = j.runs.last().unwrap().t1 + 1000;
        let with = StripBake::build(&j, &stations, &[], 3000, now);
        let without = StripBake::build(&j, &[], &[], 3000, now);

        // The legacy span is ADDED in front of the runs era.
        assert!(
            with.timeline.total_work > without.timeline.total_work,
            "the checkpoint era adds width"
        );
        // The runs era is unchanged: the fleck walk never sees stations, so the
        // grain count is identical (positions shift by the era offset — fine).
        assert_eq!(with.flecks.len(), without.flecks.len(), "runs-era flecks unchanged");
        assert!(!with.flecks.is_empty(), "the runs era still lays flecks");
        // Both eras are present: a checkpoint tick sits left of the first run.
        let first_run_x = with.timeline.work_at(j.runs[0].t0);
        assert!(
            with.stations.iter().any(|s| s.x < first_run_x - 1.),
            "a checkpoint sits left of the runs era"
        );
        // The envelope carries steps from BOTH eras (3 checkpoints + the runs).
        assert!(
            with.envelope.len() > without.envelope.len(),
            "checkpoint steps join the run steps"
        );
    }
}
