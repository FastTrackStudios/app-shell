//! The shell's state in context ([`Shell`]) and its root ([`AppShell`]).

use std::rc::Rc;

use dioxus::prelude::*;
use futures_util::StreamExt;

use super::kinds::{TabKind, TabKinds};
use super::popup::{MenuItem, Popup, PopupContent, PopupLayer};
use super::stack::DragLayer;
use super::switcher::Switcher;
use super::title::TitleBar;
use super::window::{InPageWindows, PhoneView, WindowBody};
use super::windows::{HubMessage, LayoutHub, WindowControls, Windows};
use crate::commands::{Command, DefaultKeymap, KeyChord, Keymap, Ran};
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
        if width < 600.0 {
            Self::Phone
        } else {
            Self::Desktop
        }
    }
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
    kinds: TabKinds,
    keymap: KeymapHandle,
    hub: Option<LayoutHub>,
}

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

    /// Open a menu at `(x, y)` (window points) holding `items`.
    pub fn menu(&self, x: f64, y: f64, items: Vec<MenuItem>) {
        self.popup(x, y, PopupContent::Menu(items));
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

    fn key(&self, chord: &KeyChord) -> bool {
        let keymap = self.hooks.peek().keymap.clone();
        match keymap.0.command(chord) {
            Some(command) => {
                self.command(command);
                true
            }
            None => false,
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

/// The hover affordances: additive only (every component lays out
/// without them), one static sheet both renderers read.
pub const SHELL_CSS: &str = r"
.as-hov:hover { background-color: var(--as-raised); }
.as-tab:hover { color: var(--as-text); }
.as-x { opacity: 0.55; }
.as-x:hover { opacity: 1; background-color: var(--as-raised); }
.as-item:hover { background-color: var(--as-accent-soft); }
.as-divider:hover { background-color: var(--as-accent); }
";

/// The shell's sheet, in a component of its own that never changes.
#[component]
fn ShellStyle() -> Element {
    rsx! { document::Style { {SHELL_CSS} } }
}

/// One window of a layout, its tabs drawn by the app's kinds.
///
/// The app owns the layout: `layout` is its signal, and `on_change` hears
/// every change the shell makes (save it wherever you like). Keys go
/// through `keymap` (the [`DefaultKeymap`] when unset) to commands, which
/// `on_command` hears first.
///
/// The shell fills its parent: give it a sized parent (the window), and
/// put nothing under it that should take the pointer during a drag.
#[allow(clippy::too_many_arguments)]
#[component]
pub fn AppShell(
    layout: Signal<Workspace>,
    kinds: TabKinds,
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
    /// The title bar's title (with `decorations`).
    title: Option<String>,
    /// Draw the window's title bar (and its buttons, where the platform
    /// provides [`WindowControls`]). On by default where it does.
    decorations: Option<bool>,
) -> Element {
    let controls = try_use_context::<WindowControls>();
    let windows = try_use_context::<Windows>();
    let hub = try_use_context::<LayoutHub>();
    let window_id = use_signal(|| window.clone());
    let measured = use_signal(|| None::<FormFactor>);
    let mut form_sig = use_signal(|| form.unwrap_or(FormFactor::Desktop));
    let hooks = use_hook(|| CopyValue::new(Hooks::default()));
    let mut hooks_w = hooks;
    *hooks_w.write() = Hooks {
        on_change,
        on_command,
        on_new_tab,
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
        hooks,
    });
    let want = form.or(*measured.read()).unwrap_or(FormFactor::Desktop);
    if *form_sig.peek() != want {
        form_sig.set(want);
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

    let decorations = decorations.unwrap_or(controls.is_some());
    let in_page = is_main && windows.is_none();
    let form_now = *form_sig.read();
    let switcher = *shell.switcher.read();
    let style = format!(
        "{} position: relative; width: 100%; height: 100%; display: flex; flex-direction: column; \
         overflow: hidden; background-color: var(--as-bg); color: var(--as-text); \
         font-family: var(--as-font); font-size: var(--as-font-size); outline: none; \
         user-select: none; box-sizing: border-box;",
        theme.style()
    );
    let window_for_body = window.clone();
    rsx! {
        ShellStyle {}
        div {
            class: "app-shell",
            "data-window": "{window.0}",
            "data-form": if form_now == FormFactor::Phone { "phone" } else { "desktop" },
            style: "{style}",
            tabindex: 0,
            onmounted: move |event| {
                if form.is_some() {
                    return;
                }
                let mut measured = measured;
                spawn(async move {
                    if let Ok(rect) = event.data().get_client_rect().await {
                        measured.set(Some(FormFactor::for_width(rect.size.width)));
                    }
                });
            },
            onkeydown: move |event| {
                if shell.key(&chord_of(&event)) {
                    event.prevent_default();
                    event.stop_propagation();
                }
                if event.key().to_string() == "Escape" {
                    shell.close_popup();
                    shell.show_switcher(false);
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
            if decorations {
                TitleBar { title: title.clone().unwrap_or_default() }
            }
            div {
                style: "position: relative; flex: 1 1 0px; min-height: 0; min-width: 0; display: flex; overflow: hidden;",
                if form_now == FormFactor::Phone {
                    PhoneView { window: window_for_body.clone() }
                } else {
                    WindowBody { window: window_for_body.clone() }
                }
                if in_page && form_now == FormFactor::Desktop {
                    InPageWindows {}
                }
            }
            DragLayer {}
            if switcher {
                Switcher {}
            }
            PopupLayer {}
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
