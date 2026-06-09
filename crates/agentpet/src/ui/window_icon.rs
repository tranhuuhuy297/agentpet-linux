//! Stamps `_NET_WM_ICON` onto the app's toplevel windows so the GNOME dock and
//! alt-tab show the otter immediately — the same approach Electron/Qt apps use.
//!
//! GTK4 dropped the per-window icon API and leaves the dock to resolve the
//! window → `.desktop` → `Icon=` chain through the icon theme. That path only
//! refreshes on a GNOME relogin and shows nothing when the app runs uninstalled
//! (source checkout / AppImage). Embedding the pixels in each window sidesteps
//! the theme/cache entirely, so the icon is correct the moment a window maps.
//! The desktop-file icon stays as the launcher (app-grid) fallback.

use gtk4::prelude::*;
use gtk4::ApplicationWindow;
use std::sync::OnceLock;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, PropMode};
use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;

/// Full-colour otter — the same source image as the installed desktop icon.
const APP_ICON_PNG: &[u8] = include_bytes!("../../../../assets/agentpet.png");

/// Sizes baked into the property. The WM picks the closest one, so a small set
/// covering dock + alt-tab renders crisply without shipping the full 512².
const ICON_SIZES: [u32; 4] = [48, 64, 128, 256];

/// Connects a map handler that writes `_NET_WM_ICON` once the window is realized
/// (the X11 surface — and thus the XID — only exists after map). Setting it on
/// every map is harmless (REPLACE) and covers hide-on-close windows that remap.
pub fn install(window: &ApplicationWindow) {
    window.connect_map(|win| {
        let data = icon_argb();
        if data.is_empty() {
            return;
        }
        if let Some(xid) = window_xid(win) {
            if let Err(e) = apply(xid, data) {
                eprintln!("agentpet: window icon setup failed: {e}");
            }
        }
    });
}

/// `_NET_WM_ICON` payload (EWMH): for each size, `width, height,` then
/// `width * height` pixels packed as `0xAARRGGBB` CARDINALs. Decoded and scaled
/// once, then shared across every window.
fn icon_argb() -> &'static [u32] {
    static DATA: OnceLock<Vec<u32>> = OnceLock::new();
    DATA.get_or_init(|| {
        let Ok(img) = image::load_from_memory(APP_ICON_PNG) else {
            return Vec::new();
        };
        let rgba = img.to_rgba8();
        let mut data = Vec::new();
        for &size in &ICON_SIZES {
            let scaled =
                image::imageops::resize(&rgba, size, size, image::imageops::FilterType::Lanczos3);
            data.push(size);
            data.push(size);
            for p in scaled.pixels() {
                let [r, g, b, a] = p.0;
                data.push((a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32);
            }
        }
        data
    })
}

fn window_xid(win: &ApplicationWindow) -> Option<u32> {
    let surface = win.surface()?;
    let x11 = surface.downcast::<gdk4_x11::X11Surface>().ok()?;
    Some(x11.xid() as u32)
}

fn apply(window: u32, data: &[u32]) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, _screen) = x11rb::connect(None)?;
    let atom = conn.intern_atom(false, b"_NET_WM_ICON")?.reply()?.atom;
    conn.change_property32(PropMode::REPLACE, window, atom, AtomEnum::CARDINAL, data)?;
    conn.flush()?;
    Ok(())
}
