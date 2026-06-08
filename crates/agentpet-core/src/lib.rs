//! Pure, platform-free domain logic for AgentPet on Linux.
//!
//! Ports the macOS `AgentPetCore` target 1:1: normalised agent state, the
//! session store with time-based pruning, per-agent event→state mapping, mood
//! aggregation, the Unix-socket wire format, spritesheet slicing, and hook
//! installation. Deliberately free of GTK/X11 and (in the hot paths) of
//! wall-clock reads — callers pass `now` so behaviour is deterministic and
//! unit-testable without a display server.

pub mod catalog;
pub mod config;
pub mod event;
pub mod hooks;
pub mod ipc;
pub mod mapper;
pub mod mood;
pub mod payloads;
pub mod session;
pub mod sprite;
pub mod state;

pub use catalog::{AgentCatalog, AgentIntegration};
pub use event::AgentEvent;
pub use hooks::{AgentHookSpec, AgentHooks, HookInstaller};
pub use mapper::StateMapper;
pub use mood::MoodResolver;
pub use payloads::{ClaudeHookPayload, HookArguments, HookPayload, RunArguments};
pub use session::{AgentSession, SessionStore};
pub use sprite::{PetBindings, PetManifest};
pub use state::{AgentKind, AgentSource, AgentState, PetMood, UnixTime};
