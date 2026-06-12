//! Caption rendering for the pet window. Two modes:
//! - no waiting sessions → a single state pill (`● Name · working/done/idle`);
//! - ≥1 waiting session → the agent name plus one pill per waiting session
//!   (amber dot + project + elapsed timer), capped with a "+N" overflow pill.
//!
//! Split out of `pet/mod.rs` to keep each file focused. All drawing is cairo, in
//! the pet's transparent canvas; the caller positions the sprite above this block.

use agentpet_core::state::PetMood;
use gtk4::cairo::Context as CairoContext;
use gtk4::pango;

/// One waiting session shown under the pet: its project and when it began waiting.
#[derive(Clone)]
pub struct WaitingRow {
    pub project: String,
    /// Unix time the session entered `waiting`; used to render an elapsed timer.
    pub state_since: f64,
}

/// At most this many waiting rows are listed; the rest collapse into a "+N" pill.
pub(crate) const MAX_ROWS: usize = 4;
/// Vertical pitch (px) between stacked caption pills.
const ROW_PITCH: f64 = 17.0;
/// Height (px) of one pill.
const PILL_H: f64 = 15.0;
/// Gap (px) between the sprite and the first waiting pill.
const GAP_TOP: f64 = 4.0;

/// Status pills: light text on a dark translucent ground.
const PILL_BG: (f64, f64, f64, f64) = (0.08, 0.09, 0.12, 0.66);
const PILL_FG: (f64, f64, f64, f64) = (0.96, 0.97, 1.0, 0.96);
/// Speech bubble: the inverse palette, so it reads as the pet "speaking"
/// rather than as telemetry.
const BUBBLE_BG: (f64, f64, f64, f64) = (0.97, 0.97, 0.99, 0.92);
const BUBBLE_FG: (f64, f64, f64, f64) = (0.10, 0.11, 0.15, 0.95);

/// Extra canvas height (px) reserved above the sprite for the speech bubble:
/// one pill plus a small gap to the sprite. Constant while `show_chat` is on so
/// mood changes never thrash window geometry.
pub(crate) fn bubble_band_height() -> i32 {
    (PILL_H + GAP_TOP) as i32 + 1
}

/// Draws the speech bubble: one centred light pill at the top of the bubble
/// band (the caller reserves `bubble_band_height()` above the sprite).
/// Returns the canvas width that would show the text at full size.
pub(crate) fn draw_bubble(cr: &CairoContext, w: i32, text: &str) -> f64 {
    draw_pill(cr, w, 1.0, text, None, BUBBLE_BG, BUBBLE_FG)
}

/// Extra canvas height (px) the waiting list needs below the square sprite area.
/// Zero when nothing is waiting (the single-line caption overlays the sprite, as
/// before). Header pill + up to `MAX_ROWS` rows + an optional overflow pill.
pub(crate) fn waiting_block_height(n: usize) -> i32 {
    if n == 0 {
        return 0;
    }
    let pills = 1 + n.min(MAX_ROWS) + usize::from(n > MAX_ROWS);
    (pills as f64 * ROW_PITCH + GAP_TOP) as i32
}

/// Draws the single-line state pill at the bottom of the square sprite area: a
/// mood-coloured status dot, the agent name, and — only when `with_state` —
/// the state word (e.g. "● Claude Code · working"). When the speech bubble is
/// shown it already carries the mood wording, so the pill drops the redundant
/// state word and keeps just the dot + name. Colour + wording match the
/// Monitor. Font auto-shrinks so the caption fits the pet's width. Returns the
/// canvas width that would show the text at full size (so the window can grow).
pub(crate) fn draw_label(
    cr: &CairoContext,
    w: i32,
    h: i32,
    name: &str,
    mood: PetMood,
    with_state: bool,
) -> f64 {
    let (state_word, dot) = mood_caption(mood);
    let text = match (name.is_empty(), with_state) {
        (true, _) => state_word.to_string(),
        (false, true) => format!("{name} · {state_word}"),
        (false, false) => name.to_string(),
    };

    let (wf, hf) = (w as f64, h as f64);
    let (pad_x, pad_y) = (8.0, 4.0);
    let dot_r = 4.0;
    let dot_gap = 6.0;
    let fixed = dot_r * 2.0 + dot_gap + pad_x * 2.0;
    let avail_text = (wf - 4.0 - fixed).max(1.0);

    let (layout, full_w) = fitted_layout(cr, &text, 13.0, 8.0, avail_text);
    let ink = layout.pixel_extents().0;

    let bw = fixed + ink.width() as f64;
    let bh = ink.height() as f64 + pad_y * 2.0;
    let bx = (wf - bw) / 2.0;
    let by = hf - bh - 2.0;

    rounded_rect(cr, bx, by, bw, bh, bh / 2.0);
    cr.set_source_rgba(PILL_BG.0, PILL_BG.1, PILL_BG.2, PILL_BG.3);
    let _ = cr.fill();

    let (dr, dg, db) = dot;
    cr.arc(bx + pad_x + dot_r, by + bh / 2.0, dot_r, 0.0, std::f64::consts::TAU);
    cr.set_source_rgba(dr, dg, db, 1.0);
    let _ = cr.fill();

    cr.set_source_rgba(PILL_FG.0, PILL_FG.1, PILL_FG.2, PILL_FG.3);
    let text_x = bx + pad_x + dot_r * 2.0 + dot_gap;
    cr.move_to(text_x - ink.x() as f64, by + pad_y - ink.y() as f64);
    pangocairo::functions::show_layout(cr, &layout);

    full_w + fixed + 4.0
}

/// Draws the agent name then one pill per waiting session (amber dot + project +
/// elapsed), capped at `MAX_ROWS` with a "+N nữa" overflow pill. Pills stack
/// downward starting just below the square sprite area (`sprite_h`). Returns
/// the canvas width that would show the widest pill at full size.
pub(crate) fn draw_waiting(
    cr: &CairoContext,
    w: i32,
    name: &str,
    rows: &[WaitingRow],
    now: f64,
    sprite_h: i32,
) -> f64 {
    let amber = mood_caption(PetMood::Waiting).1;
    let mut y = sprite_h as f64 + GAP_TOP;
    let mut needed = 0.0_f64;

    // Header reads like the single-line caption ("● Claude Code · waiting") so the
    // waiting state shows beside the agent name, consistent with working/done.
    let header = if name.is_empty() { "waiting".to_string() } else { format!("{name} · waiting") };
    needed = needed.max(draw_pill(cr, w, y, &header, Some(amber), PILL_BG, PILL_FG));
    y += ROW_PITCH;

    for row in rows.iter().take(MAX_ROWS) {
        let elapsed = format_elapsed((now - row.state_since).max(0.0));
        let text = format!("{} · {}", row.project, elapsed);
        needed = needed.max(draw_pill(cr, w, y, &text, Some(amber), PILL_BG, PILL_FG));
        y += ROW_PITCH;
    }
    if rows.len() > MAX_ROWS {
        let text = format!("+{} nữa", rows.len() - MAX_ROWS);
        needed = needed.max(draw_pill(cr, w, y, &text, None, PILL_BG, PILL_FG));
    }
    needed
}

/// Draws one centred pill of fixed height `PILL_H` at top `top_y`, optionally with
/// a leading coloured dot. Font auto-shrinks (floor 7px) to fit the pet width.
/// Returns the canvas width that would show the text at full size.
#[allow(clippy::too_many_arguments)]
fn draw_pill(
    cr: &CairoContext,
    w: i32,
    top_y: f64,
    text: &str,
    dot: Option<(f64, f64, f64)>,
    bg: (f64, f64, f64, f64),
    fg: (f64, f64, f64, f64),
) -> f64 {
    let wf = w as f64;
    let pad_x = 7.0;
    let dot_r = if dot.is_some() { 3.5 } else { 0.0 };
    let dot_gap = if dot.is_some() { 5.0 } else { 0.0 };
    let fixed = dot_r * 2.0 + dot_gap + pad_x * 2.0;
    let avail = (wf - 4.0 - fixed).max(1.0);

    let (layout, full_w) = fitted_layout(cr, text, 11.0, 7.0, avail);
    let ink = layout.pixel_extents().0;

    let bw = fixed + ink.width() as f64;
    let bx = (wf - bw) / 2.0;

    rounded_rect(cr, bx, top_y, bw, PILL_H, PILL_H / 2.0);
    cr.set_source_rgba(bg.0, bg.1, bg.2, bg.3);
    let _ = cr.fill();

    let mut text_x = bx + pad_x;
    if let Some((r, g, b)) = dot {
        cr.arc(bx + pad_x + dot_r, top_y + PILL_H / 2.0, dot_r, 0.0, std::f64::consts::TAU);
        cr.set_source_rgba(r, g, b, 1.0);
        let _ = cr.fill();
        text_x = bx + pad_x + dot_r * 2.0 + dot_gap;
    }

    cr.set_source_rgba(fg.0, fg.1, fg.2, fg.3);
    let ty = top_y + (PILL_H - ink.height() as f64) / 2.0 - ink.y() as f64;
    cr.move_to(text_x - ink.x() as f64, ty);
    pangocairo::functions::show_layout(cr, &layout);

    full_w + fixed + 4.0
}

/// Pango layout for `text`: bold Sans at `px`, auto-shrunk (floor `min_px`) so
/// its ink width fits `avail`. Pango resolves every glyph through system font
/// fallback — emoji and non-Latin chat lines render instead of tofu boxes,
/// which cairo's toy text API (one face, no fallback) could not do.
/// Also returns the full-size ink width, which callers surface so the pet
/// window can widen until the text fits unshrunk.
fn fitted_layout(
    cr: &CairoContext,
    text: &str,
    px: f64,
    min_px: f64,
    avail: f64,
) -> (pango::Layout, f64) {
    let layout = pangocairo::functions::create_layout(cr);
    layout.set_text(text);
    let mut desc = pango::FontDescription::new();
    desc.set_family("Sans");
    desc.set_weight(pango::Weight::Bold);
    desc.set_absolute_size(px * pango::SCALE as f64);
    layout.set_font_description(Some(&desc));
    let ink_w = layout.pixel_extents().0.width() as f64;
    if ink_w > avail {
        desc.set_absolute_size((px * avail / ink_w).max(min_px) * pango::SCALE as f64);
        layout.set_font_description(Some(&desc));
        // Last resort: the window has already widened to its cap and the floor
        // font still can't fit. Ellipsize in the MIDDLE so both ends stay
        // readable — the name's start and the suffix ("· working", "· 5m 12s")
        // — instead of the pill getting clipped at the canvas edge.
        layout.set_width((avail * pango::SCALE as f64) as i32);
        layout.set_ellipsize(pango::EllipsizeMode::Middle);
    }
    (layout, ink_w)
}

/// State word + status-dot colour for a mood. Matches the Monitor's wording and
/// palette so the pet and the Monitor never disagree. `Celebrate` reads as "done".
fn mood_caption(mood: PetMood) -> (&'static str, (f64, f64, f64)) {
    match mood {
        PetMood::Working => ("working", (0.290, 0.776, 0.941)), // #4ac6f0
        PetMood::Waiting => ("waiting", (0.941, 0.690, 0.125)), // #f0b020
        PetMood::Done | PetMood::Celebrate => ("done", (0.337, 0.831, 0.447)), // #56d472
        PetMood::Idle => ("idle", (0.400, 0.400, 0.400)),       // #666666
    }
}

/// Compact elapsed-time string (mirrors the Monitor's `format_elapsed`).
fn format_elapsed(secs: f64) -> String {
    let s = secs as u64;
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m {}s", s / 60, s % 60)
    } else {
        format!("{}h {}m", s / 3600, (s % 3600) / 60)
    }
}

fn rounded_rect(cr: &CairoContext, x: f64, y: f64, w: f64, h: f64, r: f64) {
    use std::f64::consts::{FRAC_PI_2, PI};
    let r = r.min(w / 2.0).min(h / 2.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, FRAC_PI_2);
    cr.arc(x + r, y + h - r, r, FRAC_PI_2, PI);
    cr.arc(x + r, y + r, r, PI, 3.0 * FRAC_PI_2);
    cr.close_path();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_waiting_adds_no_height() {
        assert_eq!(waiting_block_height(0), 0);
    }

    #[test]
    fn height_grows_with_rows_then_caps() {
        // header + N rows (no overflow pill until > MAX_ROWS).
        let h3 = waiting_block_height(3);
        let h4 = waiting_block_height(4);
        assert!(h3 > 0 && h4 > h3, "more rows → taller");
        // 5 rows = header + 4 shown + 1 overflow = same pill count as... check it
        // adds exactly one overflow pill beyond the 4-row case.
        let h5 = waiting_block_height(5);
        let h7 = waiting_block_height(7);
        assert!(h5 > h4, "overflow pill adds height once past the cap");
        assert_eq!(h5, h7, "capped: 5 and 7 waiting render the same pill count");
    }
}
