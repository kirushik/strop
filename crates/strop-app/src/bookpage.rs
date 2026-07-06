//! The cold read's book-layout engine: tokenize → break → justify →
//! paginate, pure logic over an abstract measurer (and hyphenator) so the
//! whole pipeline unit-tests without gpui. Wave B paints the result.
//!
//! Spec: docs/impl/05-cold-read.md §2. The NORMATIVE engineering source is
//! docs/impl/cold-read/research-linebreak.md — §3.2 (greedy best-fit with
//! TeX-style badness), §4 (the parameter table; every constant below cites
//! its row), §5 (Russian rules), §6 (shaping pitfalls), §8 (the pipeline).
//! Corner rulings: docs/impl/cold-read/adjudications.md — F2 (the slice
//! string is never transformed), F9 (fragments carry whole-token ranges),
//! Regions 8–11 (paginator rules), S7 (measure shrink + ragged below the
//! justification floor), S8 (progress guarantee + relaxation order).

use std::collections::VecDeque;
use std::ops::Range;

use strop_core::document::{BlockKind, BlockMap, InlineAttr, SpanSet};

// ---- Parameters (research-linebreak §4, the spec table) -------------------

/// Word-space shrink floor: a justified gap never sets tighter than 0.80 ×
/// the nominal shaped space (InDesign's default minimum 80% ≈ Bringhurst's
/// M/5 for an M/4 space).
pub const SPACE_MIN: f32 = 0.80;
/// Preferred stretch ceiling, 1.33 × nominal (InDesign's default 133% ≈
/// TeX cmr stretch at badness 100).
pub const SPACE_MAX_PREFERRED: f32 = 1.33;
/// Acceptable stretch ceiling, 2.00 × nominal (Bringhurst's "reasonable
/// maximum" M/2). Beyond it the line still distributes the slack across
/// gaps — spaces only, letterspacing never — and is flagged `loose`
/// (the \emergencystretch analogue).
pub const SPACE_MAX_ACCEPTABLE: f32 = 2.00;
/// Per-gap stretch window: nominal × 0.33 (= SPACE_MAX_PREFERRED − 1).
pub const STRETCH_PER_GAP: f32 = 0.33;
/// Per-gap shrink window: nominal × 0.20 (= 1 − SPACE_MIN).
pub const SHRINK_PER_GAP: f32 = 0.20;
/// TeX badness saturates at 10000 ("awful"); ours does too, so penalties
/// stay comparable across pathological lines (research §3.2).
pub const BADNESS_AWFUL: f32 = 10_000.0;
/// \hyphenpenalty 50: a line ending in an INSERTED hyphen.
pub const HYPHEN_PENALTY: f32 = 50.0;
/// \exhyphenpenalty 50: a line ending at a PRE-EXISTING compound hyphen
/// («какой-» / “re-” of “re-entry”) — allowed, charged, nothing inserted.
pub const EXHYPHEN_PENALTY: f32 = 50.0;
/// Charged when a candidate would make the THIRD consecutive hyphen-ended
/// line (research §4: “2 preferred; 3 hard cap”). Larger than BADNESS_AWFUL
/// plus both hyphen penalties, so it only ever wins when no clean break
/// exists at all.
pub const HYPHEN_STREAK_PENALTY: f32 = 100_000.0;
/// Max consecutive hyphen-ended lines (the hard cap; see above).
pub const HYPHEN_STREAK_CAP: usize = 3;
/// Never hyphenate a word whose core is shorter than this many chars (CSS
/// Text 4 `hyphenate-limit-chars: auto` ≡ 5 2 2; the dictionaries enforce
/// their own edge minima — en-US 2/3, ru 2/2 — so the engine never does).
pub const HYPHEN_MIN_WORD: usize = 5;
/// Widow/orphan floor: ≥ 2 lines of a split paragraph on each side of a
/// page break (CSS initial value 2; Russian tradition bans висячие строки
/// outright). Relaxes to 1/1 on the S8 ladder.
pub const WIDOW_ORPHAN_MIN: usize = 2;
/// A heading keeps this many lines of its next block on its page
/// (standard book make-up rule; dropped last on the S8 ladder).
pub const KEEP_WITH_NEXT_LINES: usize = 2;
/// S8: below this page capacity (whole body lines) the relaxation ladder
/// engages — drop hyphen-avoidance → widow/orphan 1/1 → drop
/// keep-with-next; progress (≥ 1 line per page) is unconditional.
pub const RELAX_CAPACITY: usize = 8;
/// S7: the justification floor in characters of measure — below ~45 EN /
/// ~40 RU the block sets ragged-right, unhyphenated.
pub const JUSTIFY_FLOOR_EN_CHARS: f32 = 45.0;
pub const JUSTIFY_FLOOR_RU_CHARS: f32 = 40.0;
/// Blockquotes indent one em both sides (spec 05 §2.7).
pub const QUOTE_INDENT_EM: f32 = 1.0;
/// List text indents 1.5 em per nesting level; the marker hangs left of it.
pub const LIST_INDENT_EM: f32 = 1.5;
/// Gap between a list marker's right edge and its item text, in em.
pub const MARKER_GAP_EM: f32 = 0.5;
/// The deterministic box for an image whose dimensions are unknown — the
/// editor's own decoding/missing placeholder height (editor.rs); regions 12
/// wants the same input to paginate identically every time.
pub const IMAGE_PLACEHOLDER_HEIGHT: f32 = 56.0;

const EPS: f32 = 0.01;

// ---- The abstract oracles --------------------------------------------------

/// Inline style bits for one run of a fragment, derived from the slice's
/// spans. The paint layer maps (Role, Style) to a concrete font/size; the
/// engine only needs identity (width-cache keys and measurer dispatch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub underline: bool,
    pub highlight: bool,
    pub code: bool,
    pub link: bool,
    /// A footnote-ref span: Wave B superscripts these, numbered by ref
    /// order within the slice (regions 11).
    pub footnote: bool,
}

/// The block-derived face a line sets in: headings in Demi at ~1.15× body,
/// code in PT Mono, everything else the body face (spec 05 §2.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Body,
    Heading(u8),
    Code,
    /// A Divider block: an empty decorated line; Wave B paints the rule.
    Divider,
}

/// Width oracle over an abstract shaper. gpui's LineLayoutCache is
/// frame-scoped, so the real implementation (Wave B) keeps its own immortal
/// `(string, style) → width` cache (research §7); the tests use a fake.
pub trait Measure {
    /// Width of `text` shaped as ONE line under `role`, split into styled
    /// runs (`len` = byte length into `text`). Hyphenated prefixes arrive
    /// as the full painted string ("dif-"), never as parts (research §6).
    fn width(&mut self, text: &str, role: Role, runs: &[(usize, Style)]) -> f32;
    /// The shaped space advance for `style` under `role` — italic and bold
    /// spaces differ; never hardcoded (research §4, word-space row).
    fn space(&mut self, role: Role, style: Style) -> f32;
    /// Natural pixel size of an image asset when known; `None` yields the
    /// deterministic placeholder box (regions 12).
    fn image_size(&mut self, src: &str) -> Option<(f32, f32)>;
}

/// Hyphenation oracle: byte offsets into `word` (an alphabetic core, with
/// any author U+00AD still in place) where a break may fall. The real
/// router (hyphen.rs) guarantees char AND grapheme boundaries and applies
/// the F2 NFC-skip; `NoHyphen` is the missing-dictionary degradation.
pub trait Hyphenate {
    fn breaks(&mut self, word: &str) -> Vec<usize>;
}

/// Justify-without-hyphenation (the honest fallback arm).
pub struct NoHyphen;

impl Hyphenate for NoHyphen {
    fn breaks(&mut self, _: &str) -> Vec<usize> {
        vec![]
    }
}

// ---- The layout model -------------------------------------------------------

/// May a line end after this fragment? (spec 05 §2.1)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakClass {
    /// An ordinary inter-word gap follows: break freely.
    Free,
    /// The fragment ends at an internal compound hyphen («какой-»):
    /// breaking here is allowed, \exhyphenpenalty-charged, nothing inserted.
    AtHyphen,
    /// Glued to the next fragment: never break here (the word before a
    /// spaced «—» — тире never starts a line, research §5).
    Bound,
}

/// One positioned word-piece: the unit of shaping, painting and hit-testing.
#[derive(Debug, Clone, PartialEq)]
pub struct Frag {
    /// The painted string — soft hyphens stripped; a hyphenated prefix
    /// carries its inserted "-" (U+002D, never U+2010 — research §6).
    pub text: String,
    /// Styled runs over `text` (byte length, style): one shaping call each.
    pub runs: Vec<(usize, Style)>,
    /// Measured width of `text` under `runs`.
    pub width: f32,
    /// The nominal shaped space after this fragment (0 at paragraph end and
    /// inside split compounds — spaces are never painted, gaps are the
    /// justifier's arithmetic).
    pub space_after: f32,
    pub class: BreakClass,
    /// Slice-space char range of this fragment's own source chars (identical
    /// to document offsets modulo +base — F2).
    pub slice: Range<usize>,
    /// Slice-space char range of the whole whitespace-delimited token this
    /// fragment came from (F9): both halves of a hyphenated word share it;
    /// NBSP-joined tokens are one token; a word bound to a following «—»
    /// shares word+dash. Word-snap selection unions these.
    pub token: Range<usize>,
    /// Left edge within the page's text block, set by justification.
    pub x: f32,
}

impl Frag {
    fn end_style(&self) -> Style {
        self.runs.last().map(|r| r.1).unwrap_or_default()
    }
}

/// A list marker: pure decoration, no source range (N10 — dead to
/// selection).
#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    pub text: String,
    pub x: f32,
    pub width: f32,
}

/// One set line, positioned in page space (x/y relative to the text
/// block's top-left; the caller adds the page margins).
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub frags: Vec<Frag>,
    /// Final gap widths BETWEEN fragments (len = frags.len().saturating_sub(1)).
    pub gaps: Vec<f32>,
    pub role: Role,
    /// Left edge of the line (indent; plus the ragged-left shift for RTL).
    pub x: f32,
    /// Top of the line box in page space; set at pagination.
    pub y: f32,
    /// Whole-pixel line height.
    pub height: f32,
    pub justified: bool,
    /// Slack was distributed beyond the 2.0× acceptable window (debug flag).
    pub loose: bool,
    /// A word wider than the measure overflows left-aligned, never squeezed.
    pub overfull: bool,
    /// The line ends mid-word in a hyphen (inserted or compound) — the
    /// page-final-hyphen avoidance reads this.
    pub ends_hyphen: bool,
    /// Slice block (line) index this line belongs to.
    pub block: usize,
    /// Slice char position where this line starts (the resume anchor).
    pub anchor: usize,
    /// The paragraph contains RTL characters: set ragged-left, unjustified,
    /// unhyphenated — honest degradation (research §6).
    pub rtl: bool,
    pub marker: Option<Marker>,
}

/// One page item: a set line or an image box.
#[derive(Debug, Clone, PartialEq)]
pub enum PageItem {
    Line(Line),
    /// A deterministic image box (regions 12): the caller paints the asset
    /// or the missing-image degradation at exactly these bounds; the
    /// caption (one line, no source range — N10) sits under it.
    Image {
        block: usize,
        src: String,
        caption: String,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        anchor: usize,
    },
}

impl PageItem {
    /// Slice char anchor of this item (page-top-char resume).
    pub fn anchor(&self) -> usize {
        match self {
            PageItem::Line(l) => l.anchor,
            PageItem::Image { anchor, .. } => *anchor,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub items: Vec<PageItem>,
}

/// The paginated book: stable numbering, true total count, computed whole
/// at entry (research §7).
#[derive(Debug, Clone, PartialEq)]
pub struct BookLayout {
    pub pages: Vec<Page>,
}

impl BookLayout {
    /// First slice char on `page` — the resume anchor carried across a
    /// re-pagination (spec 05 §2.6).
    pub fn page_top_char(&self, page: usize) -> usize {
        self.pages
            .get(page)
            .and_then(|p| p.items.first())
            .map(PageItem::anchor)
            .unwrap_or(0)
    }

    /// The page containing slice char `ch` in THIS layout: reopen here
    /// after resize re-pagination (S9).
    pub fn page_of_char(&self, ch: usize) -> usize {
        let mut page = 0;
        for (i, p) in self.pages.iter().enumerate() {
            match p.items.first() {
                Some(item) if item.anchor() <= ch => page = i,
                Some(_) => break,
                None => {}
            }
        }
        page
    }
}

/// The engine's geometry parameters. The S7 width clause lives with the
/// caller — page width = min(570·scale, window), margins hold, the MEASURE
/// shrinks — the engine sees only the resulting measure and the caller's
/// justify verdict (`below_justification_floor`).
#[derive(Debug, Clone, PartialEq)]
pub struct BookMetrics {
    /// Text-block measure in px (450 at scale 1 — research-page §1.5).
    pub measure: f32,
    /// Body line height; rounded to whole px so pagination stays exact.
    pub line_height: f32,
    /// Heading line height (~1.15× body), rounded to whole px.
    pub heading_line_height: f32,
    /// Text-block height budget per page, px.
    pub page_height: f32,
    /// 1 em at body size (quote/list indents).
    pub em: f32,
    /// False below the justification floor (S7): the whole block sets
    /// ragged-right, unhyphenated.
    pub justify: bool,
}

/// S7: is `measure` too narrow to justify? Below ~45 EN / ~40 RU characters
/// even good hyphenation can't stop gappy word-spacing (research-page §1.1;
/// Bringhurst's advice is to go ragged). The caller passes the two average
/// advances measured from the actual face (URW Bookman ≈ 8.14 px EN /
/// 8.99 px RU at 16.5 px — research-page §1.1).
pub fn below_justification_floor(measure: f32, avg_en_char: f32, avg_ru_char: f32) -> bool {
    measure / avg_en_char < JUSTIFY_FLOOR_EN_CHARS
        || measure / avg_ru_char < JUSTIFY_FLOOR_RU_CHARS
}

// ---- Pipeline entry ----------------------------------------------------------

/// Paginate a manuscript slice triple (the output of `manuscript_slice_of` —
/// F1) into a book. Pure over the two oracles; same input → same layout,
/// bit for bit (the determinism gate).
pub fn paginate(
    text: &str,
    spans: &SpanSet,
    blocks: &BlockMap,
    metrics: &BookMetrics,
    m: &mut impl Measure,
    h: &mut impl Hyphenate,
) -> BookLayout {
    let m: &mut dyn Measure = m;
    let h: &mut dyn Hyphenate = h;
    // Whole-pixel line heights keep page arithmetic exact (spec 05 §3.2).
    let body_h = metrics.line_height.round().max(1.0);
    let head_h = metrics.heading_line_height.round().max(1.0);
    let rope = ropey::Rope::from_str(text);
    let em = metrics.em;
    let mut flows: Vec<Flow> = Vec::new();
    let mut counters: Vec<usize> = Vec::new(); // ordered-list numbering per depth
    for i in 0..rope.len_lines() {
        let kind = blocks.kind(i).clone();
        let cstart = rope.line_to_char(i);
        let mut ptext = rope.line(i).to_string();
        while ptext.ends_with('\n') || ptext.ends_with('\r') {
            ptext.pop();
        }
        if !matches!(kind, BlockKind::ListItem { .. }) {
            counters.clear();
        }
        match &kind {
            // Definitions stay off-page (v1 law; regions 11): the paginator
            // skips the block entirely — refs superscript at paint.
            BlockKind::FootnoteDef { .. } => continue,
            BlockKind::Image { src, caption, .. } => {
                // Deterministic box: natural size when the measurer knows
                // it, the editor's decoding placeholder otherwise
                // (regions 12). Scale to the measure; move-or-scale onto a
                // page is the assembler's call.
                let (w, hpx) = match m.image_size(src) {
                    Some((nw, nh)) if nw > 0.0 && nh > 0.0 => {
                        let w = nw.min(metrics.measure);
                        (w, nh * (w / nw))
                    }
                    _ => (metrics.measure, IMAGE_PLACEHOLDER_HEIGHT),
                };
                flows.push(Flow::Image(ImageFlow {
                    src: src.clone(),
                    caption: caption.clone(),
                    w,
                    h: hpx,
                    caption_h: if caption.trim().is_empty() { 0.0 } else { body_h },
                    block: i,
                    anchor: cstart,
                }));
                continue;
            }
            BlockKind::Divider => {
                flows.push(Flow::Para {
                    lines: vec![blank_line(Role::Divider, body_h, i, cstart)],
                    heading: false,
                });
                continue;
            }
            _ => {}
        }
        let (role, indent, rindent, mode0, line_h) = match &kind {
            // Headings set ragged: a justified two-word heading is a hole.
            BlockKind::Heading(l) => (Role::Heading(*l), 0.0, 0.0, Mode::Ragged, head_h),
            BlockKind::Blockquote => (
                Role::Body,
                em * QUOTE_INDENT_EM,
                em * QUOTE_INDENT_EM,
                Mode::Justified,
                body_h,
            ),
            // Code sets ragged and never hyphenates (spec 05 §2.7).
            BlockKind::CodeBlock { .. } => (Role::Code, 0.0, 0.0, Mode::Ragged, body_h),
            BlockKind::ListItem { depth, .. } => (
                Role::Body,
                em * LIST_INDENT_EM * (f32::from(*depth) + 1.0),
                0.0,
                Mode::Justified,
                body_h,
            ),
            _ => (Role::Body, 0.0, 0.0, Mode::Justified, body_h),
        };
        let chars: Vec<char> = ptext.chars().collect();
        if chars.iter().all(|c| c.is_whitespace()) {
            // Blank in-slice paragraphs paginate as blank lines (regions 4).
            flows.push(Flow::Para {
                lines: vec![blank_line(role, line_h, i, cstart)],
                heading: false,
            });
            continue;
        }
        // Per-char inline styles from the slice's spans. The slice STRING is
        // never transformed (F2) — styles are looked up beside it.
        let cend = cstart + chars.len();
        let mut styles = vec![Style::default(); chars.len()];
        for s in spans.spans() {
            let a = s.range.start.max(cstart);
            let b = s.range.end.min(cend);
            if a < b {
                for st in &mut styles[a - cstart..b - cstart] {
                    apply_attr(st, &s.attr);
                }
            }
        }
        let rtl = chars.iter().copied().any(is_rtl);
        let mode = if rtl {
            Mode::RaggedLeft
        } else if !metrics.justify {
            // S7: the whole block drops to ragged-right below the floor.
            Mode::Ragged
        } else {
            mode0
        };
        let marker = match &kind {
            BlockKind::ListItem { ordered, depth } => {
                let d = usize::from(*depth);
                if counters.len() <= d {
                    counters.resize(d + 1, 0);
                }
                counters.truncate(d + 1);
                let mtext = if *ordered {
                    counters[d] += 1;
                    format!("{}.", counters[d])
                } else {
                    "•".to_owned()
                };
                let mw = m.width(&mtext, Role::Body, &[(mtext.len(), Style::default())]);
                let mx = (indent - MARKER_GAP_EM * em - mw).max(0.0);
                Some(Marker { text: mtext, x: mx, width: mw })
            }
            _ => None,
        };
        let ctx = ParaCtx { chars, styles, start: cstart };
        let frags = tokenize(&ctx, m, role);
        let avail = (metrics.measure - indent - rindent).max(1.0);
        let broken = break_para(&ctx, frags, avail, mode, role, m, h);
        let n = broken.len();
        let params = LineParams {
            avail,
            indent,
            mode,
            role,
            height: line_h,
            block: i,
            rtl,
        };
        let mut lines: Vec<Line> = broken
            .into_iter()
            .enumerate()
            .map(|(k, bl)| finish_line(bl, k + 1 == n, &params))
            .collect();
        if let Some(mk) = marker
            && let Some(first) = lines.first_mut()
        {
            first.marker = Some(mk);
        }
        flows.push(Flow::Para {
            lines,
            heading: matches!(kind, BlockKind::Heading(_)),
        });
    }
    assemble(flows, metrics, body_h)
}

// ---- Tokenizer -----------------------------------------------------------------

struct ParaCtx {
    chars: Vec<char>,
    styles: Vec<Style>,
    /// Slice char offset of `chars[0]`.
    start: usize,
}

/// U+00A0 never splits — the one rule that lets authors bind whatever they
/// care about (research §5).
fn is_split_ws(c: char) -> bool {
    c.is_whitespace() && c != '\u{A0}'
}

fn is_rtl(c: char) -> bool {
    matches!(c,
        // Hebrew, Arabic, Syriac, Thaana, NKo, Samaritan, Mandaic, Arabic Ext-A
        '\u{0590}'..='\u{08FF}'
        // Presentation forms A and B
        | '\u{FB1D}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFF}')
}

fn apply_attr(s: &mut Style, attr: &InlineAttr) {
    match attr {
        InlineAttr::Strong => s.bold = true,
        InlineAttr::Emphasis => s.italic = true,
        InlineAttr::Strikethrough => s.strike = true,
        InlineAttr::Underline => s.underline = true,
        InlineAttr::Highlight => s.highlight = true,
        InlineAttr::Code => s.code = true,
        InlineAttr::Link(_) => s.link = true,
        InlineAttr::FootnoteRef(_) => s.footnote = true,
    }
}

fn tokenize(ctx: &ParaCtx, m: &mut dyn Measure, role: Role) -> Vec<Frag> {
    let mut frags: Vec<Frag> = Vec::new();
    let n = ctx.chars.len();
    let mut i = 0;
    while i < n {
        if is_split_ws(ctx.chars[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < n && !is_split_ws(ctx.chars[i]) {
            i += 1;
        }
        push_token(&mut frags, ctx, start..i, m, role);
    }
    // Spaces are never painted: each fragment carries its nominal following
    // gap instead — 0 at paragraph end, 0 inside a split compound (the
    // halves are contiguous in the source).
    let count = frags.len();
    for (k, f) in frags.iter_mut().enumerate() {
        f.space_after = if k + 1 == count || f.class == BreakClass::AtHyphen {
            0.0
        } else {
            m.space(role, f.end_style())
        };
    }
    frags
}

fn push_token(
    frags: &mut Vec<Frag>,
    ctx: &ParaCtx,
    tok: Range<usize>,
    m: &mut dyn Measure,
    role: Role,
) {
    let chars = &ctx.chars[tok.clone()];
    // A lone «—» binds to the preceding word — тире never starts a line
    // (research §5: gramota.ru; Мильчин). Both fragments share the merged
    // token range word..dash (F9), so word-snap selects them as one; a
    // paragraph-initial dash (dialogue) has nothing to bind to and passes.
    if chars == ['—'] && !frags.is_empty() {
        let prev_tok = frags.last().unwrap().token.clone();
        let union = prev_tok.start..ctx.start + tok.end;
        for f in frags.iter_mut().rev() {
            if f.token == prev_tok {
                f.token = union.clone();
            } else {
                break;
            }
        }
        frags.last_mut().unwrap().class = BreakClass::Bound;
        if let Some(mut dash) = make_frag(ctx, tok, BreakClass::Free, false, m, role) {
            dash.token = union;
            frags.push(dash);
        }
        return;
    }
    // Digit-bearing tokens never hyphenate and their internal hyphens are
    // not break opportunities (Мильчин's rule, research §1.2: «1-го»,
    // «40%-ный», v0.8.4).
    if chars.iter().any(|c| c.is_numeric()) {
        if let Some(f) = make_frag(ctx, tok, BreakClass::Free, false, m, role) {
            frags.push(f);
        }
        return;
    }
    // Internal-hyphen compounds split at the hyphen: break allowed there,
    // \exhyphenpenalty-charged, nothing inserted («какой-то»); the hyphen
    // stays with the first half. Apostrophe words carry no '-' and pass
    // whole. Every part keeps the WHOLE token's char range (F9).
    let mut parts: Vec<Range<usize>> = Vec::new();
    let mut part = tok.start;
    for k in tok.clone() {
        if ctx.chars[k] == '-' && k > part && k + 1 < tok.end && ctx.chars[k + 1] != '-' {
            parts.push(part..k + 1);
            part = k + 1;
        }
    }
    parts.push(part..tok.end);
    let last = parts.len() - 1;
    for (pi, pr) in parts.into_iter().enumerate() {
        let class = if pi < last { BreakClass::AtHyphen } else { BreakClass::Free };
        if let Some(mut f) = make_frag(ctx, pr, class, false, m, role) {
            f.token = ctx.start + tok.start..ctx.start + tok.end;
            frags.push(f);
        }
    }
}

/// Build one fragment over `local` chars: painted text (soft hyphens are
/// break marks, never painted — research §6), style runs grouped per char,
/// width measured on the final string. `hyphen` appends the inserted "-"
/// and re-measures the WHOLE prefix — never width(prefix)+width('-'), which
/// breaks ligatures and edge kerning (research §6).
fn make_frag(
    ctx: &ParaCtx,
    local: Range<usize>,
    class: BreakClass,
    hyphen: bool,
    m: &mut dyn Measure,
    role: Role,
) -> Option<Frag> {
    let mut text = String::new();
    let mut runs: Vec<(usize, Style)> = Vec::new();
    for k in local.clone() {
        let c = ctx.chars[k];
        if c == '\u{AD}' {
            continue;
        }
        let st = ctx.styles[k];
        let len = c.len_utf8();
        match runs.last_mut() {
            Some((l, s)) if *s == st => *l += len,
            _ => runs.push((len, st)),
        }
        text.push(c);
    }
    if text.is_empty() && !hyphen {
        return None;
    }
    if hyphen {
        text.push('-');
        match runs.last_mut() {
            Some((l, _)) => *l += 1,
            None => runs.push((1, Style::default())),
        }
    }
    let width = m.width(&text, role, &runs);
    let slice = ctx.start + local.start..ctx.start + local.end;
    Some(Frag {
        text,
        runs,
        width,
        space_after: 0.0,
        class,
        token: slice.clone(),
        slice,
        x: 0.0,
    })
}

// ---- The breaker (research §3.2: greedy best-fit with badness) -----------------

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Justified,
    /// Ragged-right: code blocks, headings, and the S7 below-floor arm.
    Ragged,
    /// Ragged-left for RTL paragraphs (honest degradation, research §6).
    RaggedLeft,
}

struct BrokenLine {
    frags: Vec<Frag>,
    ends_hyphen: bool,
    overfull: bool,
}

/// TeX-style badness: r over the real stretch/shrink windows, 100·|r|³,
/// saturated at BADNESS_AWFUL (research §3.2).
fn badness(widths: f32, gaps: f32, avail: f32) -> f32 {
    let slack = avail - (widths + gaps);
    let window = if slack >= 0.0 { gaps * STRETCH_PER_GAP } else { gaps * SHRINK_PER_GAP };
    if window <= 0.0 {
        return if slack.abs() < EPS { 0.0 } else { BADNESS_AWFUL };
    }
    let r = (slack / window).abs();
    (100.0 * r * r * r).min(BADNESS_AWFUL)
}

/// The consecutive-hyphen charge. `forced` marks the empty-line case (a
/// word wider than the measure): there the only alternative is an overfull
/// overflow, so the cap yields — LARGE-charged, never forbidden.
fn streak_charge(streak: usize, ends_hyphen: bool, forced: bool) -> Option<f32> {
    if !ends_hyphen {
        return Some(0.0);
    }
    if streak + 1 > HYPHEN_STREAK_CAP && !forced {
        return None;
    }
    if streak + 1 >= HYPHEN_STREAK_CAP {
        return Some(HYPHEN_STREAK_PENALTY);
    }
    Some(0.0)
}

/// Strip leading/trailing punctuation, then ask the hyphenator for the
/// alphabetic core; offsets are shifted back into `word` (research §1.2).
fn hyphen_candidates(word: &str, h: &mut dyn Hyphenate) -> Vec<usize> {
    // Digit-bearing tokens never hyphenate (research §1.2) — checked on the
    // WHOLE token, so «1 км»-style NBSP joins stay guarded too.
    if word.chars().any(char::is_numeric) {
        return vec![];
    }
    let Some(start) = word
        .char_indices()
        .find(|&(_, c)| c.is_alphabetic())
        .map(|(i, _)| i)
    else {
        return vec![];
    };
    let end = word
        .char_indices()
        .rev()
        .find(|&(_, c)| c.is_alphabetic())
        .map(|(i, c)| i + c.len_utf8())
        .unwrap();
    let core = &word[start..end];
    if core.chars().count() < HYPHEN_MIN_WORD {
        return vec![];
    }
    h.breaks(core)
        .into_iter()
        .filter(|&b| b > 0 && b < core.len() && core.is_char_boundary(b))
        .map(|b| b + start)
        .collect()
}

/// Split a word fragment at `byte` (an offset into `word`, its source-char
/// string): "prefix-" re-shaped whole + the remainder, both carrying the
/// original token range (F9).
fn split_at(
    ctx: &ParaCtx,
    frag: &Frag,
    word: &str,
    byte: usize,
    m: &mut dyn Measure,
    role: Role,
) -> Option<(Frag, Frag)> {
    let cut = word[..byte].chars().count();
    let local = frag.slice.start - ctx.start..frag.slice.end - ctx.start;
    let at = local.start + cut;
    let mut prefix = make_frag(ctx, local.start..at, BreakClass::Free, true, m, role)?;
    let mut rest = make_frag(ctx, at..local.end, frag.class, false, m, role)?;
    prefix.token = frag.token.clone();
    rest.token = frag.token.clone();
    rest.space_after = frag.space_after;
    Some((prefix, rest))
}

fn break_para(
    ctx: &ParaCtx,
    frags: Vec<Frag>,
    avail: f32,
    mode: Mode,
    role: Role,
    m: &mut dyn Measure,
    h: &mut dyn Hyphenate,
) -> Vec<BrokenLine> {
    // Bound-glued fragments form unbreakable units («слово —» — the
    // tokenizer's only Bound source). Hyphenation inside a bound group is
    // skipped: conservative, and the dash never starts a line either way.
    let mut units: VecDeque<Vec<Frag>> = VecDeque::new();
    let mut glue = false;
    for f in frags {
        let next_glue = f.class == BreakClass::Bound;
        if glue {
            units.back_mut().unwrap().push(f);
        } else {
            units.push_back(vec![f]);
        }
        glue = next_glue;
    }
    // The paragraph's last word never hyphenates (research §4).
    let last_token = units.back().and_then(|u| u.last()).map(|f| f.token.clone());
    let hyphenating = mode == Mode::Justified;
    let min_factor = if hyphenating { SPACE_MIN } else { 1.0 };

    let mut lines: Vec<BrokenLine> = Vec::new();
    let mut cur: Vec<Frag> = Vec::new();
    let (mut cur_w, mut cur_g) = (0.0f32, 0.0f32);
    let mut streak = 0usize;

    while let Some(unit) = units.pop_front() {
        let unit_w: f32 = unit.iter().map(|f| f.width).sum();
        let unit_g: f32 = unit.iter().take(unit.len() - 1).map(|f| f.space_after).sum();
        let join = if cur.is_empty() { 0.0 } else { cur.last().unwrap().space_after };
        if cur_w + unit_w + (cur_g + join + unit_g) * min_factor <= avail + EPS {
            cur_w += unit_w;
            cur_g += join + unit_g;
            cur.extend(unit);
            continue;
        }
        // The unit straddles the line end: enumerate the candidates —
        // break before it, or break at each hyphenation point of the
        // straddling word, looked up ON DEMAND only now (spec 05 §2.2).
        let mut best_cost = f32::INFINITY;
        let mut best: Option<Option<(Frag, Frag)>> = None;
        if !cur.is_empty() {
            let eh = cur.last().unwrap().class == BreakClass::AtHyphen;
            let mut cost = badness(cur_w, cur_g, avail);
            if eh {
                cost += EXHYPHEN_PENALTY;
            }
            if let Some(extra) = streak_charge(streak, eh, false) {
                best_cost = cost + extra;
                best = Some(None);
            }
        }
        if hyphenating && unit.len() == 1 && last_token.as_ref() != Some(&unit[0].token) {
            let f = &unit[0];
            let local = f.slice.start - ctx.start..f.slice.end - ctx.start;
            let word: String = ctx.chars[local].iter().collect();
            for b in hyphen_candidates(&word, h) {
                let Some((prefix, rest)) = split_at(ctx, f, &word, b, m, role) else {
                    continue;
                };
                let w = cur_w + prefix.width;
                let g = cur_g + join;
                if w + g * min_factor > avail + EPS {
                    continue; // even at minimum spacing the prefix misses
                }
                let Some(extra) = streak_charge(streak, true, cur.is_empty()) else {
                    continue;
                };
                let cost = badness(w, g, avail) + HYPHEN_PENALTY + extra;
                if cost < best_cost {
                    best_cost = cost;
                    best = Some(Some((prefix, rest)));
                }
            }
        }
        match best {
            Some(None) => {
                let eh = cur.last().unwrap().class == BreakClass::AtHyphen;
                lines.push(BrokenLine {
                    frags: std::mem::take(&mut cur),
                    ends_hyphen: eh,
                    overfull: false,
                });
                streak = if eh { streak + 1 } else { 0 };
                (cur_w, cur_g) = (0.0, 0.0);
                units.push_front(unit);
            }
            Some(Some((prefix, rest))) => {
                cur.push(prefix);
                lines.push(BrokenLine {
                    frags: std::mem::take(&mut cur),
                    ends_hyphen: true,
                    overfull: false,
                });
                streak += 1;
                (cur_w, cur_g) = (0.0, 0.0);
                let mut u = unit;
                u[0] = rest; // unit.len() == 1 on this path
                units.push_front(u);
            }
            None if cur.is_empty() => {
                // A word wider than the measure with no usable break: paint
                // at natural width, left-aligned, overflowing — never
                // squeezed, never letterspaced (research §4, overfull row).
                let eh = unit.last().unwrap().class == BreakClass::AtHyphen;
                lines.push(BrokenLine { frags: unit, ends_hyphen: eh, overfull: true });
                streak = if eh { streak + 1 } else { 0 };
            }
            None => {
                // Every candidate was streak-forbidden: break before the
                // unit anyway — progress beats the cap.
                let eh = cur.last().unwrap().class == BreakClass::AtHyphen;
                lines.push(BrokenLine {
                    frags: std::mem::take(&mut cur),
                    ends_hyphen: eh,
                    overfull: false,
                });
                streak = if eh { streak + 1 } else { 0 };
                (cur_w, cur_g) = (0.0, 0.0);
                units.push_front(unit);
            }
        }
    }
    if !cur.is_empty() {
        let eh = cur.last().unwrap().class == BreakClass::AtHyphen;
        lines.push(BrokenLine { frags: cur, ends_hyphen: eh, overfull: false });
    }
    lines
}

// ---- The justifier ------------------------------------------------------------

struct LineParams {
    avail: f32,
    indent: f32,
    mode: Mode,
    role: Role,
    height: f32,
    block: usize,
    rtl: bool,
}

/// Distribute slack across word gaps only — exact f32 arithmetic, spaces
/// never painted; the last line sets natural, never justified (the one
/// non-negotiable of justified setting — research §4).
fn finish_line(bl: BrokenLine, is_last: bool, p: &LineParams) -> Line {
    let mut frags = bl.frags;
    let n = frags.len();
    let mut gaps: Vec<f32> = frags
        .iter()
        .take(n.saturating_sub(1))
        .map(|f| f.space_after)
        .collect();
    let natural: f32 = frags.iter().map(|f| f.width).sum::<f32>() + gaps.iter().sum::<f32>();
    let mut loose = false;
    let justified = p.mode == Mode::Justified && !is_last && !bl.overfull && !gaps.is_empty();
    if justified {
        let slack = p.avail - natural;
        let per: Vec<f32> = gaps
            .iter()
            .map(|g| g * if slack >= 0.0 { STRETCH_PER_GAP } else { SHRINK_PER_GAP })
            .collect();
        let window: f32 = per.iter().sum();
        if window > 0.0 {
            let r = slack / window;
            for (g, w) in gaps.iter_mut().zip(per) {
                let next = *g + r * w;
                // Beyond 2.0× nominal: distribute anyway, flag for debug
                // (the \emergencystretch analogue — research §4).
                if next > *g * SPACE_MAX_ACCEPTABLE + EPS {
                    loose = true;
                }
                *g = next;
            }
        }
    }
    let x = p.indent
        + if p.mode == Mode::RaggedLeft {
            (p.avail - natural).max(0.0)
        } else {
            0.0
        };
    let mut pen = x;
    for (i, f) in frags.iter_mut().enumerate() {
        f.x = pen;
        pen += f.width + gaps.get(i).copied().unwrap_or(0.0);
    }
    let anchor = frags.first().map(|f| f.slice.start).unwrap_or(0);
    Line {
        frags,
        gaps,
        role: p.role,
        x,
        y: 0.0,
        height: p.height,
        justified,
        loose,
        overfull: bl.overfull,
        ends_hyphen: bl.ends_hyphen,
        block: p.block,
        anchor,
        rtl: p.rtl,
        marker: None,
    }
}

fn blank_line(role: Role, height: f32, block: usize, anchor: usize) -> Line {
    Line {
        frags: vec![],
        gaps: vec![],
        role,
        x: 0.0,
        y: 0.0,
        height,
        justified: false,
        loose: false,
        overfull: false,
        ends_hyphen: false,
        block,
        anchor,
        rtl: false,
        marker: None,
    }
}

// ---- The paginator (spec 05 §2.7; S8) -------------------------------------------

struct ImageFlow {
    src: String,
    caption: String,
    w: f32,
    h: f32,
    caption_h: f32,
    block: usize,
    anchor: usize,
}

enum Flow {
    Para { lines: Vec<Line>, heading: bool },
    Image(ImageFlow),
}

/// S8 relaxation levels: 0 full rules · 1 hyphen-avoidance dropped ·
/// 2 widow/orphan 1/1 · 3 keep-with-next dropped. Base level is 1 when the
/// page holds fewer than RELAX_CAPACITY body lines; a stuck decision on an
/// empty page escalates one step at a time. Deterministic.
fn wo_min(level: usize) -> usize {
    if level >= 2 { 1 } else { WIDOW_ORPHAN_MIN }
}

fn keep_lines(level: usize) -> usize {
    match level {
        0 | 1 => KEEP_WITH_NEXT_LINES,
        2 => 1,
        _ => 0,
    }
}

fn head_height(next: &Flow, keep: usize) -> f32 {
    match next {
        Flow::Para { lines, .. } => lines
            .iter()
            .take(keep.min(lines.len()))
            .map(|l| l.height)
            .sum(),
        // Images keep whole: the kept piece is the whole box.
        Flow::Image(img) => img.h + img.caption_h,
    }
}

struct Asm {
    pages: Vec<Page>,
    cur: Vec<PageItem>,
    y: f32,
    page_h: f32,
    measure: f32,
    base: usize,
}

impl Asm {
    fn close(&mut self) {
        if !self.cur.is_empty() {
            self.pages.push(Page { items: std::mem::take(&mut self.cur) });
            self.y = 0.0;
        }
    }

    /// A heading never sets last on a page and keeps lines of its next
    /// block (spec 05 §2.7) — evaluated on the SLICE's block sequence only,
    /// so a slice-final heading never reaches here (regions 9).
    fn reserve_heading(&mut self, lines: &[Line], next: &Flow) {
        let own: f32 = lines.iter().map(|l| l.height).sum();
        let mut level = self.base;
        loop {
            let keep = keep_lines(level);
            if keep == 0 {
                return; // S8: keep-with-next dropped last
            }
            let needed = own + head_height(next, keep);
            if self.y + needed <= self.page_h + EPS {
                return;
            }
            if !self.cur.is_empty() {
                self.close();
                continue;
            }
            if level < 3 {
                level += 1; // the S8 ladder
                continue;
            }
            return;
        }
    }

    fn place_para(&mut self, lines: Vec<Line>) {
        let mut rest: VecDeque<Line> = lines.into();
        let mut first_chunk = true;
        while !rest.is_empty() {
            let mut level = self.base;
            let k = loop {
                let room = self.page_h - self.y;
                let mut fit = 0usize;
                let mut h = 0.0f32;
                for l in &rest {
                    if h + l.height > room + EPS {
                        break;
                    }
                    h += l.height;
                    fit += 1;
                }
                let n = rest.len();
                if fit >= n {
                    break n;
                }
                let mut k = fit;
                let wo = wo_min(level);
                // Widows: the tail carried over must hold ≥ wo lines.
                if n - k < wo {
                    k = n.saturating_sub(wo);
                }
                // Orphans: so must the head chunk where the para begins.
                if first_chunk && k < wo {
                    k = 0;
                }
                // Avoid a page-final hyphen when one-line movement fixes it
                // (research §4: a reader flipping a page mid-word is the
                // worst hyphen there is). Dropped first on the S8 ladder.
                if level == 0
                    && k >= 2
                    && rest[k - 1].ends_hyphen
                    && !rest[k - 2].ends_hyphen
                    && !(first_chunk && k - 1 < wo)
                {
                    k -= 1;
                }
                if k == 0 {
                    // Closing here would leave a heading as the page's last
                    // line (its keep-with-next promised these lines) — a
                    // stuck decision like the empty page: ride the ladder.
                    let heading_last = matches!(
                        self.cur.last(),
                        Some(PageItem::Line(l)) if matches!(l.role, Role::Heading(_))
                    );
                    if !self.cur.is_empty() && !heading_last {
                        self.close();
                        level = self.base;
                        continue;
                    }
                    if level < 3 {
                        level += 1; // the S8 ladder
                        continue;
                    }
                    if heading_last && fit == 0 {
                        // No room for even one line under the heading: the
                        // keep is unsatisfiable — the heading sets last
                        // (keep-with-next dropped, S8's final step).
                        self.close();
                        level = self.base;
                        continue;
                    }
                    // Unconditional progress: every page consumes at least
                    // one line, even if it must clip (S8).
                    break fit.max(1);
                }
                break k;
            };
            for _ in 0..k {
                let mut l = rest.pop_front().unwrap();
                l.y = self.y;
                self.y += l.height;
                self.cur.push(PageItem::Line(l));
            }
            first_chunk = false;
            if !rest.is_empty() {
                self.close();
            }
        }
    }

    /// Images keep whole: taller than a full page → scale to the page;
    /// taller than the room left → move to the next page (spec 05 §2.7).
    fn place_image(&mut self, img: ImageFlow) {
        let (mut w, mut h) = (img.w, img.h);
        if h + img.caption_h > self.page_h {
            let s = (self.page_h - img.caption_h).max(1.0) / h;
            h *= s;
            w *= s;
        }
        if self.y + h + img.caption_h > self.page_h + EPS {
            self.close();
        }
        self.cur.push(PageItem::Image {
            block: img.block,
            src: img.src,
            caption: img.caption,
            x: ((self.measure - w) / 2.0).max(0.0),
            y: self.y,
            width: w,
            height: h,
            anchor: img.anchor,
        });
        self.y += h + img.caption_h;
    }
}

fn assemble(flows: Vec<Flow>, metrics: &BookMetrics, body_h: f32) -> BookLayout {
    let capacity = (metrics.page_height / body_h).floor() as usize;
    let mut asm = Asm {
        pages: vec![],
        cur: vec![],
        y: 0.0,
        page_h: metrics.page_height,
        measure: metrics.measure,
        base: usize::from(capacity < RELAX_CAPACITY),
    };
    let mut it = flows.into_iter().peekable();
    while let Some(flow) = it.next() {
        match flow {
            Flow::Para { lines, heading } => {
                if heading && let Some(next) = it.peek() {
                    asm.reserve_heading(&lines, next);
                }
                asm.place_para(lines);
            }
            Flow::Image(img) => asm.place_image(img),
        }
    }
    asm.close();
    if asm.pages.is_empty() {
        // The empty book: one honest blank page (regions 4).
        asm.pages.push(Page { items: vec![] });
    }
    BookLayout { pages: asm.pages }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 10 px per char, 10 px space — lines compute by hand.
    struct FakeM;

    impl Measure for FakeM {
        fn width(&mut self, text: &str, _: Role, _: &[(usize, Style)]) -> f32 {
            text.chars().count() as f32 * 10.0
        }
        fn space(&mut self, _: Role, _: Style) -> f32 {
            10.0
        }
        fn image_size(&mut self, src: &str) -> Option<(f32, f32)> {
            (src == "known.png").then_some((300.0, 150.0))
        }
    }

    /// Dictionary-ish: break at every char boundary ≥ 2 chars from each edge.
    struct FakeH;

    impl Hyphenate for FakeH {
        fn breaks(&mut self, word: &str) -> Vec<usize> {
            let idx: Vec<usize> = word.char_indices().map(|(i, _)| i).collect();
            let n = idx.len();
            if n < 5 {
                return vec![];
            }
            (2..=n - 2).map(|k| idx[k]).collect()
        }
    }

    /// The crate's soft-hyphen convention (hyphenator.rs
    /// soft_hyphen_indices): the byte index OF each shy, as the only breaks.
    struct ShyH;

    impl Hyphenate for ShyH {
        fn breaks(&mut self, word: &str) -> Vec<usize> {
            word.match_indices('\u{ad}').map(|(i, _)| i).collect()
        }
    }

    fn metrics(measure: f32, page_lines: usize) -> BookMetrics {
        BookMetrics {
            measure,
            line_height: 25.0,
            heading_line_height: 29.0,
            page_height: page_lines as f32 * 25.0,
            em: 16.0,
            justify: true,
        }
    }

    fn lay(text: &str, blocks: BlockMap, measure: f32, page_lines: usize) -> BookLayout {
        paginate(text, &SpanSet::default(), &blocks, &metrics(measure, page_lines), &mut FakeM, &mut FakeH)
    }

    fn lines(book: &BookLayout) -> Vec<&Line> {
        book.pages
            .iter()
            .flat_map(|p| &p.items)
            .filter_map(|i| match i {
                PageItem::Line(l) => Some(l),
                _ => None,
            })
            .collect()
    }

    fn words(line: &Line) -> Vec<&str> {
        line.frags.iter().map(|f| f.text.as_str()).collect()
    }

    /// Both directions of the §3.2 candidate choice, both feasible: at 260 px
    /// the hyphenated end is tighter (badness 24+50 < 219); at 254 px the
    /// clean break is (112 < 100+50).
    #[test]
    fn badness_picks_the_lower_cost_line_end() {
        // "jj" trails so the hyphenation target isn't the paragraph's last
        // word (which never hyphenates — research §4).
        let text = "aa bb cc dd ee ff gg hh iiiiiiii jj";
        let book = lay(text, BlockMap::new(1), 260.0, 30);
        let ls = lines(&book);
        assert_eq!(words(ls[0]).last(), Some(&"ii-"), "hyphen wins at 260");
        assert_eq!(words(ls[1])[0], "iiiiii", "the remainder opens line 2");
        let book = lay(text, BlockMap::new(1), 254.0, 30);
        let ls = lines(&book);
        assert_eq!(words(ls[0]).len(), 8, "clean break wins at 254");
        assert!(!ls[0].ends_hyphen);
    }

    /// Hyphen-heavy text: the LARGE streak penalty holds consecutive
    /// hyphen-ended lines to two whenever a clean break exists.
    #[test]
    fn hyphen_streak_never_exceeds_two_with_clean_breaks_available() {
        let text = "pppppppppppp ".repeat(10);
        let book = lay(text.trim(), BlockMap::new(1), 170.0, 100);
        let ls = lines(&book);
        let mut streak = 0usize;
        let mut max_streak = 0usize;
        let mut hyphens = 0usize;
        for l in &ls {
            if l.ends_hyphen {
                streak += 1;
                hyphens += 1;
            } else {
                streak = 0;
            }
            max_streak = max_streak.max(streak);
        }
        assert!(hyphens > 0, "the fixture must actually hyphenate");
        assert_eq!(max_streak, 2, "the third consecutive hyphen is LARGE-charged away");
    }

    /// «слово —» — тире never starts a line, and word+dash share one token
    /// range (F9).
    #[test]
    fn dash_binds_to_the_preceding_word() {
        let book = lay("aaaa bbbb — cccc", BlockMap::new(1), 100.0, 30);
        let ls = lines(&book);
        for l in &ls {
            assert_ne!(words(l).first(), Some(&"—"), "a dash must never start a line");
        }
        let l2 = ls[1];
        assert_eq!(words(l2), vec!["bbbb", "—"]);
        assert_eq!(l2.frags[0].token, 5..11, "word and dash share word..dash");
        assert_eq!(l2.frags[1].token, 5..11);
        assert_eq!(l2.frags[0].class, BreakClass::Bound);
    }

    /// U+00A0 never splits: an NBSP-joined pair is one token, one fragment.
    #[test]
    fn nbsp_joined_tokens_stay_one_fragment() {
        let book = lay("aaaa bbbb\u{a0}cccc", BlockMap::new(1), 300.0, 30);
        let ls = lines(&book);
        assert_eq!(words(ls[0]), vec!["aaaa", "bbbb\u{a0}cccc"]);
        // And under pressure the pair still refuses to split at the NBSP:
        // the digit-guarded join overflows whole instead of hyphenating.
        let book = lay("xxxx 1\u{a0}kilometers", BlockMap::new(1), 50.0, 30);
        let ls = lines(&book);
        assert_eq!(words(ls[1]), vec!["1\u{a0}kilometers"]);
        assert!(ls[1].overfull);
    }

    /// Digit-bearing tokens never hyphenate and their internal hyphens are
    /// not break opportunities.
    #[test]
    fn digit_tokens_pass_whole() {
        let book = lay("1941-1945 v0.8.4 1-го", BlockMap::new(1), 600.0, 30);
        let ls = lines(&book);
        assert_eq!(words(ls[0]), vec!["1941-1945", "v0.8.4", "1-го"]);
        // Narrow: each overflows whole — no split at the internal hyphen.
        let book = lay("1941-1945 v0.8.4 1-го", BlockMap::new(1), 50.0, 30);
        let ls = lines(&book);
        assert_eq!(ls.len(), 3);
        assert!(ls.iter().all(|l| l.frags.len() == 1 && !l.ends_hyphen));
    }

    /// The last line sets natural, never justified (research §4).
    #[test]
    fn last_line_sets_natural_never_justified() {
        let book = lay("aaaa bbbb cccc dddd", BlockMap::new(1), 90.0, 30);
        let ls = lines(&book);
        assert_eq!(ls.len(), 2);
        assert!(ls[0].justified);
        let last = ls[1];
        assert!(!last.justified);
        assert_eq!(last.gaps, vec![10.0], "nominal gap, no distribution");
    }

    /// «какой-то» splits at its own hyphen: two fragments, exhyphen-charged
    /// break, nothing inserted, one shared token range.
    #[test]
    fn compounds_split_at_the_hyphen_nothing_inserted() {
        let book = lay("какой-то", BlockMap::new(1), 600.0, 30);
        let ls = lines(&book);
        assert_eq!(words(ls[0]), vec!["какой-", "то"]);
        let (a, b) = (&ls[0].frags[0], &ls[0].frags[1]);
        assert_eq!(a.class, BreakClass::AtHyphen);
        assert_eq!(a.slice, 0..6);
        assert_eq!(b.slice, 6..8);
        assert_eq!(a.token, 0..8, "both halves carry the whole token (F9)");
        assert_eq!(b.token, 0..8);
        // Narrow: the break lands at the existing hyphen, nothing added.
        let book = lay("хх какой-то", BlockMap::new(1), 90.0, 30);
        let ls = lines(&book);
        assert!(ls[0].ends_hyphen, "the compound break is a hyphen end");
        assert_eq!(words(ls[0]).last(), Some(&"какой-"));
        assert_eq!(words(ls[1])[0], "то");
    }

    /// A U+00AD word breaks at the soft hyphen only; the shy is never
    /// painted; both halves share char- and grapheme-aligned token ranges
    /// (F9 + research §6).
    #[test]
    fn soft_hyphen_breaks_paint_clean_and_share_the_token() {
        // The shy word is char positions 6..16 ("co" + U+00AD + "operate");
        // "end" trails so the last-word rule stays out of the way.
        let text = "xx yy co\u{ad}operate end";
        let mut m = FakeM;
        let mut h = ShyH;
        let book = paginate(text, &SpanSet::default(), &BlockMap::new(1), &metrics(100.0, 30), &mut m, &mut h);
        let ls = lines(&book);
        assert_eq!(words(ls[0]), vec!["xx", "yy", "co-"], "break at the shy position");
        assert_eq!(words(ls[1])[0], "operate");
        let (pre, rest) = (&ls[0].frags[2], &ls[1].frags[0]);
        assert!(!pre.text.contains('\u{ad}'), "the shy is never painted");
        assert_eq!(pre.token, 6..16, "both halves carry the whole token");
        assert_eq!(rest.token, 6..16);
        assert_eq!(pre.slice.end, rest.slice.start, "slice ranges stay contiguous");
        // An UNBROKEN shy word paints without the shy too.
        let book = paginate(text, &SpanSet::default(), &BlockMap::new(1), &metrics(600.0, 30), &mut m, &mut h);
        let ls = lines(&book);
        assert_eq!(words(ls[0])[2], "cooperate");
        assert_eq!(ls[0].frags[2].slice, 6..16, "slice keeps SOURCE chars incl. the shy");
    }

    /// F9 + F5: a dictionary-hyphenated word's halves carry equal token
    /// ranges, and slicing the SOURCE by that range yields the whole word —
    /// what cr_copy will paste (never the painted hyphen).
    #[test]
    fn hyphenated_halves_share_the_token_and_the_source_substring() {
        let text = "aa bb cc dd ee ff gg hh iiiiiiii jj";
        let book = lay(text, BlockMap::new(1), 260.0, 30);
        let ls = lines(&book);
        let pre = ls[0].frags.last().unwrap();
        let rest = &ls[1].frags[0];
        assert_eq!(pre.text, "ii-");
        assert_eq!(pre.token, rest.token);
        let src: String = text.chars().skip(pre.token.start).take(pre.token.end - pre.token.start).collect();
        assert_eq!(src, "iiiiiiii", "token range slices the source word whole");
        assert_eq!(pre.slice.end, rest.slice.start);
    }

    /// Justified gaps reconstruct the measure exactly; slack beyond 2.0×
    /// nominal still distributes but flags the line loose (research §4).
    #[test]
    fn justified_gaps_reconstruct_the_measure_and_flag_loose() {
        // Hyphenation would rescue this line; NoHyphen forces the loose arm.
        let mut m = FakeM;
        let mut h = NoHyphen;
        let book = paginate(
            "aa bb ccccccccccccccccc dd",
            &SpanSet::default(),
            &BlockMap::new(1),
            &metrics(200.0, 30),
            &mut m,
            &mut h,
        );
        let ls = lines(&book);
        let l = ls[0];
        assert!(l.justified && l.loose, "distributed past 2.0x and flagged");
        let end = l.frags.last().unwrap().x + l.frags.last().unwrap().width;
        assert!((end - 200.0).abs() < 0.01, "the line fills the measure exactly");
        // A normally justified line lands on the measure too.
        let book = lay("aa bb cc dd ee ff gg hh iiiiiiii", BlockMap::new(1), 260.0, 30);
        let ls = lines(&book);
        let l = ls[0];
        assert!(l.justified && !l.loose);
        let end = l.frags.last().unwrap().x + l.frags.last().unwrap().width;
        assert!((end - 260.0).abs() < 0.01);
    }

    /// A word wider than the measure overflows left-aligned, never squeezed.
    #[test]
    fn overfull_words_overflow_left_aligned() {
        let mut m = FakeM;
        let mut h = NoHyphen;
        let book = paginate(
            "aaaaaaaaaaaaaaaaaaaaaaaaa",
            &SpanSet::default(),
            &BlockMap::new(1),
            &metrics(100.0, 30),
            &mut m,
            &mut h,
        );
        let ls = lines(&book);
        assert!(ls[0].overfull);
        assert_eq!(ls[0].x, 0.0, "left-aligned");
        assert!(ls[0].frags[0].width > 100.0, "painted at natural width");
        assert!(!ls[0].justified);
    }

    /// RTL paragraphs set ragged-left, unjustified, unhyphenated (§2.5).
    #[test]
    fn rtl_paragraphs_set_ragged_left_unjustified() {
        let book = lay("שלום עולם שלום עולם שלום", BlockMap::new(1), 100.0, 30);
        let ls = lines(&book);
        assert!(ls.len() > 1);
        for l in &ls {
            assert!(l.rtl && !l.justified && !l.ends_hyphen);
            let natural: f32 =
                l.frags.iter().map(|f| f.width).sum::<f32>() + l.gaps.iter().sum::<f32>();
            assert!((l.x - (100.0 - natural).max(0.0)).abs() < 0.01, "ragged-left = right-aligned");
        }
    }

    /// S7: below the justification floor the block sets ragged-right,
    /// unhyphenated — even where hyphenation would have fired.
    #[test]
    fn ragged_below_the_floor_never_hyphenates() {
        let mut met = metrics(170.0, 100);
        met.justify = false;
        let text = "pppppppppppp ".repeat(6);
        let book = paginate(text.trim(), &SpanSet::default(), &BlockMap::new(1), &met, &mut FakeM, &mut FakeH);
        for l in lines(&book) {
            assert!(!l.justified && !l.ends_hyphen);
            assert!(!l.frags.iter().any(|f| f.text.ends_with('-')));
        }
        // The floor helper itself (research-page §1.1 advances).
        assert!(!below_justification_floor(450.0, 8.14, 8.99));
        assert!(below_justification_floor(330.0, 8.14, 8.99), "50 RU chars need 450 px");
    }

    // ---- The paginator ----

    /// `n` body lines' worth of prose at measure 170: 2n eight-char
    /// digit-bearing words (which never hyphenate) set exactly 2 per line.
    fn prose(n: usize) -> String {
        (0..2 * n)
            .map(|i| format!("w{i:07}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn page_line_counts(book: &BookLayout) -> Vec<usize> {
        book.pages.iter().map(|p| p.items.len()).collect()
    }

    /// Widows/orphans ≥ 2 lines each side of a page break (research §4).
    #[test]
    fn widow_orphan_floors_hold() {
        // capacity 10; para2 = 3 lines: a 2-line foot chunk would leave a
        // 1-line widow, a 1-line foot chunk is an orphan — it moves whole.
        let text = format!("{}\n{}", prose(8), prose(3));
        let book = lay(&text, BlockMap::new(2), 170.0, 10);
        assert_eq!(page_line_counts(&book), vec![8, 3], "para2 moved whole");
        // para2 = 4 lines: 2 + 2 satisfies both floors -> it splits.
        let text = format!("{}\n{}", prose(8), prose(4));
        let book = lay(&text, BlockMap::new(2), 170.0, 10);
        assert_eq!(page_line_counts(&book), vec![10, 2]);
    }

    /// A heading is never the last block on a page: it keeps ≥ 2 lines of
    /// its following paragraph (spec 05 §2.7).
    #[test]
    fn heading_keeps_two_lines_of_its_paragraph() {
        let text = format!("{}\nHeading here\n{}", prose(8), prose(4));
        let blocks = BlockMap::from_kinds(vec![
            BlockKind::Paragraph,
            BlockKind::Heading(2),
            BlockKind::Paragraph,
        ]);
        let book = lay(&text, blocks, 170.0, 10);
        assert_eq!(book.pages[0].items.len(), 8, "the heading moved off page 1");
        let PageItem::Line(l) = &book.pages[1].items[0] else { panic!("a line") };
        assert!(matches!(l.role, Role::Heading(_)), "the heading opens page 2");
        // And no page in the book ends on a heading.
        for p in &book.pages {
            if let Some(PageItem::Line(l)) = p.items.last() {
                assert!(!matches!(l.role, Role::Heading(_)));
            }
        }
    }

    /// A slice-final heading sets on the last page — the keep rule is
    /// evaluated on the SLICE's block sequence only, so it goes vacuous
    /// (regions 9).
    #[test]
    fn slice_final_heading_sets_on_the_last_page() {
        // Fits the first page: sets as its (vacuously legal) last line.
        let text = format!("{}\nThe End", prose(8));
        let blocks =
            BlockMap::from_kinds(vec![BlockKind::Paragraph, BlockKind::Heading(1)]);
        let book = lay(&text, blocks.clone(), 170.0, 10);
        assert_eq!(book.pages.len(), 1);
        let PageItem::Line(l) = book.pages[0].items.last().unwrap() else { panic!() };
        assert!(matches!(l.role, Role::Heading(_)));
        // No room left: it opens (and closes) the last page alone.
        let text = format!("{}\nThe End", prose(9));
        let book = lay(&text, blocks, 170.0, 10);
        assert_eq!(page_line_counts(&book), vec![9, 1]);
    }

    /// S8 step 1: the page-final-hyphen avoidance holds at book capacity
    /// and is the first rule dropped below RELAX_CAPACITY.
    #[test]
    fn relaxation_drops_hyphen_avoidance_below_eight_lines() {
        // One paragraph, 11 lines; line 8 ends at a compound hyphen.
        let text = format!(
            "{} cccccccc aaaaaaa-bbbbbbb dddddddd eeeeeeee ffffffff gggggggg",
            prose(7)
        );
        let book = lay(&text, BlockMap::new(1), 170.0, 8);
        assert_eq!(page_line_counts(&book), vec![7, 4], "one line moved off the hyphen");
        if let PageItem::Line(l) = book.pages[0].items.last().unwrap() {
            assert!(!l.ends_hyphen, "page 1 no longer ends mid-word");
        }
        // At 4-line capacity the ladder's base drops the avoidance: the
        // hyphen-ended line stays page-final.
        let book = lay(&text, BlockMap::new(1), 170.0, 4);
        assert_eq!(page_line_counts(&book), vec![4, 4, 3]);
        let PageItem::Line(l) = book.pages[1].items.last().unwrap() else { panic!() };
        assert!(l.ends_hyphen, "avoidance dropped below RELAX_CAPACITY");
    }

    /// S8 steps 2 and 3 at 4-line capacity: widow/orphan relax to 1/1
    /// before keep-with-next is dropped, and only then does a heading set
    /// page-final.
    #[test]
    fn relaxation_ladder_at_four_line_capacity() {
        // A 2-line heading + prose: heading(58) + 2 body lines (50) > 100,
        // but the level-2 floor (1 line) fits — page 1 = heading + 1 line.
        let text = format!("aaaaaaaa bbbbbbbb cccccccc dddddddd\n{}", prose(3));
        let blocks =
            BlockMap::from_kinds(vec![BlockKind::Heading(2), BlockKind::Paragraph]);
        let book = lay(&text, blocks, 170.0, 4);
        assert_eq!(page_line_counts(&book), vec![3, 2]);
        let p1: Vec<&Line> = book.pages[0]
            .items
            .iter()
            .filter_map(|i| match i {
                PageItem::Line(l) => Some(l),
                _ => None,
            })
            .collect();
        assert!(matches!(p1[0].role, Role::Heading(_)));
        assert!(matches!(p1[1].role, Role::Heading(_)));
        assert!(matches!(p1[2].role, Role::Body), "widow/orphan 1/1 kept one line");
        // A 3-line heading can't keep even one line: keep-with-next drops
        // (the ladder's last step) and the heading sets alone.
        let text = format!(
            "aaaaaaaa bbbbbbbb cccccccc dddddddd eeeeeeee ffffffff\n{}",
            prose(3)
        );
        let blocks =
            BlockMap::from_kinds(vec![BlockKind::Heading(2), BlockKind::Paragraph]);
        let book = lay(&text, blocks, 170.0, 4);
        assert_eq!(page_line_counts(&book), vec![3, 3]);
        for i in &book.pages[0].items {
            let PageItem::Line(l) = i else { panic!() };
            assert!(matches!(l.role, Role::Heading(_)), "the heading owns page 1 alone");
        }
    }

    /// S8: every page consumes at least one line, unconditionally.
    #[test]
    fn progress_guarantee_at_one_line_capacity() {
        let book = lay(&prose(5), BlockMap::new(1), 170.0, 1);
        assert_eq!(page_line_counts(&book), vec![1, 1, 1, 1, 1]);
    }

    /// One paragraph spanning ≥ 3 pages; page-top-char resume is exact on
    /// the same layout and lands on the containing page after remeasure.
    #[test]
    fn page_top_char_resume_across_repagination() {
        let text = prose(60);
        let book = lay(&text, BlockMap::new(1), 170.0, 10);
        assert!(book.pages.len() >= 3, "spans at least three pages");
        for p in 0..book.pages.len() {
            assert_eq!(book.page_of_char(book.page_top_char(p)), p, "self-resume is exact");
        }
        let tc = book.page_top_char(3);
        let book2 = lay(&text, BlockMap::new(1), 170.0, 7);
        let p2 = book2.page_of_char(tc);
        assert!(book2.page_top_char(p2) <= tc);
        assert!(p2 + 1 >= book2.pages.len() || tc < book2.page_top_char(p2 + 1));
        assert!(
            book2.pages[p2].items.iter().any(|i| match i {
                PageItem::Line(l) =>
                    l.anchor == tc || l.frags.iter().any(|f| f.slice.contains(&tc)),
                _ => false,
            }),
            "the resumed page contains the old page-top char"
        );
    }

    /// Per-BlockKind rules: footnote defs skipped, quotes indented 1 em,
    /// code ragged, list markers as unanchored decoration, blank lines kept.
    #[test]
    fn block_kinds_render_by_their_rules() {
        let text = "Head words\n\nBody words here\nquoted words\ncode xx\nitem one\nitem two\n[^a]: def text\nimage-line\ntail words";
        let kinds = vec![
            BlockKind::Heading(2),
            BlockKind::Paragraph, // blank
            BlockKind::Paragraph,
            BlockKind::Blockquote,
            BlockKind::CodeBlock { info: String::new() },
            BlockKind::ListItem { ordered: true, depth: 0 },
            BlockKind::ListItem { ordered: true, depth: 0 },
            BlockKind::FootnoteDef { id: "a".into() },
            BlockKind::Image {
                src: "known.png".into(),
                alt: String::new(),
                caption: "cap".into(),
            },
            BlockKind::Paragraph,
        ];
        let book = lay(text, BlockMap::from_kinds(kinds), 600.0, 40);
        let ls = lines(&book);
        let by_block = |b: usize| ls.iter().copied().find(|l| l.block == b);
        assert!(matches!(by_block(0).unwrap().role, Role::Heading(2)));
        assert_eq!(by_block(0).unwrap().height, 29.0);
        let blank = by_block(1).unwrap();
        assert!(blank.frags.is_empty(), "blank paragraphs paginate as blank lines");
        assert_eq!(blank.x + by_block(3).unwrap().x, 16.0, "quotes indent one em");
        let code = by_block(4).unwrap();
        assert!(matches!(code.role, Role::Code) && !code.justified, "code sets ragged");
        let (i1, i2) = (by_block(5).unwrap(), by_block(6).unwrap());
        assert_eq!(i1.marker.as_ref().unwrap().text, "1.");
        assert_eq!(i2.marker.as_ref().unwrap().text, "2.");
        assert!(i1.marker.as_ref().unwrap().x < i1.x, "the marker hangs left of the text");
        assert_eq!(i1.x, 24.0, "list text indents 1.5 em");
        assert!(by_block(7).is_none(), "FootnoteDef blocks are skipped entirely");
        let img = book.pages.iter().flat_map(|p| &p.items).find_map(|i| match i {
            PageItem::Image { block: 8, width, height, x, caption, .. } =>
                Some((*width, *height, *x, caption.clone())),
            _ => None,
        });
        let (w, h, x, caption) = img.expect("the image box exists");
        assert_eq!((w, h), (300.0, 150.0), "natural size fits the measure");
        assert_eq!(x, 150.0, "centered");
        assert_eq!(caption, "cap");
    }

    /// Images keep whole: move to the next page when the room is short,
    /// scale to the page when taller than one (spec 05 §2.7); unknown
    /// dimensions get the deterministic placeholder box (regions 12).
    #[test]
    fn images_keep_whole_move_or_scale() {
        let text = format!("{}\nimg", prose(2));
        let blocks = BlockMap::from_kinds(vec![
            BlockKind::Paragraph,
            BlockKind::Image {
                src: "known.png".into(),
                alt: String::new(),
                caption: "cap".into(),
            },
        ]);
        let book = lay(&text, blocks, 170.0, 4);
        assert_eq!(book.pages.len(), 2, "the image moved whole");
        let PageItem::Image { height, width, y, .. } = &book.pages[1].items[0] else {
            panic!("an image box")
        };
        assert!((height - 75.0).abs() < 0.01, "scaled to the page minus its caption line");
        assert_eq!(*y, 0.0);
        assert!(*width < 170.0, "aspect kept while scaling");
        // Unknown src: the editor's 56 px placeholder at full measure.
        let text = format!("{}\nimg", prose(1));
        let blocks = BlockMap::from_kinds(vec![
            BlockKind::Paragraph,
            BlockKind::Image { src: "missing.png".into(), alt: String::new(), caption: String::new() },
        ]);
        let book = lay(&text, blocks, 170.0, 4);
        let PageItem::Image { height, width, .. } = &book.pages[0].items[1] else {
            panic!("an image box")
        };
        assert_eq!(*height, IMAGE_PLACEHOLDER_HEIGHT);
        assert_eq!(*width, 170.0);
    }

    /// Blank in-slice paragraphs keep their line; the empty slice sets one
    /// honest blank page (regions 4).
    #[test]
    fn blank_paragraphs_and_the_empty_book() {
        let book = lay("aaaa bbbb\n\ncccc dddd", BlockMap::new(3), 600.0, 30);
        let ls = lines(&book);
        assert_eq!(ls.len(), 3);
        assert!(ls[1].frags.is_empty());
        assert_eq!(ls[1].anchor, 10, "the blank line anchors at its own char");
        let book = lay("", BlockMap::new(1), 600.0, 30);
        assert_eq!(book.pages.len(), 1, "pages = max(1, computed)");
    }

    // ---- Determinism gate + microbench ----

    /// Varied per-char widths so the badness paths exercise for real.
    struct BenchM;

    impl Measure for BenchM {
        fn width(&mut self, text: &str, role: Role, _: &[(usize, Style)]) -> f32 {
            let base: f32 = text.chars().map(|c| 5.0 + (c as u32 % 8) as f32).sum();
            match role {
                Role::Heading(_) => base * 1.15,
                _ => base,
            }
        }
        fn space(&mut self, _: Role, _: Style) -> f32 {
            8.0
        }
        fn image_size(&mut self, _: &str) -> Option<(f32, f32)> {
            None
        }
    }

    /// A deterministic ~N-word bilingual doc: EN/RU words, headings,
    /// quotes, spaced dashes and a compound — the shapes the engine meets.
    fn synthetic(words: usize) -> (String, BlockMap) {
        let en = ["the", "quick", "typography", "justification", "reading", "tremendous",
            "hyphenation", "paragraph", "between", "understanding", "margins", "chapter"];
        let ru = ["слово", "предложение", "перенос", "страница", "чтение", "редактор",
            "рукопись", "глава", "понимание", "типографика", "выключка", "какой-то"];
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut rnd = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as usize
        };
        let mut paras: Vec<String> = Vec::new();
        let mut kinds: Vec<BlockKind> = Vec::new();
        let mut done = 0usize;
        while done < words {
            let heading = kinds.len() % 9 == 3;
            let quote = kinds.len() % 9 == 7;
            let len = if heading { 4 } else { 40 + rnd() % 50 };
            let mut p: Vec<String> = Vec::new();
            for k in 0..len {
                let r = rnd();
                let w = if r % 3 == 0 { ru[r % ru.len()] } else { en[r % en.len()] };
                p.push(w.to_owned());
                if !heading && k % 17 == 11 {
                    p.push("—".to_owned());
                }
            }
            done += len;
            paras.push(p.join(" "));
            kinds.push(if heading {
                BlockKind::Heading(2)
            } else if quote {
                BlockKind::Blockquote
            } else {
                BlockKind::Paragraph
            });
        }
        (paras.join("\n"), BlockMap::from_kinds(kinds))
    }

    /// The Wave-A determinism gate + entry-budget microbench: a ~5,000-word
    /// bilingual doc paginates twice, bit-identically. Timing prints with
    /// --nocapture; the real-shaping budget rides examples/bookface_audit.
    #[test]
    fn determinism_five_thousand_words_paginate_identically() {
        let (text, blocks) = synthetic(5_000);
        let met = BookMetrics {
            measure: 450.0,
            line_height: 25.0,
            heading_line_height: 29.0,
            page_height: 725.0,
            em: 16.5,
            justify: true,
        };
        let spans = SpanSet::default();
        let t0 = std::time::Instant::now();
        let a = paginate(&text, &spans, &blocks, &met, &mut BenchM, &mut FakeH);
        let first = t0.elapsed();
        let t1 = std::time::Instant::now();
        let b = paginate(&text, &spans, &blocks, &met, &mut BenchM, &mut FakeH);
        let second = t1.elapsed();
        assert!(a.pages.len() > 5, "a real book: {} pages", a.pages.len());
        assert_eq!(a, b, "two runs must produce IDENTICAL pagination");
        println!(
            "bookpage microbench: 5k words -> {} pages; run1 {first:?}, run2 {second:?} (fake measurer)",
            a.pages.len()
        );
    }
}
