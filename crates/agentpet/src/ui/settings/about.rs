//! About tab: app identity, project links, and credits, centred like the
//! design reference.

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Image, Label, Orientation, PolicyType, ScrolledWindow};

pub fn build() -> ScrolledWindow {
    let page = GtkBox::new(Orientation::Vertical, 0);
    page.set_margin_top(34);
    page.set_margin_bottom(26);
    page.set_margin_start(24);
    page.set_margin_end(24);

    let icon = app_icon();
    icon.set_pixel_size(88);
    icon.set_margin_bottom(16);
    page.append(&icon);

    let title = Label::new(Some("AgentPet for Linux"));
    title.add_css_class("about-title");
    page.append(&title);

    let ver = Label::new(Some(&format!("v{} · Rust + GTK4", env!("CARGO_PKG_VERSION"))));
    ver.add_css_class("about-ver");
    ver.set_margin_top(3);
    page.append(&ver);

    let desc = Label::new(Some(
        "Watch several AI coding agents running in parallel and see — at a glance — \
         which one is working, which is done, and which is waiting for your input.",
    ));
    desc.set_wrap(true);
    desc.set_justify(gtk4::Justification::Center);
    desc.set_max_width_chars(46);
    desc.add_css_class("about-desc");
    desc.set_margin_top(16);
    page.append(&desc);

    let links = GtkBox::new(Orientation::Vertical, 0);
    links.add_css_class("boxed");
    links.set_margin_top(22);
    links.append(&link_row(
        "Source code",
        "<a href='https://github.com/tranhuuhuy297/agentpet-linux'>github.com/tranhuuhuy297/agentpet-linux ↗</a>",
    ));
    links.append(&link_row(
        "Based on",
        "<a href='https://github.com/ntd4996/agentpet'>AgentPet for macOS ↗</a>",
    ));
    links.append(&link_row(
        "Pet library",
        "<a href='https://petdex.crafter.run/'>petdex.crafter.run ↗</a>",
    ));
    links.append(&link_row("License", "MIT"));
    links.append(&link_row("Check for updates", "<tt>agentpet update</tt>"));
    page.append(&links);

    let credits = Label::new(Some(
        "App code MIT-licensed. Pet assets are owned by their submitters.\n\
         Runs under XWayland · keep-above · skip-taskbar.",
    ));
    credits.set_justify(gtk4::Justification::Center);
    credits.add_css_class("credits");
    credits.set_margin_top(18);
    page.append(&credits);

    let scrolled = ScrolledWindow::new();
    scrolled.set_policy(PolicyType::Never, PolicyType::Automatic);
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&page));
    scrolled
}

/// The installed app icon ("agentpet" in the hicolor theme), falling back to
/// the repo asset when running from a source checkout.
fn app_icon() -> Image {
    let themed = gtk4::gdk::Display::default()
        .map(|d| gtk4::IconTheme::for_display(&d).has_icon("agentpet"))
        .unwrap_or(false);
    if themed {
        return Image::from_icon_name("agentpet");
    }
    // Source checkout: target/{debug,release}/agentpet → ../../../assets/.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(repo) = exe.ancestors().nth(3) {
            let png = repo.join("assets/agentpet.png");
            if png.exists() {
                return Image::from_file(png);
            }
        }
    }
    Image::from_icon_name("application-x-executable")
}

/// Boxed-list row with a plain left label and a markup value on the right.
fn link_row(left: &str, value_markup: &str) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 12);
    let l = Label::new(Some(left));
    l.set_xalign(0.0);
    l.set_hexpand(true);
    l.add_css_class("rtitle");
    let v = Label::new(None);
    v.set_markup(value_markup);
    v.set_xalign(1.0);
    v.add_css_class("rsub");
    row.append(&l);
    row.append(&v);
    row
}
