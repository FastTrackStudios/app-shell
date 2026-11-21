//! The components (feature `ui`): [`AppShell`] draws one window of a
//! layout; everything under it reaches the layout through [`use_shell`].
//!
//! They render the same on dioxus-web and dioxus-native (Blitz): inline
//! styles reading the theme's CSS variables, no `position: fixed`, no
//! `overflow: auto`, flex children that shrink (`min-width: 0`), pointer
//! events for every drag (no HTML drag and drop), and nothing measured
//! inside an event handler.

pub mod icons;
mod kinds;
mod popup;
mod shell;
mod split;
mod stack;
mod switcher;
#[cfg(test)]
mod tests;
mod title;
mod window;
mod windows;

pub use icons::{Icon, IconView};
pub use kinds::{Render, TabCtx, TabKind, TabKinds};
pub use popup::{MenuAction, MenuItem, Popup, PopupContent, PopupLayer};
pub use shell::{
    AppShell, FormFactor, KeymapHandle, SHELL_CSS, Shell, use_shell, use_window_layout,
    window_title,
};
pub use split::NodeView;
pub use stack::{StackView, TabBar, TabBody};
pub use switcher::Switcher;
pub use title::{BarButton, TitleBar};
pub use window::{InPageWindows, PhoneView, WindowBody};
pub use windows::{HubMessage, LayoutHub, WindowControls, WindowSystem, Windows};
