//! The Settings window: connect agents (hook install toggles), pick a pet from
//! the locally-installed Petdex packs, and About. Ports `SetupView.swift`,
//! styled after the libadwaita design reference (headerbar view switcher +
//! boxed groups).

mod about;
mod chat_page;
mod general;
mod pet_page;
mod style;

use crate::snapshot::UiCommand;
use async_channel::Sender;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, HeaderBar, Label, Stack, StackSwitcher};

pub struct SettingsWindow {
    window: ApplicationWindow,
    pets: pet_page::PetPage,
}

impl SettingsWindow {
    pub fn new(app: &Application, cmd: Sender<UiCommand>) -> Self {
        style::ensure_loaded();

        let window = ApplicationWindow::builder()
            .application(app)
            .title("AgentPet — Settings")
            .default_width(560)
            .default_height(600)
            .build();
        window.set_hide_on_close(true);
        super::window_icon::install(&window); // otter in the dock/alt-tab

        let chat = chat_page::build(cmd.clone());
        let pets = pet_page::PetPage::new(cmd);
        let stack = Stack::new();
        stack.add_titled(&general::build(), Some("general"), "General");
        stack.add_titled(pets.widget(), Some("pet"), "Pet");
        stack.add_titled(&chat, Some("chat"), "Chat");
        stack.add_titled(&about::build(), Some("about"), "About");
        stack.set_visible_child_name("general");

        // Headerbar with the window title at the left and the view switcher
        // centred, like the design reference.
        let switcher = StackSwitcher::new();
        switcher.set_stack(Some(&stack));
        let header = HeaderBar::new();
        let title = Label::new(Some("Settings"));
        title.add_css_class("win-title");
        // Breathing room from the window's left edge / controls.
        title.set_margin_start(8);
        header.pack_start(&title);
        header.set_title_widget(Some(&switcher));
        window.set_titlebar(Some(&header));
        window.set_child(Some(&stack));

        SettingsWindow { window, pets }
    }

    pub fn show(&self) {
        self.window.present();
        // Re-scan on every open so pets installed via the CLI since last time
        // show up without restarting the app.
        self.pets.refresh();
    }
}
