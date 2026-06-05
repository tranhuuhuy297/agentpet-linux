//! GTK CSS recreating the libadwaita boxed-list look of the Settings design
//! reference (same visual language as the docs/ landing page).

use std::sync::Once;

static LOAD: Once = Once::new();

/// Installs the settings stylesheet on the default display (idempotent).
pub fn ensure_loaded() {
    LOAD.call_once(|| {
        let provider = gtk4::CssProvider::new();
        provider.load_from_data(CSS);
        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}

const CSS: &str = "
.win-title { font-weight: 700; }

/* group label + boxed list */
.group-title { font-weight: 700; font-size: 13px; }
.group-sub { font-size: 12px; opacity: 0.65; }
.boxed {
  background: alpha(currentColor, 0.05);
  border: 1px solid alpha(currentColor, 0.12);
  border-radius: 12px;
}
.boxed > box { padding: 11px 14px; }
.boxed > box:not(:first-child) { border-top: 1px solid alpha(currentColor, 0.08); }
list.boxed > row { background: transparent; padding: 11px 14px; }
list.boxed > row:not(:first-child) { border-top: 1px solid alpha(currentColor, 0.08); }

/* row text */
.rtitle { font-size: 14px; }
.rsub { font-size: 12px; opacity: 0.7; }
.mono { font-family: monospace; }
.rsub.mono { font-size: 11px; }
.warn { color: #e5a50a; opacity: 1; }

/* agent monogram badge (brand colours mirror the design reference) */
.ricon {
  min-width: 30px; min-height: 30px; border-radius: 8px;
  color: #ffffff; font-family: monospace; font-weight: 600; font-size: 14px;
}
.agent-claude { background-color: #d97757; }
.agent-codex { background-color: #10a37f; }
.agent-gemini { background-color: #4285f4; }
.agent-cursor { background-color: #7c8cf8; }
.agent-opencode { background-color: #56d472; }
.agent-windsurf { background-color: #21b6c9; }
.agent-cli, .agent-unknown { background-color: #9aa9b6; }

/* pet gallery blob */
.pet-blob { min-width: 30px; min-height: 30px; border-radius: 15px; }
.blob-0 { background-color: #5ec6f0; }
.blob-1 { background-color: #f08cd9; }
.blob-2 { background-color: #56d472; }
.blob-3 { background-color: #f0b020; }
.blob-4 { background-color: #b6bfca; }
.blob-5 { background-color: #ff8a5c; }
.blob-6 { background-color: #8a7cf0; }
.blob-7 { background-color: #9ad0c2; }

/* 'starter' chip */
.tag {
  font-size: 10px; font-family: monospace; color: #3584e4;
  background: alpha(#3584e4, 0.16); border-radius: 5px; padding: 1px 6px;
}

/* installed-pet button state */
button.added {
  background: none; border: 1px solid alpha(currentColor, 0.2); opacity: 0.7;
}

/* about page */
.about-title { font-size: 22px; font-weight: 700; }
.about-ver { font-family: monospace; font-size: 13px; opacity: 0.65; }
.about-desc { font-size: 14px; opacity: 0.75; }
.credits { font-size: 11px; opacity: 0.55; }
";
