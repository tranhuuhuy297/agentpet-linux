//! StatusNotifierItem tray (ksni). Shows running/waiting state at a glance and
//! offers Monitor/Settings/Quit. Ports `StatusBarController.swift`.
//!
//! Requires the GNOME AppIndicator extension at runtime. The icon is a
//! black-outlined otter pixmap (line art over a white fill, so the face reads
//! at tray size) that flips to a warning glyph on "needs input"; the count
//! lives in the tooltip.

use crate::snapshot::UiCommand;
use async_channel::Sender;
use std::sync::OnceLock;

/// Black line-art otter, pre-rendered from the app icon (see assets/).
const TRAY_ICON_PNG: &[u8] = include_bytes!("../../../../assets/agentpet-tray.png");

/// SNI pixmap sizes hosts commonly pick from.
const TRAY_SIZES: [u32; 5] = [16, 22, 24, 32, 48];

/// Decodes the tray PNG and scales it to every SNI size.
/// ARGB32 in network byte order, per the SNI spec.
fn tray_icons() -> &'static Vec<ksni::Icon> {
    static ICONS: OnceLock<Vec<ksni::Icon>> = OnceLock::new();
    ICONS.get_or_init(|| {
        let Ok(img) = image::load_from_memory(TRAY_ICON_PNG) else {
            return Vec::new();
        };
        let rgba = img.to_rgba8();
        TRAY_SIZES
            .iter()
            .map(|&size| {
                let scaled = image::imageops::resize(
                    &rgba,
                    size,
                    size,
                    image::imageops::FilterType::Lanczos3,
                );
                let mut data = Vec::with_capacity((size * size * 4) as usize);
                for p in scaled.pixels() {
                    let [r, g, b, a] = p.0;
                    data.extend_from_slice(&[a, r, g, b]);
                }
                ksni::Icon { width: size as i32, height: size as i32, data }
            })
            .collect()
    })
}

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
        // Hosts prefer a theme icon when one is named, so only name one for the
        // attention state; otherwise the otter pixmap below is used.
        if self.waiting > 0 {
            "dialog-warning-symbolic".into()
        } else {
            String::new()
        }
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        if self.waiting > 0 {
            Vec::new()
        } else {
            tray_icons().clone()
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
