//! Chat tab: configure the pet's speech bubble — show/hide it, pick between the
//! built-in line set and custom lines, and edit the custom lines per mood (one
//! line per text row). Every change persists to `Config` and sends
//! `UiCommand::ReloadPets` so live pets pick it up immediately.

use crate::snapshot::UiCommand;
use agentpet_core::chat;
use agentpet_core::config::Config;
use agentpet_core::state::PetMood;
use async_channel::Sender;
use gtk4::prelude::*;
use gtk4::{
    glib, Align, Box as GtkBox, CheckButton, Label, Orientation, PolicyType, ScrolledWindow,
    Switch, TextView, WrapMode,
};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

/// Pause (ms) after the last keystroke before a custom-line edit is persisted,
/// so typing doesn't write the config file per character.
const SAVE_DEBOUNCE_MS: u64 = 400;

/// Loads the config, applies `update`, saves, and reloads live pets — the one
/// write path every control on this page funnels through.
fn persist(cmd: &Sender<UiCommand>, update: impl FnOnce(&mut Config)) {
    let mut cfg = Config::load();
    update(&mut cfg);
    if let Err(e) = cfg.save() {
        eprintln!("agentpet: saving chat settings failed: {e}");
    }
    let _ = cmd.try_send(UiCommand::ReloadPets);
}

pub fn build(cmd: Sender<UiCommand>) -> ScrolledWindow {
    let cfg = Config::load();

    let page = GtkBox::new(Orientation::Vertical, 8);
    page.set_margin_top(22);
    page.set_margin_bottom(26);
    page.set_margin_start(24);
    page.set_margin_end(24);

    page.append(&group_title("Speech bubble"));
    let sub = Label::new(Some(
        "Each pet shows a small bubble above its head with a short line that matches its mood.",
    ));
    sub.set_xalign(0.0);
    sub.set_wrap(true);
    sub.add_css_class("group-sub");
    page.append(&sub);
    page.append(&toggle_group(&cfg, &cmd));

    let lines_title = group_title("Lines");
    lines_title.set_margin_top(16);
    page.append(&lines_title);
    page.append(&source_group(&cfg, &cmd));

    let custom_title = group_title("Custom lines");
    custom_title.set_margin_top(16);
    page.append(&custom_title);
    let hint = Label::new(Some(
        "One line per row; the pet rotates through them. Leave a mood empty to keep its built-in lines. Applies when \"My custom lines\" is selected.",
    ));
    hint.set_xalign(0.0);
    hint.set_wrap(true);
    hint.add_css_class("group-sub");
    page.append(&hint);
    for mood in PetMood::ALL {
        page.append(&mood_editor(mood, &cfg, &cmd));
    }

    let scrolled = ScrolledWindow::new();
    scrolled.set_policy(PolicyType::Never, PolicyType::Automatic);
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&page));
    scrolled
}

fn group_title(text: &str) -> Label {
    let label = Label::new(Some(text));
    label.set_xalign(0.0);
    label.add_css_class("group-title");
    label
}

/// Boxed row with the show/hide switch.
fn toggle_group(cfg: &Config, cmd: &Sender<UiCommand>) -> GtkBox {
    let boxed = GtkBox::new(Orientation::Vertical, 0);
    boxed.add_css_class("boxed");

    let row = GtkBox::new(Orientation::Horizontal, 14);
    let title = Label::new(Some("Show chat bubble"));
    title.set_xalign(0.0);
    title.add_css_class("rtitle");
    title.set_hexpand(true);

    let sw = Switch::new();
    sw.set_valign(Align::Center);
    sw.set_active(cfg.show_chat);
    let cmd = cmd.clone();
    sw.connect_state_set(move |_, state| {
        persist(&cmd, |cfg| cfg.show_chat = state);
        glib::Propagation::Proceed
    });

    row.append(&title);
    row.append(&sw);
    boxed.append(&row);
    boxed
}

/// Boxed radio pair selecting `chat_source` ("system" / "custom").
fn source_group(cfg: &Config, cmd: &Sender<UiCommand>) -> GtkBox {
    let boxed = GtkBox::new(Orientation::Vertical, 0);
    boxed.add_css_class("boxed");

    let system = CheckButton::with_label("Built-in lines");
    let custom = CheckButton::with_label("My custom lines");
    custom.set_group(Some(&system));
    // Set the initial state BEFORE connecting handlers so restoring the saved
    // choice doesn't immediately re-save it.
    if cfg.chat_source == "custom" {
        custom.set_active(true);
    } else {
        system.set_active(true);
    }
    for (button, source) in [(&system, "system"), (&custom, "custom")] {
        let cmd = cmd.clone();
        button.connect_toggled(move |b| {
            if b.is_active() {
                persist(&cmd, |cfg| cfg.chat_source = source.to_string());
            }
        });
    }

    let row = GtkBox::new(Orientation::Vertical, 6);
    row.append(&system);
    row.append(&custom);
    boxed.append(&row);
    boxed
}

/// One mood's editor: title, the built-in lines as a reference, and a TextView
/// whose content persists to `chat_custom[mood]` on a typing debounce.
fn mood_editor(mood: PetMood, cfg: &Config, cmd: &Sender<UiCommand>) -> GtkBox {
    let boxed = GtkBox::new(Orientation::Vertical, 0);
    boxed.add_css_class("boxed");
    boxed.set_margin_top(6);

    let inner = GtkBox::new(Orientation::Vertical, 4);
    let title = Label::new(Some(&mood_label(mood)));
    title.set_xalign(0.0);
    title.add_css_class("rtitle");
    let builtin = Label::new(Some(&format!("Built-in: {}", chat::system_lines(mood).join("  ·  "))));
    builtin.set_xalign(0.0);
    builtin.set_wrap(true);
    builtin.add_css_class("rsub");

    let view = TextView::new();
    view.set_wrap_mode(WrapMode::WordChar);
    view.set_accepts_tab(false); // Tab moves focus instead of inserting \t
    let buffer = view.buffer();
    buffer.set_text(&cfg.chat_custom.get(mood.raw()).map(|l| l.join("\n")).unwrap_or_default());
    {
        // Debounce token: each keystroke bumps the generation; only the timeout
        // matching the latest one writes (same pattern as the pet-size slider).
        let debounce = Rc::new(Cell::new(0u64));
        let cmd = cmd.clone();
        buffer.connect_changed(move |buf| {
            let text = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
            let generation = debounce.get().wrapping_add(1);
            debounce.set(generation);
            let (debounce, cmd) = (debounce.clone(), cmd.clone());
            glib::timeout_add_local_once(Duration::from_millis(SAVE_DEBOUNCE_MS), move || {
                if debounce.get() == generation {
                    let lines: Vec<String> =
                        text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
                    persist(&cmd, |cfg| {
                        cfg.chat_custom.insert(mood.raw().to_string(), lines);
                    });
                }
            });
        });
    }

    inner.append(&title);
    inner.append(&builtin);
    inner.append(&view);
    boxed.append(&inner);
    boxed
}

/// "idle" → "Idle" etc., for the per-mood editor titles.
fn mood_label(mood: PetMood) -> String {
    let raw = mood.raw();
    let mut chars = raw.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
