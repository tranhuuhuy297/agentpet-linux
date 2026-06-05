//! StatusNotifierItem tray (ksni). Shows running/waiting state at a glance and
//! offers Monitor/Settings/Quit. Ports `StatusBarController.swift`.
//!
//! Requires the GNOME AppIndicator extension at runtime. A count badge rendered
//! into the icon pixmap is a follow-up; for now the count lives in the tooltip
//! and the icon changes on "needs input".

use crate::snapshot::UiCommand;
use async_channel::Sender;

pub struct AgentTray {
    pub running: usize,
    pub waiting: usize,
    pub cmd: Sender<UiCommand>,
}

impl ksni::Tray for AgentTray {
    fn id(&self) -> String {
        "agentpet".into()
    }

    fn title(&self) -> String {
        "AgentPet".into()
    }

    fn icon_name(&self) -> String {
        if self.waiting > 0 {
            "dialog-warning-symbolic".into()
        } else if self.running > 0 {
            "face-smile-symbolic".into()
        } else {
            "face-plain-symbolic".into()
        }
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let description = if self.waiting > 0 {
            format!("{} agent(s) need input", self.waiting)
        } else if self.running > 0 {
            format!("{} agent(s) working", self.running)
        } else {
            "No active agents".into()
        };
        ksni::ToolTip {
            title: "AgentPet".into(),
            description,
            icon_name: String::new(),
            icon_pixmap: Vec::new(),
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.cmd.try_send(UiCommand::ShowMonitor);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::{MenuItem, StandardItem};
        vec![
            StandardItem {
                label: "Monitor".into(),
                activate: Box::new(|t: &mut AgentTray| {
                    let _ = t.cmd.try_send(UiCommand::ShowMonitor);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Settings".into(),
                activate: Box::new(|t: &mut AgentTray| {
                    let _ = t.cmd.try_send(UiCommand::OpenSettings);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit AgentPet".into(),
                activate: Box::new(|t: &mut AgentTray| {
                    let _ = t.cmd.try_send(UiCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Spawns the tray on its own thread and returns a handle for live updates, or
/// `None` if the tray can't start (e.g. no StatusNotifier host) — the app still
/// runs with just the pet + monitor.
pub fn spawn(cmd: Sender<UiCommand>) -> Option<ksni::blocking::Handle<AgentTray>> {
    use ksni::blocking::TrayMethods;
    match (AgentTray { running: 0, waiting: 0, cmd }).spawn() {
        Ok(handle) => Some(handle),
        Err(e) => {
            eprintln!("agentpet: tray unavailable ({e}); is the GNOME AppIndicator extension installed?");
            None
        }
    }
}
