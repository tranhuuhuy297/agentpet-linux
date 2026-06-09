//! Caption rendering for the pet window. Two modes:
//! - no waiting sessions → a single state pill (`● Name · working/done/idle`);
//! - ≥1 waiting session → the agent name plus one pill per waiting session
//!   (amber dot + project + elapsed timer), capped with a "+N" overflow pill.
//!
//! Split out of `pet/mod.rs` to keep each file focused. All drawing is cairo, in
//! the pet's transparent canvas; the caller positions the sprite above this block.

use agentpet_core::state::PetMood;
use gtk4::cairo::{self, Context as CairoContext};

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
/// mood-coloured status dot, the agent name, and the state word (e.g.
/// "● Claude Code · working"). Colour + wording match the Monitor so the two
/// agree. Font auto-shrinks so the caption fits the pet's width.
pub(crate) fn draw_label(cr: &CairoContext, w: i32, h: i32, name: &str, mood: PetMood) {
    let (state_word, dot) = mood_caption(mood);
    let text = if name.is_empty() {
        state_word.to_string()
    } else {
        format!("{name} · {state_word}")
    };

    let (wf, hf) = (w as f64, h as f64);
    cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);

    let (pad_x, pad_y) = (8.0, 4.0);
    let dot_r = 4.0;
    let dot_gap = 6.0;
    let fixed = dot_r * 2.0 + dot_gap + pad_x * 2.0;
    let avail_text = (wf - 4.0 - fixed).max(1.0);

    let base = 13.0;
    cr.set_font_size(base);
    let Ok(probe) = cr.text_extents(&text) else { return };
    let font_size = if probe.width() > avail_text {
        (base * avail_text / probe.width()).max(8.0)
    } else {
        base
    };
    cr.set_font_size(font_size);
    let Ok(ext) = cr.text_extents(&text) else { return };

    let bw = fixed + ext.width();
    let bh = ext.height() + pad_y * 2.0;
    let bx = (wf - bw) / 2.0;
    let by = hf - bh - 2.0;

    rounded_rect(cr, bx, by, bw, bh, bh / 2.0);
    cr.set_source_rgba(0.08, 0.09, 0.12, 0.66);
    let _ = cr.fill();

    let (dr, dg, db) = dot;
    cr.arc(bx + pad_x + dot_r, by + bh / 2.0, dot_r, 0.0, std::f64::consts::TAU);
    cr.set_source_rgba(dr, dg, db, 1.0);
    let _ = cr.fill();

    cr.set_source_rgba(0.96, 0.97, 1.0, 0.96);
    let text_x = bx + pad_x + dot_r * 2.0 + dot_gap - ext.x_bearing();
    cr.move_to(text_x, by + pad_y - ext.y_bearing());
    let _ = cr.show_text(&text);
}

/// Draws the agent name then one pill per waiting session (amber dot + project +
/// elapsed), capped at `MAX_ROWS` with a "+N nữa" overflow pill. Pills stack
/// downward starting just below the square sprite area (`sprite_h`).
pub(crate) fn draw_waiting(
    cr: &CairoContext,
    w: i32,
    name: &str,
    rows: &[WaitingRow],
    now: f64,
    sprite_h: i32,
) {
    let amber = mood_caption(PetMood::Waiting).1;
    let mut y = sprite_h as f64 + GAP_TOP;

    draw_pill(cr, w, y, name, None);
    y += ROW_PITCH;

    for row in rows.iter().take(MAX_ROWS) {
        let elapsed = format_elapsed((now - row.state_since).max(0.0));
        let text = format!("{} · {}", row.project, elapsed);
        draw_pill(cr, w, y, &text, Some(amber));
        y += ROW_PITCH;
    }
    if rows.len() > MAX_ROWS {
        draw_pill(cr, w, y, &format!("+{} nữa", rows.len() - MAX_ROWS), None);
    }
}

/// Draws one centred pill of fixed height `PILL_H` at top `top_y`, optionally with
/// a leading coloured dot. Font auto-shrinks (floor 7px) to fit the pet width.
fn draw_pill(cr: &CairoContext, w: i32, top_y: f64, text: &str, dot: Option<(f64, f64, f64)>) {
    let wf = w as f64;
    cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);

    let pad_x = 7.0;
    let dot_r = if dot.is_some() { 3.5 } else { 0.0 };
    let dot_gap = if dot.is_some() { 5.0 } else { 0.0 };
    let fixed = dot_r * 2.0 + dot_gap + pad_x * 2.0;
    let avail = (wf - 4.0 - fixed).max(1.0);

    let base = 11.0;
    cr.set_font_size(base);
    let Ok(probe) = cr.text_extents(text) else { return };
    let fs = if probe.width() > avail {
        (base * avail / probe.width()).max(7.0)
    } else {
        base
    };
    cr.set_font_size(fs);
    let Ok(ext) = cr.text_extents(text) else { return };

    let bw = fixed + ext.width();
    let bx = (wf - bw) / 2.0;

    rounded_rect(cr, bx, top_y, bw, PILL_H, PILL_H / 2.0);
    cr.set_source_rgba(0.08, 0.09, 0.12, 0.66);
    let _ = cr.fill();

    let mut text_x = bx + pad_x;
    if let Some((r, g, b)) = dot {
        cr.arc(bx + pad_x + dot_r, top_y + PILL_H / 2.0, dot_r, 0.0, std::f64::consts::TAU);
        cr.set_source_rgba(r, g, b, 1.0);
        let _ = cr.fill();
        text_x = bx + pad_x + dot_r * 2.0 + dot_gap;
    }

    cr.set_source_rgba(0.96, 0.97, 1.0, 0.96);
    let ty = top_y + (PILL_H - ext.height()) / 2.0 - ext.y_bearing();
    cr.move_to(text_x - ext.x_bearing(), ty);
    let _ = cr.show_text(text);
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
