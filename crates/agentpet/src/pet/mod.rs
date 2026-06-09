//! The floating desktop pet window. Built on the validated Phase-0 approach:
//! a borderless, transparent, always-on-top (EWMH keep-above), skip-taskbar GTK4
//! window under XWayland, draggable via X11 `ConfigureWindow`.
//!
//! Renders the selected pet pack's sprite frames (mood → clip via bindings,
//! animated at a per-mood frame rate). Falls back to a mood-coloured blob when
//! no pack is installed.

use crate::snapshot::UiCommand;
use agentpet_core::sprite::{PetBindings, PetPack};
use agentpet_core::state::PetMood;
use gtk4::cairo;
use gtk4::cairo::Context as CairoContext;
use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, CssProvider, DrawingArea, GestureClick, GestureDrag};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageData, ClientMessageEvent, ConfigureWindowAux, ConnectionExt, EventMask,
    PropMode,
};
use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;

const SIZE: i32 = 140;
/// Horizontal gap between adjacent agents' pets at first placement.
const SLOT_GAP: i32 = 24;
/// Top-left anchor for the first pet; later slots step to the right.
const ANCHOR_X: i32 = 80;
const ANCHOR_Y: i32 = 120;

/// Sliced pet-pack frames, ready to paint (one inner Vec per animation clip).
type Clips = Rc<RefCell<Vec<Vec<cairo::ImageSurface>>>>;

pub struct PetWindow {
    window: ApplicationWindow,
    mood: Rc<Cell<PetMood>>,
    clips: Clips,
    bindings: Rc<RefCell<PetBindings>>,
}

impl PetWindow {
    /// Creates a pet window. `slot` (0, 1, 2, …) staggers its initial position so
    /// each agent's pet starts in a distinct, non-overlapping spot. `label` names
    /// the agent (e.g. "Claude Code"), drawn as a caption so identical pet packs
    /// stay tellable apart.
    pub fn new(
        app: &Application,
        cmd: async_channel::Sender<UiCommand>,
        slot: i32,
        label: &str,
    ) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("AgentPet")
            .default_width(SIZE)
            .default_height(SIZE)
            .decorated(false)
            .resizable(false)
            .build();
        // Scope transparency to the pet alone — a display-wide rule would make
        // the Settings/Monitor windows transparent too.
        window.add_css_class("agentpet-pet");

        install_transparent_css();

        let mood = Rc::new(Cell::new(PetMood::Idle));
        let phase = Rc::new(Cell::new(0.0_f64));
        // While the user is dragging, the pet holds still: pausing the tick frees
        // the main loop to service motion events and stops it competing with
        // window-move redraws (which was part of the residual drag jitter).
        let dragging = Rc::new(Cell::new(false));
        let clips: Clips = Rc::new(RefCell::new(Vec::new()));
        let bindings = Rc::new(RefCell::new(PetBindings::defaults(0)));

        let area = DrawingArea::new();
        area.set_content_width(SIZE);
        area.set_content_height(SIZE);
        {
            let (mood, phase, clips, bindings) =
                (mood.clone(), phase.clone(), clips.clone(), bindings.clone());
            let label = label.to_string();
            area.set_draw_func(move |_, cr, w, h| {
                draw(cr, w, h, mood.get(), phase.get(), &clips.borrow(), &bindings.borrow());
                draw_label(cr, w, h, &label);
            });
        }
        {
            // A coarse fixed timer instead of a per-vsync tick: advancing the
            // phase at ~12.5 Hz and redrawing only when the visible output
            // (frame index or pixel-snapped bob) actually changed keeps the
            // always-on pet near-zero CPU while idle.
            // Hold the area weakly so the timer self-cancels once this pet's
            // window is closed (a pet per agent comes and goes); a strong ref
            // would keep ticking and redrawing a destroyed widget forever.
            let (mood, phase, dragging) = (mood.clone(), phase.clone(), dragging.clone());
            let area_weak = area.downgrade();
            let last_drawn = Cell::new((PetMood::Idle as u8, 0_usize, i32::MIN));
            glib::timeout_add_local(std::time::Duration::from_millis(TICK_MS), move || {
                let Some(area) = area_weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                if dragging.get() {
                    return glib::ControlFlow::Continue;
                }
                let m = mood.get();
                phase.set(phase.get() + phase_rate(m) * (TICK_MS as f64 / 1000.0));
                let p = phase.get();
                let key = (
                    m as u8,
                    (p * frame_rate(m)) as usize,
                    (p.sin() * bob_amplitude(m)).round() as i32,
                );
                if last_drawn.get() != key {
                    last_drawn.set(key);
                    area.queue_draw();
                }
                glib::ControlFlow::Continue
            });
        }
        window.set_child(Some(&area));

        attach_drag(&window, dragging);
        attach_right_click(&window, cmd);

        window.connect_map(move |win| {
            if let Some(xid) = window_xid(win) {
                if let Err(e) = apply_pet_traits(xid) {
                    eprintln!("agentpet: pet window X11 setup failed: {e}");
                }
                // Stagger each agent's pet so multiple pets don't stack.
                let x = ANCHOR_X + slot * (SIZE + SLOT_GAP);
                let _ = move_window(xid, x, ANCHOR_Y);
            }
        });

        window.present();
        PetWindow { window, mood, clips, bindings }
    }

    pub fn set_mood(&self, mood: PetMood) {
        self.mood.set(mood);
    }

    /// Closes and destroys the window (its agent went idle / ended).
    pub fn close(&self) {
        self.window.close();
    }

    /// Loads a pet pack's frames (or clears them, falling back to the blob).
    pub fn set_pack(&self, pack: Option<&PetPack>) {
        match pack {
            Some(pack) => {
                let clips: Vec<Vec<cairo::ImageSurface>> = pack
                    .clips
                    .iter()
                    .map(|frames| frames.iter().filter_map(to_surface).collect())
                    .collect();
                *self.bindings.borrow_mut() = PetBindings::defaults(clips.len());
                *self.clips.borrow_mut() = clips;
            }
            None => {
                self.clips.borrow_mut().clear();
            }
        }
    }
}

fn install_transparent_css() {
    // One display-wide provider suffices; pets are created and destroyed
    // repeatedly, so guard against stacking a fresh provider on every pet.
    thread_local! {
        static INSTALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    if INSTALLED.with(|i| i.replace(true)) {
        return;
    }
    let provider = CssProvider::new();
    provider.load_from_data("window.agentpet-pet { background-color: transparent; }");
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn draw(
    cr: &CairoContext,
    w: i32,
    h: i32,
    mood: PetMood,
    phase: f64,
    clips: &[Vec<cairo::ImageSurface>],
    bindings: &PetBindings,
) {
    // Pixel-snapped so it matches the redraw-skipping key in the tick timer.
    let bob = (phase.sin() * bob_amplitude(mood)).round();
    if clips.is_empty() {
        draw_blob(cr, w, h, mood, bob);
        return;
    }
    let clip_idx = bindings.clip_index(mood).min(clips.len() - 1);
    let frames = &clips[clip_idx];
    if frames.is_empty() {
        draw_blob(cr, w, h, mood, bob);
        return;
    }
    let idx = ((phase * frame_rate(mood)) as usize) % frames.len();
    let surface = &frames[idx];

    let (fw, fh) = (surface.width() as f64, surface.height() as f64);
    let target = (w.min(h) as f64) * 0.92;
    let scale = target / fw.max(fh);
    let (dw, dh) = (fw * scale, fh * scale);
    let x = (w as f64 - dw) / 2.0;
    let y = (h as f64 - dh) / 2.0 + bob;

    let _ = cr.save();
    cr.translate(x, y);
    cr.scale(scale, scale);
    let _ = cr.set_source_surface(surface, 0.0, 0.0);
    let _ = cr.paint();
    let _ = cr.restore();
}

/// Animation timer period. 80 ms (12.5 Hz) is above every clip's effective
/// frame rate, so motion stays smooth while redraws stay rare.
const TICK_MS: u64 = 80;

/// Phase advance in rad/s per mood (matches the old per-vsync speeds at 60 Hz).
fn phase_rate(mood: PetMood) -> f64 {
    match mood {
        PetMood::Working | PetMood::Celebrate => 4.8,
        PetMood::Waiting => 3.0,
        _ => 1.8,
    }
}

fn bob_amplitude(mood: PetMood) -> f64 {
    match mood {
        PetMood::Working | PetMood::Celebrate => 9.0,
        PetMood::Waiting => 5.0,
        _ => 3.0,
    }
}

/// Frames advanced per unit phase — higher moods animate faster.
fn frame_rate(mood: PetMood) -> f64 {
    match mood {
        PetMood::Working | PetMood::Celebrate => 4.0,
        PetMood::Waiting => 2.0,
        _ => 1.5,
    }
}

fn draw_blob(cr: &CairoContext, w: i32, h: i32, mood: PetMood, bob: f64) {
    let w = w as f64;
    let h = h as f64;
    let (r, g, b) = mood_color(mood);
    let cx = w / 2.0;
    let cy = h / 2.0 + bob;
    let radius = w.min(h) * 0.32;

    cr.arc(cx, cy, radius, 0.0, std::f64::consts::TAU);
    cr.set_source_rgba(r, g, b, 0.95);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(r * 0.35, g * 0.35, b * 0.45, 0.9);
    cr.set_line_width(3.0);
    let _ = cr.stroke();

    for dx in [-0.35, 0.35] {
        cr.arc(cx + radius * dx, cy - radius * 0.15, radius * 0.12, 0.0, std::f64::consts::TAU);
        cr.set_source_rgba(0.05, 0.1, 0.15, 1.0);
        let _ = cr.fill();
    }
}

/// Draws the agent name as a translucent pill caption at the bottom of the pet,
/// so multiple pets (especially ones sharing a pack) stay identifiable.
fn draw_label(cr: &CairoContext, w: i32, h: i32, text: &str) {
    if text.is_empty() {
        return;
    }
    let (w, h) = (w as f64, h as f64);
    cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    cr.set_font_size(13.0);
    let Ok(ext) = cr.text_extents(text) else { return };

    let (pad_x, pad_y) = (8.0, 4.0);
    let bw = ext.width() + pad_x * 2.0;
    let bh = ext.height() + pad_y * 2.0;
    let bx = (w - bw) / 2.0;
    let by = h - bh - 2.0;

    rounded_rect(cr, bx, by, bw, bh, bh / 2.0);
    cr.set_source_rgba(0.08, 0.09, 0.12, 0.66);
    let _ = cr.fill();

    cr.set_source_rgba(0.96, 0.97, 1.0, 0.96);
    cr.move_to(bx + pad_x - ext.x_bearing(), by + pad_y - ext.y_bearing());
    let _ = cr.show_text(text);
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

fn mood_color(mood: PetMood) -> (f64, f64, f64) {
    match mood {
        PetMood::Idle => (0.60, 0.65, 0.72),
        PetMood::Working => (0.36, 0.78, 0.96),
        PetMood::Waiting => (0.98, 0.75, 0.25),
        PetMood::Done => (0.45, 0.85, 0.55),
        PetMood::Celebrate => (0.96, 0.55, 0.85),
    }
}

/// Converts an RGBA image to a premultiplied-ARGB32 cairo surface (BGRA byte
/// order on little-endian).
pub(crate) fn to_surface(img: &image::RgbaImage) -> Option<cairo::ImageSurface> {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).ok()?;
    let stride = surface.stride() as usize;
    {
        let mut data = surface.data().ok()?;
        for y in 0..h as usize {
            for x in 0..w as usize {
                let px = img.get_pixel(x as u32, y as u32).0;
                let a = px[3] as u16;
                let pr = (px[0] as u16 * a / 255) as u8;
                let pg = (px[1] as u16 * a / 255) as u8;
                let pb = (px[2] as u16 * a / 255) as u8;
                let off = y * stride + x * 4;
                data[off] = pb;
                data[off + 1] = pg;
                data[off + 2] = pr;
                data[off + 3] = px[3];
            }
        }
    }
    Some(surface)
}

// MARK: - X11 window traits (validated in pet-spike)

fn window_xid(win: &ApplicationWindow) -> Option<u32> {
    let surface = win.surface()?;
    let x11 = surface.downcast::<gdk4_x11::X11Surface>().ok()?;
    Some(x11.xid() as u32)
}

fn apply_pet_traits(window: u32) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, screen_num) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen_num].root;

    let net_wm_state = intern(&conn, b"_NET_WM_STATE")?;
    let above = intern(&conn, b"_NET_WM_STATE_ABOVE")?;
    let skip_taskbar = intern(&conn, b"_NET_WM_STATE_SKIP_TASKBAR")?;
    let skip_pager = intern(&conn, b"_NET_WM_STATE_SKIP_PAGER")?;

    conn.change_property32(
        PropMode::REPLACE,
        window,
        net_wm_state,
        AtomEnum::ATOM,
        &[above, skip_taskbar, skip_pager],
    )?;
    const ADD: u32 = 1;
    const SOURCE_APP: u32 = 1;
    for (a, b) in [(above, skip_taskbar), (skip_pager, 0)] {
        let data = ClientMessageData::from([ADD, a, b, SOURCE_APP, 0]);
        let event = ClientMessageEvent::new(32, window, net_wm_state, data);
        conn.send_event(
            false,
            root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        )?;
    }
    conn.flush()?;
    Ok(())
}

fn intern(conn: &impl Connection, name: &[u8]) -> Result<u32, Box<dyn std::error::Error>> {
    Ok(conn.intern_atom(false, name)?.reply()?.atom)
}

/// State held for the duration of a single drag gesture: one X11 connection
/// opened at drag start and reused for every motion event. Reconnecting per
/// `drag_update` (which fires dozens of times per second) paid a full X11
/// handshake each time — the original source of drag jitter.
struct DragSession {
    conn: x11rb::rust_connection::RustConnection,
    xid: u32,
    root: u32,
    /// Pointer-minus-window-origin (root coords) captured at grab time. Each
    /// event re-derives the window position from the live pointer so the grabbed
    /// pixel stays under the cursor. This is self-correcting: moving the window
    /// never feeds back into the gesture's (window-relative) coordinate frame,
    /// which is what made accumulating raw gesture deltas drift and jitter.
    grab_offset: (i32, i32),
}

impl DragSession {
    fn start(window: &ApplicationWindow) -> Option<Self> {
        let xid = window_xid(window)?;
        let (conn, screen_num) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots[screen_num].root;
        let origin = conn.translate_coordinates(xid, root, 0, 0).ok()?.reply().ok()?;
        let ptr = conn.query_pointer(root).ok()?.reply().ok()?;
        let grab_offset = (
            ptr.root_x as i32 - origin.dst_x as i32,
            ptr.root_y as i32 - origin.dst_y as i32,
        );
        Some(Self { conn, xid, root, grab_offset })
    }

    fn track_pointer(&self) -> Result<(), Box<dyn std::error::Error>> {
        let ptr = self.conn.query_pointer(self.root)?.reply()?;
        let (gx, gy) = self.grab_offset;
        self.conn.configure_window(
            self.xid,
            &ConfigureWindowAux::new()
                .x(ptr.root_x as i32 - gx)
                .y(ptr.root_y as i32 - gy),
        )?;
        self.conn.flush()?;
        Ok(())
    }
}

fn attach_drag(window: &ApplicationWindow, dragging: Rc<Cell<bool>>) {
    let drag = GestureDrag::new();
    let session: Rc<RefCell<Option<DragSession>>> = Rc::new(RefCell::new(None));
    {
        let (window, session, dragging) = (window.clone(), session.clone(), dragging.clone());
        drag.connect_drag_begin(move |_, _, _| {
            *session.borrow_mut() = DragSession::start(&window);
            dragging.set(true);
        });
    }
    {
        let session = session.clone();
        drag.connect_drag_update(move |_, _, _| {
            if let Some(s) = session.borrow().as_ref() {
                let _ = s.track_pointer();
            }
        });
    }
    {
        let session = session.clone();
        drag.connect_drag_end(move |_, _, _| {
            session.borrow_mut().take();
            dragging.set(false);
        });
    }
    window.add_controller(drag);
}

fn attach_right_click(window: &ApplicationWindow, cmd: async_channel::Sender<UiCommand>) {
    let click = GestureClick::new();
    click.set_button(3);
    click.connect_pressed(move |_, _, _, _| {
        let _ = cmd.try_send(UiCommand::ShowMonitor);
    });
    window.add_controller(click);
}

fn move_window(window: u32, x: i32, y: i32) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, _screen) = x11rb::connect(None)?;
    conn.configure_window(window, &ConfigureWindowAux::new().x(x).y(y))?;
    conn.flush()?;
    Ok(())
}
