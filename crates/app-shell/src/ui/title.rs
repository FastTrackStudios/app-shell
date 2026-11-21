//! A window's title bar: its title, the dock and switcher buttons, and
//! (where the platform draws no frame) the window's own buttons.

use dioxus::prelude::*;

use super::icons::{self, Icon, IconView};
use super::shell::use_shell;
use super::windows::WindowControls;
use crate::commands::Command;
use crate::model::Side;

/// The title bar. Its empty part moves the window (where the platform
/// gives [`WindowControls::drag`]); a double press there maximises it.
#[component]
pub fn TitleBar(title: String) -> Element {
    let shell = use_shell();
    let controls = try_use_context::<WindowControls>();
    let count = shell.layout.read().tab_count();
    let drag = controls.and_then(|c| c.drag);
    let zoom = controls.and_then(|c| c.toggle_maximize);
    rsx! {
        div {
            class: "as-titlebar",
            style: "flex: none; height: var(--as-title-h); display: flex; align-items: center; gap: 2px; padding: 0 4px 0 12px; background-color: var(--as-surface); border-bottom: 1px solid var(--as-border); min-width: 0; overflow: hidden;",
            div {
                class: "as-title",
                style: "flex: 1 1 0px; min-width: 0; height: 100%; display: flex; align-items: center; white-space: nowrap; overflow: hidden; font-weight: 600;",
                onpointerdown: move |event| {
                    if let Some(drag) = drag {
                        event.stop_propagation();
                        drag.call(());
                    }
                },
                ondoubleclick: move |_| {
                    if let Some(zoom) = zoom {
                        zoom.call(());
                    }
                },
                "{title}"
            }
            BarButton { icon: icons::PANEL_LEFT, label: "Left dock", on_press: move |()| shell.command(Command::ToggleDock(Side::Left)) }
            BarButton { icon: icons::PANEL_BOTTOM, label: "Bottom dock", on_press: move |()| shell.command(Command::ToggleDock(Side::Bottom)) }
            BarButton { icon: icons::PANEL_RIGHT, label: "Right dock", on_press: move |()| shell.command(Command::ToggleDock(Side::Right)) }
            div {
                class: "as-hov as-switcher-button",
                title: "All tabs",
                style: "display: flex; align-items: center; gap: 4px; height: 26px; padding: 0 8px; border-radius: var(--as-radius); color: var(--as-muted);",
                onclick: move |_| shell.command(Command::OpenSwitcher),
                IconView { icon: icons::LAYERS }
                span { "{count}" }
            }
            if let Some(c) = controls {
                div { style: "width: 8px; flex: none;" }
                if let Some(f) = c.minimize {
                    BarButton { icon: icons::MINUS, label: "Minimise", on_press: move |()| f.call(()) }
                }
                if let Some(f) = c.toggle_maximize {
                    BarButton { icon: icons::SQUARE, label: "Maximise", on_press: move |()| f.call(()) }
                }
                if let Some(f) = c.close {
                    BarButton { icon: icons::X, label: "Close", on_press: move |()| f.call(()) }
                }
            }
        }
    }
}

/// A square icon button in a bar.
#[component]
pub fn BarButton(icon: Icon, label: String, on_press: EventHandler<()>) -> Element {
    rsx! {
        div {
            class: "as-hov",
            role: "button",
            title: "{label}",
            "aria-label": "{label}",
            style: "flex: none; display: flex; align-items: center; justify-content: center; width: 28px; height: 26px; border-radius: var(--as-radius); color: var(--as-muted);",
            onpointerdown: move |event| event.stop_propagation(),
            onclick: move |_| on_press.call(()),
            IconView { icon }
        }
    }
}
