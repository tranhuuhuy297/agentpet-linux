//! Pet tab: searchable Petdex gallery rendered as a boxed list. Each row gets
//! a coloured blob avatar; the installed pet shows a disabled "✓ Your pet".

use crate::petdex::{RemotePet, STARTER_SLUG};
use crate::snapshot::{GalleryRequest, GalleryResult};
use agentpet_core::config::Config;
use async_channel::Sender;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Label, ListBox, Orientation, PolicyType, ScrolledWindow,
    SearchEntry,
};
use std::cell::RefCell;
use std::rc::Rc;

const MAX_ROWS: usize = 60;
const BLOB_COLORS: usize = 8;

pub struct PetPage {
    root: GtkBox,
    list: ListBox,
    status: Label,
    search: SearchEntry,
    all_pets: Rc<RefCell<Vec<RemotePet>>>,
    selected: Rc<RefCell<Option<String>>>,
    gallery_tx: Sender<GalleryRequest>,
}

impl PetPage {
    pub fn new(gallery_tx: Sender<GalleryRequest>) -> Self {
        let search = SearchEntry::new();
        search.set_placeholder_text(Some("Search pets…"));
        let list = ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::None);
        list.add_css_class("boxed");
        let scrolled = ScrolledWindow::new();
        scrolled.set_policy(PolicyType::Never, PolicyType::Automatic);
        scrolled.set_vexpand(true);
        scrolled.set_child(Some(&list));
        let status = Label::new(Some("Open to load the pet library"));
        status.set_xalign(0.0);
        status.add_css_class("rsub");

        let root = GtkBox::new(Orientation::Vertical, 10);
        root.set_margin_top(22);
        root.set_margin_bottom(26);
        root.set_margin_start(24);
        root.set_margin_end(24);
        root.append(&search);
        root.append(&status);
        root.append(&scrolled);

        let page = PetPage {
            root,
            list,
            status,
            search,
            all_pets: Rc::new(RefCell::new(Vec::new())),
            selected: Rc::new(RefCell::new(Config::load().selected_pet_id)),
            gallery_tx,
        };
        page.wire_search();
        page
    }

    pub fn widget(&self) -> &GtkBox {
        &self.root
    }

    /// Kicks off the manifest fetch (called on first open).
    pub fn begin_loading(&self) {
        self.status.set_text("Loading pet library…");
        let _ = self.gallery_tx.try_send(GalleryRequest::Fetch);
    }

    pub fn apply_result(&self, result: GalleryResult) {
        match result {
            GalleryResult::Manifest(pets) => {
                self.status
                    .set_text(&format!("{} pets available · served by Petdex", pets.len()));
                *self.all_pets.borrow_mut() = pets;
                self.render();
            }
            GalleryResult::Downloaded(id) => {
                self.status.set_text(&format!("Installed '{id}' — now your pet"));
                *self.selected.borrow_mut() = Some(id);
                self.render();
            }
            // The worker already sends a complete, user-facing sentence that
            // names the culprit (Petdex's hosting vs. the user's connection).
            GalleryResult::Failed(e) => {
                self.status.set_text(&e);
            }
        }
    }

    fn wire_search(&self) {
        let (list, all, selected, tx) = (
            self.list.clone(),
            self.all_pets.clone(),
            self.selected.clone(),
            self.gallery_tx.clone(),
        );
        self.search.connect_search_changed(move |entry| {
            render_into(&list, &all.borrow(), &entry.text(), &selected.borrow(), &tx);
        });
    }

    fn render(&self) {
        render_into(
            &self.list,
            &self.all_pets.borrow(),
            &self.search.text(),
            &self.selected.borrow(),
            &self.gallery_tx,
        );
    }
}

fn render_into(
    list: &ListBox,
    pets: &[RemotePet],
    query: &str,
    selected: &Option<String>,
    tx: &Sender<GalleryRequest>,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let q = query.to_lowercase();
    let filtered = pets
        .iter()
        .filter(|p| q.is_empty() || p.name().to_lowercase().contains(&q) || p.slug.contains(&q))
        .take(MAX_ROWS);
    for pet in filtered {
        // Best effort: the saved id is the pack's manifest id, which for
        // Petdex packs matches the slug.
        let is_selected = selected.as_deref() == Some(pet.slug.as_str());
        list.append(&pet_row(pet, is_selected, tx));
    }
}

fn pet_row(pet: &RemotePet, is_selected: bool, tx: &Sender<GalleryRequest>) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 14);

    let blob = Label::new(None);
    blob.set_valign(Align::Center);
    blob.add_css_class("pet-blob");
    blob.add_css_class(&format!("blob-{}", blob_color_index(&pet.slug)));

    let text = GtkBox::new(Orientation::Vertical, 2);
    let title = GtkBox::new(Orientation::Horizontal, 8);
    let name = Label::new(Some(pet.name()));
    name.set_xalign(0.0);
    name.add_css_class("rtitle");
    title.append(&name);
    if pet.slug == STARTER_SLUG {
        let tag = Label::new(Some("starter"));
        tag.set_valign(Align::Center);
        tag.add_css_class("tag");
        title.append(&tag);
    }
    let author = Label::new(Some(&format!("by {}", pet.author())));
    author.set_xalign(0.0);
    author.add_css_class("rsub");
    text.append(&title);
    text.append(&author);
    text.set_hexpand(true);

    let add = if is_selected {
        let btn = Button::with_label("✓ Your pet");
        btn.set_sensitive(false);
        btn.add_css_class("added");
        btn
    } else {
        let btn = Button::with_label("Add");
        let (tx, pet) = (tx.clone(), pet.clone());
        btn.connect_clicked(move |b| {
            b.set_label("Adding…");
            b.set_sensitive(false);
            let _ = tx.try_send(GalleryRequest::Download(pet.clone()));
        });
        btn
    };
    add.set_valign(Align::Center);

    row.append(&blob);
    row.append(&text);
    row.append(&add);
    row
}

/// Stable palette pick per slug (mirrors the design reference's blob colours).
fn blob_color_index(slug: &str) -> usize {
    slug.bytes().fold(0usize, |acc, b| acc.wrapping_add(b as usize)) % BLOB_COLORS
}
