//! Pet tab: pick a pet per agent. A dropdown selects which agent (Claude Code,
//! Codex, …) you're assigning a pet to; the searchable Petdex gallery below
//! shows each pet with an action that targets the selected agent:
//! - not installed → "Add" (downloads, then assigns to the agent),
//! - installed      → "Use" (assigns without re-downloading),
//! - already chosen → a disabled "✓ <Agent>'s pet" marker.

use crate::petdex::{pets_dir, RemotePet, STARTER_SLUG};
use crate::snapshot::{GalleryRequest, GalleryResult, UiCommand};
use agentpet_core::catalog::AgentCatalog;
use agentpet_core::config::Config;
use agentpet_core::state::AgentKind;
use async_channel::Sender;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, DropDown, Label, ListBox, Orientation, PolicyType,
    ScrolledWindow, SearchEntry,
};
use std::cell::RefCell;
use std::rc::Rc;

const MAX_ROWS: usize = 60;
const BLOB_COLORS: usize = 8;

/// Shared, cloneable handles the row callbacks need to re-render and persist a
/// pet choice. Cloning is cheap (everything inside is `Rc`/`Sender`/widget ref).
#[derive(Clone)]
struct PetCtx {
    list: ListBox,
    all_pets: Rc<RefCell<Vec<RemotePet>>>,
    search: SearchEntry,
    /// Agent currently targeted by the gallery actions.
    agent: Rc<RefCell<AgentKind>>,
    /// Pet pack id chosen for the targeted agent (drives the "✓" marker).
    pick: Rc<RefCell<Option<String>>>,
    /// Agent that initiated an in-flight download (assigned on completion).
    pending: Rc<RefCell<Option<AgentKind>>>,
    gallery_tx: Sender<GalleryRequest>,
    cmd_tx: Sender<UiCommand>,
}

impl PetCtx {
    fn render(&self) {
        let list = &self.list;
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }
        let pets = self.all_pets.borrow();
        let q = self.search.text().to_lowercase();
        let pick = self.pick.borrow().clone();
        let agent = *self.agent.borrow();
        let filtered = pets
            .iter()
            .filter(|p| q.is_empty() || p.name().to_lowercase().contains(&q) || p.slug.contains(&q))
            .take(MAX_ROWS);
        for pet in filtered {
            list.append(&self.pet_row(pet, agent, pick.as_deref()));
        }
    }

    /// Assigns a pet pack to `agent`, persists it, refreshes live pets, and
    /// re-renders if the targeted agent is the one on screen. Reloads config
    /// from disk first so a concurrent default-pet write (e.g. the download
    /// worker) is preserved rather than clobbered.
    fn assign(&self, agent: AgentKind, pack_id: String) {
        let mut cfg = Config::load();
        cfg.set_pet_for(agent, pack_id.clone());
        let _ = cfg.save();
        if *self.agent.borrow() == agent {
            *self.pick.borrow_mut() = Some(pack_id);
        }
        let _ = self.cmd_tx.try_send(UiCommand::ReloadPets);
        self.render();
    }

    fn pet_row(&self, pet: &RemotePet, agent: AgentKind, pick: Option<&str>) -> GtkBox {
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

        // Petdex pack ids match their slug, so compare against the slug.
        let is_pick = pick == Some(pet.slug.as_str());
        let action = if is_pick {
            let btn = Button::with_label(&format!("✓ {}", agent_short(agent)));
            btn.set_sensitive(false);
            btn.add_css_class("added");
            btn
        } else if pet_installed(&pet.slug) {
            let btn = Button::with_label("Use");
            let (ctx, slug) = (self.clone(), pet.slug.clone());
            btn.connect_clicked(move |_| ctx.assign(agent, slug.clone()));
            btn
        } else {
            let btn = Button::with_label("Add");
            let (ctx, pet) = (self.clone(), pet.clone());
            btn.connect_clicked(move |b| {
                b.set_label("Adding…");
                b.set_sensitive(false);
                // Remember which agent to assign the pet to once it downloads.
                *ctx.pending.borrow_mut() = Some(agent);
                let _ = ctx.gallery_tx.try_send(GalleryRequest::Download(pet.clone()));
            });
            btn
        };
        action.set_valign(Align::Center);

        row.append(&blob);
        row.append(&text);
        row.append(&action);
        row
    }
}

pub struct PetPage {
    root: GtkBox,
    status: Label,
    ctx: PetCtx,
    requested: Rc<RefCell<bool>>,
}

impl PetPage {
    pub fn new(gallery_tx: Sender<GalleryRequest>, cmd_tx: Sender<UiCommand>) -> Self {
        let agents = AgentCatalog::all();
        let first_agent = agents.first().map(|a| a.kind).unwrap_or(AgentKind::Claude);

        // Agent selector: which agent the gallery actions assign a pet to.
        let names: Vec<&str> = agents.iter().map(|a| a.display_name).collect();
        let agent_dropdown = DropDown::from_strings(&names);
        let agent_row = GtkBox::new(Orientation::Horizontal, 10);
        let agent_label = Label::new(Some("Pet for"));
        agent_label.add_css_class("rtitle");
        agent_row.append(&agent_label);
        agent_row.append(&agent_dropdown);

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
        root.append(&agent_row);
        root.append(&search);
        root.append(&status);
        root.append(&scrolled);

        let cfg = Config::load();
        let ctx = PetCtx {
            list,
            all_pets: Rc::new(RefCell::new(Vec::new())),
            search: search.clone(),
            agent: Rc::new(RefCell::new(first_agent)),
            pick: Rc::new(RefCell::new(cfg.pet_id_for(first_agent).map(str::to_string))),
            pending: Rc::new(RefCell::new(None)),
            gallery_tx,
            cmd_tx,
        };

        // Re-render when the search text or the targeted agent changes.
        {
            let ctx = ctx.clone();
            search.connect_search_changed(move |_| ctx.render());
        }
        {
            let (ctx, agents) = (ctx.clone(), agents.clone());
            agent_dropdown.connect_selected_notify(move |dd| {
                if let Some(a) = agents.get(dd.selected() as usize) {
                    *ctx.agent.borrow_mut() = a.kind;
                    // Reload from disk so the marker reflects this agent's pick.
                    *ctx.pick.borrow_mut() = Config::load().pet_id_for(a.kind).map(str::to_string);
                    ctx.render();
                }
            });
        }

        PetPage { root, status, ctx, requested: Rc::new(RefCell::new(false)) }
    }

    pub fn widget(&self) -> &GtkBox {
        &self.root
    }

    /// Kicks off the manifest fetch (called on first open).
    pub fn begin_loading(&self) {
        if *self.requested.borrow() {
            return;
        }
        *self.requested.borrow_mut() = true;
        self.status.set_text("Loading pet library…");
        let _ = self.ctx.gallery_tx.try_send(GalleryRequest::Fetch);
    }

    pub fn apply_result(&self, result: GalleryResult) {
        match result {
            GalleryResult::Manifest(pets) => {
                self.status
                    .set_text(&format!("{} pets available · served by Petdex", pets.len()));
                *self.ctx.all_pets.borrow_mut() = pets;
                self.ctx.render();
            }
            GalleryResult::Downloaded(id) => {
                // Assign the freshly-installed pet to whichever agent requested
                // it (falling back to the on-screen agent).
                let agent =
                    self.ctx.pending.borrow_mut().take().unwrap_or(*self.ctx.agent.borrow());
                self.status
                    .set_text(&format!("Installed '{id}' — now {}'s pet", agent_short(agent)));
                self.ctx.assign(agent, id);
            }
            // The worker already sends a complete, user-facing sentence that
            // names the culprit (Petdex's hosting vs. the user's connection).
            GalleryResult::Failed(e) => {
                self.status.set_text(&e);
                self.ctx.render(); // restore any "Adding…" button to its label
            }
        }
    }
}

/// Short agent label for the action button (e.g. "Claude Code" → "Claude").
fn agent_short(kind: AgentKind) -> &'static str {
    match kind {
        AgentKind::Claude => "Claude",
        AgentKind::Codex => "Codex",
        AgentKind::Cli => "CLI",
        AgentKind::Unknown => "agent",
    }
}

/// True if a pack for `slug` is already downloaded under `~/.agentpet/pets`.
fn pet_installed(slug: &str) -> bool {
    pets_dir().join(slug).join("pet.json").exists()
}

/// Stable palette pick per slug (mirrors the design reference's blob colours).
fn blob_color_index(slug: &str) -> usize {
    slug.bytes().fold(0usize, |acc, b| acc.wrapping_add(b as usize)) % BLOB_COLORS
}
