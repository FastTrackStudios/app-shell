//! A tab stack: its tab bar, its current tab's content, and the drop
//! zones a dragged tab lands in.

use dioxus::prelude::*;

use super::icons::{self, IconView};
use super::kinds::{TabCtx, TabKind};
use super::popup::MenuItem;
use super::shell::{Gesture, Shell, Zone, use_shell};
use crate::commands::Command;
use crate::model::{Chrome, Group, GroupId, PaneId, Side, WindowId};
use crate::ops::Dest;

/// One stack, filling its parent.
#[component]
pub fn StackView(group: Group, window: WindowId) -> Element {
    let shell = use_shell();
    let layout = shell.layout.read();
    let current = group.current_tab().cloned();
    let active = layout.active.clone();
    let focused = current.is_some() && active.as_ref() == current.as_ref();
    let pane = current.as_ref().and_then(|p| layout.pane(p)).cloned();
    drop(layout);
    let dragging = shell.dragging();
    let gid = group.id;
    rsx! {
        div {
            class: "as-stack",
            "data-group": "{gid.0}",
            style: "position: relative; flex: 1 1 0px; min-width: 0; min-height: 0; display: flex; flex-direction: column; overflow: hidden; background-color: var(--as-bg);",
            if group.chrome != Chrome::Bare {
                TabBar { group: group.clone() }
            }
            div {
                style: "position: relative; flex: 1 1 0px; min-height: 0; min-width: 0; display: flex; overflow: hidden;",
                onpointerdown: move |_| {
                    if let Some(c) = &current
                        && !focused {
                            shell.apply_quiet(|w| w.activate(c));
                        }
                },
                match pane {
                    Some(pane) => rsx! {
                        TabBody {
                            key: "{pane.id.0}",
                            kind: shell.kind(&pane.view.0),
                            ctx: TabCtx { pane: pane.clone(), window: window.clone(), active: focused },
                        }
                    },
                    None => rsx! { EmptyStack { group: gid } },
                }
                if dragging {
                    DropZones { group: gid }
                }
            }
        }
    }
}

/// A tab's content: its kind's render, in a component of its own.
#[component]
pub fn TabBody(kind: Option<TabKind>, ctx: TabCtx) -> Element {
    let body = match &kind {
        Some(kind) => (kind.render)(ctx.clone()),
        None => rsx! {
            div {
                style: "margin: auto; color: var(--as-muted); text-align: center; padding: 16px;",
                "Nothing draws “{ctx.pane.view.0}” tabs here."
            }
        },
    };
    rsx! {
        div {
            class: "as-tab-body",
            "data-pane": "{ctx.pane.id.0}",
            style: "flex: 1 1 0px; min-width: 0; min-height: 0; display: flex; flex-direction: column; overflow: hidden;",
            {body}
        }
    }
}

/// A stack with nothing in it (the main window's last).
#[component]
fn EmptyStack(group: GroupId) -> Element {
    let shell = use_shell();
    let new_tab = shell.new_tab();
    rsx! {
        div {
            style: "margin: auto; display: flex; flex-direction: column; align-items: center; gap: 10px; color: var(--as-muted);",
            "No tabs open"
            if let Some(new_tab) = new_tab {
                div {
                    class: "as-hov",
                    style: "display: flex; align-items: center; gap: 6px; padding: 6px 12px; border: 1px solid var(--as-border); border-radius: var(--as-radius); cursor: pointer; color: var(--as-text);",
                    onclick: move |_| new_tab.call(group),
                    IconView { icon: icons::PLUS }
                    "New tab"
                }
            }
        }
    }
}

/// A stack's tab bar.
#[component]
pub fn TabBar(group: Group) -> Element {
    let shell = use_shell();
    let layout = shell.layout.read();
    let tabs: Vec<(usize, PaneId, String, bool, String)> = group
        .tabs
        .iter()
        .enumerate()
        .filter_map(|(i, id)| {
            let p = layout.pane(id)?;
            Some((i, id.clone(), p.title.clone(), p.pinned, p.view.0.clone()))
        })
        .collect();
    let focused = group.current_tab().is_some() && layout.active.as_ref() == group.current_tab();
    drop(layout);
    let icons_only = group.chrome == Chrome::Icons;
    let gid = group.id;
    let n = group.tabs.len();
    let dragging = shell.dragging();
    let hover = *shell.hover.read();
    let end_lit = matches!(hover, Some(h) if h.zone == Zone::Bar(gid, n));
    let new_tab = shell.new_tab();
    rsx! {
        div {
            class: "as-tabbar",
            style: "flex: none; height: var(--as-tab-h); display: flex; align-items: stretch; background-color: var(--as-surface); border-bottom: 1px solid var(--as-border); overflow: hidden; min-width: 0;",
            for (i, id, title, pinned, view) in tabs {
                TabButton {
                    key: "{id.0}",
                    group: gid,
                    index: i,
                    pane: id.clone(),
                    title,
                    pinned,
                    view,
                    current: i == group.current,
                    focused: focused && i == group.current,
                    icons_only,
                }
            }
            div {
                style: "position: relative; flex: 1 1 0px; min-width: 16px; display: flex; align-items: center; justify-content: flex-end; box-shadow: inset 2px 0 0 0 {lit(end_lit)};",
                if dragging {
                    div {
                        style: "position: absolute; inset: 0;",
                        onpointermove: move |_| shell.hover(Zone::Bar(gid, n), Dest::Group { group: gid, index: Some(n) }),
                    }
                }
                if let Some(new_tab) = new_tab {
                    div {
                        class: "as-hov",
                        title: "New tab",
                        style: "display: flex; align-items: center; justify-content: center; width: 28px; height: 100%; cursor: pointer; color: var(--as-muted);",
                        onclick: move |_| new_tab.call(gid),
                        IconView { icon: icons::PLUS }
                    }
                }
                div {
                    class: "as-hov",
                    title: "Stack menu",
                    style: "display: flex; align-items: center; justify-content: center; width: 28px; height: 100%; cursor: pointer; color: var(--as-muted);",
                    onclick: move |event| {
                        let p = event.client_coordinates();
                        shell.menu(p.x - 180.0, p.y + 8.0, stack_menu(&shell, gid));
                    },
                    IconView { icon: icons::MORE }
                }
            }
        }
    }
}

/// The colour a lit insertion line or zone has.
pub(crate) const fn lit(on: bool) -> &'static str {
    if on {
        "var(--as-accent)"
    } else {
        "transparent"
    }
}

/// One tab in a bar.
#[component]
fn TabButton(
    group: GroupId,
    index: usize,
    pane: PaneId,
    title: String,
    pinned: bool,
    view: String,
    current: bool,
    /// The active tab of the whole layout: marked with the accent.
    focused: bool,
    icons_only: bool,
) -> Element {
    let shell = use_shell();
    let icon = shell.kind(&view).map_or(icons::FILE, |k| k.icon);
    let dragging = shell.dragging();
    let hover = *shell.hover.read();
    let before = matches!(hover, Some(h) if h.zone == Zone::Bar(group, index));
    let (bg, fg) = if current {
        ("var(--as-bg)", "var(--as-text)")
    } else {
        ("transparent", "var(--as-muted)")
    };
    let pad = if icons_only { "0 8px" } else { "0 6px 0 10px" };
    let press = pane.clone();
    let press_title = title.clone();
    let menu_pane = pane.clone();
    let close_pane = pane.clone();
    rsx! {
        div {
            class: "as-tab as-hov",
            "data-pane": "{pane.0}",
            "data-current": "{current}",
            title: "{title}",
            style: "position: relative; flex: 0 1 auto; min-width: 0; max-width: 220px; display: flex; align-items: center; gap: 6px; padding: {pad}; background-color: {bg}; color: {fg}; border-right: 1px solid var(--as-border); cursor: default; white-space: nowrap; overflow: hidden; box-shadow: inset 2px 0 0 0 {lit(before)}, inset 0 2px 0 0 {lit(focused)};",
            onpointerdown: move |event| {
                if event.trigger_button() != Some(dioxus::html::input_data::MouseButton::Primary) {
                    return;
                }
                let p = event.client_coordinates();
                shell.close_popup();
                shell.apply_quiet(|w| w.activate(&press));
                let mut gesture = shell.gesture;
                gesture.set(Some(Gesture::Tab {
                    pane: press.clone(),
                    title: press_title.clone(),
                    start: (p.x, p.y),
                    at: (p.x, p.y),
                    live: false,
                }));
            },
            oncontextmenu: move |event| {
                event.prevent_default();
                let p = event.client_coordinates();
                shell.menu(p.x, p.y, tab_menu(&menu_pane, pinned));
            },
            IconView { icon }
            if !icons_only {
                span { style: "overflow: hidden; white-space: nowrap; min-width: 0; flex: 0 1 auto;", "{title}" }
                if pinned {
                    IconView { icon: icons::PIN, size: 11.0 }
                } else {
                    div {
                        class: "as-x",
                        title: "Close",
                        style: "flex: none; display: flex; align-items: center; justify-content: center; width: 18px; height: 18px; border-radius: 4px;",
                        onpointerdown: move |event| event.stop_propagation(),
                        onclick: move |event| {
                            event.stop_propagation();
                            shell.apply(|w| w.close(&close_pane));
                        },
                        IconView { icon: icons::X, size: 12.0 }
                    }
                }
            }
            if dragging {
                // Each half of the tab: before it, or after it.
                div {
                    style: "position: absolute; left: 0; top: 0; bottom: 0; width: 50%;",
                    onpointermove: move |_| shell.hover(Zone::Bar(group, index), Dest::Group { group, index: Some(index) }),
                }
                div {
                    style: "position: absolute; right: 0; top: 0; bottom: 0; width: 50%;",
                    onpointermove: move |_| {
                        let after = index.saturating_add(1);
                        shell.hover(Zone::Bar(group, after), Dest::Group { group, index: Some(after) });
                    },
                }
            }
        }
    }
}

/// A tab's context menu.
pub(crate) fn tab_menu(pane: &PaneId, pinned: bool) -> Vec<MenuItem> {
    let on = |label: &str, icon, command| {
        MenuItem::command(label, Some(pane.clone()), command).icon(icon)
    };
    vec![
        on("Close", icons::X, Command::CloseTab),
        on("Close others", icons::X, Command::CloseOthers),
        MenuItem::separator(),
        on("Split right", icons::COLUMNS, Command::Split(Side::Right)),
        on("Split down", icons::ROWS, Command::Split(Side::Bottom)),
        on(
            "Move to new window",
            icons::EXTERNAL,
            Command::MoveToNewWindow,
        ),
        on(
            "Move to main window",
            icons::SQUARE,
            Command::MoveToMainWindow,
        ),
        MenuItem::separator(),
        on(
            if pinned { "Unpin" } else { "Pin" },
            icons::PIN,
            Command::TogglePin,
        ),
        on("Zoom", icons::MAXIMIZE, Command::ToggleZoom),
    ]
}

/// A stack's menu (its "…" button).
fn stack_menu(shell: &Shell, group: GroupId) -> Vec<MenuItem> {
    let current = shell
        .layout
        .peek()
        .group(group)
        .and_then(|g| g.current_tab().cloned());
    let mut items = Vec::new();
    if let Some(p) = &current {
        items.extend(tab_menu(
            p,
            shell.layout.peek().pane(p).is_some_and(|x| x.pinned),
        ));
        items.push(MenuItem::separator());
    }
    items.push(MenuItem::command("Show all tabs", None, Command::OpenSwitcher).icon(icons::LAYERS));
    items
}

/// Where a dragged tab lands in a stack: its middle (a new tab there) or
/// a band along a side (a split there). Bands are a quarter of the
/// stack, kept between 24 and 96 points ([`crate::SPLIT_SHARE`]).
#[component]
fn DropZones(group: GroupId) -> Element {
    let shell = use_shell();
    let hover = *shell.hover.read();
    let lit_side = match hover {
        Some(h) => match h.zone {
            Zone::Group(g, side) if g == group => Some(side),
            _ => None,
        },
        None => None,
    };
    let highlight = match lit_side {
        // Every edge and size, each time: a renderer that patches a
        // changed style property by property keeps none of the last one.
        Some(None) => Some("left: 0; right: 0; top: 0; bottom: 0; width: auto; height: auto;"),
        Some(Some(Side::Left)) => {
            Some("left: 0; right: auto; top: 0; bottom: 0; width: 50%; height: auto;")
        }
        Some(Some(Side::Right)) => {
            Some("left: auto; right: 0; top: 0; bottom: 0; width: 50%; height: auto;")
        }
        Some(Some(Side::Top)) => {
            Some("left: 0; right: 0; top: 0; bottom: auto; width: auto; height: 50%;")
        }
        Some(Some(Side::Bottom)) => {
            Some("left: 0; right: 0; top: auto; bottom: 0; width: auto; height: 50%;")
        }
        None => None,
    };
    let band = |side: Side| match side {
        Side::Left => "left: 0; top: 0; bottom: 0; width: 25%; min-width: 24px; max-width: 96px;",
        Side::Right => "right: 0; top: 0; bottom: 0; width: 25%; min-width: 24px; max-width: 96px;",
        Side::Top => "left: 0; right: 0; top: 0; height: 25%; min-height: 24px; max-height: 96px;",
        Side::Bottom => {
            "left: 0; right: 0; bottom: 0; height: 25%; min-height: 24px; max-height: 96px;"
        }
    };
    rsx! {
        div {
            class: "as-drop-zones",
            style: "position: absolute; inset: 0; z-index: 20;",
            if let Some(h) = highlight {
                div {
                    class: "as-drop-highlight",
                    style: "position: absolute; {h} background-color: var(--as-accent-soft); border: 2px solid var(--as-accent); box-sizing: border-box; pointer-events: none; border-radius: var(--as-radius);",
                }
            }
            div {
                style: "position: absolute; inset: 0;",
                onpointermove: move |_| shell.hover(Zone::Group(group, None), Dest::Group { group, index: None }),
            }
            for side in [Side::Left, Side::Right, Side::Top, Side::Bottom] {
                div {
                    style: "position: absolute; {band(side)}",
                    onpointermove: move |_| shell.hover(Zone::Group(group, Some(side)), Dest::Edge { group, side }),
                }
            }
        }
    }
}

/// Over everything while a tab is dragged: its ghost under the pointer,
/// the window's edges (its docks) and the "new window" target.
#[component]
pub fn DragLayer() -> Element {
    let shell = use_shell();
    let gesture = shell.gesture.read().clone();
    let Some(Gesture::Tab {
        title,
        at,
        live: true,
        ..
    }) = gesture
    else {
        return rsx! {};
    };
    let hover = *shell.hover.read();
    let is = |z: Zone| matches!(hover, Some(h) if h.zone == z);
    let edge = |side: Side| match side {
        Side::Left => "left: 0; top: 0; bottom: 0; width: 28px;",
        Side::Right => "right: 0; top: 0; bottom: 0; width: 28px;",
        _ => "left: 0; right: 0; bottom: 0; height: 28px;",
    };
    let new_lit = is(Zone::NewWindow);
    let (gx, gy) = (at.0 + 12.0, at.1 + 10.0);
    rsx! {
        // No box of its own: only its children take the pointer.
        div {
            class: "as-drag-layer",
            style: "position: absolute; inset: 0; z-index: 40; pointer-events: none;",
            for side in [Side::Left, Side::Right, Side::Bottom] {
                div {
                    style: "position: absolute; {edge(side)} pointer-events: auto; background-color: {edge_bg(is(Zone::Edge(side)))};",
                    onpointermove: move |_| shell.hover(Zone::Edge(side), Dest::Dock(side)),
                }
            }
            div {
                class: "as-new-window",
                style: "position: absolute; top: 8px; left: 50%; transform: translateX(-50%); pointer-events: auto; display: flex; align-items: center; gap: 6px; padding: 6px 12px; border-radius: 999px; border: 1px solid {lit(new_lit)}; background-color: var(--as-raised); color: var(--as-text); box-shadow: 0 4px 16px rgba(0,0,0,0.35);",
                onpointermove: move |_| shell.hover(Zone::NewWindow, Dest::Window(None)),
                IconView { icon: icons::EXTERNAL }
                "New window"
            }
            div {
                class: "as-ghost",
                style: "position: absolute; left: {gx}px; top: {gy}px; padding: 4px 10px; border-radius: var(--as-radius); background-color: var(--as-raised); border: 1px solid var(--as-border); color: var(--as-text); white-space: nowrap; opacity: 0.92;",
                "{title}"
            }
        }
    }
}

const fn edge_bg(on: bool) -> &'static str {
    if on {
        "var(--as-accent-soft)"
    } else {
        "transparent"
    }
}
