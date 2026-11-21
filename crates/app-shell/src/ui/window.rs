//! One window's body: its center tree, its docks and its floats; a
//! phone's one-tab view; and, in a browser, the pop-outs drawn in the
//! page.

use dioxus::prelude::*;

use super::icons::{self, IconView};
use super::kinds::TabCtx;
use super::shell::{Gesture, use_shell};
use super::split::NodeView;
use super::stack::{StackView, TabBody};
use super::title::BarButton;
use crate::model::{Dock, Float, Node, Rect, Shown, Side, Snap, WindowId};

/// A window's center, docks and floats, filling its parent.
#[component]
pub fn WindowBody(window: WindowId) -> Element {
    let shell = use_shell();
    let layout = shell.layout.read();
    let Some(w) = layout.window(&window) else {
        return rsx! {};
    };
    // A zoomed stack fills the center.
    let center: Option<Node> = w.center.as_ref().map(|c| {
        w.zoomed
            .and_then(|z| c.group(z).cloned())
            .map_or_else(|| c.clone(), Node::Group)
    });
    let docks = w.docks.clone();
    let floats = w.floats.clone();
    drop(layout);
    let shown = |d: &Dock| d.root.is_some() && d.shown == Shown::Open;
    rsx! {
        div {
            class: "as-window-body",
            style: "position: relative; flex: 1 1 0px; min-width: 0; min-height: 0; display: flex; overflow: hidden;",
            if shown(&docks.left) {
                DockView { dock: docks.left.clone(), side: Side::Left, window: window.clone() }
            }
            div {
                style: "position: relative; flex: 1 1 0px; min-width: 0; min-height: 0; display: flex; flex-direction: column;",
                div {
                    class: "as-center",
                    style: "position: relative; flex: 1 1 0px; min-width: 0; min-height: 0; display: flex;",
                    if let Some(node) = center {
                        NodeView { node, window: window.clone() }
                    }
                }
                if shown(&docks.bottom) {
                    DockView { dock: docks.bottom.clone(), side: Side::Bottom, window: window.clone() }
                }
            }
            if shown(&docks.right) {
                DockView { dock: docks.right.clone(), side: Side::Right, window: window.clone() }
            }
            for float in floats.into_iter().filter(|f| !f.minimised) {
                FloatView { key: "{float.group.id.0}", float, window: window.clone() }
            }
        }
    }
}

/// A dock: its tree at its size, with an edge to drag.
#[component]
fn DockView(dock: Dock, side: Side, window: WindowId) -> Element {
    let shell = use_shell();
    let Some(root) = dock.root.clone() else {
        return rsx! {};
    };
    let size = dock.size;
    let (outer, edge, cursor) = match side {
        Side::Left => (
            format!("width: {size}px; border-right: 1px solid var(--as-border);"),
            "right: -3px; top: 0; bottom: 0; width: 6px;",
            "col-resize",
        ),
        Side::Right => (
            format!("width: {size}px; border-left: 1px solid var(--as-border);"),
            "left: -3px; top: 0; bottom: 0; width: 6px;",
            "col-resize",
        ),
        _ => (
            format!("height: {size}px; border-top: 1px solid var(--as-border);"),
            "top: -3px; left: 0; right: 0; height: 6px;",
            "row-resize",
        ),
    };
    let name = match side {
        Side::Left => "left",
        Side::Right => "right",
        _ => "bottom",
    };
    rsx! {
        div {
            class: "as-dock",
            "data-dock": "{name}",
            style: "position: relative; flex: none; {outer} min-width: 0; min-height: 0; display: flex; background-color: var(--as-surface);",
            NodeView { node: root, window }
            div {
                class: "as-divider",
                style: "position: absolute; {edge} cursor: {cursor}; z-index: 10;",
                onpointerdown: move |event| {
                    event.stop_propagation();
                    let p = event.client_coordinates();
                    let mut gesture = shell.gesture;
                    gesture.set(Some(Gesture::Dock { side, start: (p.x, p.y), size }));
                },
            }
        }
    }
}

/// A stack floating over the window, moved by its grip.
#[component]
fn FloatView(float: Float, window: WindowId) -> Element {
    let shell = use_shell();
    let r = float.rect;
    let place = match float.snap {
        Some(Snap::Maximised) => "inset: 0;".to_owned(),
        Some(Snap::Half(Side::Left)) => "left: 0; top: 0; bottom: 0; width: 50%;".to_owned(),
        Some(Snap::Half(_)) => "right: 0; top: 0; bottom: 0; width: 50%;".to_owned(),
        Some(Snap::EdgeDock {
            side: Side::Left,
            width,
        }) => {
            format!("left: 0; top: 0; bottom: 0; width: {width}px;")
        }
        Some(Snap::EdgeDock { width, .. }) => {
            format!("right: 0; top: 0; bottom: 0; width: {width}px;")
        }
        None => format!(
            "left: {}px; top: {}px; width: {}px; height: {}px;",
            r.x, r.y, r.w, r.h
        ),
    };
    let pane = float.group.current_tab().cloned();
    let group = float.group.clone();
    rsx! {
        div {
            class: "as-float",
            style: "position: absolute; {place} z-index: 30; display: flex; flex-direction: column; border: 1px solid var(--as-border); border-radius: var(--as-radius); overflow: hidden; box-shadow: 0 12px 36px rgba(0,0,0,0.45); background-color: var(--as-bg);",
            div {
                class: "as-grip",
                style: "flex: none; height: 10px; background-color: var(--as-raised); cursor: move;",
                onpointerdown: move |event| {
                    let Some(pane) = pane.clone() else { return };
                    event.stop_propagation();
                    let p = event.client_coordinates();
                    let mut gesture = shell.gesture;
                    gesture.set(Some(Gesture::Float { pane, start: (p.x, p.y), rect: r }));
                },
            }
            StackView { group, window }
        }
    }
}

/// The pop-out windows, drawn in the page (where the platform has no
/// windows of its own): each a frame over the main window, its title bar
/// moving it, its close button giving its tabs back.
#[component]
pub fn InPageWindows() -> Element {
    let shell = use_shell();
    let layout = shell.layout.read();
    let windows: Vec<(usize, WindowId, Option<Rect>, String)> = layout
        .windows
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, w)| {
            (
                i,
                w.id.clone(),
                w.bounds,
                super::shell::window_title(&layout, &w.id),
            )
        })
        .collect();
    drop(layout);
    rsx! {
        for (i, id, bounds, title) in windows {
            InPageWindow { key: "{id.0}", index: i, id, bounds, title }
        }
    }
}

#[component]
fn InPageWindow(index: usize, id: WindowId, bounds: Option<Rect>, title: String) -> Element {
    let shell = use_shell();
    // Where the layout says, else cascaded from the top left.
    let step = f32::from(u16::try_from(index.min(20)).unwrap_or(20)) * 28.0;
    let r = bounds.unwrap_or(Rect::new(60.0 + step, 60.0 + step, 560.0, 380.0));
    let moving = id.clone();
    let closing = id.clone();
    rsx! {
        div {
            class: "as-inpage-window",
            "data-window": "{id.0}",
            style: "position: absolute; left: {r.x}px; top: {r.y}px; width: {r.w}px; height: {r.h}px; max-width: 100%; max-height: 100%; z-index: 35; display: flex; flex-direction: column; border: 1px solid var(--as-border); border-radius: 8px; overflow: hidden; box-shadow: 0 18px 48px rgba(0,0,0,0.5); background-color: var(--as-bg);",
            div {
                style: "flex: none; height: 30px; display: flex; align-items: center; gap: 6px; padding: 0 4px 0 10px; background-color: var(--as-raised); border-bottom: 1px solid var(--as-border); cursor: move;",
                onpointerdown: move |event| {
                    let p = event.client_coordinates();
                    let mut gesture = shell.gesture;
                    gesture.set(Some(Gesture::Window { window: moving.clone(), start: (p.x, p.y), rect: r }));
                },
                IconView { icon: icons::EXTERNAL, size: 12.0 }
                span { style: "flex: 1 1 0px; min-width: 0; overflow: hidden; white-space: nowrap; font-weight: 600;", "{title}" }
                BarButton {
                    icon: icons::X,
                    label: "Close window (its tabs go back)",
                    on_press: move |()| {
                        shell.apply(|w| w.close_window(&closing));
                    },
                }
            }
            WindowBody { window: id.clone() }
        }
    }
}

/// A phone: the active tab alone, a bar over it with the tab count that
/// opens the switcher.
#[component]
pub fn PhoneView(window: WindowId) -> Element {
    let shell = use_shell();
    let layout = shell.layout.read();
    // The active tab when it is in this window, else this window's first.
    let in_window = |p: &crate::model::PaneId| {
        layout
            .locate(p)
            .and_then(|l| layout.windows.get(l.window))
            .is_some_and(|w| w.id == window)
    };
    let pane = layout
        .active
        .clone()
        .filter(|p| in_window(p))
        .or_else(|| {
            layout
                .window(&window)
                .and_then(|w| w.first_group())
                .and_then(|g| g.current_tab().cloned())
        })
        .and_then(|p| layout.pane(&p).cloned());
    let count = layout.tab_count();
    drop(layout);
    let title = pane
        .as_ref()
        .map_or_else(|| "No tabs".to_owned(), |p| p.title.clone());
    let menu_pane = pane.as_ref().map(|p| (p.id.clone(), p.pinned));
    rsx! {
        div {
            class: "as-phone",
            style: "position: relative; flex: 1 1 0px; min-width: 0; min-height: 0; display: flex; flex-direction: column;",
            div {
                class: "as-phone-bar",
                style: "flex: none; height: 44px; display: flex; align-items: center; gap: 8px; padding: 0 8px 0 14px; background-color: var(--as-surface); border-bottom: 1px solid var(--as-border);",
                span { style: "flex: 1 1 0px; min-width: 0; overflow: hidden; white-space: nowrap; font-weight: 600; font-size: 15px;", "{title}" }
                if let Some((p, pinned)) = menu_pane {
                    BarButton {
                        icon: icons::MORE,
                        label: "Tab menu",
                        on_press: move |()| shell.menu(16.0, 48.0, super::stack::tab_menu(&p, pinned)),
                    }
                }
                div {
                    class: "as-hov as-tabs-button",
                    role: "button",
                    title: "All tabs",
                    style: "display: flex; align-items: center; justify-content: center; min-width: 30px; height: 26px; padding: 0 6px; border: 2px solid var(--as-text); border-radius: 7px; font-weight: 700; font-size: 13px; box-sizing: border-box;",
                    onclick: move |_| shell.show_switcher(true),
                    "{count}"
                }
            }
            div {
                style: "position: relative; flex: 1 1 0px; min-height: 0; min-width: 0; display: flex; overflow: hidden;",
                match pane {
                    Some(pane) => rsx! {
                        TabBody {
                            key: "{pane.id.0}",
                            kind: shell.kind(&pane.view.0),
                            ctx: TabCtx { pane: pane.clone(), window: window.clone(), active: true },
                        }
                    },
                    None => rsx! {
                        div { style: "margin: auto; color: var(--as-muted);", "No tabs open" }
                    },
                }
            }
        }
    }
}
