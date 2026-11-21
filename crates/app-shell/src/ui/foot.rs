//! A phone's foot: the app's accessory (Now Playing, a transport, an
//! upload's progress) docked over a floating tab pill of the app's primary
//! destinations and an optional round primary-action button, clear of the
//! home indicator. On a desktop only the accessory shows, docked under the
//! content; the sidebar holds the destinations.

use dioxus::prelude::*;

use super::icons::{Icon, IconView};
use super::shell::use_shell;

/// One of the app's primary destinations: a tab of the phone's pill.
#[derive(Debug, Clone, PartialEq)]
pub struct Destination {
    /// What `on_destination` is told.
    pub id: String,
    pub label: String,
    pub icon: Icon,
    /// A count or a dot's text, shown over the icon.
    pub badge: Option<String>,
}

impl Destination {
    pub fn new(id: impl Into<String>, label: impl Into<String>, icon: Icon) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon,
            badge: None,
        }
    }

    #[must_use]
    pub fn badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }
}

/// The round button beside the pill: the app's primary action (Upload,
/// New, Play).
#[derive(Clone, PartialEq)]
pub struct PrimaryAction {
    pub label: String,
    pub icon: Icon,
    pub on_press: Callback<()>,
    /// Filled with the accent (a primary action), or the pill's colour.
    pub prominent: bool,
}

impl PrimaryAction {
    pub fn new(label: impl Into<String>, icon: Icon, on_press: impl FnMut(()) + 'static) -> Self {
        Self {
            label: label.into(),
            icon,
            on_press: Callback::new(on_press),
            prominent: true,
        }
    }
}

/// Padding that clears the home indicator: `px` where the platform has
/// no safe area (Blitz already keeps its viewport inside it), the
/// browser's inset when larger. Written twice, so a renderer that cannot
/// read `env()` keeps the first.
pub(crate) fn bottom_pad(px: f64, less: f64) -> String {
    format!(
        "padding-bottom: {px}px; padding-bottom: max({px}px, calc(env(safe-area-inset-bottom, 0px) - {less}px));"
    )
}

/// The same for the top (a notch, the status bar).
pub(crate) fn top_pad(px: f64) -> String {
    format!("padding-top: {px}px; padding-top: calc(env(safe-area-inset-top, 0px) + {px}px);")
}

/// A phone's foot: the accessory over the pill and the round button.
#[component]
pub(crate) fn PhoneFoot(
    accessory: Option<Element>,
    destinations: Vec<Destination>,
    current: Option<String>,
    on_destination: Option<EventHandler<String>>,
    action: Option<PrimaryAction>,
) -> Element {
    if accessory.is_none() && destinations.is_empty() && action.is_none() {
        return rsx! {};
    }
    let pad = bottom_pad(12.0, 4.0);
    rsx! {
        div {
            class: "as-foot",
            style: "flex: none; display: flex; flex-direction: column; background-color: var(--as-bg);",
            if let Some(acc) = accessory {
                div {
                    class: "as-accessory",
                    style: "margin: 6px 12px 0 12px; border-radius: 16px; background-color: var(--as-raised); border: 1px solid var(--as-border); box-shadow: 0 6px 20px rgba(0, 0, 0, 0.45); overflow: hidden; min-height: 0;",
                    {acc}
                }
            }
            if !destinations.is_empty() || action.is_some() {
                div {
                    class: "as-tabrow",
                    style: "display: flex; align-items: flex-end; gap: 8px; padding: 6px 16px 0 16px; {pad}",
                    if !destinations.is_empty() {
                        TabPill { destinations, current, on_destination }
                    } else {
                        div { style: "flex: 1 1 0px;" }
                    }
                    if let Some(action) = action {
                        RoundButton { action }
                    }
                }
            }
        }
    }
}

/// The floating pill of destinations, each an icon over its label.
#[component]
fn TabPill(
    destinations: Vec<Destination>,
    current: Option<String>,
    on_destination: Option<EventHandler<String>>,
) -> Element {
    let shell = use_shell();
    rsx! {
        div {
            class: "as-pill",
            role: "tablist",
            style: "flex: 1 1 0px; min-width: 0; height: 54px; box-sizing: border-box; display: flex; align-items: center; gap: 2px; padding: 4px; border-radius: 27px; background-color: var(--as-raised); border: 1px solid var(--as-border); box-shadow: 0 6px 20px rgba(0, 0, 0, 0.45);",
            for d in destinations {
                {
                    let on = current.as_deref() == Some(d.id.as_str());
                    let (bg, fg) = if on { ("var(--as-selected)", "var(--as-text)") } else { ("transparent", "var(--as-muted)") };
                    let id = d.id.clone();
                    rsx! {
                        div {
                            key: "{d.id}",
                            class: "as-pill-tab",
                            role: "tab",
                            "aria-selected": "{on}",
                            "aria-label": "{d.label}",
                            style: "position: relative; flex: 1 1 0px; min-width: 0; height: 46px; border-radius: 23px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 2px; background-color: {bg}; color: {fg};",
                            onclick: move |_| {
                                shell.close_overlays();
                                if let Some(f) = on_destination {
                                    f.call(id.clone());
                                }
                            },
                            IconView { icon: d.icon, size: 20.0 }
                            span { style: "font-size: 10px; font-weight: 650; line-height: 1; white-space: nowrap; overflow: hidden; max-width: 100%;", "{d.label}" }
                            if let Some(b) = d.badge {
                                span {
                                    class: "as-pill-badge",
                                    style: "position: absolute; top: 3px; left: 50%; margin-left: 6px; min-width: 16px; height: 16px; padding: 0 4px; box-sizing: border-box; border-radius: 8px; background-color: var(--as-accent); color: #ffffff; font-size: 10px; font-weight: 700; display: flex; align-items: center; justify-content: center;",
                                    "{b}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The round primary-action button.
#[component]
fn RoundButton(action: PrimaryAction) -> Element {
    let (bg, fg) = if action.prominent {
        ("var(--as-accent)", "#ffffff")
    } else {
        ("var(--as-raised)", "var(--as-text)")
    };
    let press = action.on_press;
    rsx! {
        div {
            class: "as-round",
            role: "button",
            "aria-label": "{action.label}",
            title: "{action.label}",
            style: "flex: none; width: 54px; height: 54px; border-radius: 27px; display: flex; align-items: center; justify-content: center; background-color: {bg}; color: {fg}; border: 1px solid var(--as-border); box-shadow: 0 6px 20px rgba(0, 0, 0, 0.45);",
            onclick: move |_| press.call(()),
            IconView { icon: action.icon, size: 22.0 }
        }
    }
}

/// A desktop's accessory: docked under the content.
#[component]
pub(crate) fn DeskAccessory(accessory: Element) -> Element {
    rsx! {
        div {
            class: "as-accessory",
            style: "flex: none; border-top: 1px solid var(--as-border); background-color: var(--as-surface);",
            {accessory}
        }
    }
}
