//! Popups: a menu or the app's own content, over the window at a point.
//!
//! Blitz has no `position: fixed`, so a popup cannot be drawn where it is
//! asked for: it is described ([`Popup`]) and drawn by the shell's root,
//! over everything, its backdrop closing it.

use dioxus::prelude::*;

use super::icons::{CHECK, Icon, IconView};
use super::shell::use_shell;
use crate::commands::Command;
use crate::model::PaneId;

/// What a menu row does.
#[derive(Clone, PartialEq)]
pub enum MenuAction {
    /// Run a shell command, on `pane` when given (it is activated first).
    Command {
        pane: Option<PaneId>,
        command: Command,
    },
    /// Call the app.
    Call(Callback<()>),
}

/// One row of a menu; a row with no action and no label is a separator,
/// one with a label and no action a line of text ([`MenuItem::note`]),
/// or a section's name ([`MenuItem::heading`]).
#[derive(Clone, PartialEq)]
pub struct MenuItem {
    pub label: String,
    pub icon: Option<Icon>,
    pub action: Option<MenuAction>,
    /// Drawn in the danger colour.
    pub danger: bool,
    /// A toggle's or a choice's tick: `Some(true)` ticked, `Some(false)`
    /// room left for one; `None` for a plain row.
    pub checked: Option<bool>,
    /// A section's name, drawn small over the rows under it.
    pub heading: bool,
}

impl MenuItem {
    /// A row running `command` (on `pane`).
    pub fn command(label: impl Into<String>, pane: Option<PaneId>, command: Command) -> Self {
        Self {
            label: label.into(),
            icon: None,
            action: Some(MenuAction::Command { pane, command }),
            danger: false,
            checked: None,
            heading: false,
        }
    }

    /// A row calling the app.
    pub fn call(label: impl Into<String>, call: Callback<()>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            action: Some(MenuAction::Call(call)),
            danger: false,
            checked: None,
            heading: false,
        }
    }

    /// A line between rows.
    #[must_use]
    pub const fn separator() -> Self {
        Self {
            label: String::new(),
            icon: None,
            action: None,
            danger: false,
            checked: None,
            heading: false,
        }
    }

    /// A section's name over the rows that follow.
    pub fn heading(label: impl Into<String>) -> Self {
        Self {
            heading: true,
            ..Self::note(label)
        }
    }

    /// A line of text that does nothing (a menu with nothing in it says
    /// so).
    pub fn note(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            ..Self::separator()
        }
    }

    /// Ticked when `on`: one of a choice, or a toggle. Rows of the same
    /// menu without one keep the tick's room, so the labels line up.
    #[must_use]
    pub const fn checked(mut self, on: bool) -> Self {
        self.checked = Some(on);
        self
    }

    #[must_use]
    pub const fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    #[must_use]
    pub const fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    fn is_separator(&self) -> bool {
        self.action.is_none() && self.label.is_empty()
    }

    /// Text that does nothing: a heading or a note.
    pub(crate) fn is_text(&self) -> bool {
        self.action.is_none() && !self.label.is_empty()
    }
}

/// Where a menu opened from a button goes: under the button the press
/// `event` landed on (its left edge), `height` its height, in window
/// points.
#[must_use]
pub fn under(event: &MouseEvent, height: f64) -> (f64, f64) {
    let at = event.client_coordinates();
    let within = event.element_coordinates();
    (at.x - within.x, at.y - within.y + height + 4.0)
}

/// What a popup holds.
#[derive(Clone, PartialEq)]
pub enum PopupContent {
    Menu(Vec<MenuItem>),
    /// The app's own content, drawn in a panel.
    Custom(Callback<(), Element>),
}

/// A popup asked for: where (window points) and what.
#[derive(Clone, PartialEq)]
pub struct Popup {
    pub x: f64,
    pub y: f64,
    pub content: PopupContent,
}

/// The popup on show, if any: drawn by the shell's root.
#[component]
pub fn PopupLayer() -> Element {
    let shell = use_shell();
    let Some(popup) = shell.popup.read().clone() else {
        return rsx! {};
    };
    // Kept inside the window: at least the menu's narrowest from its
    // right edge.
    let (vw, _) = *shell.viewport.peek();
    let right = if vw > 0.0 { vw - 184.0 } else { f64::MAX };
    let (x, y) = (popup.x.min(right).max(4.0), popup.y.max(4.0));
    rsx! {
        div {
            class: "as-popup-layer",
            style: "position: absolute; inset: 0; z-index: 60;",
            onpointerdown: move |_| shell.close_popup(),
            oncontextmenu: move |event| {
                event.prevent_default();
                shell.close_popup();
            },
            div {
                class: "as-popup",
                style: "position: absolute; left: {x}px; top: {y}px; min-width: 180px; max-width: 320px; padding: 4px; display: flex; flex-direction: column; background-color: var(--as-raised); border: 1px solid var(--as-border); border-radius: var(--as-radius); box-shadow: 0 8px 28px rgba(0,0,0,0.4);",
                onpointerdown: move |event| event.stop_propagation(),
                match popup.content {
                    PopupContent::Menu(items) => {
                        let ticks = items.iter().any(|i| i.checked.is_some());
                        rsx! {
                            for (i, item) in items.into_iter().enumerate() {
                                MenuRow { key: "{i}", item, ticks }
                            }
                        }
                    }
                    PopupContent::Custom(render) => render.call(()),
                }
            }
        }
    }
}

#[component]
fn MenuRow(item: MenuItem, ticks: bool) -> Element {
    let shell = use_shell();
    if item.is_separator() {
        return rsx! {
            div { style: "height: 1px; margin: 4px 2px; background-color: var(--as-border);" }
        };
    }
    if item.heading {
        return rsx! {
            div {
                class: "as-menu-heading",
                style: "padding: 6px 8px 2px; font-size: 11px; font-weight: 600; color: var(--as-heading); white-space: nowrap; overflow: hidden;",
                "{item.label}"
            }
        };
    }
    if item.is_text() {
        return rsx! {
            div {
                class: "as-menu-note",
                style: "padding: 6px 8px; color: var(--as-muted); white-space: nowrap; overflow: hidden;",
                "{item.label}"
            }
        };
    }
    let color = if item.danger {
        "var(--as-danger)"
    } else {
        "var(--as-text)"
    };
    let action = item.action.clone();
    rsx! {
        div {
            class: "as-item",
            role: "menuitem",
            style: "display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: 4px; color: {color}; cursor: default; white-space: nowrap; overflow: hidden;",
            onclick: move |_| {
                shell.close_popup();
                match &action {
                    Some(MenuAction::Command { pane: Some(p), command }) => shell.command_on(p, *command),
                    Some(MenuAction::Command { pane: None, command }) => shell.command(*command),
                    Some(MenuAction::Call(f)) => f.call(()),
                    None => {}
                }
            },
            if ticks {
                div {
                    style: "width: 14px; flex: none; display: flex;",
                    if item.checked == Some(true) {
                        IconView { icon: CHECK }
                    }
                }
            }
            div {
                style: "width: 14px; flex: none; display: flex;",
                if let Some(icon) = item.icon {
                    IconView { icon }
                }
            }
            span { style: "overflow: hidden; white-space: nowrap; min-width: 0;", "{item.label}" }
        }
    }
}
