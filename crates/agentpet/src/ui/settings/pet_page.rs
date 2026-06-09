//! Pet tab: pick a pet per agent from the packs you've installed locally with
//! the Petdex CLI. A dropdown selects which agent (Claude Code, Codex, …) you're
//! assigning a pet to; a short guide shows the `npx petdex install` command and
//! a Refresh button re-scans after you install more. The list below shows each
//! installed pet with an action targeting the selected agent:
//! - installed      → "Use" (assigns it to the agent),
//! - already chosen → a disabled "✓ <Agent>'s pet" marker.
//!
//! AgentPet performs no downloads; installation is delegated to the official
//! Petdex CLI, which writes packs to `~/.petdex/pets/<slug>/`.

use crate::petdex::{installed_dir, scan_installed, InstalledPet, INSTALL_HINT};
use crate::snapshot::UiCommand;
use agentpet_core::catalog::AgentCatalog;
use agentpet_core::config::Config;
use agentpet_core::sprite::load_pack;
use agentpet_core::state::AgentKind;
use async_channel::Sender;
use crate::pet::{clamp_pet_size, MAX_PET_SIZE, MIN_PET_SIZE};
use gtk4::prelude::*;
use gtk4::{
    gdk, glib, Align, Box as GtkBox, Button, DropDown, Image, Label, ListBox, Orientation,
    PolicyType, Scale, ScrolledWindow, SearchEntry,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

const MAX_ROWS: usize = 200;
const BLOB_COLORS: usize = 8;
/// On-screen size (px) of each row's pet icon.
const ICON_PX: i32 = 36;
/// The Petdex gallery, linked from the install guide so users can find slugs.
const PETDEX_URL: &str = "https://petdex.dev";
const PETDEX_HOST: &str = "petdex.dev";

/// Shared, cloneable handles the row callbacks need to re-render and persist a
/// pet choice. Cloning is cheap (everything inside is `Rc`/`Sender`/widget ref).
#[derive(Clone)]
struct PetCtx {
    list: ListBox,
    installed: Rc<RefCell<Vec<InstalledPet>>>,
    search: SearchEntry,
    status: Label,
    /// Agent currently targeted by the gallery actions.
    agent: Rc<RefCell<AgentKind>>,
    /// Pet pack id chosen for the targeted agent (drives the "✓" marker).
    pick: Rc<RefCell<Option<String>>>,
    /// Cached first-frame sprite textures, keyed by slug — built once per pack so
    /// re-renders (search keystrokes, agent switches) don't re-slice spritesheets.
    thumbs: Rc<RefCell<HashMap<String, gdk::Texture>>>,
    cmd_tx: Sender<UiCommand>,
}

impl PetCtx {
    /// Re-scans `~/.petdex/pets`, updates the status line, and re-renders the
    /// list (called on open and on Refresh).
    fn refresh(&self) {
        let pets = scan_installed();
        // Build a sprite thumbnail for any newly-seen pack (cached by slug).
        {
            let mut thumbs = self.thumbs.borrow_mut();
            for pet in &pets {
                if !thumbs.contains_key(&pet.slug) {
                    if let Some(tex) = load_thumbnail(&pet.slug) {
                        thumbs.insert(pet.slug.clone(), tex);
                    }
                }
            }
        }
        self.status.set_text(&status_text(pets.len()));
        *self.installed.borrow_mut() = pets;
        self.render();
    }

    fn render(&self) {
        let list = &self.list;
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }
        let pets = self.installed.borrow();
        let q = self.search.text().to_lowercase();
        let pick = self.pick.borrow().clone();
        let agent = *self.agent.borrow();
        let filtered = pets
            .iter()
            .filter(|p| {
                q.is_empty()
                    || p.display_name.to_lowercase().contains(&q)
                    || p.slug.contains(&q)
            })
            .take(MAX_ROWS);
        for pet in filtered {
            list.append(&self.pet_row(pet, agent, pick.as_deref()));
        }
    }

    /// Assigns a pet pack to `agent`, persists it, refreshes live pets, and
    /// re-renders if the targeted agent is the one on screen. Reloads config
    /// from disk first so a concurrent per-agent write is preserved.
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

    fn pet_row(&self, pet: &InstalledPet, agent: AgentKind, pick: Option<&str>) -> GtkBox {
        let row = GtkBox::new(Orientation::Horizontal, 14);

        // Show the pet's own sprite (first frame); fall back to a coloured blob
        // for packs whose spritesheet failed to decode.
        let icon: gtk4::Widget = match self.thumbs.borrow().get(&pet.slug) {
            Some(tex) => {
                let img = Image::from_paintable(Some(tex));
                img.set_pixel_size(ICON_PX);
                img.upcast()
            }
            None => {
                let blob = Label::new(None);
                blob.add_css_class("pet-blob");
                blob.add_css_class(&format!("blob-{}", blob_color_index(&pet.slug)));
                blob.upcast()
            }
        };
        icon.set_valign(Align::Center);

        let text = GtkBox::new(Orientation::Vertical, 2);
        let name = Label::new(Some(&pet.display_name));
        name.set_xalign(0.0);
        name.add_css_class("rtitle");
        let slug = Label::new(Some(&pet.slug));
        slug.set_xalign(0.0);
        slug.add_css_class("rsub");
        slug.add_css_class("mono");
        text.append(&name);
        text.append(&slug);
        text.set_hexpand(true);

        // Compare against the pack id (what `assign` persists as the pick).
        let is_pick = pick == Some(pet.id.as_str());
        let action = if is_pick {
            let btn = Button::with_label(&format!("✓ {}", agent_label(agent)));
            btn.set_sensitive(false);
            btn.add_css_class("added");
            btn
        } else {
            let btn = Button::with_label("Use");
            let (ctx, id) = (self.clone(), pet.id.clone());
            btn.connect_clicked(move |_| ctx.assign(agent, id.clone()));
            btn
        };
        action.set_valign(Align::Center);

        row.append(&icon);
        row.append(&text);
        row.append(&action);
        row
    }
}

pub struct PetPage {
    root: GtkBox,
    ctx: PetCtx,
}

impl PetPage {
    pub fn new(cmd_tx: Sender<UiCommand>) -> Self {
        let agents = AgentCatalog::all();
        let first_agent = agents.first().map(|a| a.kind).unwrap_or(AgentKind::Claude);

        // Agent selector: which agent the actions assign a pet to.
        let names: Vec<&str> = agents.iter().map(|a| a.display_name).collect();
        let agent_dropdown = DropDown::from_strings(&names);
        let agent_row = GtkBox::new(Orientation::Horizontal, 10);
        let agent_label = Label::new(Some("Pet for"));
        agent_label.add_css_class("rtitle");
        agent_row.append(&agent_label);
        agent_row.append(&agent_dropdown);

        // How-to-install guide with a Refresh button to re-scan after install.
        let refresh = Button::with_label("Refresh");
        refresh.set_valign(Align::Center);
        let guide = build_install_guide(&refresh);

        let search = SearchEntry::new();
        search.set_placeholder_text(Some("Search installed pets…"));

        let list = ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::None);
        list.add_css_class("boxed");
        let scrolled = ScrolledWindow::new();
        scrolled.set_policy(PolicyType::Never, PolicyType::Automatic);
        scrolled.set_vexpand(true);
        scrolled.set_child(Some(&list));

        let status = Label::new(Some(""));
        status.set_xalign(0.0);
        status.add_css_class("rsub");

        let root = GtkBox::new(Orientation::Vertical, 10);
        root.set_margin_top(22);
        root.set_margin_bottom(26);
        root.set_margin_start(24);
        root.set_margin_end(24);
        root.append(&build_size_row(cmd_tx.clone()));
        root.append(&agent_row);
        root.append(&guide);
        root.append(&search);
        root.append(&status);
        root.append(&scrolled);

        let cfg = Config::load();
        let ctx = PetCtx {
            list,
            installed: Rc::new(RefCell::new(Vec::new())),
            search: search.clone(),
            status,
            agent: Rc::new(RefCell::new(first_agent)),
            pick: Rc::new(RefCell::new(cfg.pet_id_for(first_agent).map(str::to_string))),
            thumbs: Rc::new(RefCell::new(HashMap::new())),
            cmd_tx,
        };

        // Re-render when the search text changes; re-scan on Refresh.
        {
            let ctx = ctx.clone();
            search.connect_search_changed(move |_| ctx.render());
        }
        {
            let ctx = ctx.clone();
            refresh.connect_clicked(move |_| ctx.refresh());
        }
        // Switch the targeted agent and reflect its current pick.
        {
            let (ctx, agents) = (ctx.clone(), agents.clone());
            agent_dropdown.connect_selected_notify(move |dd| {
                if let Some(a) = agents.get(dd.selected() as usize) {
                    *ctx.agent.borrow_mut() = a.kind;
                    *ctx.pick.borrow_mut() = Config::load().pet_id_for(a.kind).map(str::to_string);
                    ctx.render();
                }
            });
        }

        PetPage { root, ctx }
    }

    pub fn widget(&self) -> &GtkBox {
        &self.root
    }

    /// Re-scans installed packs and re-renders (called whenever Settings opens).
    pub fn refresh(&self) {
        self.ctx.refresh();
    }
}

/// Status line under the guide: a count, or an empty-state nudge to install.
fn status_text(count: usize) -> String {
    match count {
        0 => format!("No pets installed yet — run `{INSTALL_HINT}`, then Refresh."),
        1 => "1 pet installed · from ~/.petdex/pets".to_string(),
        n => format!("{n} pets installed · from ~/.petdex/pets"),
    }
}

/// Global pet-size slider row at the top of the Pet tab. Moving it resizes every
/// live pet instantly via `ResizePets` (disk-free), and persists the choice to
/// `Config.pet_size` on a 250 ms debounce so a drag doesn't thrash the file.
fn build_size_row(cmd_tx: Sender<UiCommand>) -> GtkBox {
    let outer = GtkBox::new(Orientation::Vertical, 0);
    outer.add_css_class("boxed");

    // Inner box so the `.boxed > box` padding rule applies — without it the title
    // and slider sit flush against the rounded border (same wrap as the guide).
    let inner = GtkBox::new(Orientation::Vertical, 6);

    let title = Label::new(Some("Pet size"));
    title.set_xalign(0.0);
    title.add_css_class("group-title");

    let initial = clamp_pet_size(Config::load().pet_size);
    let scale = Scale::with_range(
        Orientation::Horizontal,
        MIN_PET_SIZE as f64,
        MAX_PET_SIZE as f64,
        1.0,
    );
    scale.set_value(initial as f64);
    scale.set_hexpand(true);
    scale.set_draw_value(true);
    scale.set_value_pos(gtk4::PositionType::Right);
    scale.set_digits(0);

    // Debounce token: each move bumps the generation; only the timeout matching
    // the latest generation actually writes, so disk is touched once per pause.
    let debounce = Rc::new(Cell::new(0u64));
    scale.connect_value_changed(move |s| {
        let px = s.value().round() as i32;
        let _ = cmd_tx.try_send(UiCommand::ResizePets(px));
        let generation = debounce.get().wrapping_add(1);
        debounce.set(generation);
        let debounce = debounce.clone();
        glib::timeout_add_local_once(Duration::from_millis(250), move || {
            if debounce.get() == generation {
                let mut cfg = Config::load();
                // Clamp on persist too, so the stored value can never drift from
                // what the live pet shows (which goes through the same clamp).
                cfg.pet_size = clamp_pet_size(px as f64) as f64;
                let _ = cfg.save();
            }
        });
    });

    inner.append(&title);
    inner.append(&scale);
    outer.append(&inner);
    outer
}

/// A boxed guide telling the user how to install pets via the Petdex CLI, with
/// the (selectable) command on its own line and the Refresh button on the right.
fn build_install_guide(refresh: &Button) -> GtkBox {
    let outer = GtkBox::new(Orientation::Horizontal, 12);
    outer.add_css_class("boxed");

    let text = GtkBox::new(Orientation::Vertical, 4);
    text.set_hexpand(true);
    let title = Label::new(Some("Install pets with the Petdex CLI"));
    title.set_xalign(0.0);
    title.add_css_class("group-title");
    let cmd = Label::new(Some(INSTALL_HINT));
    cmd.set_xalign(0.0);
    cmd.set_selectable(true); // so the command can be copied
    cmd.add_css_class("mono");
    // A clickable link to the Petdex gallery so the user can find pet slugs to
    // install. GTK's default `activate-link` handler opens the URI in the
    // browser; markup keeps the rest of the sentence as plain caption text.
    let note = Label::new(None);
    note.set_xalign(0.0);
    note.set_use_markup(true);
    note.set_markup(&format!(
        "Browse pets and copy a slug at <a href=\"{PETDEX_URL}\">{PETDEX_HOST}</a>, then Refresh."
    ));
    note.add_css_class("group-sub");
    text.append(&title);
    text.append(&cmd);
    text.append(&note);

    refresh.set_valign(Align::Center);
    outer.append(&text);
    outer.append(refresh);
    outer
}

/// Full agent name for the picked marker (e.g. "Claude Code"), from the catalog
/// so it stays in sync with the agent selector; falls back for wrapper kinds.
fn agent_label(kind: AgentKind) -> &'static str {
    AgentCatalog::all()
        .into_iter()
        .find(|a| a.kind == kind)
        .map(|a| a.display_name)
        .unwrap_or(match kind {
            AgentKind::Cli => "CLI",
            _ => "agent",
        })
}

/// Stable palette pick per slug (mirrors the design reference's blob colours).
fn blob_color_index(slug: &str) -> usize {
    slug.bytes().fold(0usize, |acc, b| acc.wrapping_add(b as usize)) % BLOB_COLORS
}

/// Renders a pack's first sprite frame into a GPU texture for the row icon.
/// Returns `None` if the pack can't be loaded/sliced (caller shows a blob).
fn load_thumbnail(slug: &str) -> Option<gdk::Texture> {
    let pack = load_pack(&installed_dir().join(slug))?;
    let frame = pack.clip(0).first()?;
    let (w, h) = (frame.width() as i32, frame.height() as i32);
    // image's `RgbaImage` is straight (non-premultiplied) RGBA8 — matches
    // `R8g8b8a8`, so the sprite's transparency is preserved.
    let bytes = glib::Bytes::from(frame.as_raw().as_slice());
    let texture =
        gdk::MemoryTexture::new(w, h, gdk::MemoryFormat::R8g8b8a8, &bytes, (w * 4) as usize);
    Some(texture.upcast())
}
