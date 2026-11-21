//! The sidebar: a full-height panel at the window's left, the traffic
//! lights at its top, a header, sections of rows and a footer. The shell
//! draws the frame (docked, resizable and collapsible on a desktop; a
//! drawer on a phone); the app fills it with [`Sidebar`],
//! [`SidebarSection`], [`SidebarItem`] and [`SidebarSwitcher`].

use dioxus::prelude::*;

use super::chrome::BarButton;
use super::chrome::{DragArea, WindowLights};
use super::icons::{self, Icon, IconView};
use super::shell::{Gesture, use_shell};

/// The narrowest and widest a docked sidebar is dragged to, points.
pub const SIDEBAR_MIN: f32 = 160.0;
pub const SIDEBAR_MAX: f32 = 420.0;
/// A docked sidebar's width to start with.
pub const SIDEBAR_WIDTH: f32 = 220.0;

/// The sidebar's state, as the app may keep it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SidebarState {
    /// Shown (docked) on a desktop.
    pub open: bool,
    /// Its width, points.
    pub width: f32,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            open: true,
            width: SIDEBAR_WIDTH,
        }
    }
}

const PANEL: &str = "background-color: var(--as-sidebar-bg); backdrop-filter: var(--as-sidebar-blur); \
                     -webkit-backdrop-filter: var(--as-sidebar-blur); display: flex; flex-direction: column; \
                     min-height: 0; overflow: hidden; box-sizing: border-box;";

/// The docked sidebar: the app's content in the shell's panel, the
/// traffic lights in its top row, an edge to drag.
#[component]
pub(crate) fn DockedSidebar(content: Element) -> Element {
    let shell = use_shell();
    let width = *shell.sidebar_w.read();
    // The edge sits beside the panel, not in it: the panel clips (and its
    // backdrop filter makes it a stacking context), so an edge inside it
    // would lose its outer half under the page.
    rsx! {
        div {
            style: "position: relative; flex: none; width: {width}px; height: 100%; display: flex;",
            div {
                class: "as-sidebar",
                "data-sidebar": "docked",
                style: "flex: 1 1 0px; min-width: 0; height: 100%; border-right: 1px solid var(--as-border); {PANEL}",
                DragArea {
                    style: "flex: none; height: var(--as-topbar-h); min-width: 0;".to_owned(),
                    WindowLights {}
                }
                {content}
            }
            div {
                class: "as-divider as-sidebar-edge",
                style: "position: absolute; right: -3px; top: 0; bottom: 0; width: 6px; cursor: col-resize; z-index: 10;",
                onpointerdown: move |event| {
                    event.stop_propagation();
                    let p = event.client_coordinates();
                    let mut gesture = shell.gesture;
                    gesture.set(Some(Gesture::Sidebar { start: (p.x, p.y), width }));
                },
                ondoubleclick: move |_| shell.set_sidebar(SidebarState { open: true, width: SIDEBAR_WIDTH }),
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
                style: "position: absolute; left: 0; top: 0; bottom: 0; width: {w}px; transform: translateX({x}px); transition: {transition}; pointer-events: {events}; box-shadow: 8px 0 32px rgba(0, 0, 0, 0.4); {PANEL} {pad}",
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
    rsx! {
        if let Some(header) = header {
            div { class: "as-sidebar-header", style: "flex: none; padding: 0 8px 6px 8px;", {header} }
        }
        div {
            class: "as-sidebar-body",
            style: "flex: 1 1 0px; min-height: 0; overflow-y: scroll; scrollbar-width: thin; padding: 2px 8px 8px 8px; display: flex; flex-direction: column; gap: 10px;",
            {children}
        }
        if let Some(footer) = footer {
            div { class: "as-sidebar-footer", style: "flex: none; padding: 8px; border-top: 1px solid var(--as-border);", {footer} }
        }
    }
}

/// A section of the sidebar: a small uppercase heading that folds it.
#[component]
pub fn SidebarSection(
    title: String,
    #[props(default = true)] open: bool,
    #[props(default = true)] collapsible: bool,
    children: Element,
) -> Element {
    let mut shown = use_signal(|| open);
    let is_open = *shown.read() || !collapsible;
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
                role: "button",
                "aria-expanded": "{is_open}",
                style: "display: flex; align-items: center; gap: 4px; height: 22px; padding: 0 6px; font-size: 11px; font-weight: 600; letter-spacing: 0.05em; text-transform: uppercase; color: var(--as-heading); white-space: nowrap; overflow: hidden;",
                onclick: move |_| {
                    if collapsible {
                        shown.toggle();
                    }
                },
                span { style: "flex: 1 1 0px; min-width: 0; overflow: hidden;", "{title}" }
                if collapsible {
                    IconView { icon: chevron, size: 12.0 }
                }
            }
            if is_open {
                {children}
            }
        }
    }
}

/// A row of the sidebar: an icon, a label, an optional count or badge;
/// lit when `selected`. A press closes the phone's drawer, then calls
/// `on_press`.
#[component]
pub fn SidebarItem(
    label: String,
    icon: Option<Icon>,
    /// A count or a short badge at the right.
    badge: Option<String>,
    #[props(default)] selected: bool,
    on_press: Option<EventHandler<()>>,
) -> Element {
    let shell = use_shell();
    let (bg, fg, weight) = if selected {
        ("var(--as-selected)", "var(--as-selected-text)", "500")
    } else {
        ("transparent", "var(--as-text)", "400")
    };
    let hov = if selected { "" } else { "as-hov" };
    rsx! {
        div {
            class: "as-sidebar-item {hov}",
            role: "button",
            "aria-current": if selected { "page" } else { "false" },
            style: "display: flex; align-items: center; gap: 8px; height: 28px; padding: 0 8px; border-radius: var(--as-radius); background-color: {bg}; color: {fg}; font-weight: {weight}; white-space: nowrap; overflow: hidden; flex: none;",
            onclick: move |_| {
                shell.set_drawer(false);
                if let Some(f) = on_press {
                    f.call(());
                }
            },
            if let Some(icon) = icon {
                div { style: "flex: none; display: flex; color: var(--as-muted);", IconView { icon, size: 15.0 } }
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
    }
}

/// A sidebar header's switcher (a library, an organisation): a mark, a
/// name over a detail, and ⇅; `on_press` opens the app's chooser.
#[component]
pub fn SidebarSwitcher(
    title: String,
    detail: Option<String>,
    icon: Option<Icon>,
    on_press: Option<EventHandler<()>>,
) -> Element {
    let initial = title
        .chars()
        .next()
        .unwrap_or(' ')
        .to_uppercase()
        .to_string();
    rsx! {
        div {
            class: "as-switcher-row as-hov",
            role: "button",
            style: "display: flex; align-items: center; gap: 8px; height: 40px; padding: 0 8px; border-radius: 8px; border: 1px solid var(--as-border); background-color: var(--as-search-bg); box-sizing: border-box; overflow: hidden;",
            onclick: move |_| {
                if let Some(f) = on_press {
                    f.call(());
                }
            },
            div {
                style: "flex: none; width: 24px; height: 24px; border-radius: 6px; background-color: var(--as-accent); color: #ffffff; display: flex; align-items: center; justify-content: center; font-weight: 700; font-size: 12px;",
                match icon {
                    Some(icon) => rsx! { IconView { icon, size: 14.0, color: "#ffffff".to_owned() } },
                    None => rsx! { "{initial}" },
                }
            }
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
