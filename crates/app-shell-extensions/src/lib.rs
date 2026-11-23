//! Architect's extensions in an app-shell window.
//!
//! `architect-extensions` is the runtime (manifests, the host, linked,
//! process, Luau, WASI and data extensions) and draws nothing;
//! `architect-extensions-dioxus` is what an extension whose views are
//! Dioxus components links. This crate is the chrome around them, on
//! app-shell: what loaded extensions add to the shell (tabs, toolbars,
//! menus, status items), their windows and sheets, their views in the
//! shell's panes, and the app's interface for them.
//!
//! It is being moved here from `architect-extensions`' `ui` feature
//! (FastTrackStudios/architect, branch `strip/application-framework`).

pub use architect_extensions;
pub use architect_extensions_dioxus;
