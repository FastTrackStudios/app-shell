//! The shell's state in context ([`Shell`]) and its root ([`AppShell`]).

use std::rc::Rc;

use dioxus::prelude::*;
use futures_util::StreamExt;

use super::bar::{BarContributions, BarFiller, BarItem, BarSpan};
use super::chrome::TopBar;
use super::foot::{DeskAccessory, Destination, PhoneFoot, PrimaryAction};
use super::kinds::{TabKind, TabKinds};
use super::overlay::{
    PaletteItem, SheetContent, SheetEntry, SheetId, SheetLayer, SheetSpec, ToastEntry, ToastLayer,
    ToastSpec,
};
use super::popup::MenuAction;
use super::popup::{MenuItem, Popup, PopupContent, PopupLayer};
use super::sidebar::{
    DockedSidebar, DrawerSidebar, RightSidebar, RightSidebarState, SIDEBAR_MAX, SIDEBAR_MIN,
    SidebarState, Tip, TipLayer, right_drag, sidebar_drag,
};
use super::stack::DragLayer;
use super::switcher::Switcher;
use super::window::{InPageWindows, SoloView, WindowBody};
use super::windows::{HubMessage, LayoutHub, ViewportSize, Windows};
use crate::commands::{Command, DefaultKeymap, KeyChord, Keymap, Ran};
use crate::gestures::{
    DRAG_SLOP, Detent, PHONE_BELOW, Settle, drawer_progress, drawer_settles_open, drawer_width,
    sheet_settle, starts_edge_swipe,
};
use crate::model::{Axis, GroupId, Pane, PaneId, Rect, Side, SplitId, WindowId, Workspace};
use crate::ops::{Dest, Target, WorkspaceError};
use crate::theme::Theme;

/// How the shell lays a window out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormFactor {
    /// Splits, docks, floats and tab bars.
    Desktop,
    /// One tab at a time, the others in the switcher.
    Phone,
}

impl FormFactor {
    /// A window this wide, in points: a phone under 600.
    #[must_use]
    pub fn for_width(width: f64) -> Self {
        if width < PHONE_BELOW {
            Self::Phone
        } else {
            Self::Desktop
        }
    }
}

/// Whether the shell draws tabs.
///
/// Tabs are opt-in: by default a window is one content area with no tab
/// bar, no drop zones and no switcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tabs {
    /// One content area: the shell's `children`, or (with a `layout`) the
    /// layout's active tab, alone. Tab keys do nothing.
    #[default]
    Off,
    /// One content area while the layout is simple ([`Workspace::is_simple`]):
    /// the tab chrome appears once the app opens a second tab, a split, a
    /// dock, a float or a pop-out, and goes when it is simple again.
    Auto,
    /// Tab bars, splits, docks, floats, pop-outs and the switcher.
    On,
}

/// The app's [`Keymap`], as a prop (compared by identity).
#[derive(Clone)]
pub struct KeymapHandle(pub Rc<dyn Keymap>);

impl KeymapHandle {
    pub fn new(keymap: impl Keymap + 'static) -> Self {
        Self(Rc::new(keymap))
    }
}

impl PartialEq for KeymapHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Default for KeymapHandle {
    fn default() -> Self {
        Self::new(DefaultKeymap)
    }
}

/// Where a [`BoundKey`] holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyScope {
    /// Everywhere in the window, after the [`Keymap`]'s commands.
    Window,
    /// While the active tab is of this kind, before the [`Keymap`]: a
    /// view's own keys (⌘Z in a chart, plain letters for note input).
    Kind(String),
}

/// What a [`BoundKey`] does, told the active tab; whether it took the key.
pub type KeyRun = Rc<dyn Fn(Option<&Pane>) -> bool>;

/// A key bound to something the app does itself (an extension's action),
/// not a layout [`Command`]: [`Shell::bind_keys`].
#[derive(Clone)]
pub struct BoundKey {
    pub chord: KeyChord,
    pub scope: KeyScope,
    /// Does it, told the active tab: false when it did not take the key
    /// (a plain letter typed into a text field), which then goes on.
    pub run: KeyRun,
}

impl BoundKey {
    /// `chord` in `scope`, running `run`.
    pub fn new(
        chord: KeyChord,
        scope: KeyScope,
        run: impl Fn(Option<&Pane>) -> bool + 'static,
    ) -> Self {
        Self {
            chord,
            scope,
            run: Rc::new(run),
        }
    }
}

impl std::fmt::Debug for BoundKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoundKey")
            .field("chord", &self.chord)
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}

/// An overlay of the shell's palette slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaletteOverlay {
    /// The command palette (Mod+K).
    Palette,
    /// Open anything (⌘O).
    QuickOpen,
}

impl PaletteOverlay {
    /// `palette` or `quick-open`.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "palette" => Some(Self::Palette),
            "quick-open" => Some(Self::QuickOpen),
            _ => None,
        }
    }

    /// Its name: `palette` or `quick-open`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Palette => "palette",
            Self::QuickOpen => "quick-open",
        }
    }
}

/// A pointer gesture under way.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Gesture {
    /// A tab pressed; `live` once it moved far enough to be a drag.
    Tab {
        pane: PaneId,
        title: String,
        start: (f64, f64),
        at: (f64, f64),
        live: bool,
    },
    /// The right sidebar's edge dragged.
    RightSidebar { start: (f64, f64), width: f32 },
    /// A split's divider dragged: between child `index` and the next.
    Divider {
        split: SplitId,
        index: usize,
        axis: Axis,
        start: (f64, f64),
        shares: Vec<f32>,
        /// The split's length along its axis, once measured.
        length: Option<f64>,
    },
    /// A dock's edge dragged.
    Dock {
        side: Side,
        start: (f64, f64),
        size: f32,
    },
    /// A float moved by its grip.
    Float {
        pane: PaneId,
        start: (f64, f64),
        rect: Rect,
    },
    /// The docked sidebar's edge dragged: its full width, and whether it
    /// was the rail, when the drag began.
    Sidebar {
        start: (f64, f64),
        width: f32,
        rail: bool,
    },
    /// A phone's drawer under the finger: opening from the edge (`from`
    /// 0) or closing (`from` 1).
    Drawer { start: (f64, f64), from: f64 },
    /// A bottom sheet dragged by its handle.
    Sheet { id: u64, start: (f64, f64) },
    /// A pop-out drawn in the page, moved by its title bar.
    Window {
        window: WindowId,
        start: (f64, f64),
        rect: Rect,
    },
}

/// Where a dragged tab would land, and which zone says so (for its
/// highlight).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Hover {
    pub dest: Dest,
    pub zone: Zone,
}

/// A drop zone, named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Zone {
    /// A stack's middle (a new tab there) or a side of it (a split).
    Group(GroupId, Option<Side>),
    /// Before tab `index` of a stack's bar.
    Bar(GroupId, usize),
    /// A window's edge: its dock.
    Edge(Side),
    /// A window of its own.
    NewWindow,
}

/// What the app hands the shell, kept as it renders (not reactive).
#[derive(Clone, Default)]
struct Hooks {
    on_change: Option<EventHandler<Workspace>>,
    on_command: Option<Callback<Command, bool>>,
    on_new_tab: Option<EventHandler<GroupId>>,
    on_sidebar: Option<EventHandler<SidebarState>>,
    on_right_sidebar: Option<EventHandler<RightSidebarState>>,
    palette: Option<Callback<(), Vec<PaletteItem>>>,
    kinds: TabKinds,
    keymap: KeymapHandle,
    hub: Option<LayoutHub>,
}

/// The keys bound with [`Shell::bind_keys`], by who bound them.
type Bound = Vec<(String, Vec<BoundKey>)>;

/// The shell of one window, in context for everything under
/// [`AppShell`]: the layout, and what a component inside does to it.
#[derive(Clone, Copy, PartialEq)]
pub struct Shell {
    /// The layout (the app's signal).
    pub layout: Signal<Workspace>,
    /// The window this shell draws.
    pub window: Signal<WindowId>,
    pub(crate) form: Signal<FormFactor>,
    pub(crate) gesture: Signal<Option<Gesture>>,
    pub(crate) hover: Signal<Option<Hover>>,
    /// Whether a zone heard the pointer move this time (the root clears
    /// the hover when none did).
    pub(crate) hit: CopyValue<bool>,
    pub(crate) switcher: Signal<bool>,
    pub(crate) popup: Signal<Option<Popup>>,
    /// Whether tabs are drawn: the app's mode.
    pub(crate) tabs: Signal<Tabs>,
    /// Whether the app gave a sidebar.
    pub(crate) sidebar_given: Signal<bool>,
    /// The docked sidebar: shown, and its width.
    pub(crate) sidebar_open: Signal<bool>,
    pub(crate) sidebar_w: Signal<f32>,
    /// Folded to the rail, as chosen (`None`: by the window's width).
    pub(crate) sidebar_rail: Signal<Option<bool>>,
    /// Whether the app gave a right sidebar.
    pub(crate) right_given: Signal<bool>,
    /// The right sidebar: shown, and its width.
    pub(crate) right_open: Signal<bool>,
    pub(crate) right_w: Signal<f32>,
    /// What apps, pages and extensions put in the top bar ([`super::bar`]).
    pub(crate) bar: Signal<BarContributions>,
    /// Each sidebar section's folding, by its id: kept across the rail,
    /// the drawer and the sidebar coming back.
    pub(crate) section_open: Signal<std::collections::HashMap<String, bool>>,
    /// A rail icon's name on show.
    pub(crate) tip: Signal<Option<Tip>>,
    /// Where the shell's root is in the window (its overlays are placed
    /// from it).
    pub(crate) origin: Signal<(f64, f64)>,
    /// The phone's drawer is out.
    pub(crate) drawer: Signal<bool>,
    /// How open the drawer is under the finger (0 to 1), while dragged.
    pub(crate) drawer_drag: Signal<Option<f64>>,
    /// The last drawer gesture moved (a tap on the scrim closes; a drag
    /// that let go does not).
    pub(crate) drawer_dragged: CopyValue<bool>,
    /// The sheets open, the last on top.
    pub(crate) sheets: Signal<Vec<SheetEntry>>,
    /// The toasts waiting, the first on show.
    pub(crate) toasts: Signal<Vec<ToastEntry>>,
    next_id: CopyValue<u64>,
    /// Keys the app binds past its keymap ([`Shell::bind_keys`]).
    bound: CopyValue<Bound>,
    /// What draws each palette overlay in the shell's place
    /// ([`Shell::set_palette_view`]).
    palette_views: CopyValue<Vec<(PaletteOverlay, Callback<(), Element>)>>,
    /// The query the next palette opens with.
    palette_query: CopyValue<String>,
    /// The window's size, points (0 until measured).
    pub(crate) viewport: Signal<(f64, f64)>,
    hooks: CopyValue<Hooks>,
}

/// The [`Shell`] of the window the calling component is in.
#[must_use]
pub fn use_shell() -> Shell {
    use_context::<Shell>()
}

impl Shell {
    /// Change the layout with `op`, then tell the app (`on_change`) and
    /// the other windows. A failed `op` leaves the layout as it was.
    pub fn apply<T>(
        &self,
        op: impl FnOnce(&mut Workspace) -> Result<T, WorkspaceError>,
    ) -> Option<T> {
        let out = self.apply_quiet(op)?;
        self.changed();
        Some(out)
    }

    /// [`Self::apply`] without telling anyone: a gesture's steps, told
    /// once at its end ([`Self::changed`]).
    pub fn apply_quiet<T>(
        &self,
        op: impl FnOnce(&mut Workspace) -> Result<T, WorkspaceError>,
    ) -> Option<T> {
        let mut layout = self.layout;
        let mut next = layout.peek().clone();
        match op(&mut next) {
            Ok(out) => {
                if !next.same_arrangement(&layout.peek()) {
                    layout.set(next);
                }
                Some(out)
            }
            Err(error) => {
                tracing::debug!(app_shell.op_error = %error, "layout operation refused");
                None
            }
        }
    }

    /// Tell the app and the other windows the layout is what it is now.
    pub fn changed(&self) {
        let hooks = self.hooks.peek().clone();
        let layout = self.layout.peek().clone();
        if let Some(hub) = &hooks.hub {
            hub.publish(&self.window.peek(), &layout);
        }
        if let Some(on_change) = hooks.on_change {
            on_change.call(layout);
        }
    }

    /// Open `pane` at `target` (an open id comes forward instead).
    pub fn open(&self, pane: Pane, target: Target) -> Option<PaneId> {
        self.apply(|w| w.open(pane, target))
    }

    /// Run `command`: the app's `on_command` hears it first and may take
    /// it; otherwise the shell runs it on the layout.
    pub fn command(&self, command: Command) {
        let on_command = self.hooks.peek().on_command;
        if on_command.is_some_and(|f| f.call(command)) {
            return;
        }
        if command == Command::OpenPalette {
            self.open_palette();
            return;
        }
        if command == Command::ToggleRightSidebar {
            if self.has_right_sidebar() && self.form() != FormFactor::Phone {
                let now = self.right_sidebar_state();
                self.set_right_sidebar(RightSidebarState {
                    open: !now.open,
                    ..now
                });
            }
            return;
        }
        if command == Command::ToggleSidebar {
            if self.has_sidebar() {
                if self.form() == FormFactor::Phone {
                    let out = *self.drawer.peek();
                    self.set_drawer(!out);
                } else {
                    // Folded to the rail and back (Architect's); a hidden
                    // sidebar comes back at its full width.
                    let now = self.sidebar_state();
                    let rail = now.open && !self.sidebar_folded();
                    self.set_sidebar(SidebarState {
                        open: true,
                        rail: Some(rail),
                        ..now
                    });
                }
            }
            return;
        }
        let mut layout = self.layout;
        let mut next = layout.peek().clone();
        match next.run(command) {
            Ok(Ran::Changed) => {
                layout.set(next);
                self.changed();
            }
            Ok(Ran::Ui) => {
                if command == Command::OpenSwitcher {
                    let mut s = self.switcher;
                    s.set(true);
                }
            }
            Ok(Ran::Nothing) => {}
            Err(error) => {
                tracing::debug!(app_shell.command = ?command, app_shell.op_error = %error, "command refused")
            }
        }
    }

    /// Run `command` on `pane`: it becomes the active tab first.
    pub fn command_on(&self, pane: &PaneId, command: Command) {
        self.apply_quiet(|w| w.activate(pane));
        self.command(command);
    }

    /// Show the tab switcher (or hide it).
    pub fn show_switcher(&self, show: bool) {
        let mut s = self.switcher;
        s.set(show);
    }

    /// Open a menu holding `items`: at `(x, y)` (window points) on a
    /// desktop, an action sheet on a phone.
    pub fn menu(&self, x: f64, y: f64, items: Vec<MenuItem>) {
        if *self.form.peek() == FormFactor::Phone {
            self.close_popup();
            self.open_sheet(SheetSpec {
                title: None,
                detent: Detent::Fit,
                detents: vec![Detent::Fit],
                content: SheetContent::Menu(items),
                on_close: None,
            });
        } else {
            self.popup(x, y, PopupContent::Menu(items));
        }
    }

    /// Open a sheet over everything open: a bottom sheet on a phone, a
    /// centred panel on a desktop. See [`super::use_sheet`] for one that
    /// closes with the component that opened it.
    pub fn open_sheet(&self, spec: SheetSpec) -> SheetId {
        let mut next = self.next_id;
        let n = *next.peek();
        next.set(n.saturating_add(1));
        let id = SheetId(n);
        let detent = spec.detent;
        let mut sheets = self.sheets;
        sheets.write().push(SheetEntry {
            id,
            spec,
            detent,
            drag: 0.0,
        });
        id
    }

    /// Close the sheet `id`, telling its `on_close`.
    pub fn close_sheet(&self, id: SheetId) {
        let on_close = self
            .sheets
            .peek()
            .iter()
            .find(|s| s.id == id)
            .and_then(|s| s.spec.on_close);
        self.close_sheet_quiet(id);
        if let Some(f) = on_close {
            f.call(());
        }
    }

    /// Close the sheet `id` without telling it (its owner closed it).
    pub(crate) fn close_sheet_quiet(&self, id: SheetId) {
        let mut sheets = self.sheets;
        if sheets.peek().iter().any(|s| s.id == id) {
            sheets.write().retain(|s| s.id != id);
        }
    }

    /// Close the sheet on top, if any; whether there was one.
    pub fn close_top_sheet(&self) -> bool {
        let top = self.sheets.peek().last().map(|s| s.id);
        top.inspect(|id| self.close_sheet(*id)).is_some()
    }

    /// Do what a menu row or a palette entry says.
    pub(crate) fn run_action(&self, action: Option<MenuAction>) {
        match action {
            Some(MenuAction::Command {
                pane: Some(p),
                command,
            }) => self.command_on(&p, command),
            Some(MenuAction::Command {
                pane: None,
                command,
            }) => self.command(command),
            Some(MenuAction::Call(f)) => f.call(()),
            None => {}
        }
    }

    /// Show `message` for a moment, after the toasts before it.
    pub fn toast(&self, message: impl Into<String>) {
        self.show_toast(ToastSpec::new(message));
    }

    /// Show a toast (with an action, a duration of its own).
    pub fn show_toast(&self, spec: ToastSpec) {
        let mut next = self.next_id;
        let n = *next.peek();
        next.set(n.saturating_add(1));
        let mut toasts = self.toasts;
        toasts.write().push(ToastEntry { id: n, spec });
    }

    /// Open the command palette with the app's entries (`palette`):
    /// a sheet on a phone, a panel near the top on a desktop. When a view
    /// draws it ([`Self::set_palette_view`]), that view, in its place.
    pub fn open_palette(&self) {
        self.open_overlay(PaletteOverlay::Palette, "");
    }

    /// Open open-anything (⌘O): what the openers in the
    /// [`super::PaletteSources`] in context find, or the view that draws
    /// it.
    pub fn open_quick_open(&self) {
        self.open_overlay(PaletteOverlay::QuickOpen, "");
    }

    /// Open `overlay`, its field holding `query` (`theme:` asks one
    /// opener). One open already closes first.
    pub fn open_overlay(&self, overlay: PaletteOverlay, query: &str) {
        self.close_palette();
        self.close_popup();
        let mut q = self.palette_query;
        q.set(query.to_owned());
        let content = match (self.palette_view(overlay), overlay) {
            (Some(draw), _) => SheetContent::Overlay(draw),
            (None, PaletteOverlay::Palette) => SheetContent::Palette(
                self.hooks
                    .peek()
                    .palette
                    .map(|f| f.call(()))
                    .unwrap_or_default(),
            ),
            (None, PaletteOverlay::QuickOpen) => SheetContent::QuickOpen,
        };
        self.open_sheet(SheetSpec {
            title: None,
            detent: Detent::Large,
            detents: vec![Detent::Large],
            content,
            on_close: None,
        });
    }

    /// Close the palette's overlay, if one is open.
    pub fn close_palette(&self) {
        let open: Vec<SheetId> = self
            .sheets
            .peek()
            .iter()
            .filter(|s| {
                matches!(
                    s.spec.content,
                    SheetContent::Palette(_) | SheetContent::QuickOpen | SheetContent::Overlay(_)
                )
            })
            .map(|s| s.id)
            .collect();
        for id in open {
            self.close_sheet(id);
        }
    }

    /// Draws `overlay` with `draw` in place of the shell's own (an
    /// extension's palette); `None` gives it back to the shell.
    pub fn set_palette_view(&self, overlay: PaletteOverlay, draw: Option<Callback<(), Element>>) {
        let mut views = self.palette_views;
        let mut all = views.write();
        all.retain(|(o, _)| *o != overlay);
        if let Some(draw) = draw {
            all.push((overlay, draw));
        }
    }

    /// What draws `overlay` in the shell's place, if anything.
    #[must_use]
    pub fn palette_view(&self, overlay: PaletteOverlay) -> Option<Callback<(), Element>> {
        self.palette_views
            .peek()
            .iter()
            .find(|(o, _)| *o == overlay)
            .map(|(_, d)| *d)
    }

    /// The query the palette opening now starts with (then none).
    pub(crate) fn take_palette_query(&self) -> String {
        let mut q = self.palette_query;
        std::mem::take(&mut *q.write())
    }

    /// Close what floats over the page for a navigation: the popup, the
    /// drawer, the switcher.
    pub fn close_overlays(&self) {
        self.close_popup();
        self.set_drawer(false);
        self.show_switcher(false);
    }

    /// The window's size, points (`(0, 0)` until measured).
    #[must_use]
    pub fn viewport(&self) -> (f64, f64) {
        *self.viewport.read()
    }

    /// Open a popup at `(x, y)` (window points).
    pub fn popup(&self, x: f64, y: f64, content: PopupContent) {
        let mut p = self.popup;
        p.set(Some(Popup { x, y, content }));
    }

    /// Close the popup, if one is open.
    pub fn close_popup(&self) {
        let mut p = self.popup;
        if p.peek().is_some() {
            p.set(None);
        }
    }

    /// Whether the app gave a sidebar.
    #[must_use]
    pub fn has_sidebar(&self) -> bool {
        *self.sidebar_given.read()
    }

    /// Whether the window draws tabs now ([`Tabs`]).
    #[must_use]
    pub fn tabs_shown(&self) -> bool {
        match *self.tabs.read() {
            Tabs::Off => false,
            Tabs::On => true,
            Tabs::Auto => !self.layout.read().is_simple(),
        }
    }

    /// Open or close the phone's sidebar drawer.
    pub fn set_drawer(&self, out: bool) {
        if out {
            let mut dragged = self.drawer_dragged;
            dragged.set(false);
        }
        let mut drawer = self.drawer;
        if *drawer.peek() != out {
            drawer.set(out);
        }
    }

    /// The docked sidebar's state now.
    #[must_use]
    pub fn sidebar_state(&self) -> SidebarState {
        SidebarState {
            open: *self.sidebar_open.peek(),
            width: *self.sidebar_w.peek(),
            rail: *self.sidebar_rail.peek(),
        }
    }

    /// Whether the docked sidebar is folded to the rail now (chosen, or by
    /// the window's width).
    #[must_use]
    pub fn sidebar_folded(&self) -> bool {
        let rail = *self.sidebar_rail.read();
        let w = self.viewport.read().0;
        SidebarState {
            rail,
            ..SidebarState::default()
        }
        .folded(w)
    }

    /// Show, hide, fold or resize the docked sidebar, telling the app.
    pub fn set_sidebar(&self, state: SidebarState) {
        let width = state.width.clamp(SIDEBAR_MIN, SIDEBAR_MAX);
        let (mut open, mut w, mut rail) = (self.sidebar_open, self.sidebar_w, self.sidebar_rail);
        if *open.peek() != state.open {
            open.set(state.open);
        }
        if (*w.peek() - width).abs() > f32::EPSILON {
            w.set(width);
        }
        if *rail.peek() != state.rail {
            rail.set(state.rail);
        }
        if let Some(f) = self.hooks.peek().on_sidebar {
            f.call(SidebarState { width, ..state });
        }
    }

    /// Whether the app gave a right sidebar.
    #[must_use]
    pub fn has_right_sidebar(&self) -> bool {
        *self.right_given.read()
    }

    /// The right sidebar's state now.
    #[must_use]
    pub fn right_sidebar_state(&self) -> RightSidebarState {
        RightSidebarState {
            open: *self.right_open.peek(),
            width: *self.right_w.peek(),
        }
    }

    /// Show, hide or resize the right sidebar, telling the app.
    pub fn set_right_sidebar(&self, state: RightSidebarState) {
        let width = state.width.clamp(SIDEBAR_MIN, SIDEBAR_MAX);
        let (mut open, mut w) = (self.right_open, self.right_w);
        if *open.peek() != state.open {
            open.set(state.open);
        }
        if (*w.peek() - width).abs() > f32::EPSILON {
            w.set(width);
        }
        if let Some(f) = self.hooks.peek().on_right_sidebar {
            f.call(RightSidebarState { width, ..state });
        }
    }

    /// Put an item in a place of the top bar (in place of one with its
    /// id) — [`super::bar`].
    pub fn put_bar_item(&self, item: BarItem) {
        let mut bar = self.bar;
        bar.write().put(item);
    }

    /// Take an item out of the top bar.
    pub fn remove_bar_item(&self, id: &str) {
        let mut bar = self.bar;
        if bar.peek().items_with_id(id) {
            bar.write().remove(id);
        }
    }

    /// Take over a span of the top bar.
    pub fn fill_bar(&self, filler: BarFiller) {
        let mut bar = self.bar;
        bar.write().fill(filler);
    }

    /// Give back a span `owner` took over.
    pub fn unfill_bar(&self, owner: &str, span: BarSpan) {
        let mut bar = self.bar;
        bar.write().unfill(owner, span);
    }

    /// The kind a pane names, if the app registered it.
    #[must_use]
    pub fn kind(&self, id: &str) -> Option<TabKind> {
        self.hooks.peek().kinds.get(id).cloned()
    }

    /// The kinds the app registered.
    #[must_use]
    pub fn kinds(&self) -> TabKinds {
        self.hooks.peek().kinds.clone()
    }

    /// Whether the app wants a "new tab" button on the tab bars.
    #[must_use]
    pub(crate) fn new_tab(&self) -> Option<EventHandler<GroupId>> {
        self.hooks.peek().on_new_tab
    }

    /// The layout form this window has.
    #[must_use]
    pub fn form(&self) -> FormFactor {
        *self.form.read()
    }

    /// A drop zone heard the pointer: `dest`, lit as `zone`.
    pub(crate) fn hover(&self, zone: Zone, dest: Dest) {
        let mut hit = self.hit;
        hit.set(true);
        let next = Some(Hover { dest, zone });
        let mut hover = self.hover;
        if *hover.peek() != next {
            hover.set(next);
        }
    }

    /// Whether a tab is being dragged.
    pub(crate) fn dragging(&self) -> bool {
        matches!(&*self.gesture.read(), Some(Gesture::Tab { live: true, .. }))
    }

    /// [`Self::dragging`], not subscribing.
    #[cfg(test)]
    pub(crate) fn dragging_now(&self) -> bool {
        matches!(&*self.gesture.peek(), Some(Gesture::Tab { live: true, .. }))
    }

    /// What the keys `owner` binds are from now on (none: it binds none),
    /// past the app's [`Keymap`]: a tab kind's before it, the window's
    /// after it. Between owners, the first to bind a chord wins it.
    pub fn bind_keys(&self, owner: &str, keys: Vec<BoundKey>) {
        let mut bound = self.bound;
        let mut all = bound.write();
        if keys.is_empty() {
            all.retain(|(o, _)| o != owner);
        } else if let Some(at) = all.iter_mut().find(|(o, _)| o == owner) {
            at.1 = keys;
        } else {
            all.push((owner.to_owned(), keys));
        }
    }

    /// Does what `chord` asks in this window, as a key pressed in it:
    /// the active tab's kind's bound keys, then the [`Keymap`]'s command,
    /// then the window's bound keys. Whether something took it.
    pub fn press_key(&self, chord: &KeyChord) -> bool {
        let active = {
            let layout = self.layout.peek();
            layout.active.as_ref().and_then(|a| layout.pane(a)).cloned()
        };
        let bound: Vec<BoundKey> = self
            .bound
            .peek()
            .iter()
            .flat_map(|(_, keys)| keys.iter().filter(|k| k.chord == *chord).cloned())
            .collect();
        let kind = active.as_ref().map(|p| p.view.0.as_str());
        let in_kind = bound
            .iter()
            .filter(|k| matches!(&k.scope, KeyScope::Kind(v) if Some(v.as_str()) == kind));
        for k in in_kind {
            if (k.run)(active.as_ref()) {
                return true;
            }
        }
        if self.key(chord) {
            return true;
        }
        bound
            .iter()
            .filter(|k| k.scope == KeyScope::Window)
            .any(|k| (k.run)(active.as_ref()))
    }

    fn key(&self, chord: &KeyChord) -> bool {
        let keymap = self.hooks.peek().keymap.clone();
        let tabs = self.tabs_shown();
        match keymap.0.command(chord) {
            // Without tabs, only the commands that are not about tabs.
            Some(command)
                if tabs
                    || matches!(
                        command,
                        Command::ToggleSidebar | Command::ToggleRightSidebar | Command::OpenPalette
                    ) =>
            {
                self.command(command);
                true
            }
            _ => false,
        }
    }

    /// The pointer moved to `at`, with no button or with one held.
    pub(crate) fn moved(&self, at: (f64, f64)) {
        let mut gesture = self.gesture;
        let current = gesture.peek().clone();
        match current {
            Some(Gesture::Tab {
                pane,
                title,
                start,
                live,
                ..
            }) => {
                let far = (at.0 - start.0).hypot(at.1 - start.1) > 5.0;
                gesture.set(Some(Gesture::Tab {
                    pane,
                    title,
                    start,
                    at,
                    live: live || far,
                }));
            }
            Some(Gesture::Divider {
                split,
                index,
                axis,
                start,
                shares,
                length: Some(length),
            }) if length > 0.0 => {
                let along = match axis {
                    Axis::Row => at.0 - start.0,
                    Axis::Column => at.1 - start.1,
                };
                let next = resized(&shares, index, narrow(along / length));
                self.apply_quiet(|w| w.set_shares(split, &next));
            }
            Some(Gesture::Dock { side, start, size }) => {
                let delta = match side {
                    Side::Left => at.0 - start.0,
                    Side::Right => start.0 - at.0,
                    Side::Bottom | Side::Top => start.1 - at.1,
                };
                self.apply_quiet(|w| w.set_dock_size(side, size + narrow(delta)));
            }
            Some(Gesture::Drawer { start, from }) => {
                let (dx, dy) = (at.0 - start.0, at.1 - start.1);
                let mut drag = self.drawer_drag;
                // A vertical move first is the page's scroll, not the drawer.
                if drag.peek().is_none() && dy.abs() > DRAG_SLOP && dy.abs() > dx.abs() {
                    gesture.set(None);
                } else if dx.abs() >= DRAG_SLOP || drag.peek().is_some() {
                    let w = drawer_width(self.viewport.peek().0.max(320.0));
                    drag.set(Some(drawer_progress(from, dx, w)));
                }
            }
            Some(Gesture::Sheet { id, start }) => {
                let dy = at.1 - start.1;
                let mut sheets = self.sheets;
                if let Some(e) = sheets.write().iter_mut().find(|e| e.id.0 == id) {
                    e.drag = dy;
                }
            }
            Some(Gesture::RightSidebar { start, width }) if (at.0 - start.0).abs() >= DRAG_SLOP => {
                let vw = self.viewport.peek().0;
                let next = right_drag(width, narrow(at.0 - start.0), vw);
                let mut w = self.right_w;
                if (*w.peek() - next).abs() > f32::EPSILON {
                    w.set(next);
                }
            }
            Some(Gesture::Sidebar { start, width, rail })
                if (at.0 - start.0).abs() >= DRAG_SLOP =>
            {
                let vw = self.viewport.peek().0;
                let (folded, next) = sidebar_drag(width, rail, narrow(at.0 - start.0), vw);
                let (mut w, mut r) = (self.sidebar_w, self.sidebar_rail);
                if (*w.peek() - next).abs() > f32::EPSILON {
                    w.set(next);
                }
                if *r.peek() != Some(folded) {
                    r.set(Some(folded));
                }
            }
            Some(Gesture::Float { pane, start, rect }) => {
                let moved = Rect::new(
                    rect.x + narrow(at.0 - start.0),
                    (rect.y + narrow(at.1 - start.1)).max(0.0),
                    rect.w,
                    rect.h,
                );
                self.apply_quiet(|w| w.set_float_rect(&pane, moved));
            }
            Some(Gesture::Window {
                window,
                start,
                rect,
            }) => {
                let moved = Rect::new(
                    rect.x + narrow(at.0 - start.0),
                    (rect.y + narrow(at.1 - start.1)).max(0.0),
                    rect.w,
                    rect.h,
                );
                self.apply_quiet(|w| w.set_window_bounds(&window, moved));
            }
            _ => {}
        }
        let mut hit = self.hit;
        if !hit.replace(false) {
            let mut hover = self.hover;
            if hover.peek().is_some() {
                hover.set(None);
            }
        }
    }

    /// The pointer let go at `at`: a drag lands, a resize is told.
    pub(crate) fn released(&self, at: (f64, f64)) {
        let mut gesture = self.gesture;
        let Some(done) = gesture.take() else {
            return;
        };
        let mut hover = self.hover;
        let landed = hover.take();
        match done {
            Gesture::Tab {
                pane, live: true, ..
            } => {
                if let Some(Hover { dest, .. }) = landed {
                    let dest = match dest {
                        // In the page, the new window opens where the tab
                        // was let go.
                        Dest::Window(None) => Dest::Window(Some(Rect::new(
                            narrow(at.0 - 120.0).max(0.0),
                            narrow(at.1 - 16.0).max(0.0),
                            560.0,
                            380.0,
                        ))),
                        other => other,
                    };
                    self.apply(|w| w.move_pane(&pane, dest));
                }
            }
            Gesture::Tab { .. } => {}
            Gesture::RightSidebar { start, .. } => {
                if (at.0 - start.0).abs() < DRAG_SLOP {
                    return;
                }
                let now = self.right_sidebar_state();
                self.set_right_sidebar(RightSidebarState { open: true, ..now });
            }
            Gesture::Sidebar { start, .. } => {
                // A press that did not move chooses nothing.
                if (at.0 - start.0).abs() < DRAG_SLOP {
                    return;
                }
                let now = self.sidebar_state();
                self.set_sidebar(SidebarState { open: true, ..now });
            }
            Gesture::Drawer { start, from } => {
                let dx = at.0 - start.0;
                let w = drawer_width(self.viewport.peek().0.max(320.0));
                let mut dragged = self.drawer_dragged;
                dragged.set(dx.abs() >= DRAG_SLOP);
                let mut drag = self.drawer_drag;
                drag.set(None);
                self.set_drawer(drawer_settles_open(from, dx, w));
            }
            Gesture::Sheet { id, start } => {
                let dy = at.1 - start.1;
                let h = self.viewport.peek().1.max(320.0);
                let entry = self.sheets.peek().iter().find(|e| e.id.0 == id).cloned();
                if let Some(e) = entry {
                    match sheet_settle(e.detent, &e.spec.detents, dy, h) {
                        Settle::Dismiss => self.close_sheet(e.id),
                        Settle::Stay(d) => {
                            let mut sheets = self.sheets;
                            if let Some(x) = sheets.write().iter_mut().find(|x| x.id.0 == id) {
                                x.detent = d;
                                x.drag = 0.0;
                            }
                        }
                    }
                }
            }
            _ => self.changed(),
        }
    }
}

/// `shares` with the boundary after child `index` moved by `delta` (a
/// share of the whole), each child kept at least 5%.
pub(crate) fn resized(shares: &[f32], index: usize, delta: f32) -> Vec<f32> {
    let mut next = shares.to_vec();
    let (Some(&a), Some(&b)) = (shares.get(index), shares.get(index.saturating_add(1))) else {
        return next;
    };
    let pair = a + b;
    let min = 0.05_f32.min(pair / 2.0);
    let a2 = (a + delta).clamp(min, pair - min);
    if let Some(s) = next.get_mut(index) {
        *s = a2;
    }
    if let Some(s) = next.get_mut(index.saturating_add(1)) {
        *s = pair - a2;
    }
    next
}

/// A screen length as the model's points.
#[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
pub(crate) const fn narrow(v: f64) -> f32 {
    v as f32
}

/// The hover, press and focus affordances and the scrollbars: additive
/// only (every component lays out without them), one static sheet both
/// renderers read.
///
/// The scrollbars are thin, rounded and in the theme's colours
/// (`--as-scrollbar`, `--as-scrollbar-hover`) everywhere inside the shell,
/// the app's own content included: WebKit and Blink by their
/// `::-webkit-scrollbar` parts, Firefox by `scrollbar-width` and
/// `scrollbar-color` (set only where the WebKit parts are unknown: in
/// Blink they would switch the parts off).
pub const SHELL_CSS: &str = r"
.as-hov:hover { background-color: var(--as-raised); }
.as-tab:hover { color: var(--as-text); }
.as-x { opacity: 0.55; }
.as-x:hover { opacity: 1; background-color: var(--as-raised); }
.as-item:hover { background-color: var(--as-accent-soft); }
.as-divider:hover { background-color: var(--as-accent); }
.as-lights .as-glyph { opacity: 0; }
.as-lights:hover .as-glyph { opacity: 1; }
.as-search:focus-within { border-color: var(--as-accent); }
.as-row { cursor: default; outline: none; }
.as-row-hov:hover { background-color: var(--as-hover); }
.as-row:active { background-color: var(--as-pressed); }
.as-row:focus-visible, .as-section-heading:focus-visible { box-shadow: inset 0 0 0 2px var(--as-focus); }
.as-section-heading { cursor: default; outline: none; }
.as-section-heading:hover { color: var(--as-muted); }
.as-section-chevron { opacity: 0.55; }
.as-section-heading:hover .as-section-chevron, .as-section-heading:focus-visible .as-section-chevron { opacity: 1; }
.as-section-action { opacity: 0; }
.as-section-heading:hover .as-section-action, .as-section-heading:focus-within .as-section-action { opacity: 1; }
.as-row-chevron:hover { color: var(--as-text); }
.as-sidebar-section:first-child > .as-rail-rule { display: none; }
.as-sidebar-edge:hover .as-edge-line, .as-sidebar-edge[data-dragging=true] .as-edge-line { background-color: var(--as-accent); }
.app-shell::-webkit-scrollbar, .app-shell ::-webkit-scrollbar { width: 10px; height: 10px; }
.app-shell::-webkit-scrollbar-track, .app-shell ::-webkit-scrollbar-track,
.app-shell::-webkit-scrollbar-corner, .app-shell ::-webkit-scrollbar-corner { background-color: transparent; }
.app-shell::-webkit-scrollbar-thumb, .app-shell ::-webkit-scrollbar-thumb {
  background-color: var(--as-scrollbar); border-radius: 999px;
  border: 3px solid transparent; background-clip: padding-box; min-height: 32px;
}
.app-shell::-webkit-scrollbar-thumb:hover, .app-shell ::-webkit-scrollbar-thumb:hover { background-color: var(--as-scrollbar-hover); }
.app-shell ::-webkit-scrollbar-button { display: none; }
@supports not selector(::-webkit-scrollbar) {
  .app-shell, .app-shell * { scrollbar-width: thin; scrollbar-color: var(--as-scrollbar) transparent; }
}
";

/// The shell's sheet, in a component of its own that never changes.
#[component]
fn ShellStyle() -> Element {
    rsx! { document::Style { {SHELL_CSS} } }
}

/// An app's window: a top bar, an optional sidebar, and one content area,
/// or (when the app turns tabs on) tab stacks, splits, docks and pop-outs.
///
/// The simplest app passes nothing but its content as children:
/// `AppShell { title: "Notes", MyPage {} }`. Tabs are opt-in ([`Tabs`]):
/// give a `layout` (the app's `Signal<Workspace>`) and `kinds`, and set
/// `tabs`. `on_change` hears every change the shell makes to the layout
/// (save it wherever you like). Keys go through `keymap` (the
/// [`DefaultKeymap`] when unset) to commands, which `on_command` hears
/// first.
///
/// The shell fills its parent: give it a sized parent (the window), and
/// put nothing under it that should take the pointer during a drag.
#[allow(clippy::too_many_arguments)]
#[component]
pub fn AppShell(
    /// The layout: the app's signal. Without one the shell keeps its own
    /// (one empty area) and draws `children`.
    layout: Option<Signal<Workspace>>,
    #[props(default)] kinds: TabKinds,
    /// Whether tabs are drawn (off by default).
    #[props(default)]
    tabs: Tabs,
    on_change: Option<EventHandler<Workspace>>,
    on_command: Option<Callback<Command, bool>>,
    /// "New tab" buttons on the tab bars, asking the app to open one in
    /// this stack.
    on_new_tab: Option<EventHandler<GroupId>>,
    /// The window this shell draws: the main one unless set.
    #[props(default = WindowId::main())]
    window: WindowId,
    #[props(default)] theme: Theme,
    #[props(default)] keymap: KeymapHandle,
    /// Desktop or phone; measured from the shell's width when unset.
    form: Option<FormFactor>,
    /// The top bar's title (the default top bar's).
    title: Option<String>,
    /// The top bar, when the app fills its slots ([`TopBar`]). Pass an
    /// empty `rsx! {}` for none.
    top_bar: Option<Element>,
    /// The sidebar's content ([`super::Sidebar`]): docked at the left on
    /// a desktop, a drawer on a phone.
    sidebar: Option<Element>,
    /// The sidebar's state to start with.
    sidebar_state: Option<SidebarState>,
    /// Hears the sidebar shown, hidden or resized (to keep it).
    on_sidebar: Option<EventHandler<SidebarState>>,
    /// The right sidebar's content (an inspector, Now Playing): docked at
    /// the right under the top bar on a desktop, its toggle at the top
    /// bar's right. None: no right sidebar and no toggle.
    right_sidebar: Option<Element>,
    /// The right sidebar's state to start with.
    right_sidebar_state: Option<RightSidebarState>,
    /// Hears the right sidebar shown, hidden or resized (to keep it).
    on_right_sidebar: Option<EventHandler<RightSidebarState>>,
    /// The app's primary destinations: on a phone, the tab pill at the
    /// foot (on a desktop the sidebar holds them).
    #[props(default)]
    destinations: Vec<Destination>,
    /// The destination on show (lit in the pill).
    destination: Option<String>,
    on_destination: Option<EventHandler<String>>,
    /// The round button beside the pill (a phone): the primary action.
    action: Option<PrimaryAction>,
    /// Docked over the pill on a phone, under the content on a desktop:
    /// Now Playing, a transport, an upload's progress.
    accessory: Option<Element>,
    /// The command palette's entries (Mod+K, `Shell::open_palette`).
    palette: Option<Callback<(), Vec<PaletteItem>>>,
    /// The content area, without a `layout` or with tabs off.
    children: Element,
) -> Element {
    let windows = try_use_context::<Windows>();
    let hub = try_use_context::<LayoutHub>();
    let own_layout = use_signal(Workspace::new);
    let has_layout = layout.is_some();
    let layout = layout.unwrap_or(own_layout);
    let window_id = use_signal(|| window.clone());
    let given_size = try_use_context::<ViewportSize>();
    let measured = use_signal(|| (0.0_f64, 0.0_f64));
    let mut form_sig = use_signal(|| form.unwrap_or(FormFactor::Desktop));
    let start = sidebar_state.unwrap_or_default();
    let right_start = right_sidebar_state.unwrap_or_default();
    let hooks = use_hook(|| CopyValue::new(Hooks::default()));
    let mut hooks_w = hooks;
    *hooks_w.write() = Hooks {
        on_change,
        on_command,
        on_new_tab,
        on_sidebar,
        on_right_sidebar,
        palette,
        kinds,
        keymap,
        hub: hub.clone(),
    };
    let shell = use_context_provider(|| Shell {
        layout,
        window: window_id,
        form: form_sig,
        gesture: Signal::new(None),
        hover: Signal::new(None),
        hit: CopyValue::new(false),
        switcher: Signal::new(false),
        popup: Signal::new(None),
        tabs: Signal::new(tabs),
        sidebar_given: Signal::new(sidebar.is_some()),
        sidebar_open: Signal::new(start.open),
        sidebar_w: Signal::new(start.width.clamp(SIDEBAR_MIN, SIDEBAR_MAX)),
        sidebar_rail: Signal::new(start.rail),
        right_given: Signal::new(right_sidebar.is_some()),
        right_open: Signal::new(right_start.open),
        right_w: Signal::new(right_start.width.clamp(SIDEBAR_MIN, SIDEBAR_MAX)),
        bar: Signal::new(BarContributions::default()),
        section_open: Signal::new(std::collections::HashMap::new()),
        tip: Signal::new(None),
        origin: Signal::new((0.0, 0.0)),
        drawer: Signal::new(false),
        drawer_drag: Signal::new(None),
        drawer_dragged: CopyValue::new(false),
        sheets: Signal::new(Vec::new()),
        toasts: Signal::new(Vec::new()),
        next_id: CopyValue::new(1),
        bound: CopyValue::new(Vec::new()),
        palette_views: CopyValue::new(Vec::new()),
        palette_query: CopyValue::new(String::new()),
        viewport: Signal::new((0.0, 0.0)),
        hooks,
    });
    // The window's size: the platform's (native), else measured here.
    let size = given_size.map_or_else(|| *measured.read(), |v| *v.0.read());
    let mut viewport = shell.viewport;
    if *viewport.peek() != size {
        viewport.set(size);
    }
    let want = form
        .or((size.0 > 0.0).then(|| FormFactor::for_width(size.0)))
        .unwrap_or(FormFactor::Desktop);
    if *form_sig.peek() != want {
        form_sig.set(want);
    }
    let (mut tabs_sig, mut given) = (shell.tabs, shell.sidebar_given);
    if *tabs_sig.peek() != tabs {
        tabs_sig.set(tabs);
    }
    if *given.peek() != sidebar.is_some() {
        given.set(sidebar.is_some());
    }
    let mut right_given = shell.right_given;
    if *right_given.peek() != right_sidebar.is_some() {
        right_given.set(right_sidebar.is_some());
    }

    // The other windows' changes, and what the platform did to pop-outs.
    use_hook({
        let hub = hub.clone();
        let window = window.clone();
        move || {
            let Some(hub) = hub else { return };
            if window.is_main() {
                hub.publish(&window, &layout.peek());
            }
            let mut rx = hub.subscribe(&window);
            spawn(async move {
                let mut layout = layout;
                while let Some(message) = rx.next().await {
                    match message {
                        HubMessage::Layout { layout: next, .. } => {
                            if !next.same_arrangement(&layout.peek()) {
                                layout.set(next.clone());
                            }
                            // The main window's app keeps the layout.
                            if shell.window.peek().is_main()
                                && let Some(f) = shell.hooks.peek().on_change
                            {
                                f.call(next);
                            }
                        }
                        HubMessage::Closed(id) => {
                            shell.apply(|w| w.close_window(&id));
                        }
                        HubMessage::Moved(id, bounds) => {
                            shell.apply(|w| w.set_window_bounds(&id, bounds));
                        }
                    }
                }
            });
        }
    });

    // The main window opens and closes the platform's windows for its
    // pop-outs.
    let opened = use_hook(|| CopyValue::new(Vec::<WindowId>::new()));
    let is_main = window.is_main();
    let win_sys = windows.clone();
    use_effect(move || {
        let ws = layout.read();
        let (Some(sys), true) = (&win_sys, is_main) else {
            return;
        };
        let want: Vec<(WindowId, String, Option<Rect>)> = ws
            .windows
            .iter()
            .skip(1)
            .map(|w| (w.id.clone(), window_title(&ws, &w.id), w.bounds))
            .collect();
        let mut opened = opened;
        let had = opened.peek().clone();
        for id in &had {
            if !want.iter().any(|(w, _, _)| w == id) {
                sys.0.close(id);
            }
        }
        for (id, title, bounds) in &want {
            if !had.contains(id) {
                sys.0.open(id, title, *bounds);
            }
        }
        opened.set(want.into_iter().map(|(w, _, _)| w).collect());
    });

    let in_page = is_main && windows.is_none();
    let form_now = *form_sig.read();
    let phone = form_now == FormFactor::Phone;
    let tabs_now = shell.tabs_shown();
    let switcher = *shell.switcher.read() && tabs_now;
    let docked = !phone && sidebar.is_some() && *shell.sidebar_open.read();
    let right_docked = !phone && right_sidebar.is_some() && *shell.right_open.read();
    let rail = docked && shell.sidebar_folded();
    let drawer = phone && sidebar.is_some();
    let touch = if phone { " touch-action: pan-y;" } else { "" };
    let style = format!(
        "{} position: relative; width: 100%; height: 100%; display: flex; flex-direction: column; \
         overflow: hidden; background-color: var(--as-bg); color: var(--as-text); \
         font-family: var(--as-font); font-size: var(--as-font-size); outline: none; \
         user-select: none; box-sizing: border-box;{touch}",
        theme.style()
    );
    let window_for_body = window.clone();
    let top = top_bar.unwrap_or_else(|| rsx! { TopBar { title: title.clone() } });
    rsx! {
        ShellStyle {}
        div {
            class: "app-shell",
            "data-window": "{window.0}",
            "data-form": if phone { "phone" } else { "desktop" },
            "data-tabs": "{tabs_now}",
            style: "{style}",
            tabindex: 0,
            onmounted: move |event| {
                let mut measured = measured;
                spawn(async move {
                    if let Ok(rect) = event.data().get_client_rect().await {
                        measured.set((rect.size.width, rect.size.height));
                        let mut origin = shell.origin;
                        origin.set((rect.origin.x, rect.origin.y));
                    }
                });
            },
            // A browser tells the shell when the window changes size
            // (Blitz does not: the native side gives a `ViewportSize`).
            onresize: move |event| {
                if let Ok(size) = event.data().get_border_box_size() {
                    let mut measured = measured;
                    if *measured.peek() != (size.width, size.height) {
                        measured.set((size.width, size.height));
                    }
                }
            },
            onpointerdown: move |event| {
                let p = event.client_coordinates();
                if shell.form() == FormFactor::Phone
                    && shell.has_sidebar()
                    && !*shell.drawer.peek()
                    && shell.gesture.peek().is_none()
                    && starts_edge_swipe(p.x)
                {
                    let mut g = shell.gesture;
                    g.set(Some(Gesture::Drawer { start: (p.x, p.y), from: 0.0 }));
                }
            },
            onkeydown: move |event| {
                if shell.press_key(&chord_of(&event)) {
                    event.prevent_default();
                    event.stop_propagation();
                }
                if event.key().to_string() == "Escape" && !shell.close_top_sheet() {
                    shell.close_popup();
                    shell.show_switcher(false);
                    shell.set_drawer(false);
                }
            },
            onpointermove: move |event| {
                let p = event.client_coordinates();
                shell.moved((p.x, p.y));
            },
            onpointerup: move |event| {
                let p = event.client_coordinates();
                shell.released((p.x, p.y));
            },
            // The top bar runs across the window (the window's controls, the
            // two sidebar toggles, the places apps fill); under it the left
            // sidebar, the frames and the right sidebar.
            {top}
            div {
                class: "as-middle",
                style: "position: relative; flex: 1 1 0px; min-height: 0; min-width: 0; display: flex; flex-direction: row;",
                if docked {
                    if let Some(content) = sidebar.clone() {
                        DockedSidebar { content, rail }
                    }
                }
                // The frames: the app's content (one view, or with tabs on
                // the stacks, splits and docks of the window's layout).
                div {
                    class: "as-frames",
                    style: "position: relative; flex: 1 1 0px; min-height: 0; min-width: 0; display: flex; flex-direction: column;",
                    div {
                        class: "as-body",
                        style: "position: relative; flex: 1 1 0px; min-height: 0; min-width: 0; display: flex; overflow: hidden;",
                        if !tabs_now {
                            if has_layout {
                                SoloView { window: window_for_body.clone() }
                            } else {
                                div {
                                    class: "as-content",
                                    style: "flex: 1 1 0px; min-width: 0; min-height: 0; display: flex; flex-direction: column; overflow: hidden;",
                                    {children}
                                }
                            }
                        } else if phone {
                            SoloView { window: window_for_body.clone() }
                        } else {
                            WindowBody { window: window_for_body.clone() }
                        }
                        if in_page && tabs_now && !phone {
                            InPageWindows {}
                        }
                    }
                    if !phone {
                        if let Some(acc) = accessory.clone() {
                            DeskAccessory { accessory: acc }
                        }
                    }
                }
                if right_docked {
                    if let Some(content) = right_sidebar.clone() {
                        RightSidebar { content }
                    }
                }
            }
            if phone {
                PhoneFoot {
                    accessory: accessory.clone(),
                    destinations: destinations.clone(),
                    current: destination.clone(),
                    on_destination,
                    action: action.clone(),
                }
            }
            if drawer {
                if let Some(content) = sidebar.clone() {
                    DrawerSidebar { content }
                }
            }
            DragLayer {}
            if switcher {
                Switcher {}
            }
            PopupLayer {}
            SheetLayer {}
            ToastLayer {}
            if rail {
                TipLayer {}
            }
        }
    }
}

/// The title a pop-out's OS window gets: its active tab's.
#[must_use]
pub fn window_title(ws: &Workspace, window: &WindowId) -> String {
    let Some(w) = ws.window(window) else {
        return String::new();
    };
    let tab = w
        .first_group()
        .and_then(|g| g.current_tab())
        .and_then(|p| ws.pane(p))
        .map(|p| p.title.clone());
    tab.unwrap_or_else(|| "Window".to_owned())
}

/// A key event as a [`KeyChord`].
fn chord_of(event: &KeyboardEvent) -> KeyChord {
    let m = event.modifiers();
    let mut chord = KeyChord::new(event.key().to_string());
    chord.ctrl = m.ctrl();
    chord.shift = m.shift();
    chord.alt = m.alt();
    chord.meta = m.meta();
    chord
}

/// A popout window's layout signal, started from the hub's latest: what
/// a pop-out's root passes its [`AppShell`].
#[must_use]
pub fn use_window_layout() -> Signal<Workspace> {
    let hub = try_use_context::<LayoutHub>();
    use_signal(move || hub.and_then(|h| h.latest()).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::resized;

    #[test]
    fn a_divider_moves_one_boundary_and_keeps_each_side_visible() {
        let next = resized(&[0.5, 0.5], 0, 0.2);
        assert!((next[0] - 0.7).abs() < 1e-6 && (next[1] - 0.3).abs() < 1e-6);
        let next = resized(&[0.25, 0.25, 0.5], 1, 0.9);
        assert!((next[1] - 0.70).abs() < 1e-6 && (next[2] - 0.05).abs() < 1e-6);
        assert!((next[0] - 0.25).abs() < 1e-6, "the others stay");
        assert_eq!(
            resized(&[1.0], 0, 0.3),
            vec![1.0],
            "no boundary after the last"
        );
    }
}
