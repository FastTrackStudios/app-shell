//! An app's shell for Dioxus: windows, their splits, tab stacks and tabs,
//! docks, floats, pop-out windows and the tab switcher.
//!
//! Two halves:
//!
//! - **The layout**, plain data ([`Workspace`]): every window an app has
//!   open, each a tree of [`Split`]s ending in tab stacks ([`Group`]s),
//!   docks on its sides and stacks floating over it, and every tab
//!   ([`Pane`]) wherever it is. The operations ([`Workspace::open`],
//!   [`Workspace::move_pane`], [`Workspace::close`], …) are what every
//!   gesture and key comes down to; after each one the layout is
//!   [repaired](Workspace::repair) to its invariants. It serializes with
//!   serde; the app stores it wherever it likes.
//! - **The components** (feature `ui`, on by default): [`ui::AppShell`]
//!   draws one window of a layout and turns pointer drags and keys into
//!   operations, on dioxus-web and dioxus-native (Blitz) alike.
//!
//! The seams the app owns: the layout's storage (a `Signal<Workspace>`
//! and an `on_change`), its keys ([`Keymap`], [`Command`]), its theme
//! ([`Theme`], CSS variables) and its tab kinds (`ui::TabKinds`: id,
//! title, icon, render function). Native OS windows for pop-outs are
//! the `app-shell-native` crate's.

mod commands;
mod drop;
mod geometry;
pub mod gestures;
mod model;
mod ops;
mod repair;
mod switcher;
mod theme;
#[cfg(feature = "ui")]
pub mod ui;

pub use commands::{Bindings, Command, DefaultKeymap, KeyChord, Keymap, Ran};
pub use drop::{
    Bar, Drop, EDGE_BAND, PIN_MARGIN, SPLIT_MAX, SPLIT_MIN, SPLIT_SHARE, bar_drop, contains,
    drop_at, drop_in, float_rect, snap_at,
};
pub use geometry::{Direction, Placed, Slide, area_rects, neighbour, scroll_to, slides};
pub use model::{
    Above, Aliases, Area, Axis, Child, Chrome, ClosedPane, Corner, Dock, Docks, Float, Group,
    GroupAlias, GroupId, History, LinkId, Location, Node, Pane, PaneId, Pin, Rect, Shown, Side,
    Snap, Split, SplitId, ViewKind, Window, WindowAlias, WindowId, Workspace,
};
pub use ops::{Dest, Target, WorkspaceError};
pub use switcher::{Card, CardGrid, Place, reorder_dest};
pub use theme::{LIGHT, Theme, VARIABLES};
