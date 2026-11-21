//! The tab switcher: every tab of a place as a card, over the window.
//! A phone's way between tabs, and a desktop's overview (Mod+Shift+E).

use dioxus::prelude::*;

use super::icons::{self, IconView};
use super::shell::use_shell;
use crate::switcher::Place;

/// The switcher, filling the shell.
#[component]
pub fn Switcher() -> Element {
    let shell = use_shell();
    let layout = shell.layout.read();
    let places = layout.places();
    let active = layout.active.clone();
    let start = active
        .as_ref()
        .and_then(|a| layout.place_of(a))
        .unwrap_or(Place::Main);
    drop(layout);
    let mut place = use_signal(|| start);
    let here = if places.contains(&place()) {
        place()
    } else {
        Place::Main
    };
    let layout = shell.layout.read();
    let cards: Vec<(String, String, String, bool)> = layout
        .cards(here)
        .into_iter()
        .filter_map(|c| {
            let p = layout.pane(&c.pane)?;
            Some((
                p.id.0.clone(),
                p.title.clone(),
                p.view.0.clone(),
                active.as_ref() == Some(&p.id),
            ))
        })
        .collect();
    drop(layout);
    let n = cards.len();
    let plural = if n == 1 { "tab" } else { "tabs" };
    rsx! {
        div {
            class: "as-switcher",
            style: "position: absolute; inset: 0; z-index: 50; display: flex; flex-direction: column; background-color: var(--as-bg);",
            div {
                style: "flex: none; display: flex; align-items: center; gap: 8px; padding: 10px 12px; border-bottom: 1px solid var(--as-border); background-color: var(--as-surface); min-width: 0;",
                span { style: "font-weight: 700; font-size: 15px; white-space: nowrap;", "{n} {plural}" }
                div {
                    style: "flex: 1 1 0px; min-width: 0; display: flex; gap: 6px; overflow: hidden;",
                    for p in places.clone() {
                        div {
                            class: "as-hov",
                            style: "flex: none; padding: 4px 10px; border-radius: 999px; white-space: nowrap; border: 1px solid {chip(p == here)};",
                            onclick: move |_| place.set(p),
                            "{p.name()}"
                        }
                    }
                }
                div {
                    class: "as-hov",
                    role: "button",
                    style: "flex: none; padding: 4px 12px; border-radius: var(--as-radius); font-weight: 600; color: var(--as-accent);",
                    onclick: move |_| shell.show_switcher(false),
                    "Done"
                }
            }
            div {
                class: "as-cards",
                style: "flex: 1 1 0px; min-height: 0; overflow-y: scroll; padding: 16px; display: grid; grid-template-columns: repeat(auto-fill, minmax(140px, 1fr)); grid-auto-rows: max-content; gap: 18px; align-content: start;",
                for (id, title, view, is_active) in cards {
                    Card { key: "{id}", id, title, view, active: is_active }
                }
            }
        }
    }
}

const fn chip(on: bool) -> &'static str {
    if on {
        "var(--as-accent)"
    } else {
        "var(--as-border)"
    }
}

#[component]
fn Card(id: String, title: String, view: String, active: bool) -> Element {
    let shell = use_shell();
    let kind = shell.kind(&view);
    let icon = kind.as_ref().map_or(icons::FILE, |k| k.icon);
    let kind_name = kind.map_or_else(|| view.clone(), |k| k.title);
    let border = if active {
        "var(--as-accent)"
    } else {
        "var(--as-border)"
    };
    let pick = id.clone();
    let close = id.clone();
    rsx! {
        div {
            class: "as-card",
            "data-pane": "{id}",
            style: "display: flex; flex-direction: column; gap: 6px; min-width: 0;",
            div {
                style: "position: relative; aspect-ratio: 3 / 4; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; border: 2px solid {border}; border-radius: 10px; background-color: var(--as-surface); color: var(--as-muted); overflow: hidden;",
                onclick: move |_| {
                    let p = crate::model::PaneId::new(pick.clone());
                    shell.apply(|w| w.activate(&p));
                    shell.show_switcher(false);
                },
                IconView { icon, size: 28.0 }
                span { style: "font-size: 11px; white-space: nowrap; overflow: hidden; max-width: 90%;", "{kind_name}" }
                div {
                    class: "as-x",
                    title: "Close",
                    style: "position: absolute; top: 6px; right: 6px; width: 22px; height: 22px; display: flex; align-items: center; justify-content: center; border-radius: 999px; background-color: var(--as-raised); color: var(--as-text);",
                    onclick: move |event| {
                        event.stop_propagation();
                        let p = crate::model::PaneId::new(close.clone());
                        shell.apply(|w| w.close(&p));
                    },
                    IconView { icon: icons::X, size: 12.0 }
                }
            }
            span { style: "text-align: center; white-space: nowrap; overflow: hidden; min-width: 0;", "{title}" }
        }
    }
}
