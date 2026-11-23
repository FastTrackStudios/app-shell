//! The components (feature `ui`): [`AppShell`] draws one window of a
//! layout; everything under it reaches the layout through [`use_shell`].
//!
//! They render the same on dioxus-web and dioxus-native (Blitz): inline
//! styles reading the theme's CSS variables, no `position: fixed`, no
//! `overflow: auto`, flex children that shrink (`min-width: 0`), pointer
//! events for every drag (no HTML drag and drop), and nothing measured
//! inside an event handler.

mod bar;
mod chrome;
mod foot;
pub mod icons;
mod kinds;
#[cfg(feature = "icons")]
mod lucide;
pub mod motion;
mod overlay;
mod palette;
mod popup;
mod shell;
mod sidebar;
mod split;
mod stack;
mod switcher;
#[cfg(test)]
mod tests;
mod window;
mod windows;

pub use bar::{
    BarContributions, BarFiller, BarItem, BarPlace, BarRender, BarSpan, use_top_bar_item,
    use_top_bar_span,
};
pub use chrome::{
    BarButton, DragArea, LIGHT, LIGHT_GAP, LIGHTS_LEFT, LIGHTS_WIDTH, SearchField, TOP_BAR_H,
    ToolbarButton, ToolbarGroup, TopBar, WindowLights,
};
pub use foot::{Destination, PrimaryAction};
pub use icons::{Icon, IconView, PUZZLE, named, named_icon};
pub use kinds::{Render, TabCtx, TabHooks, TabKind, TabKinds};
pub use overlay::{
    PaletteItem, Sheet, SheetContent, SheetHandle, SheetId, SheetSpec, Swipeable, ToastSpec,
    use_sheet,
};
pub use palette::{PaletteSources, ViewCommand, use_palette_sources, use_view_commands};
pub use popup::{MenuAction, MenuItem, Popup, PopupContent, PopupLayer, under};
pub use shell::{
    AppShell, BoundKey, FormFactor, KeyRun, KeyScope, KeymapHandle, PaletteOverlay, SHELL_CSS,
    Shell, Tabs, use_shell, use_window_layout, window_title,
};
pub use sidebar::{
    RIGHT_SIDEBAR_WIDTH, RightSidebarState, SIDEBAR_FOLD_BELOW, SIDEBAR_MAX, SIDEBAR_MIN,
    SIDEBAR_PAGE_ROOM, SIDEBAR_RAIL, SIDEBAR_SNAP, SIDEBAR_WIDTH, ScrollArea, Sidebar, SidebarItem,
    SidebarMode, SidebarSection, SidebarState, SidebarSwitcher, use_sidebar_mode,
};
pub use split::NodeView;
pub use stack::{StackView, TabBar, TabBody};
pub use switcher::Switcher;
pub use window::{InPageWindows, SoloView, WindowBody};
pub use windows::{HubMessage, LayoutHub, ViewportSize, WindowControls, WindowSystem, Windows};
