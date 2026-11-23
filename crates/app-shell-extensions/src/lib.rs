//! Architect's extensions in an app-shell window.
//!
//! `architect-extensions` is the runtime (manifests, the host, linked,
//! process, Luau, WASI and data extensions) and draws nothing;
//! `architect-extensions-dioxus` is what an extension whose views are
//! Dioxus components links. This crate is the chrome around them: what
//! loaded extensions add to the window (tabs, toolbars, menus, status
//! items, their windows and sheets), their views in the panes, the app's
//! interface for them (notify, prompt, menu, clipboard, pickers), process
//! and WASI views as remote documents, and the extensions page.
//!
//! Moved here as it was from `architect-extensions`' `ui` feature; it
//! still draws with Architect's shell, workspace and commands, and moves
//! onto app-shell's next.

// A linked-in view's root is a fn pointer compared by Dioxus's props: a
// stale compare only redraws.
#![allow(unpredictable_function_pointer_comparisons)]

pub use architect_extensions;
pub use architect_extensions_dioxus;

pub mod embed_views;
pub mod host_ui_view;
pub mod interface;
pub mod theme_ui;
pub mod toolbar_editor;
pub mod toolbar_ui;
pub mod typing_ui;
pub mod ui;
pub mod window_ui;

#[cfg(all(feature = "remote", unix))]
pub mod barrier;
#[cfg(all(feature = "remote", unix))]
pub mod remote;
#[cfg(feature = "remote")]
pub mod scene_widget;

#[cfg(feature = "page")]
pub mod page;

#[cfg(all(feature = "pickers", target_os = "ios"))]
pub(crate) mod pickers_ios;

/// Hands the runtime what the window does for it: a process view's
/// `set_view_state`, what goes when an extension's session ends, and how
/// `EXT_CHECK` renders a process view. Idempotent; `use_extension_ui`
/// calls it.
pub fn install() {
    #[cfg(all(feature = "remote", unix))]
    {
        architect_extensions::hostapi::set_view_state_sink(remote::offer_view_state);
        architect_extensions::check::set_frame_renderer(remote::render_frame);
    }
    #[cfg(all(feature = "pickers", target_os = "ios"))]
    architect_extensions::hostapi::on_forget(pickers_ios::stop_scopes);
}

pub use embed_views::{BaseViewEmbed, EmbedAsk, EmbedPages, EmbedView, GuestEmbedView, WidgetView};
