//! Popups: a menu or the app's own content, over the window at a point.
//!
//! Blitz has no `position: fixed`, so a popup cannot be drawn where it is
//! asked for: it is described ([`Popup`]) and drawn by the shell's root,
//! over everything, its backdrop closing it.

use dioxus::prelude::*;

use super::icons::{Icon, IconView};
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

/// One row of a menu; a row with no action and no label is a separator.
#[derive(Clone, PartialEq)]
pub struct MenuItem {
    pub label: String,
    pub icon: Option<Icon>,
    pub action: Option<MenuAction>,
    /// Drawn in the danger colour.
    pub danger: bool,
}

impl MenuItem {
    /// A row running `command` (on `pane`).
    pub fn command(label: impl Into<String>, pane: Option<PaneId>, command: Command) -> Self {
        Self {
            label: label.into(),
            icon: None,
            action: Some(MenuAction::Command { pane, command }),
            danger: false,
        }
    }

    /// A row calling the app.
    pub fn call(label: impl Into<String>, call: Callback<()>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            action: Some(MenuAction::Call(call)),
            danger: false,
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
        }
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
    let (x, y) = (popup.x.max(4.0), popup.y.max(4.0));
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
                    PopupContent::Menu(items) => rsx! {
                        for (i, item) in items.into_iter().enumerate() {
                            MenuRow { key: "{i}", item }
                        }
                    },
                    PopupContent::Custom(render) => render.call(()),
                }
            }
        }
    }
}

#[component]
fn MenuRow(item: MenuItem) -> Element {
    let shell = use_shell();
    if item.is_separator() {
        return rsx! {
            div { style: "height: 1px; margin: 4px 2px; background-color: var(--as-border);" }
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
