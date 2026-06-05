//! Phase-0 feasibility spike for AgentPet-Linux's desktop pet.
//!
//! Proves the riskiest unknown: that a GTK4 window under XWayland can be made
//! **borderless, always-on-top, skip-taskbar, transparent, and click-through**,
//! and repositioned (since GTK4 deleted the WM-hint and move APIs). All of those
//! are applied via raw X11 (`x11rb`) on the window's XID, which we obtain from
//! `gdk4-x11` in the `realize` handler.
//!
//! Run:
//!   pet-spike                 # interactive: a draggable floating sprite
//!   pet-spike --click-through # input-transparent: clicks pass through to apps below
//!
//! If the interactive window floats above everything, ignores the taskbar/alt-tab,
//! has transparent corners, and can be dragged — and `--click-through` lets you
//! click "through" it onto the window beneath — the gate passes and the real pet
//! (Phase 4) can be built on this approach.

use gtk4::cairo::Context as CairoContext;
use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, CssProvider, DrawingArea, GestureDrag};
use std::cell::Cell;
use std::rc::Rc;

use x11rb::connection::Connection;
use x11rb::protocol::shape::{ConnectionExt as _, SK, SO};
use x11rb::protocol::xproto::{
    AtomEnum, ClipOrdering, ConfigureWindowAux, ConnectionExt as _, PropMode,
};

const APP_ID: &str = "online.thenightwatcher.agentpet.PetSpike";
const SIZE: i32 = 180;

fn main() -> glib::ExitCode {
    // Mirror cliccy: force the X11 backend so we run under XWayland on Wayland
    // sessions and get a real X11 window we can manipulate via x11rb.
    // SAFETY: set before any GDK/display initialisation, single-threaded here.
    unsafe { std::env::set_var("GDK_BACKEND", "x11") };

    let click_through = std::env::args().any(|a| a == "--click-through");

    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| build_window(app, click_through));
    // Don't treat CLI args as files to open.
    app.run_with_args::<&str>(&[])
}

fn build_window(app: &Application, click_through: bool) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("AgentPet pet spike")
        .default_width(SIZE)
        .default_height(SIZE)
        .decorated(false)
        .resizable(false)
        .build();

    // Transparent window background so only the sprite is visible.
    let provider = CssProvider::new();
    provider.load_from_data("window, .background { background: transparent; }");
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    // The "sprite": a circle that bobs, so we can see liveness + transparency.
    let phase = Rc::new(Cell::new(0.0_f64));
    let area = DrawingArea::new();
    area.set_content_width(SIZE);
    area.set_content_height(SIZE);
    {
        let phase = phase.clone();
        area.set_draw_func(move |_area, cr, w, h| draw_sprite(cr, w, h, phase.get()));
    }
    {
        let phase = phase.clone();
        area.add_tick_callback(move |area, _clock| {
            phase.set(phase.get() + 0.05);
            area.queue_draw();
            glib::ControlFlow::Continue
        });
    }
    window.set_child(Some(&area));

    // Drag-to-move (only meaningful when the window is NOT click-through, since
    // an empty input region passes the pointer straight through).
    if !click_through {
        attach_drag(&window);
    }

    // Apply the X11 window manager hints once the surface (and XID) exist.
    window.connect_realize(move |win| {
        if let Some(xid) = window_xid(win) {
            if let Err(e) = apply_x11_traits(xid, click_through) {
                eprintln!("pet-spike: failed to apply X11 traits: {e}");
            } else {
                eprintln!(
                    "pet-spike: applied keep-above + skip-taskbar{} on XID 0x{xid:x}",
                    if click_through { " + click-through" } else { "" }
                );
            }
        } else {
            eprintln!(
                "pet-spike: could not get an X11 XID — are we really under XWayland? \
                 (GDK_BACKEND=x11 should force it)"
            );
        }
    });

    window.present();
}

/// Draws a bobbing two-tone blob centred in the widget, with transparent margins
/// so the click-through / transparency can be visually confirmed.
fn draw_sprite(cr: &CairoContext, w: i32, h: i32, phase: f64) {
    let w = w as f64;
    let h = h as f64;
    let bob = phase.sin() * 8.0;
    let cx = w / 2.0;
    let cy = h / 2.0 + bob;
    let r = w.min(h) * 0.32;

    // body
    cr.arc(cx, cy, r, 0.0, std::f64::consts::TAU);
    cr.set_source_rgba(0.36, 0.78, 0.96, 0.95);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(0.10, 0.30, 0.45, 0.9);
    cr.set_line_width(3.0);
    let _ = cr.stroke();

    // two eyes, so "up" is obvious
    for dx in [-0.35, 0.35] {
        cr.arc(cx + r * dx, cy - r * 0.15, r * 0.12, 0.0, std::f64::consts::TAU);
        cr.set_source_rgba(0.05, 0.10, 0.15, 1.0);
        let _ = cr.fill();
    }
}

/// Resolves the X11 window id of a realised GTK window via `gdk4-x11`.
fn window_xid(win: &ApplicationWindow) -> Option<u32> {
    let surface = win.surface()?;
    let x11 = surface.downcast::<gdk4_x11::X11Surface>().ok()?;
    Some(x11.xid() as u32)
}

/// Sets `_NET_WM_STATE_ABOVE` + skip-taskbar/pager, and (optionally) an empty
/// input shape for click-through, on the given window via a fresh X connection.
fn apply_x11_traits(window: u32, click_through: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, _screen) = x11rb::connect(None)?;

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

    if click_through {
        // Empty input region ⇒ the compositor routes all pointer/keyboard input
        // to whatever is beneath the window. The window stays fully visible.
        conn.shape_rectangles(
            SO::SET,
            SK::INPUT,
            ClipOrdering::UNSORTED,
            window,
            0,
            0,
            &[],
        )?;
    }

    conn.flush()?;
    Ok(())
}

fn intern(conn: &impl Connection, name: &[u8]) -> Result<u32, Box<dyn std::error::Error>> {
    Ok(conn.intern_atom(false, name)?.reply()?.atom)
}

/// Repositions the (managed) window by issuing X11 `ConfigureWindow`, since
/// GTK4 removed `gtk_window_move`. Tracks the window's root-space origin at drag
/// start and offsets it by the gesture delta.
fn attach_drag(window: &ApplicationWindow) {
    let drag = GestureDrag::new();
    let origin = Rc::new(Cell::new((0_i32, 0_i32)));

    {
        let window = window.clone();
        let origin = origin.clone();
        drag.connect_drag_begin(move |_g, _x, _y| {
            if let Some(xid) = window_xid(&window) {
                if let Ok(pos) = window_root_origin(xid) {
                    origin.set(pos);
                }
            }
        });
    }
    {
        let window = window.clone();
        let origin = origin.clone();
        drag.connect_drag_update(move |_g, dx, dy| {
            if let Some(xid) = window_xid(&window) {
                let (ox, oy) = origin.get();
                let _ = move_window(xid, ox + dx as i32, oy + dy as i32);
            }
        });
    }
    window.add_controller(drag);
}

/// The window's top-left in root coordinates (for drag math).
fn window_root_origin(window: u32) -> Result<(i32, i32), Box<dyn std::error::Error>> {
    let (conn, screen_num) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen_num].root;
    let t = conn.translate_coordinates(window, root, 0, 0)?.reply()?;
    Ok((t.dst_x as i32, t.dst_y as i32))
}

fn move_window(window: u32, x: i32, y: i32) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, _screen) = x11rb::connect(None)?;
    conn.configure_window(window, &ConfigureWindowAux::new().x(x).y(y))?;
    conn.flush()?;
    Ok(())
}
