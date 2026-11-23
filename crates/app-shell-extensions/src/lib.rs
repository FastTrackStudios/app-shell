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
//! Moved here from `architect-extensions`' `ui` feature, onto app-shell:
//! the views are the shell's tab kinds in its panes ([`panes`]), the
//! chrome its top bar's items and the app's destinations ([`bar`]).

// A linked-in view's root is a fn pointer compared by Dioxus's props: a
// stale compare only redraws.
#![allow(unpredictable_function_pointer_comparisons)]

pub use architect_extensions;
pub use architect_extensions_dioxus;

pub mod bar;
pub mod embed_views;
pub mod field_focus;
pub mod host_ui_view;
pub mod interface;
pub mod keys;
pub mod palette;
pub mod panes;
pub mod settings;
pub mod settings_sheet;
pub mod theme_ui;
pub mod toolbar_editor;
pub mod toolbar_ui;
pub mod typing_ui;
pub mod ui;
pub mod window_ui;

#[cfg(all(feature = "remote", unix))]
pub mod barrier;
pub mod pacing;
#[cfg(all(feature = "remote", unix))]
pub mod remote;
#[cfg(feature = "remote")]
pub mod scene_widget;

pub mod kit;
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

/// The process extensions whose views the window draws: hand them to the
/// app's runner (`ProcessRunner::new()?.with_processes(processes())`), and
/// every pane, embed and overlay finds them, in the main window and in
/// pop-outs alike, without a context of the app's. A `Processes` the app
/// provides as a Dioxus context wins over these.
#[cfg(all(feature = "remote", unix))]
#[must_use]
pub fn processes() -> architect_extensions::process::Processes {
    PROCESSES
        .get_or_init(architect_extensions::process::Processes::default)
        .clone()
}

#[cfg(all(feature = "remote", unix))]
static PROCESSES: std::sync::OnceLock<architect_extensions::process::Processes> =
    std::sync::OnceLock::new();

/// The processes a view here draws from: the context's, else the ones the
/// app gave its runner ([`processes`]); none when it gave it neither.
#[cfg(all(feature = "remote", unix))]
pub(crate) fn use_processes() -> Option<architect_extensions::process::Processes> {
    dioxus::prelude::try_use_context::<architect_extensions::process::Processes>()
        .or_else(|| PROCESSES.get().cloned())
}

pub use embed_views::{BaseViewEmbed, EmbedAsk, EmbedPages, EmbedView, GuestEmbedView, WidgetView};

pub use bar::{Slot, use_extension_bar};
pub use interface::{ExtensionOverlays, ExtensionUi, ExtensionView, use_extension_ui};
pub use panes::ContextEntry;
pub use typing_ui::{EditorAction, Typing, TypingBar};
pub use window_ui::{ExtensionPopout, main_ui};
