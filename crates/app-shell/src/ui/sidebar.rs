//! The sidebar: a full-height panel at the window's left, the traffic
//! lights at its top, a header, sections of rows and a footer. The shell
//! draws the frame; the app fills it with [`Sidebar`], [`SidebarSection`],
//! [`SidebarItem`] and [`SidebarSwitcher`].
//!
//! On a desktop the frame is docked and has two widths: the full sidebar,
//! resizable by its edge, and a **rail** of icons (the sidebar folded,
//! Architect's ribbon). The top bar's button (Mod+Shift+L) folds it to the
//! rail and back; dragging the edge narrower than [`SIDEBAR_SNAP`] folds
//! it too, and dragging the rail's edge out unfolds it. Unless the person
//! (or the app) chose, a window narrower than [`SIDEBAR_FOLD_BELOW`]
//! starts folded. On a phone the same content is a drawer.
//!
//! Rows nest ([`SidebarItem`]'s `expanded` and children), sections fold and
//! remember it, and every row and heading takes the keyboard: Tab reaches
//! it, Up and Down move between rows, Right opens a row and Left closes it
//! (or goes to its parent), Return or Space presses it.

use std::rc::Rc;

use dioxus::prelude::*;

use super::chrome::BarButton;
use super::icons::{self, Icon, IconView};
use super::shell::{Gesture, use_shell};

/// The narrowest and widest a docked sidebar is dragged to, points.
pub const SIDEBAR_MIN: f32 = 160.0;
pub const SIDEBAR_MAX: f32 = 480.0;
/// A docked sidebar's width to start with.
pub const SIDEBAR_WIDTH: f32 = 220.0;
/// The rail's width: the sidebar folded to its icons.
pub const SIDEBAR_RAIL: f32 = 52.0;
/// The edge dragged narrower than this folds the sidebar to the rail; the
/// rail's edge dragged wider unfolds it.
pub const SIDEBAR_SNAP: f32 = 112.0;
/// A window narrower than this starts with the sidebar folded to the rail
/// (until the person or the app chooses).
pub const SIDEBAR_FOLD_BELOW: f64 = 1000.0;
/// What a sidebar dragged wider leaves the page, points.
pub const SIDEBAR_PAGE_ROOM: f64 = 240.0;
/// How far each level of nesting indents a row.
const INDENT: f32 = 14.0;
/// How long the pointer rests on a rail's icon before its name shows.
const TIP_DELAY_MS: u64 = 400;

/// The sidebar's state, as the app may keep it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SidebarState {
    /// Shown (docked) on a desktop. A hidden sidebar is not drawn at all
    /// (the top bar's button brings it back).
    pub open: bool,
    /// Its full width, points (kept while it is folded to the rail).
    pub width: f32,
    /// Folded to the rail of icons: `Some` once the person or the app
    /// chose; `None` folds it while the window is narrower than
    /// [`SIDEBAR_FOLD_BELOW`].
    pub rail: Option<bool>,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            open: true,
            width: SIDEBAR_WIDTH,
            rail: None,
        }
    }
}

impl SidebarState {
    /// Whether it is folded to the rail in a window `viewport_w` wide (0:
    /// not measured yet, so not folded by the width).
    #[must_use]
    pub fn folded(&self, viewport_w: f64) -> bool {
        self.rail
            .unwrap_or(viewport_w > 0.0 && viewport_w < SIDEBAR_FOLD_BELOW)
    }
}

/// How the sidebar's content is drawn at the moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidebarMode {
    /// Docked at its full width (a desktop).
    #[default]
    Docked,
    /// Folded to the rail: icons only, names on hover (a desktop).
    Rail,
    /// A drawer over the page (a phone).
    Drawer,
}

/// How the sidebar the calling component is in is drawn: so an app's own
/// header or footer can draw itself small on the rail.
#[must_use]
pub fn use_sidebar_mode() -> SidebarMode {
    try_use_context::<Signal<SidebarMode>>().map_or(SidebarMode::Docked, |m| *m.read())
}

/// Provide the mode to the content, kept current as the frame redraws.
fn provide_mode(mode: SidebarMode) {
    let mut sig = use_context_provider(|| Signal::new(mode));
    if *sig.peek() != mode {
        sig.set(mode);
    }
}

/// Where a drag of the edge leaves the sidebar: folded to the rail, or
/// its full width. `start` is its width when the drag began (the rail's if
/// `from_rail`), `dx` how far the edge moved, `viewport_w` the window's
/// width (0: unknown).
#[must_use]
pub(crate) fn sidebar_drag(width: f32, from_rail: bool, dx: f32, viewport_w: f64) -> (bool, f32) {
    let shown = if from_rail { SIDEBAR_RAIL } else { width };
    let raw = shown + dx;
    if raw < SIDEBAR_SNAP {
        return (true, width);
    }
    (false, raw.clamp(SIDEBAR_MIN, widest(viewport_w)))
}

/// The widest the sidebar may be in a window `viewport_w` wide.
#[must_use]
pub(crate) fn widest(viewport_w: f64) -> f32 {
    if viewport_w <= 0.0 {
        return SIDEBAR_MAX;
    }
    #[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
    let room = (viewport_w - SIDEBAR_PAGE_ROOM) as f32;
    room.clamp(SIDEBAR_MIN, SIDEBAR_MAX)
}

const PANEL: &str = "background-color: var(--as-sidebar-bg); backdrop-filter: var(--as-sidebar-blur); \
                     -webkit-backdrop-filter: var(--as-sidebar-blur); display: flex; flex-direction: column; \
                     min-height: 0; overflow: hidden; box-sizing: border-box;";

/// The docked sidebar: the app's content in the shell's panel, the
/// traffic lights in its top row (at full width; on the rail they are the
/// top bar's), an edge to drag.
#[component]
pub(crate) fn DockedSidebar(content: Element, rail: bool) -> Element {
    let shell = use_shell();
    provide_mode(if rail {
        SidebarMode::Rail
    } else {
        SidebarMode::Docked
    });
    let full = *shell.sidebar_w.read();
    let width = if rail { SIDEBAR_RAIL } else { full };
    let dragging = matches!(*shell.gesture.read(), Some(Gesture::Sidebar { .. }));
    let mode = if rail { "rail" } else { "docked" };
    // The edge sits beside the panel, not in it: the panel clips (and its
    // backdrop filter makes it a stacking context), so an edge inside it
    // would lose its outer half under the page.
    rsx! {
        div {
            class: "as-sidebar-frame",
            style: "position: relative; flex: none; width: {width}px; height: 100%; display: flex;",
            div {
                class: "as-sidebar",
                "data-sidebar": mode,
                style: "flex: 1 1 0px; min-width: 0; height: 100%; border-right: 1px solid var(--as-border); {PANEL}",
                // Under the top bar, which holds the window's lights.
                div { style: "flex: none; height: 6px;" }
                {content}
            }
            div {
                class: "as-sidebar-edge",
                role: "separator",
                "aria-orientation": "vertical",
                "aria-label": "Resize the sidebar",
                "data-dragging": "{dragging}",
                style: "position: absolute; right: -4px; top: 0; bottom: 0; width: 8px; cursor: col-resize; z-index: 10; display: flex; justify-content: center;",
                onpointerdown: move |event| {
                    event.stop_propagation();
                    let p = event.client_coordinates();
                    let mut gesture = shell.gesture;
                    gesture.set(Some(Gesture::Sidebar { start: (p.x, p.y), width: full, rail }));
                },
                ondoubleclick: move |_| shell.set_sidebar(SidebarState { open: true, width: SIDEBAR_WIDTH, rail: Some(false) }),
                div { class: "as-edge-line", style: "width: 2px; height: 100%; background-color: transparent;" }
            }
        }
    }
}

/// The right sidebar's width to start with, points.
pub const RIGHT_SIDEBAR_WIDTH: f32 = 280.0;

/// The right sidebar: shown or hidden, and its width. The app's content
/// for it is its own (an inspector, Now Playing, a thread); the shell
/// keeps where it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RightSidebarState {
    pub open: bool,
    pub width: f32,
}

impl Default for RightSidebarState {
    fn default() -> Self {
        Self {
            open: true,
            width: RIGHT_SIDEBAR_WIDTH,
        }
    }
}

/// The right sidebar's width after its edge moved by `dx` from `width`:
/// moving left widens it. The page keeps [`SIDEBAR_PAGE_ROOM`].
pub(crate) fn right_drag(width: f32, dx: f32, viewport_w: f64) -> f32 {
    (width - dx).clamp(SIDEBAR_MIN, widest(viewport_w))
}

/// The right sidebar, docked at the window's right under the top bar,
/// resized from its left edge (a double press there resets it).
#[component]
pub(crate) fn RightSidebar(content: Element) -> Element {
    let shell = use_shell();
    provide_mode(SidebarMode::Docked);
    let width = *shell.right_w.read();
    let dragging = matches!(*shell.gesture.read(), Some(Gesture::RightSidebar { .. }));
    rsx! {
        div {
            class: "as-sidebar-frame as-right-frame",
            style: "position: relative; flex: none; width: {width}px; height: 100%; display: flex;",
            div {
                class: "as-sidebar-edge",
                role: "separator",
                "aria-orientation": "vertical",
                "aria-label": "Resize the right sidebar",
                "data-dragging": "{dragging}",
                style: "position: absolute; left: -4px; top: 0; bottom: 0; width: 8px; cursor: col-resize; z-index: 10; display: flex; justify-content: center;",
                onpointerdown: move |event| {
                    event.stop_propagation();
                    let p = event.client_coordinates();
                    let mut gesture = shell.gesture;
                    gesture.set(Some(Gesture::RightSidebar { start: (p.x, p.y), width }));
                },
                ondoubleclick: move |_| shell.set_right_sidebar(RightSidebarState { open: true, width: RIGHT_SIDEBAR_WIDTH }),
                div { class: "as-edge-line", style: "width: 2px; height: 100%; background-color: transparent;" }
            }
            div {
                class: "as-sidebar as-right-sidebar",
                "data-sidebar": "right",
                style: "flex: 1 1 0px; min-width: 0; height: 100%; border-left: 1px solid var(--as-border); {PANEL}",
                div { style: "flex: none; height: 6px;" }
                {content}
            }
        }
    }
}

/// The sidebar as a drawer over the page (a phone): it slides in from
/// the left edge under the finger (or the menu button) and goes the same
/// way; a tap on the scrim closes it. Always mounted on a phone, so it
/// can follow a swipe from its first point.
#[component]
pub(crate) fn DrawerSidebar(content: Element) -> Element {
    let shell = use_shell();
    provide_mode(SidebarMode::Drawer);
    let open = *shell.drawer.read();
    let drag = *shell.drawer_drag.read();
    let p = drag.unwrap_or(if open { 1.0 } else { 0.0 });
    let vw = shell.viewport().0;
    let w = crate::gestures::drawer_width(if vw > 0.0 { vw } else { 390.0 });
    let x = (p - 1.0) * w;
    let shown = p > 0.0;
    let events = if shown { "auto" } else { "none" };
    let transition = if drag.is_some() {
        "none".to_owned()
    } else {
        "transform 0.28s cubic-bezier(0.2, 0.8, 0.2, 1), opacity 0.28s ease".to_owned()
    };
    let pad = super::foot::top_pad(8.0);
    let grab = move |event: PointerEvent| {
        // A new press: whether it drags is this press's to say.
        let mut dragged = shell.drawer_dragged;
        dragged.set(false);
        if !*shell.drawer.peek() {
            return;
        }
        let p = event.client_coordinates();
        let mut g = shell.gesture;
        g.set(Some(Gesture::Drawer {
            start: (p.x, p.y),
            from: 1.0,
        }));
    };
    rsx! {
        div {
            class: "as-drawer",
            "data-open": "{open}",
            style: "position: absolute; left: 0; top: 0; right: 0; bottom: 0; z-index: 45; pointer-events: none;",
            div {
                class: "as-scrim",
                style: "position: absolute; left: 0; top: 0; right: 0; bottom: 0; background-color: var(--as-overlay); opacity: {p}; pointer-events: {events}; transition: {transition};",
                onpointerdown: grab,
                onclick: move |_| {
                    let mut dragged = shell.drawer_dragged;
                    if dragged.replace(false) {
                        return;
                    }
                    shell.set_drawer(false);
                },
            }
            div {
                class: "as-sidebar",
                "data-sidebar": "drawer",
                // An opaque base under the panel's translucent colour: over
                // the page a drawer must not show it through (a browser
                // blurs it; Blitz cannot).
                style: "position: absolute; left: 0; top: 0; bottom: 0; width: {w}px; transform: translateX({x}px); transition: {transition}; pointer-events: {events}; box-shadow: 8px 0 32px rgba(0, 0, 0, 0.4); {PANEL} background-image: linear-gradient(var(--as-sidebar-bg), var(--as-sidebar-bg)); background-color: var(--as-bg); {pad}",
                onpointerdown: grab,
                div {
                    style: "flex: none; height: 36px; display: flex; align-items: center; justify-content: flex-end; padding: 0 6px;",
                    BarButton { icon: icons::X, label: "Close menu", on_press: move |()| shell.set_drawer(false) }
                }
                {content}
            }
        }
    }
}

/// The app's sidebar: a `header` (a library or organisation switcher:
/// [`SidebarSwitcher`]), its sections, scrolling, and a `footer`.
#[component]
pub fn Sidebar(header: Option<Element>, footer: Option<Element>, children: Element) -> Element {
    let rail = use_sidebar_mode() == SidebarMode::Rail;
    use_context_provider(Rows::default);
    let (head_pad, body_pad, foot_pad, gap) = if rail {
        ("0 6px 6px 6px", "2px 6px 8px 6px", "6px", "4px")
    } else {
        ("0 8px 6px 8px", "2px 8px 8px 8px", "8px", "10px")
    };
    rsx! {
        if let Some(header) = header {
            div { class: "as-sidebar-header", style: "flex: none; padding: {head_pad};", {header} }
        }
        div {
            class: "as-sidebar-body as-scroll",
            role: "tree",
            style: "flex: 1 1 0px; min-height: 0; overflow-y: scroll; overflow-x: hidden; padding: {body_pad}; display: flex; flex-direction: column; gap: {gap};",
            {children}
        }
        if let Some(footer) = footer {
            div { class: "as-sidebar-footer", style: "flex: none; padding: {foot_pad}; border-top: 1px solid var(--as-border); display: flex; flex-direction: column; gap: 1px;", {footer} }
        }
    }
}

/// A section of the sidebar: a small uppercase heading that folds it
/// (when `collapsible`), with an optional `count` after its title and an
/// `action` (a "+" button, say) that shows while the pointer is over it.
///
/// Whether it is open is kept by the shell under its `title` (or `id`),
/// so it stays as it was when the sidebar folds to the rail, turns into
/// the phone's drawer or comes back; `open` is how it starts and
/// `on_toggle` hears each change (to keep it). On the rail a section is a
/// thin rule over its rows' icons, open whatever it was.
#[component]
pub fn SidebarSection(
    title: String,
    /// What the shell keeps its folding under (its `title` when unset).
    id: Option<String>,
    #[props(default = true)] open: bool,
    #[props(default = true)] collapsible: bool,
    /// After the title: a count, dimmed.
    count: Option<String>,
    /// At the heading's right, shown on hover or keyboard focus.
    action: Option<Element>,
    on_toggle: Option<EventHandler<bool>>,
    children: Element,
) -> Element {
    let shell = use_shell();
    let mode = use_sidebar_mode();
    let key = id.unwrap_or_else(|| title.clone());
    let kept = shell.section_open.read().get(&key).copied();
    let is_open = kept.unwrap_or(open) || !collapsible;
    let key_t = key.clone();
    let toggle = use_callback(move |()| {
        if !collapsible {
            return;
        }
        let now = !shell
            .section_open
            .peek()
            .get(&key_t)
            .copied()
            .unwrap_or(open);
        let mut kept = shell.section_open;
        kept.write().insert(key_t.clone(), now);
        if let Some(f) = on_toggle {
            f.call(now);
        }
    });
    let row = use_row(0);
    if mode == SidebarMode::Rail {
        return rsx! {
            div {
                class: "as-sidebar-section",
                "data-open": "true",
                style: "display: flex; flex-direction: column; align-items: center; gap: 2px;",
                div { class: "as-rail-rule", style: "flex: none; width: 24px; height: 1px; margin: 4px 0; background-color: var(--as-border);" }
                {children}
            }
        };
    }
    let chevron = if is_open {
        icons::CHEVRON_DOWN
    } else {
        icons::CHEVRON_RIGHT
    };
    rsx! {
        div {
            class: "as-sidebar-section",
            "data-open": "{is_open}",
            style: "display: flex; flex-direction: column; gap: 1px;",
            div {
                class: "as-section-heading",
                role: if collapsible { "button" } else { "heading" },
                tabindex: if collapsible { "0" } else { "-1" },
                "aria-expanded": if collapsible { if is_open { "true" } else { "false" } } else { "" },
                style: "display: flex; align-items: center; gap: 4px; height: 24px; padding: 0 4px 0 6px; border-radius: var(--as-radius); font-size: 11px; font-weight: 600; letter-spacing: 0.05em; text-transform: uppercase; color: var(--as-heading); white-space: nowrap; overflow: hidden; flex: none;",
                onmounted: move |e| row.mounted(e),
                onclick: move |_| toggle.call(()),
                onkeydown: move |event: KeyboardEvent| {
                    let key = event.key().to_string();
                    let handled = match key.as_str() {
                        "Enter" | " " => {
                            toggle.call(());
                            true
                        }
                        "ArrowRight" if !is_open => {
                            toggle.call(());
                            true
                        }
                        "ArrowLeft" if is_open && collapsible => {
                            toggle.call(());
                            true
                        }
                        other => row.navigate(other),
                    };
                    if handled {
                        event.prevent_default();
                        event.stop_propagation();
                    }
                },
                span { style: "flex: 1 1 0px; min-width: 0; overflow: hidden;",
                    "{title}"
                    if let Some(count) = count {
                        span { class: "as-section-count", style: "margin-left: 6px; font-weight: 500; letter-spacing: 0; opacity: 0.8;", "{count}" }
                    }
                }
                if let Some(action) = action {
                    div {
                        class: "as-section-action",
                        style: "flex: none; display: flex; align-items: center; text-transform: none; letter-spacing: 0;",
                        onclick: move |event| event.stop_propagation(),
                        onkeydown: move |event| event.stop_propagation(),
                        {action}
                    }
                }
                if collapsible {
                    div { class: "as-section-chevron", style: "flex: none; display: flex;",
                        IconView { icon: chevron, size: 12.0 }
                    }
                }
            }
            if is_open {
                {children}
            }
        }
    }
}

/// How deep the rows being drawn are nested.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
struct Depth(u16);

/// A row of the sidebar: an icon, a label, an optional count or badge;
/// lit when `selected`. A press closes the phone's drawer, then calls
/// `on_press`.
///
/// Rows nest: give `expanded` (whether it starts open) and children, and
/// the row gets a chevron that opens and closes it without pressing it,
/// its children drawn under it one level in, with a guide down their
/// left. `on_expand` hears each change.
///
/// On the rail only its icon shows (the label's first letter without
/// one), its name on hover, and a dot for a badge; nested rows hide.
#[component]
pub fn SidebarItem(
    label: String,
    icon: Option<Icon>,
    /// A count or a short badge at the right.
    badge: Option<String>,
    #[props(default)] selected: bool,
    /// Some: the row opens to its children (and starts open if `true`).
    expanded: Option<bool>,
    on_expand: Option<EventHandler<bool>>,
    on_press: Option<EventHandler<()>>,
    children: Element,
) -> Element {
    let shell = use_shell();
    let mode = use_sidebar_mode();
    let depth = try_use_context::<Depth>().unwrap_or_default().0;
    let mut open = use_signal(|| expanded.unwrap_or(false));
    let branch = expanded.is_some();
    let is_open = branch && *open.read();
    let row = use_row(depth);
    let mut toggle = move |to: bool| {
        if *open.peek() != to {
            open.set(to);
            if let Some(f) = on_expand {
                f.call(to);
            }
        }
    };
    let press = move || {
        shell.set_drawer(false);
        if let Some(f) = on_press {
            f.call(());
        }
    };
    let (bg, fg, weight, ink) = if selected {
        (
            "var(--as-selected)",
            "var(--as-selected-text)",
            "500",
            "var(--as-accent)",
        )
    } else {
        ("transparent", "var(--as-text)", "400", "var(--as-muted)")
    };
    let current = if selected { "page" } else { "false" };
    let keydown = move |event: KeyboardEvent| {
        let key = event.key().to_string();
        let handled = match key.as_str() {
            "Enter" | " " => {
                press();
                true
            }
            "ArrowRight" if branch && !*open.peek() => {
                toggle(true);
                true
            }
            "ArrowLeft" if branch && *open.peek() => {
                toggle(false);
                true
            }
            "ArrowLeft" if depth > 0 => {
                row.parent();
                true
            }
            other => row.navigate(other),
        };
        if handled {
            event.prevent_default();
            event.stop_propagation();
        }
    };

    if mode == SidebarMode::Rail {
        if depth > 0 {
            return rsx! {};
        }
        let initial = label
            .chars()
            .next()
            .unwrap_or(' ')
            .to_uppercase()
            .to_string();
        let tip_label = label.clone();
        let tip_label_f = label.clone();
        let hov = if selected { "" } else { " as-row-hov" };
        return rsx! {
            div {
                class: "as-sidebar-item as-row{hov}",
                role: "treeitem",
                tabindex: "0",
                "aria-label": "{label}",
                "aria-current": current,
                "data-rail": "true",
                style: "position: relative; flex: none; width: 36px; height: 36px; display: flex; align-items: center; justify-content: center; border-radius: var(--as-radius); background-color: {bg}; color: {ink};",
                onmounted: move |e| row.mounted(e),
                onclick: move |_| press(),
                onkeydown: keydown,
                onpointerenter: move |_| row.tip(tip_label.clone(), true),
                onpointerleave: move |_| row.untip(),
                onfocus: move |_| row.tip(tip_label_f.clone(), false),
                onblur: move |_| row.untip(),
                match icon {
                    Some(icon) => rsx! { IconView { icon, size: 18.0 } },
                    None => rsx! { span { style: "font-weight: 600; font-size: 13px;", "{initial}" } },
                }
                if badge.as_deref().is_some_and(|b| !b.is_empty()) {
                    span { class: "as-rail-dot", style: "position: absolute; top: 6px; right: 6px; width: 6px; height: 6px; border-radius: 3px; background-color: var(--as-accent);" }
                }
            }
        };
    }

    let hov = if selected { "" } else { " as-row-hov" };
    #[allow(clippy::cast_lossless, clippy::as_conversions)]
    let pad = 8.0 + f32::from(depth) * INDENT;
    let chevron = if is_open {
        icons::CHEVRON_DOWN
    } else {
        icons::CHEVRON_RIGHT
    };
    let level = depth.saturating_add(1);
    let expanded_attr = if branch {
        if is_open { "true" } else { "false" }
    } else {
        ""
    };
    rsx! {
        div {
            class: "as-sidebar-item as-row{hov}",
            role: "treeitem",
            tabindex: "0",
            "aria-current": current,
            "aria-selected": if selected { "true" } else { "false" },
            "aria-level": "{level}",
            "aria-expanded": expanded_attr,
            style: "position: relative; display: flex; align-items: center; gap: 8px; height: 28px; padding: 0 8px 0 {pad}px; border-radius: var(--as-radius); background-color: {bg}; color: {fg}; font-weight: {weight}; white-space: nowrap; overflow: hidden; flex: none;",
            onmounted: move |e| row.mounted(e),
            onclick: move |_| press(),
            onkeydown: keydown,
            if branch {
                div {
                    class: "as-row-chevron",
                    "aria-label": if is_open { "Collapse" } else { "Expand" },
                    style: "flex: none; width: 14px; height: 14px; margin-right: -4px; display: flex; align-items: center; justify-content: center; color: var(--as-muted);",
                    onclick: move |event| {
                        event.stop_propagation();
                        let now = !*open.peek();
                        toggle(now);
                    },
                    IconView { icon: chevron, size: 12.0 }
                }
            } else if depth > 0 {
                div { style: "flex: none; width: 14px; margin-right: -4px;" }
            }
            if let Some(icon) = icon {
                div { style: "flex: none; display: flex; color: {ink};", IconView { icon, size: 15.0 } }
            }
            span { style: "flex: 1 1 0px; min-width: 0; overflow: hidden;", "{label}" }
            if let Some(badge) = badge {
                span {
                    class: "as-badge",
                    style: "flex: none; font-size: 11px; color: var(--as-muted); padding: 0 2px;",
                    "{badge}"
                }
            }
        }
        if is_open {
            Nest { depth: depth.saturating_add(1), {children} }
        }
    }
}

/// A row's children, one level in, a guide down their left.
#[component]
fn Nest(depth: u16, children: Element) -> Element {
    use_context_provider(|| Depth(depth));
    #[allow(clippy::cast_lossless, clippy::as_conversions)]
    let guide = 8.0 + f32::from(depth.saturating_sub(1)) * INDENT + 6.5;
    rsx! {
        div {
            class: "as-sidebar-nest",
            role: "group",
            style: "position: relative; display: flex; flex-direction: column; gap: 1px;",
            div { class: "as-indent-guide", style: "position: absolute; top: 0; bottom: 0; left: {guide}px; width: 1px; background-color: var(--as-border); pointer-events: none;" }
            {children}
        }
    }
}

/// A sidebar header's switcher (a library, an organisation): a mark, a
/// name over a detail, and ⇅; `on_press` opens the app's chooser. On the
/// rail, the mark alone.
#[component]
pub fn SidebarSwitcher(
    title: String,
    detail: Option<String>,
    icon: Option<Icon>,
    on_press: Option<EventHandler<()>>,
) -> Element {
    let rail = use_sidebar_mode() == SidebarMode::Rail;
    let row = use_row(0);
    let initial = title
        .chars()
        .next()
        .unwrap_or(' ')
        .to_uppercase()
        .to_string();
    let press = move || {
        if let Some(f) = on_press {
            f.call(());
        }
    };
    let mark = rsx! {
        div {
            style: "flex: none; width: 24px; height: 24px; border-radius: 6px; background-color: var(--as-accent); color: #ffffff; display: flex; align-items: center; justify-content: center; font-weight: 700; font-size: 12px;",
            match icon {
                Some(icon) => rsx! { IconView { icon, size: 14.0, color: "#ffffff".to_owned() } },
                None => rsx! { "{initial}" },
            }
        }
    };
    let keydown = move |event: KeyboardEvent| {
        let key = event.key().to_string();
        let handled = match key.as_str() {
            "Enter" | " " => {
                press();
                true
            }
            other => row.navigate(other),
        };
        if handled {
            event.prevent_default();
            event.stop_propagation();
        }
    };
    if rail {
        let (a, b) = (title.clone(), title.clone());
        return rsx! {
            div {
                class: "as-switcher-row as-row as-row-hov",
                role: "button",
                tabindex: "0",
                "aria-label": "{title}",
                style: "display: flex; align-items: center; justify-content: center; width: 40px; height: 40px; border-radius: 8px; box-sizing: border-box;",
                onmounted: move |e| row.mounted(e),
                onclick: move |_| press(),
                onkeydown: keydown,
                onpointerenter: move |_| row.tip(a.clone(), true),
                onpointerleave: move |_| row.untip(),
                onfocus: move |_| row.tip(b.clone(), false),
                onblur: move |_| row.untip(),
                {mark}
            }
        };
    }
    rsx! {
        div {
            class: "as-switcher-row as-row as-hov",
            role: "button",
            tabindex: "0",
            style: "display: flex; align-items: center; gap: 8px; height: 40px; padding: 0 8px; border-radius: 8px; border: 1px solid var(--as-border); background-color: var(--as-search-bg); box-sizing: border-box; overflow: hidden;",
            onmounted: move |e| row.mounted(e),
            onclick: move |_| press(),
            onkeydown: keydown,
            {mark}
            div {
                style: "flex: 1 1 0px; min-width: 0; display: flex; flex-direction: column; overflow: hidden;",
                span { style: "font-weight: 600; white-space: nowrap; overflow: hidden;", "{title}" }
                if let Some(detail) = detail {
                    span { style: "font-size: 11px; color: var(--as-muted); white-space: nowrap; overflow: hidden;", "{detail}" }
                }
            }
            div { style: "flex: none; display: flex; color: var(--as-muted);", IconView { icon: icons::CHEVRONS_UP_DOWN, size: 14.0 } }
        }
    }
}

/// A scrolling column, its scrollbar the shell's (thin, rounded, in the
/// theme's colours): for an app's own lists inside or beside the shell.
#[component]
pub fn ScrollArea(
    #[props(default)] class: String,
    #[props(default)] style: String,
    /// Scroll sideways too.
    #[props(default)]
    horizontal: bool,
    children: Element,
) -> Element {
    let x = if horizontal { "scroll" } else { "hidden" };
    rsx! {
        div {
            class: "as-scroll {class}",
            style: "flex: 1 1 0px; min-height: 0; min-width: 0; overflow-y: scroll; overflow-x: {x}; {style}",
            {children}
        }
    }
}

// --- The keyboard: every row and heading in one sidebar, in order. ---

/// The focusable rows of one [`Sidebar`], each with its depth.
#[derive(Clone, Copy, Default)]
struct Rows {
    list: CopyValue<Vec<(u64, u16, Rc<MountedData>)>>,
    next: CopyValue<u64>,
}

/// A row's hold on the sidebar's keyboard order and on the rail's tip.
#[derive(Clone, Copy)]
struct Row {
    id: u64,
    depth: u16,
    rows: Option<Rows>,
    node: Signal<Option<Rc<MountedData>>>,
    shell: super::shell::Shell,
    hovered: Signal<bool>,
}

fn use_row(depth: u16) -> Row {
    let rows = try_use_context::<Rows>();
    let shell = use_shell();
    let hovered = use_signal(|| false);
    let node = use_signal(|| None::<Rc<MountedData>>);
    let id = use_hook(|| {
        rows.map_or(0, |r| {
            let mut next = r.next;
            let id = *next.peek();
            next.set(id.saturating_add(1));
            id
        })
    });
    use_drop(move || {
        if let Some(r) = rows {
            let mut list = r.list;
            list.write().retain(|(i, _, _)| *i != id);
        }
        let mut tip = shell.tip;
        if tip.peek().as_ref().is_some_and(|t| t.id == id) {
            tip.set(None);
        }
    });
    Row {
        id,
        depth,
        rows,
        node,
        shell,
        hovered,
    }
}

impl Row {
    /// The row is in the document: it joins the keyboard's order.
    fn mounted(self, event: MountedEvent) {
        let data = event.data();
        let mut node = self.node;
        node.set(Some(data.clone()));
        if let Some(r) = self.rows {
            let mut list = r.list;
            let mut l = list.write();
            l.retain(|(i, _, _)| *i != self.id);
            l.push((self.id, self.depth, data));
        }
    }

    /// Up, Down, Home or End: move the focus; `false` for other keys.
    fn navigate(self, key: &str) -> bool {
        let step = match key {
            "ArrowDown" => Step::Next,
            "ArrowUp" => Step::Prev,
            "Home" => Step::First,
            "End" => Step::Last,
            _ => return false,
        };
        self.focus(step);
        true
    }

    /// Focus the row this one is nested in.
    fn parent(self) {
        self.focus(Step::Parent);
    }

    fn focus(self, step: Step) {
        let Some(rows) = self.rows else { return };
        let (me, depth) = (self.id, self.depth);
        let list = rows.list.peek().clone();
        spawn(async move {
            // In the order they are drawn: by their place down the sidebar.
            let mut placed = Vec::with_capacity(list.len());
            for (id, d, node) in list {
                if let Ok(rect) = node.get_client_rect().await {
                    placed.push((rect.origin.y, id, d, node));
                }
            }
            placed.sort_by(|a, b| a.0.total_cmp(&b.0));
            let Some(at) = placed.iter().position(|p| p.1 == me) else {
                return;
            };
            let to = match step {
                Step::Next => at.checked_add(1).filter(|&i| i < placed.len()),
                Step::Prev => at.checked_sub(1),
                Step::First => Some(0),
                Step::Last => placed.len().checked_sub(1),
                Step::Parent => placed
                    .iter()
                    .take(at)
                    .rposition(|p| p.2 < depth && p.2.saturating_add(1) == depth),
            };
            if let Some(target) = to.and_then(|i| placed.get(i)) {
                let _ = target.3.set_focus(true).await;
            }
        });
    }

    /// Show `label` beside this rail icon: after a rest under the pointer
    /// (`delayed`), or at once (keyboard focus).
    fn tip(self, label: String, delayed: bool) {
        let mut hovered = self.hovered;
        hovered.set(true);
        let (id, shell) = (self.id, self.shell);
        let Some(node) = self.node.peek().clone() else {
            return;
        };
        spawn(async move {
            if delayed {
                futures_timer::Delay::new(std::time::Duration::from_millis(TIP_DELAY_MS)).await;
                if !*hovered.peek() {
                    return;
                }
            }
            let Ok(rect) = node.get_client_rect().await else {
                return;
            };
            let origin = *shell.origin.peek();
            let mut tip = shell.tip;
            tip.set(Some(Tip {
                id,
                label,
                x: rect.origin.x + rect.size.width + 8.0 - origin.0,
                y: rect.origin.y + rect.size.height / 2.0 - origin.1,
            }));
        });
    }

    fn untip(self) {
        let mut hovered = self.hovered;
        hovered.set(false);
        let mut tip = self.shell.tip;
        if tip.peek().as_ref().is_some_and(|t| t.id == self.id) {
            tip.set(None);
        }
    }
}

#[derive(Clone, Copy)]
enum Step {
    Next,
    Prev,
    First,
    Last,
    Parent,
}

/// A rail icon's name, drawn beside it at the shell's root (the rail
/// clips what goes past its edge).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Tip {
    id: u64,
    label: String,
    x: f64,
    y: f64,
}

/// The rail's tip, over everything.
#[component]
pub(crate) fn TipLayer() -> Element {
    let shell = use_shell();
    let Some(tip) = shell.tip.read().clone() else {
        return rsx! {};
    };
    let (x, y) = (tip.x, tip.y - 13.0);
    rsx! {
        div {
            class: "as-tip",
            role: "tooltip",
            style: "position: absolute; left: {x}px; top: {y}px; z-index: 80; height: 26px; display: flex; align-items: center; padding: 0 9px; border-radius: 6px; background-color: var(--as-raised); border: 1px solid var(--as-border); color: var(--as-text); font-size: 12px; font-weight: 500; white-space: nowrap; pointer-events: none; box-shadow: 0 4px 16px rgba(0, 0, 0, 0.35); box-sizing: border-box;",
            "{tip.label}"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_narrow_window_folds_the_sidebar_until_someone_chooses() {
        let auto = SidebarState::default();
        assert!(!auto.folded(0.0), "not measured: not folded");
        assert!(auto.folded(800.0));
        assert!(!auto.folded(1280.0));
        let chosen = SidebarState {
            rail: Some(false),
            ..auto
        };
        assert!(!chosen.folded(800.0));
        let rail = SidebarState {
            rail: Some(true),
            ..auto
        };
        assert!(rail.folded(1600.0));
    }

    #[test]
    fn the_edge_dragged_narrow_folds_to_the_rail_and_out_again() {
        assert_eq!(sidebar_drag(220.0, false, 40.0, 1400.0), (false, 260.0));
        assert_eq!(
            sidebar_drag(220.0, false, -60.0, 1400.0),
            (false, SIDEBAR_MIN),
            "narrow, but not past the snap"
        );
        assert_eq!(
            sidebar_drag(220.0, false, -120.0, 1400.0),
            (true, 220.0),
            "past the snap it folds and keeps its width"
        );
        assert_eq!(
            sidebar_drag(220.0, true, 20.0, 1400.0),
            (true, 220.0),
            "the rail a little wider stays the rail"
        );
        assert_eq!(
            sidebar_drag(220.0, true, 80.0, 1400.0),
            (false, SIDEBAR_MIN)
        );
        assert_eq!(
            sidebar_drag(220.0, false, 2000.0, 700.0),
            (false, 460.0),
            "the page keeps its room"
        );
        assert_eq!(
            sidebar_drag(220.0, false, 2000.0, 0.0),
            (false, SIDEBAR_MAX)
        );
    }
}
