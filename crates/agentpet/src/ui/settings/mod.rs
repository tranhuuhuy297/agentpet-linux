//! The Settings window: connect agents (hook install toggles), browse/download
//! pets (Petdex gallery), and About. Ports `SetupView.swift`, styled after the
//! libadwaita design reference (headerbar view switcher + boxed groups).

mod about;
mod general;
mod pet_page;
mod style;

use crate::snapshot::{GalleryRequest, GalleryResult, UiCommand};
use async_channel::Sender;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, HeaderBar, Label, Stack, StackSwitcher};
use std::cell::Cell;
use std::rc::Rc;

pub struct SettingsWindow {
    window: ApplicationWindow,
    pets: pet_page::PetPage,
    requested: Rc<Cell<bool>>,
}

impl SettingsWindow {
    pub fn new(
        app: &Application,
        gallery_tx: Sender<GalleryRequest>,
        cmd: Sender<UiCommand>,
    ) -> Self {
        style::ensure_loaded();

        let window = ApplicationWindow::builder()
            .application(app)
            .title("AgentPet — Settings")
            .default_width(560)
            .default_height(600)
            .build();
        window.set_hide_on_close(true);

        let pets = pet_page::PetPage::new(gallery_tx, cmd);
        let stack = Stack::new();
        stack.add_titled(&general::build(), Some("general"), "General");
        stack.add_titled(pets.widget(), Some("pet"), "Pet");
        stack.add_titled(&about::build(), Some("about"), "About");
        stack.set_visible_child_name("general");

        // Headerbar with the window title at the left and the view switcher
        // centred, like the design reference.
        let switcher = StackSwitcher::new();
        switcher.set_stack(Some(&stack));
        let header = HeaderBar::new();
        let title = Label::new(Some("Settings"));
        title.add_css_class("win-title");
        header.pack_start(&title);
        header.set_title_widget(Some(&switcher));
        window.set_titlebar(Some(&header));
        window.set_child(Some(&stack));

        SettingsWindow { window, pets, requested: Rc::new(Cell::new(false)) }
    }

    pub fn show(&self) {
        self.window.present();
        if !self.requested.get() {
            self.requested.set(true);
            self.pets.begin_loading();
        }
    }

    pub fn apply_gallery_result(&self, result: GalleryResult) {
        self.pets.apply_result(result);
    }
}
