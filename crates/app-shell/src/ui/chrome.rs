//! The window's chrome: the top bar (a macOS unified toolbar: the title
//! strip and the toolbar in one row), the traffic lights, and the toolbar's
//! buttons.
//!
//! Where the platform has a window of its own (`app-shell-native`, a
//! [`WindowControls`] in context), the strip is the window's title bar:
//! its empty part moves the window, a double press zooms it, and the
//! traffic lights sit at its left: drawn by the shell (Linux, Windows) or
//! the system's own, kept clear (macOS). In a browser it is the same bar
//! without window buttons.

use dioxus::prelude::*;

use super::icons::{self, Icon, IconView};
use super::shell::{FormFactor, use_shell};
use super::windows::WindowControls;
use crate::commands::Command;

/// The top bar's height in points: the row the traffic lights are centred
/// in. The theme's `--as-topbar-h` says the same for the CSS; the native
/// side places macOS's own lights in a row this tall.
pub const TOP_BAR_H: f64 = 44.0;
/// How far in the first traffic light sits, points (a Mac's).
pub const LIGHTS_LEFT: f64 = 14.0;
/// A traffic light's size and the gap between two, points.
pub const LIGHT: f64 = 12.0;
pub const LIGHT_GAP: f64 = 8.0;
/// How far the lights reach, with the room after them.
pub const LIGHTS_WIDTH: f64 = LIGHTS_LEFT + 3.0 * LIGHT + 2.0 * LIGHT_GAP + 10.0;

/// The window's traffic lights: close, minimise, zoom, red, yellow and
/// green, their glyphs showing while the pointer is over them, grey while
/// the window is not the key window. Nothing in a browser; room only where
/// the system draws its own (macOS).
#[component]
pub fn WindowLights() -> Element {
    let Some(controls) = try_use_context::<WindowControls>() else {
        return rsx! {};
    };
    if let Some(width) = controls.system_lights {
        return rsx! {
            div { class: "as-lights-room", style: "flex: none; width: {width}px; align-self: stretch;" }
        };
    }
    let focused = controls.focused.is_none_or(|f| *f.read());
    let light = |var: &str| {
        let colour = if focused {
            format!("var({var})")
        } else {
            "var(--as-light-off)".to_owned()
        };
        format!(
            "width: {LIGHT}px; height: {LIGHT}px; box-sizing: border-box; border-radius: 50%; \
             background-color: {colour}; display: flex; align-items: center; justify-content: center; \
             box-shadow: inset 0 0 0 0.5px rgba(0, 0, 0, 0.18); flex: none;"
        )
    };
    let (red, yellow, green) = (
        light("--as-light-close"),
        light("--as-light-min"),
        light("--as-light-zoom"),
    );
    let pad = LIGHTS_LEFT;
    rsx! {
        div {
            class: "as-lights",
            "data-focused": "{focused}",
            style: "flex: none; display: flex; align-items: center; gap: {LIGHT_GAP}px; padding: 0 10px 0 {pad}px; align-self: stretch;",
            onpointerdown: move |event| event.stop_propagation(),
            ondoubleclick: move |event| event.stop_propagation(),
            Light { label: "Close", style: red, glyph: icons::X, ink: "#4d0000", on_press: controls.close }
            Light { label: "Minimise", style: yellow, glyph: icons::MINUS, ink: "#5a3c00", on_press: controls.minimize }
            Light { label: "Zoom", style: green, glyph: icons::MAXIMIZE, ink: "#003a00", on_press: controls.toggle_maximize }
        }
    }
}

#[component]
fn Light(
    label: String,
    style: String,
    glyph: Icon,
    ink: String,
    on_press: Option<Callback<()>>,
) -> Element {
    rsx! {
        div {
            role: "button",
            "aria-label": "{label}",
            title: "{label}",
            style: "{style}",
            onclick: move |_| {
                if let Some(f) = on_press {
                    f.call(());
                }
            },
            div { class: "as-glyph", style: "display: flex;",
                IconView { icon: glyph, size: 8.0, color: ink.clone() }
            }
        }
    }
}

/// Empty title-strip space: a press moves the window, a double press
/// zooms it (where the platform has a window of its own).
#[component]
pub fn DragArea(
    #[props(default = "flex: 1 1 0px; min-width: 0;".to_owned())] style: String,
    children: Element,
) -> Element {
    let controls = try_use_context::<WindowControls>();
    let drag = controls.and_then(|c| c.drag);
    let zoom = controls.and_then(|c| c.toggle_maximize);
    rsx! {
        div {
            class: "as-drag",
            style: "{style} align-self: stretch; display: flex; align-items: center;",
            onpointerdown: move |_| {
                if let Some(drag) = drag {
                    drag.call(());
                }
            },
            ondoubleclick: move |_| {
                if let Some(zoom) = zoom {
                    zoom.call(());
                }
            },
            {children}
        }
    }
}

/// Holds an app's slot in the title strip: a press there is the slot's,
/// not a window drag.
#[component]
fn Slot(style: String, children: Element) -> Element {
    rsx! {
        div {
            style: "{style}",
            onpointerdown: move |event| event.stop_propagation(),
            ondoubleclick: move |event| event.stop_propagation(),
            {children}
        }
    }
}

/// The top bar: a macOS unified toolbar.
///
/// From the left: the traffic lights (when no sidebar holds them), the
/// sidebar button, back and forward, the title or the app's `lead` (a
/// breadcrumb); the app's `search` centred; its `actions` at the right
/// (view modes, buttons: [`ToolbarButton`], [`ToolbarGroup`]). Every empty
/// part moves the window. On a phone the row is the menu button (which
/// opens the sidebar as a drawer), back, the title and the actions, with
/// the search on a row of its own under it.
///
/// The shell draws one by default (titled by its `title`); pass your own
/// as the shell's `top_bar` to fill its slots.
#[allow(clippy::too_many_arguments)]
#[component]
pub fn TopBar(
    title: Option<String>,
    /// In place of the title: a breadcrumb, a path, anything.
    lead: Option<Element>,
    /// Centred: a search field ([`SearchField`]) or anything.
    search: Option<Element>,
    /// At the right: buttons and view modes.
    actions: Option<Element>,
    /// A phone's actions in place of `actions` (one or two: a phone's bar
    /// has room for the title, back and little else).
    phone_actions: Option<Element>,
    on_back: Option<EventHandler<()>>,
    on_forward: Option<EventHandler<()>>,
    #[props(default = true)] can_back: bool,
    #[props(default = true)] can_forward: bool,
) -> Element {
    let shell = use_shell();
    let phone = shell.form() == FormFactor::Phone;
    let has_sidebar = shell.has_sidebar();
    let sidebar_open = *shell.sidebar_open.read();
    let tabs = shell.tabs_shown();
    // The lights are the docked sidebar's at its full width; folded to the
    // rail, the top bar runs across the window and holds them.
    let folded = has_sidebar && sidebar_open && !phone && shell.sidebar_folded();
    let lights_here = !(has_sidebar && sidebar_open && !phone) || folded;
    let layout = shell.layout.read();
    let count = layout.tab_count();
    let active_title = layout
        .active
        .as_ref()
        .and_then(|a| layout.pane(a))
        .map(|p| (p.id.clone(), p.title.clone(), p.pinned));
    drop(layout);
    // A phone with tabs titles its bar with the tab on show.
    let title = match (&active_title, phone && tabs) {
        (Some((_, t, _)), true) => t.clone(),
        _ => title.unwrap_or_default(),
    };
    let title_view = rsx! {
        match lead {
            Some(lead) => lead,
            None => rsx! {
                span {
                    class: "as-topbar-title",
                    style: "font-weight: 600; white-space: nowrap; overflow: hidden; min-width: 0;",
                    "{title}"
                }
            },
        }
    };
    let nav = rsx! {
        if let Some(back) = on_back {
            BarButton { icon: icons::ARROW_LEFT, label: "Back", disabled: !can_back, on_press: move |()| back.call(()) }
        }
        if let Some(forward) = on_forward {
            if !phone {
                BarButton { icon: icons::ARROW_RIGHT, label: "Forward", disabled: !can_forward, on_press: move |()| forward.call(()) }
            }
        }
    };
    let tab_tools = rsx! {
        if tabs {
            if phone {
                if let Some((p, _, pinned)) = active_title.clone() {
                    BarButton {
                        icon: icons::MORE,
                        label: "Tab menu",
                        on_press: move |()| shell.menu(16.0, TOP_BAR_H + 4.0, super::stack::tab_menu(&p, pinned)),
                    }
                }
                div {
                    class: "as-hov as-tabs-button",
                    role: "button",
                    title: "All tabs",
                    style: "flex: none; display: flex; align-items: center; justify-content: center; min-width: 26px; height: 24px; padding: 0 5px; border: 2px solid var(--as-text); border-radius: 7px; font-weight: 700; font-size: 12px; box-sizing: border-box; margin: 0 4px;",
                    onpointerdown: move |event| event.stop_propagation(),
                    onclick: move |_| shell.show_switcher(true),
                    "{count}"
                }
            } else {
                div {
                    class: "as-hov as-switcher-button",
                    role: "button",
                    title: "All tabs",
                    style: "flex: none; display: flex; align-items: center; gap: 4px; height: 28px; padding: 0 8px; border-radius: var(--as-radius); color: var(--as-muted);",
                    onpointerdown: move |event| event.stop_propagation(),
                    onclick: move |_| shell.command(Command::OpenSwitcher),
                    IconView { icon: icons::LAYERS }
                    span { "{count}" }
                }
            }
        }
    };
    let sidebar_button = rsx! {
        if has_sidebar {
            BarButton {
                icon: if phone { icons::MENU } else { icons::PANEL_LEFT },
                label: if phone { "Menu" } else { "Sidebar" },
                on_press: move |()| {
                    if phone {
                        shell.set_drawer(true);
                    } else {
                        shell.command(Command::ToggleSidebar);
                    }
                },
            }
        }
    };
    if phone {
        let actions = phone_actions.or(actions);
        let notch = super::foot::top_pad(0.0);
        return rsx! {
            div {
                class: "as-topbar",
                "data-form": "phone",
                style: "flex: none; display: flex; flex-direction: column; background-color: var(--as-topbar-bg); border-bottom: 1px solid var(--as-border); {notch}",
                div {
                    style: "height: var(--as-topbar-h); display: flex; align-items: center; gap: 2px; padding: 0 6px; min-width: 0;",
                    if lights_here { WindowLights {} }
                    {sidebar_button}
                    {nav}
                    DragArea { style: "flex: 1 1 0px; min-width: 0; padding: 0 6px;".to_owned(), {title_view} }
                    if let Some(actions) = actions {
                        Slot { style: "flex: none; display: flex; align-items: center; gap: 2px;".to_owned(), {actions} }
                    }
                    {tab_tools}
                }
                if let Some(search) = search {
                    Slot { style: "padding: 0 10px 8px 10px; display: flex;".to_owned(), {search} }
                }
            }
        };
    }
    rsx! {
        div {
            class: "as-topbar",
            "data-form": "desktop",
            style: "flex: none; height: var(--as-topbar-h); display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, auto) minmax(0, 1fr); align-items: center; background-color: var(--as-topbar-bg); border-bottom: 1px solid var(--as-border); min-width: 0;",
            DragArea {
                style: "min-width: 0; height: 100%; gap: 2px; padding-left: 6px; overflow: hidden;".to_owned(),
                if lights_here { WindowLights {} }
                {sidebar_button}
                {nav}
                div { style: "width: 6px; flex: none;" }
                Slot { style: "min-width: 0; display: flex; align-items: center; overflow: hidden;".to_owned(), {title_view} }
            }
            if let Some(search) = search {
                Slot { style: "min-width: 0; display: flex; align-items: center; padding: 0 12px;".to_owned(), {search} }
            } else {
                DragArea { style: "height: 100%;".to_owned() }
            }
            DragArea {
                style: "min-width: 0; height: 100%; justify-content: flex-end; gap: 2px; padding-right: 8px;".to_owned(),
                if let Some(actions) = actions {
                    Slot { style: "flex: none; display: flex; align-items: center; gap: 2px;".to_owned(), {actions} }
                }
                {tab_tools}
            }
        }
    }
}

/// A square icon button in a bar.
#[component]
pub fn BarButton(
    icon: Icon,
    label: String,
    on_press: EventHandler<()>,
    #[props(default)] disabled: bool,
) -> Element {
    let opacity = if disabled { "0.35" } else { "1" };
    rsx! {
        div {
            class: "as-hov",
            role: "button",
            title: "{label}",
            "aria-label": "{label}",
            "aria-disabled": "{disabled}",
            style: "flex: none; display: flex; align-items: center; justify-content: center; width: 28px; height: 28px; border-radius: var(--as-radius); color: var(--as-muted); opacity: {opacity};",
            onpointerdown: move |event| event.stop_propagation(),
            ondoubleclick: move |event| event.stop_propagation(),
            onclick: move |_| {
                if !disabled {
                    on_press.call(());
                }
            },
            IconView { icon, size: 16.0 }
        }
    }
}

/// A toolbar button for the top bar's actions: an icon, optionally a
/// label, lit when `active` (a view mode that is on).
#[component]
pub fn ToolbarButton(
    icon: Icon,
    label: String,
    #[props(default)] show_label: bool,
    #[props(default)] active: bool,
    on_press: EventHandler<()>,
) -> Element {
    let (bg, fg) = if active {
        ("var(--as-selected)", "var(--as-selected-text)")
    } else {
        ("transparent", "var(--as-muted)")
    };
    rsx! {
        div {
            class: "as-hov as-tool",
            role: "button",
            title: "{label}",
            "aria-label": "{label}",
            "aria-pressed": "{active}",
            style: "flex: none; display: flex; align-items: center; gap: 6px; height: 28px; min-width: 28px; justify-content: center; padding: 0 6px; box-sizing: border-box; border-radius: var(--as-radius); background-color: {bg}; color: {fg};",
            onpointerdown: move |event| event.stop_propagation(),
            onclick: move |_| on_press.call(()),
            IconView { icon, size: 16.0 }
            if show_label {
                span { style: "white-space: nowrap;", "{label}" }
            }
        }
    }
}

/// Toolbar buttons joined in one rounded group (a view-mode switch).
#[component]
pub fn ToolbarGroup(children: Element) -> Element {
    rsx! {
        div {
            class: "as-tool-group",
            style: "flex: none; display: flex; align-items: center; gap: 1px; padding: 2px; border-radius: calc(var(--as-radius) + 2px); background-color: var(--as-search-bg);",
            {children}
        }
    }
}

/// A search field for the top bar's centre: a rounded pill with a glass.
#[component]
pub fn SearchField(
    #[props(default)] value: String,
    #[props(default = "Search".to_owned())] placeholder: String,
    on_input: Option<EventHandler<String>>,
    on_submit: Option<EventHandler<String>>,
) -> Element {
    rsx! {
        div {
            class: "as-search",
            style: "display: flex; align-items: center; gap: 6px; flex: 1 1 340px; width: 340px; min-width: 0; max-width: 420px; height: 28px; padding: 0 10px; box-sizing: border-box; border-radius: 8px; background-color: var(--as-search-bg); border: 1px solid var(--as-border); color: var(--as-muted);",
            IconView { icon: icons::SEARCH, size: 14.0 }
            input {
                r#type: "text",
                value: "{value}",
                placeholder: "{placeholder}",
                "aria-label": "{placeholder}",
                style: "flex: 1 1 0px; min-width: 0; border: none; outline: none; background-color: transparent; color: var(--as-text); font-family: var(--as-font); font-size: var(--as-font-size); padding: 0;",
                oninput: move |event| {
                    if let Some(f) = on_input {
                        f.call(event.value());
                    }
                },
                onkeydown: move |event| {
                    if event.key().to_string() == "Enter"
                        && let Some(f) = on_submit
                    {
                        f.call(value.clone());
                    }
                },
            }
        }
    }
}
