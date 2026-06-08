//! The monitor window: a live list of agent sessions with a status dot, project,
//! current activity, and a per-state elapsed timer. Ports `MenuBarContentView`.

use crate::snapshot::UiCommand;
use agentpet_core::session::AgentSession;
use agentpet_core::state::AgentState;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Label, ListBox, Orientation,
    PolicyType, ScrolledWindow,
};
use std::cell::RefCell;
use std::rc::Rc;

pub struct MonitorWindow {
    window: ApplicationWindow,
    list: ListBox,
    sessions: Rc<RefCell<Vec<AgentSession>>>,
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

        // Tick the elapsed timers once a second — but only while the window is
        // actually on screen (it hides on close and would otherwise re-render
        // invisibly forever).
        {
            let (window, list, sessions) = (window.clone(), list.clone(), sessions.clone());
            gtk4::glib::timeout_add_seconds_local(1, move || {
                if window.is_visible() {
                    render(&list, &sessions.borrow());
                }
                gtk4::glib::ControlFlow::Continue
            });
        }

        MonitorWindow { window, list, sessions }
    }

    pub fn set_sessions(&self, sessions: &[AgentSession]) {
        *self.sessions.borrow_mut() = sessions.to_vec();
        if self.window.is_visible() {
            render(&self.list, &self.sessions.borrow());
        }
    }

    pub fn show(&self) {
        // Render before presenting — the list may be stale from a hidden spell.
        render(&self.list, &self.sessions.borrow());
        self.window.present();
    }
}

fn render(list: &ListBox, sessions: &[AgentSession]) {
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
        list.append(&row(s, now));
    }
}

fn row(s: &AgentSession, now: f64) -> Label {
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

    let markup = format!(
        "<span foreground='{dot}'>●</span>  <b>{}</b>\n<span size='small' foreground='#999'>{} · {}</span>",
        glib_escape(&project),
        glib_escape(&activity),
        elapsed,
    );
    let label = Label::new(None);
    label.set_markup(&markup);
    label.set_xalign(0.0);
    label.set_margin_top(6);
    label.set_margin_bottom(6);
    label.set_margin_start(10);
    label.set_margin_end(10);
    label
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
