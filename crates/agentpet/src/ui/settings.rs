//! The Settings window: connect agents (hook install toggles), browse/download
//! pets (Petdex gallery), and About. Ports `SetupView.swift`.

use crate::petdex::RemotePet;
use crate::platform::autostart;
use crate::snapshot::{GalleryRequest, GalleryResult};
use agentpet_core::catalog::AgentCatalog;
use agentpet_core::hooks::{AgentHooks, HookInstaller};
use async_channel::Sender;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Label, ListBox, Orientation,
    PolicyType, ScrolledWindow, SearchEntry, Stack, StackSwitcher, Switch,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const MAX_ROWS: usize = 60;

pub struct SettingsWindow {
    window: ApplicationWindow,
    gallery_tx: Sender<GalleryRequest>,
    list: ListBox,
    status: Label,
    search: SearchEntry,
    all_pets: Rc<RefCell<Vec<RemotePet>>>,
    requested: Rc<Cell<bool>>,
}

impl SettingsWindow {
    pub fn new(app: &Application, gallery_tx: Sender<GalleryRequest>) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("AgentPet — Settings")
            .default_width(520)
            .default_height(560)
            .build();
        window.set_hide_on_close(true);

        let stack = Stack::new();
        let all_pets = Rc::new(RefCell::new(Vec::<RemotePet>::new()));

        // --- Pet tab (gallery) ---
        let search = SearchEntry::new();
        search.set_placeholder_text(Some("Search pets…"));
        let list = ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::None);
        let scrolled = ScrolledWindow::new();
        scrolled.set_policy(PolicyType::Never, PolicyType::Automatic);
        scrolled.set_vexpand(true);
        scrolled.set_child(Some(&list));
        let status = Label::new(Some("Open to load the pet library"));
        status.set_xalign(0.0);
        status.add_css_class("dim-label");

        let pet_box = GtkBox::new(Orientation::Vertical, 8);
        pet_box.set_margin_top(12);
        pet_box.set_margin_bottom(12);
        pet_box.set_margin_start(12);
        pet_box.set_margin_end(12);
        pet_box.append(&search);
        pet_box.append(&status);
        pet_box.append(&scrolled);

        stack.add_titled(&build_general(app), Some("general"), "General");
        stack.add_titled(&pet_box, Some("pet"), "Pet");
        stack.add_titled(&build_about(), Some("about"), "About");

        let switcher = StackSwitcher::new();
        switcher.set_stack(Some(&stack));
        switcher.set_halign(Align::Center);
        switcher.set_margin_top(10);

        let root = GtkBox::new(Orientation::Vertical, 0);
        root.append(&switcher);
        root.append(&stack);
        window.set_child(Some(&root));

        let win = SettingsWindow {
            window,
            gallery_tx,
            list,
            status,
            search,
            all_pets,
            requested: Rc::new(Cell::new(false)),
        };
        win.wire_search();
        win
    }

    pub fn show(&self) {
        self.window.present();
        if !self.requested.get() {
            self.requested.set(true);
            self.status.set_text("Loading pet library…");
            let _ = self.gallery_tx.try_send(GalleryRequest::Fetch);
        }
    }

    pub fn apply_gallery_result(&self, result: GalleryResult) {
        match result {
            GalleryResult::Manifest(pets) => {
                self.status.set_text(&format!("{} pets available", pets.len()));
                *self.all_pets.borrow_mut() = pets;
                self.render();
            }
            GalleryResult::Downloaded(id) => {
                self.status.set_text(&format!("Installed '{id}' — now your pet"));
            }
            GalleryResult::Failed(e) => {
                self.status.set_text(&format!("Couldn't load the pet library: {e}"));
            }
        }
    }

    fn wire_search(&self) {
        let (list, all, tx) = (self.list.clone(), self.all_pets.clone(), self.gallery_tx.clone());
        self.search.connect_search_changed(move |entry| {
            render_into(&list, &all.borrow(), &entry.text(), &tx);
        });
    }

    fn render(&self) {
        render_into(&self.list, &self.all_pets.borrow(), &self.search.text(), &self.gallery_tx);
    }
}

fn render_into(list: &ListBox, pets: &[RemotePet], query: &str, tx: &Sender<GalleryRequest>) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let q = query.to_lowercase();
    let filtered = pets
        .iter()
        .filter(|p| q.is_empty() || p.name().to_lowercase().contains(&q) || p.slug.contains(&q))
        .take(MAX_ROWS);
    for pet in filtered {
        list.append(&pet_row(pet, tx));
    }
}

fn pet_row(pet: &RemotePet, tx: &Sender<GalleryRequest>) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 8);
    row.set_margin_top(6);
    row.set_margin_bottom(6);
    row.set_margin_start(8);
    row.set_margin_end(8);

    let text = GtkBox::new(Orientation::Vertical, 0);
    let name = Label::new(None);
    name.set_markup(&format!("<b>{}</b>", glib_escape(pet.name())));
    name.set_xalign(0.0);
    let author = Label::new(Some(&format!("by {}", pet.author())));
    author.set_xalign(0.0);
    author.add_css_class("dim-label");
    text.append(&name);
    text.append(&author);
    text.set_hexpand(true);

    let add = Button::with_label("Add");
    add.set_valign(Align::Center);
    {
        let (tx, pet) = (tx.clone(), pet.clone());
        add.connect_clicked(move |btn| {
            btn.set_label("Adding…");
            btn.set_sensitive(false);
            let _ = tx.try_send(GalleryRequest::Download(pet.clone()));
        });
    }

    row.append(&text);
    row.append(&add);
    row
}

/// General tab: per-agent hook install toggles.
fn build_general(_app: &Application) -> GtkBox {
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "agentpet".to_string());

    let container = GtkBox::new(Orientation::Vertical, 6);
    container.set_margin_top(12);
    container.set_margin_bottom(12);
    container.set_margin_start(12);
    container.set_margin_end(12);

    let heading = Label::new(None);
    heading.set_markup("<b>Connect your agents</b>");
    heading.set_xalign(0.0);
    container.append(&heading);

    for agent in AgentCatalog::all() {
        let Some(spec) = AgentHooks::spec(agent.kind) else { continue };
        let command = format!("\"{}\" hook --agent {}", exe, agent.kind.raw());

        let row = GtkBox::new(Orientation::Horizontal, 8);
        row.set_margin_top(4);
        row.set_margin_bottom(4);

        let text = GtkBox::new(Orientation::Vertical, 0);
        let name = Label::new(Some(agent.display_name));
        name.set_xalign(0.0);
        text.append(&name);
        if let Some(note) = agent.note {
            let n = Label::new(Some(note));
            n.set_xalign(0.0);
            n.add_css_class("dim-label");
            text.append(&n);
        }
        text.set_hexpand(true);

        let sw = Switch::new();
        sw.set_valign(Align::Center);
        sw.set_active(HookInstaller::is_installed_on_disk(&spec.settings_path, &spec.events, spec.style));
        {
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

        row.append(&text);
        row.append(&sw);
        container.append(&row);
    }

    // Startup section.
    let startup = Label::new(None);
    startup.set_markup("<b>Startup</b>");
    startup.set_xalign(0.0);
    startup.set_margin_top(12);
    container.append(&startup);

    let row = GtkBox::new(Orientation::Horizontal, 8);
    let label = Label::new(Some("Start AgentPet at login"));
    label.set_xalign(0.0);
    label.set_hexpand(true);
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
    row.append(&label);
    row.append(&sw);
    container.append(&row);

    container
}

fn build_about() -> GtkBox {
    let about = GtkBox::new(Orientation::Vertical, 8);
    about.set_margin_top(16);
    about.set_margin_bottom(16);
    about.set_margin_start(16);
    about.set_margin_end(16);
    about.set_valign(Align::Start);

    let title = Label::new(None);
    title.set_markup(&format!(
        "<b>AgentPet for Linux</b>  <span foreground='#888'>v{}</span>",
        env!("CARGO_PKG_VERSION")
    ));
    title.set_xalign(0.0);

    let body = Label::new(None);
    body.set_markup(
        "Watch multiple AI coding agents at a glance.\n\n\
         Pets come from <a href='https://github.com/crafter-station/petdex'>Petdex</a> (MIT).\n\
         Source: <a href='https://github.com/tranhuuhuy297/agentpet-linux'>github.com/tranhuuhuy297/agentpet-linux</a>",
    );
    body.set_xalign(0.0);
    body.set_use_markup(true);

    about.append(&title);
    about.append(&body);
    about
}

fn glib_escape(s: &str) -> String {
    gtk4::glib::markup_escape_text(s).to_string()
}
