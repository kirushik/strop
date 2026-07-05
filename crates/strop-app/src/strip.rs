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
// The band, top → bottom (design §1): a control row carrying the rail + thumb
// + readout/Now/Restore, a label lane for the two rows of station names, the
// fabric itself, and a thin date lane. The four sum to STRIP_H.
pub const STRIP_H: f32 = 196.;
pub const TOP_ROW_H: f32 = 30.;
pub const LABEL_LANE_H: f32 = 22.;
pub const FABRIC_H: f32 = 130.;
pub const DATE_LANE_H: f32 = 14.;
/// Fabric top, measured from the strip's own top edge: the envelope hangs from
/// here and the flecks fall below it.
pub const FAB_Y0: f32 = TOP_ROW_H + LABEL_LANE_H;
/// The rail line's y within the control row — the thumb rides it.
pub const RAIL_Y: f32 = 20.;
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
/// The working-time width a folded gap collapses to (a "seam"): enough to read
/// as a break, small enough that a fortnight of overnight gaps costs ~pixels.
pub const SEAM_PX: f32 = 10.;
/// Every run gets at least this much x, so even a one-op run has somewhere to
/// hang its flecks (a run is seconds long — below a pixel at the fixed scale —
/// so the density that reads as flow/deliberation is emergent from run
/// ADJACENCY along x, not intra-run spread; the review's fleck-cap concern is
/// moot here because the run's own x-extent is already sub-pixel).
pub const MIN_RUN_PX: f32 = 0.6;

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
pub const READOUT_CHIP: u32 = 0x111009;
/// The cream page-fill under the envelope and the envelope stroke itself
/// (design §1 — the corridor fix filled the rail→envelope band faint so it
/// reads as a page, not a floating line).
pub const CREAM: u32 = 0xE9E2D0;
pub const CREAM_FILL_ALPHA: f32 = 0.13;
pub const ENVELOPE_ALPHA: f32 = 0.9;
/// Cool veil for an AI pass (the machine read everything → a full-height
/// translucent column) and the cool thread for a card's open life.
pub const VEIL: u32 = 0x86B0E6;
pub const VEIL_ALPHA: f32 = 0.10;
pub const THREAD: u32 = 0x86B0E6;
/// Sage terminal dot for a resolved card / a restore tick (docs/color-language).
pub const SAGE: u32 = 0x7D8C66;
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

impl Timeline {
    /// Walk the journal's ACTIVITY in order — runs and event instants both —
    /// folding >15 min gaps, extending to `now_ms`. Events count as activity
    /// because a pass typically lands a lull AFTER the last keystroke: built
    /// from runs alone, its veil would fall inside the folded gap and paint
    /// collapsed onto the seam (found on the first real screenshot).
    pub fn build(journal: &Journal, now_ms: i64) -> Self {
        let mut activity: Vec<(i64, i64, bool)> = journal
            .runs
            .iter()
            .map(|r| (r.t0, r.t1.max(r.t0 + 1), true))
            .collect();
        activity.extend(journal.events.iter().map(|e| (e.t(), e.t(), false)));
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
        let mut push = |wall0: i64, wall1: i64, work_start: &mut f32, folded: bool, min: f32| {
            let span = if folded {
                SEAM_PX
            } else {
                ((wall1 - wall0) as f32 * PX_PER_MS).max(min)
            };
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
        for (t0, t1, is_run) in activity {
            if t0 > prev {
                let folded = t0 - prev > GAP_FOLD_MS;
                push(prev, t0, &mut work, folded, 0.);
            }
            // A run's own span is floored so even a one-op run has an x-home;
            // an event instant contributes no width of its own (the paint side
            // gives veils their 4px), it only keeps its neighborhood unfolded.
            if t1 > prev || is_run {
                let end = t1.max(prev);
                push(t0.max(prev), end.max(t0.max(prev)), &mut work, false, if is_run { MIN_RUN_PX } else { 0. });
                prev = end.max(prev);
            }
        }
        // Extend to the present so the rail's right end is "now".
        let end_ms = now_ms.max(prev);
        if end_ms > prev {
            let folded = end_ms - prev > GAP_FOLD_MS;
            push(prev, end_ms, &mut work, folded, 0.);
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
}

/// A margin card's lifespan, for the thread it draws (design §1: a cool thread
/// from raised to resolved/dismissed; its length is how long the question
/// stayed open). Times in ms; `raised_ms` is the note's `created_unix × 1000`.
#[derive(Clone, Copy, Debug)]
pub struct CardSnap {
    pub raised_ms: i64,
    pub closed_ms: Option<i64>,
    /// The anchor's depth in the document, 0 (top) .. 1 (bottom).
    pub depth: f32,
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

/// A cool full-height column: the machine read here (a `Pass` event).
#[derive(Clone, Copy, Debug)]
pub struct Veil {
    pub x: f32,
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

/// A folded-gap marker (a >15 min break): a hairline at the seam's x.
#[derive(Clone, Copy, Debug)]
pub struct Seam {
    pub x: f32,
}

/// A quiet date in the bottom lane ("Today" / "Tue 1 Jul").
#[derive(Clone, Debug)]
pub struct DateTick {
    pub x: f32,
    pub label: String,
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
        let timeline = Timeline::build(journal, now_ms);

        // --- Envelope y-scale, fixed ONCE (design §1) ------------------------
        // Cumulative length, seeded at the journal-start anchor length so the
        // envelope reflects the true document, not just journaled churn.
        let mut len: i64 = seed_len as i64;
        let mut max_len: i64 = len.max(1);
        for run in &journal.runs {
            len = (len + run.delta_chars()).max(0);
            max_len = max_len.max(len);
        }
        let scale = max_len as f32 * 1.1; // headroom for a restore past now-length
        let depth_y = |chars: i64| -> f32 { FAB_Y0 + (chars as f32 / scale) * FABRIC_H };

        // --- Flecks + envelope: one walk over the runs -----------------------
        let mut flecks: Vec<Fleck> = Vec::new();
        let mut envelope: Vec<EnvPoint> = Vec::new();
        len = seed_len as i64;
        for (i, run) in journal.runs.iter().enumerate() {
            let x0 = timeline.work_at(run.t0);
            let x1 = timeline.work_at(run.t1.max(run.t0 + 1)).max(x0 + MIN_RUN_PX);
            // Fleck depth: the edit's position within the document as it stood.
            let doc_depth = if len > 0 {
                (run.pos as f32 / len.max(1) as f32).clamp(0., 1.)
            } else {
                0.
            };
            let band_y = FAB_Y0 + doc_depth * FABRIC_H;
            let ins = run.ins_words();
            // Deleted words are estimated from del_chars (the text itself is
            // not stored — forward replay never needs it): ~5.5 chars/word.
            let del = (run.del_chars as f32 / 5.5).round() as usize;
            let n = (ins + del).min(FLECK_CAP);
            for k in 0..n {
                let (jx, jy) = jitter(i as u64, k);
                let x = x0 + jx * (x1 - x0);
                // Spread within a small vertical band around the edit's depth.
                let y = (band_y + (jy - 0.5) * 14.).clamp(FAB_Y0, FAB_Y0 + FABRIC_H - FLECK);
                flecks.push(Fleck {
                    x,
                    y,
                    del: k >= ins,
                });
            }
            len = (len + run.delta_chars()).max(0);
            envelope.push(EnvPoint {
                x: x1,
                y: depth_y(len),
            });
        }

        // --- Veils (Pass events) & seams -------------------------------------
        let mut veils: Vec<Veil> = Vec::new();
        for ev in &journal.events {
            if let JournalEvent::Pass { t, .. } = ev {
                veils.push(Veil {
                    x: timeline.work_at(*t),
                });
            }
        }
        let seams: Vec<Seam> = timeline
            .segs
            .iter()
            .filter(|s| s.folded)
            .map(|s| Seam { x: s.work0 })
            .collect();

        // --- Threads (card lifespans) ----------------------------------------
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
                    y: FAB_Y0 + c.depth.clamp(0., 1.) * FABRIC_H,
                    resolved: c.resolved,
                    open,
                }
            })
            .collect();

        // --- Stations (checkpoints) + Restore/Export ticks -------------------
        let mut baked: Vec<Station> = Vec::new();
        for st in stations {
            baked.push(Station {
                x: timeline.work_at(st.created_ms),
                label: station_display(&st.name),
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
        let dates = build_dates(&journal.runs, &timeline, now_ms);

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
            now_ms,
        }
    }
}

/// Session-start dates for the bottom lane, one per >15 min gap (a new sitting)
/// plus the first, de-duplicated to one label per calendar day.
fn build_dates(runs: &[EditRun], timeline: &Timeline, now_ms: i64) -> Vec<DateTick> {
    let mut dates: Vec<DateTick> = Vec::new();
    let mut last_day = i64::MIN;
    let mut prev_t1 = i64::MIN;
    for run in runs {
        let new_session = prev_t1 == i64::MIN || run.t0 - prev_t1 > GAP_FOLD_MS;
        let day = run.t0.div_euclid(86_400_000);
        if new_session && day != last_day {
            dates.push(DateTick {
                x: timeline.work_at(run.t0),
                label: date_label(run.t0 / 1000, now_ms / 1000),
            });
            last_day = day;
        }
        prev_t1 = run.t1;
    }
    dates
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
    } else if name == "Session start" || name == "Started" || name == "Fresh tutorial" {
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
/// lowest rank — design §2); everything else shows its own name.
fn station_display(name: &str) -> String {
    if name.starts_with("Checkpoint ") {
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
    // Each row records the occupied x-intervals already claimed on it.
    let mut rows: [Vec<(f32, f32)>; 2] = [Vec::new(), Vec::new()];
    for &i in &order {
        let w = label_width(&stations[i].label);
        // Near the right edge a label would overflow — flip it left of its tick.
        let flip = stations[i].x + w > total_work + 4.;
        let (start, end) = if flip {
            (stations[i].x - w, stations[i].x)
        } else {
            (stations[i].x, stations[i].x + w)
        };
        let mut placed = false;
        for (r, occ) in rows.iter_mut().enumerate() {
            let clear = occ.iter().all(|&(s, e)| end + 4. <= s || start >= e + 4.);
            if clear {
                occ.push((start, end));
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

/// The readout chip's text (spec §2): `{date} · {n} words`, tabular, NEVER a
/// sentence and NEVER a station name (P8's template ban). The year shows only
/// when it isn't the current one (design §2 — histories never expire).
pub fn format_readout(wall_ms: i64, words: usize, now_ms: i64) -> String {
    let (y, m, d, hh, mm, wd) = civil(wall_ms / 1000);
    let (cur_y, ..) = civil(now_ms / 1000);
    let mon = MONTHS[(m - 1) as usize];
    let date = if y == cur_y {
        format!("{} {d} {mon}, {hh:02}:{mm:02}", WEEKDAYS[wd])
    } else {
        format!("{} {d} {mon} {y}, {hh:02}:{mm:02}", WEEKDAYS[wd])
    };
    format!("{date} · {} words", group_thousands(words))
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
}

impl Default for Strip {
    fn default() -> Self {
        Self {
            open: false,
            pos_ms: 0,
            pin_ms: None,
            parked: false,
            scrubbing: false,
            view_offset: 0.,
            bakes: 0,
            bake: None,
            scratch: None,
            words_at: 0,
            pin_words: 0,
        }
    }
}

impl Strip {
    /// Parked in the past (previewing) — the gate for Restore/Now styling and
    /// for the restore-then-type notch.
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
        let tl = Timeline::build(&j, now);
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
        let tl = Timeline::build(&j, now);
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
    fn empty_journal_bakes_without_panicking() {
        let j = Journal::default();
        let bake = StripBake::build(&j, &[], &[], 0, 1_700_000_000_000);
        assert!(bake.flecks.is_empty());
        assert!(bake.envelope.is_empty());
        assert_eq!(bake.timeline.total_work, 0.);
    }
}
