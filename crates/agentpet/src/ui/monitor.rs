//! The monitor window: a live list of agent sessions with a status dot, project,
//! current activity, and a per-state elapsed timer. Ports `MenuBarContentView`.

use crate::snapshot::UiCommand;
use agentpet_core::session::AgentSession;
use agentpet_core::state::{AgentKind, AgentState};
use gtk4::cairo;
use gtk4::cairo::Context as CairoContext;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, DrawingArea, Label, ListBox,
    Orientation, PolicyType, ScrolledWindow,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Per-agent-kind cache of the small pet sprite drawn in each row. `None` marks
/// "looked up, no installed pack" so we don't re-slice a spritesheet every
/// second-tick re-render. Cleared by `reload_pets` when the selection changes.
type PetIcons = Rc<RefCell<HashMap<AgentKind, Option<cairo::ImageSurface>>>>;

/// Side length (px) of the agent badge and pet icon drawn at the start of a row.
const ICON_SIZE: i32 = 26;

/// Official agent marks, embedded so they ship inside the single binary (the
/// app runs from `~/.local` with no assets dir alongside). Claude Code's pixel
/// creature and Codex's terminal-prompt cloud.
const CLAUDE_LOGO_PNG: &[u8] = include_bytes!("../../../../assets/agents/claude.png");
const CODEX_LOGO_PNG: &[u8] = include_bytes!("../../../../assets/agents/codex.png");

pub struct MonitorWindow {
    window: ApplicationWindow,
    list: ListBox,
    sessions: Rc<RefCell<Vec<AgentSession>>>,
    pet_icons: PetIcons,
}

impl MonitorWindow {
    pub fn new(app: &Application, cmd: async_channel::Sender<UiCommand>) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("AgentPet — Monitor")
            .default_width(380)
            .default_height(460)
            .build();
        window.set_hide_on_close(true); // closing hides; the app keeps running

        let list = ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::None);
        let scrolled = ScrolledWindow::new();
        scrolled.set_policy(PolicyType::Never, PolicyType::Automatic);
        scrolled.set_vexpand(true);
        scrolled.set_child(Some(&list));

        // Footer with Settings / Quit (the primary access point when there's no
        // tray, e.g. GNOME without the AppIndicator extension).
        let footer = GtkBox::new(Orientation::Horizontal, 8);
        footer.set_margin_top(8);
        footer.set_margin_bottom(8);
        footer.set_margin_start(10);
        footer.set_margin_end(10);
        let spacer = GtkBox::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        let settings_btn = Button::with_label("Settings");
        let quit_btn = Button::with_label("Quit");
        quit_btn.add_css_class("destructive-action");
        for b in [&settings_btn, &quit_btn] {
            b.set_valign(Align::Center);
        }
        {
            let cmd = cmd.clone();
            settings_btn.connect_clicked(move |_| {
                let _ = cmd.try_send(UiCommand::OpenSettings);
            });
        }
        {
            let cmd = cmd.clone();
            quit_btn.connect_clicked(move |_| {
                let _ = cmd.try_send(UiCommand::Quit);
            });
        }
        footer.append(&spacer);
        footer.append(&settings_btn);
        footer.append(&quit_btn);

        let root = GtkBox::new(Orientation::Vertical, 0);
        root.append(&scrolled);
        root.append(&footer);
        window.set_child(Some(&root));

        let sessions = Rc::new(RefCell::new(Vec::<AgentSession>::new()));
        let pet_icons: PetIcons = Rc::new(RefCell::new(HashMap::new()));

        // Tick the elapsed timers once a second — but only while the window is
        // actually on screen (it hides on close and would otherwise re-render
        // invisibly forever).
        {
            let (window, list, sessions, pet_icons) =
                (window.clone(), list.clone(), sessions.clone(), pet_icons.clone());
            gtk4::glib::timeout_add_seconds_local(1, move || {
                if window.is_visible() {
                    render(&list, &sessions.borrow(), &pet_icons);
                }
                gtk4::glib::ControlFlow::Continue
            });
        }

        MonitorWindow { window, list, sessions, pet_icons }
    }

    pub fn set_sessions(&self, sessions: &[AgentSession]) {
        *self.sessions.borrow_mut() = sessions.to_vec();
        if self.window.is_visible() {
            render(&self.list, &self.sessions.borrow(), &self.pet_icons);
        }
    }

    pub fn show(&self) {
        // Render before presenting — the list may be stale from a hidden spell.
        render(&self.list, &self.sessions.borrow(), &self.pet_icons);
        self.window.present();
    }

    /// Drops the cached per-agent pet icons so the next render reloads each
    /// agent's (possibly newly-chosen) pack.
    pub fn reload_pets(&self) {
        self.pet_icons.borrow_mut().clear();
        if self.window.is_visible() {
            render(&self.list, &self.sessions.borrow(), &self.pet_icons);
        }
    }
}

fn render(list: &ListBox, sessions: &[AgentSession], pet_icons: &PetIcons) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    if sessions.is_empty() {
        let label = Label::new(Some("No active agents"));
        label.set_margin_top(16);
        label.set_margin_bottom(16);
        label.add_css_class("dim-label");
        list.append(&label);
        return;
    }
    let now = crate::unix_now();
    for s in sessions {
        list.append(&row(s, now, pet_icons));
    }
}

fn row(s: &AgentSession, now: f64, pet_icons: &PetIcons) -> GtkBox {
    let project = s
        .project
        .as_deref()
        .map(|p| {
            std::path::Path::new(p)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.to_string())
        })
        .unwrap_or_else(|| s.id.clone());

    let activity = s.message.clone().unwrap_or_else(|| state_word(s.state).to_string());
    let elapsed = format_elapsed((now - s.state_since).max(0.0));
    let dot = color_dot(s.state);

    let row = GtkBox::new(Orientation::Horizontal, 8);
    row.set_margin_top(6);
    row.set_margin_bottom(6);
    row.set_margin_start(10);
    row.set_margin_end(10);

    row.append(&agent_badge(s.agent_kind));
    row.append(&pet_icon(s.agent_kind, s.state, pet_icons));

    let markup = format!(
        "<span foreground='{dot}'>●</span>  <b>{}</b>\n<span size='small' foreground='#999'>{} · {}</span>",
        glib_escape(&project),
        glib_escape(&activity),
        elapsed,
    );
    let label = Label::new(None);
    label.set_markup(&markup);
    label.set_xalign(0.0);
    label.set_valign(Align::Center);
    label.set_hexpand(true);
    row.append(&label);
    row
}

/// A small square widget identifying which coding agent (Claude Code, Codex, …)
/// owns the session, drawn as a coloured monogram badge.
fn agent_badge(kind: AgentKind) -> DrawingArea {
    let area = DrawingArea::new();
    area.set_content_width(ICON_SIZE);
    area.set_content_height(ICON_SIZE);
    area.set_valign(Align::Center);
    area.set_draw_func(move |_, cr, w, h| draw_agent_badge(cr, w, h, kind));
    area
}

/// A small square widget showing the agent's assigned pet sprite (the first
/// idle frame), or a state-coloured dot when no pet pack is installed.
fn pet_icon(kind: AgentKind, state: AgentState, pet_icons: &PetIcons) -> DrawingArea {
    let surface = pet_surface(kind, pet_icons);
    let area = DrawingArea::new();
    area.set_content_width(ICON_SIZE);
    area.set_content_height(ICON_SIZE);
    area.set_valign(Align::Center);
    area.set_draw_func(move |_, cr, w, h| match &surface {
        Some(surface) => draw_surface_fit(cr, w, h, surface, 1.0),
        None => draw_pet_fallback(cr, w, h, color_dot(state)),
    });
    area
}

/// Returns the cached pet sprite for `kind`, loading and slicing the pack on
/// first miss. Cached (including the no-pack `None`) so re-renders are cheap.
fn pet_surface(kind: AgentKind, pet_icons: &PetIcons) -> Option<cairo::ImageSurface> {
    if let Some(cached) = pet_icons.borrow().get(&kind) {
        return cached.clone();
    }
    let surface = crate::ui::load_pack_for_kind(kind)
        .and_then(|pack| pack.clip(0).first().and_then(crate::pet::to_surface));
    pet_icons.borrow_mut().insert(kind, surface.clone());
    surface
}

fn draw_agent_badge(cr: &CairoContext, w: i32, h: i32, kind: AgentKind) {
    // Known agents show their official mark on a light rounded backing, so even
    // the black Codex mark stays visible on a dark theme.
    if let Some(logo) = agent_logo_surface(kind) {
        let (wf, hf) = (w as f64, h as f64);
        rounded_rect(cr, 0.5, 0.5, wf - 1.0, hf - 1.0, wf.min(hf) * 0.28);
        cr.set_source_rgba(0.97, 0.97, 0.99, 1.0);
        let _ = cr.fill();
        draw_surface_fit(cr, w, h, &logo, 0.82);
        return;
    }

    // `run`-wrapped CLIs / unknown sources fall back to a coloured monogram.
    let (wf, hf) = (w as f64, h as f64);
    let ((br, bg, bb), glyph) = badge_style(kind);
    rounded_rect(cr, 0.5, 0.5, wf - 1.0, hf - 1.0, wf.min(hf) * 0.28);
    cr.set_source_rgba(br, bg, bb, 1.0);
    let _ = cr.fill();

    cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    cr.set_font_size(hf * 0.6);
    cr.set_source_rgba(0.98, 0.98, 1.0, 1.0);
    if let Ok(ext) = cr.text_extents(glyph) {
        cr.move_to(
            (wf - ext.width()) / 2.0 - ext.x_bearing(),
            (hf - ext.height()) / 2.0 - ext.y_bearing(),
        );
        let _ = cr.show_text(glyph);
    }
}

/// Fallback monogram for agents without an embedded mark (the `run` wrapper and
/// unknown sources).
fn badge_style(kind: AgentKind) -> ((f64, f64, f64), &'static str) {
    match kind {
        AgentKind::Cli => ((0.38, 0.42, 0.48), ">_"),
        _ => ((0.45, 0.47, 0.52), "?"),
    }
}

/// Decodes an agent's embedded logo PNG into a cairo surface, cached per kind
/// (the bytes are static, so this decodes at most once per agent kind).
fn agent_logo_surface(kind: AgentKind) -> Option<cairo::ImageSurface> {
    thread_local! {
        static CACHE: RefCell<HashMap<AgentKind, Option<cairo::ImageSurface>>> =
            RefCell::new(HashMap::new());
    }
    let bytes = match kind {
        AgentKind::Claude => CLAUDE_LOGO_PNG,
        AgentKind::Codex => CODEX_LOGO_PNG,
        _ => return None,
    };
    CACHE.with(|cache| {
        if let Some(cached) = cache.borrow().get(&kind) {
            return cached.clone();
        }
        let surface = image::load_from_memory(bytes)
            .ok()
            .and_then(|img| crate::pet::to_surface(&img.to_rgba8()));
        cache.borrow_mut().insert(kind, surface.clone());
        surface
    })
}

/// Paints `surface` centred inside the `w`×`h` box, scaled to occupy `fill`
/// (0.0–1.0) of the shorter side while preserving aspect ratio.
fn draw_surface_fit(cr: &CairoContext, w: i32, h: i32, surface: &cairo::ImageSurface, fill: f64) {
    let (fw, fh) = (surface.width() as f64, surface.height() as f64);
    if fw <= 0.0 || fh <= 0.0 {
        return;
    }
    let scale = (w.min(h) as f64) * fill / fw.max(fh);
    let (dw, dh) = (fw * scale, fh * scale);
    let x = (w as f64 - dw) / 2.0;
    let y = (h as f64 - dh) / 2.0;

    let _ = cr.save();
    cr.translate(x, y);
    cr.scale(scale, scale);
    let _ = cr.set_source_surface(surface, 0.0, 0.0);
    let _ = cr.paint();
    let _ = cr.restore();
}

/// Drawn when the agent has no installed pet pack: a small filled dot in the
/// session's state colour.
fn draw_pet_fallback(cr: &CairoContext, w: i32, h: i32, hex: &str) {
    let (w, h) = (w as f64, h as f64);
    let (r, g, b) = parse_hex(hex);
    cr.arc(w / 2.0, h / 2.0, w.min(h) * 0.34, 0.0, std::f64::consts::TAU);
    cr.set_source_rgba(r, g, b, 0.95);
    let _ = cr.fill();
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

/// Parses a `#rrggbb` colour into 0.0–1.0 components (greys on any malformed
/// input — these come from our own `color_dot`, so it never realistically does).
fn parse_hex(hex: &str) -> (f64, f64, f64) {
    let h = hex.trim_start_matches('#');
    let comp = |i: usize| {
        u8::from_str_radix(h.get(i..i + 2).unwrap_or("99"), 16).unwrap_or(0x99) as f64 / 255.0
    };
    if h.len() == 6 {
        (comp(0), comp(2), comp(4))
    } else {
        (0.6, 0.6, 0.6)
    }
}

fn state_word(state: AgentState) -> &'static str {
    match state {
        AgentState::Registered => "registered",
        AgentState::Working => "working",
        AgentState::Waiting => "waiting for input",
        AgentState::Done => "done",
        AgentState::Idle => "idle",
    }
}

fn color_dot(state: AgentState) -> &'static str {
    match state {
        AgentState::Working => "#4ac6f0",
        AgentState::Waiting => "#f0b020",
        AgentState::Done => "#56d472",
        AgentState::Registered => "#9aa0a6",
        AgentState::Idle => "#666666",
    }
}

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

fn glib_escape(s: &str) -> String {
    gtk4::glib::markup_escape_text(s).to_string()
}
