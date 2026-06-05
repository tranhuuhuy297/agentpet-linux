//! General tab: per-agent hook install toggles and autostart, rendered as
//! boxed preference groups with the agent's brand-coloured monogram badge.

use crate::platform::autostart;
use agentpet_core::catalog::AgentCatalog;
use agentpet_core::hooks::{AgentHooks, HookInstaller};
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Label, Orientation, PolicyType, ScrolledWindow, Switch};

pub fn build() -> ScrolledWindow {
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "agentpet".to_string());

    let page = GtkBox::new(Orientation::Vertical, 8);
    page.set_margin_top(22);
    page.set_margin_bottom(26);
    page.set_margin_start(24);
    page.set_margin_end(24);

    page.append(&group_title("Connect your agents"));
    let sub = Label::new(Some(
        "Flip one on and AgentPet writes its hook config. Each session then reports its real state.",
    ));
    sub.set_xalign(0.0);
    sub.set_wrap(true);
    sub.add_css_class("group-sub");
    page.append(&sub);

    let agents = GtkBox::new(Orientation::Vertical, 0);
    agents.add_css_class("boxed");
    for agent in AgentCatalog::all() {
        let Some(spec) = AgentHooks::spec(agent.kind) else { continue };
        let command = format!("\"{}\" hook --agent {}", exe, agent.kind.raw());
        agents.append(&agent_row(&agent, &command, spec));
    }
    page.append(&agents);

    let startup = group_title("Startup");
    startup.set_margin_top(16);
    page.append(&startup);
    page.append(&startup_group(exe));

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

/// One agent row: monogram badge, name + mono hook command (or warning note),
/// and the install/uninstall switch (same toggle wiring as before).
fn agent_row(
    agent: &agentpet_core::catalog::AgentIntegration,
    command: &str,
    spec: agentpet_core::hooks::AgentHookSpec,
) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 14);

    let raw = agent.kind.raw();
    let badge = Label::new(Some(&raw[..1].to_uppercase()));
    badge.set_valign(Align::Center);
    badge.add_css_class("ricon");
    badge.add_css_class(&format!("agent-{raw}"));

    let text = GtkBox::new(Orientation::Vertical, 2);
    let name = Label::new(Some(agent.display_name));
    name.set_xalign(0.0);
    name.add_css_class("rtitle");
    text.append(&name);
    let sub = match agent.note {
        Some(note) => {
            let n = Label::new(Some(&format!("⚠ {note}")));
            n.add_css_class("warn");
            n
        }
        None => Label::new(Some(&format!("agentpet hook --agent {raw}"))),
    };
    sub.set_xalign(0.0);
    sub.set_wrap(true);
    sub.add_css_class("rsub");
    sub.add_css_class("mono");
    text.append(&sub);
    text.set_hexpand(true);

    let sw = Switch::new();
    sw.set_valign(Align::Center);
    sw.set_active(HookInstaller::is_installed_on_disk(&spec.settings_path, &spec.events, spec.style));
    {
        let command = command.to_string();
        let (path, events, style) = (spec.settings_path.clone(), spec.events.clone(), spec.style);
        sw.connect_state_set(move |_, state| {
            let result = if state {
                HookInstaller::install_to_disk(&command, &path, &events, style)
            } else {
                HookInstaller::uninstall_from_disk(&path, &events, style)
            };
            if let Err(e) = result {
                eprintln!("agentpet: hook toggle failed: {e}");
            }
            gtk4::glib::Propagation::Proceed
        });
    }

    row.append(&badge);
    row.append(&text);
    row.append(&sw);
    row
}

/// Boxed Startup group with the launch-at-login switch.
fn startup_group(exe: String) -> GtkBox {
    let boxed = GtkBox::new(Orientation::Vertical, 0);
    boxed.add_css_class("boxed");

    let row = GtkBox::new(Orientation::Horizontal, 14);
    let text = GtkBox::new(Orientation::Vertical, 2);
    let title = Label::new(Some("Start AgentPet at login"));
    title.set_xalign(0.0);
    title.add_css_class("rtitle");
    let sub = Label::new(Some("Adds a desktop entry to autostart"));
    sub.set_xalign(0.0);
    sub.add_css_class("rsub");
    text.append(&title);
    text.append(&sub);
    text.set_hexpand(true);

    let sw = Switch::new();
    sw.set_valign(Align::Center);
    sw.set_active(autostart::is_enabled());
    sw.connect_state_set(move |_, state| {
        let result = if state { autostart::enable(&exe) } else { autostart::disable() };
        if let Err(e) = result {
            eprintln!("agentpet: autostart toggle failed: {e}");
        }
        gtk4::glib::Propagation::Proceed
    });

    row.append(&text);
    row.append(&sw);
    boxed.append(&row);
    boxed
}
