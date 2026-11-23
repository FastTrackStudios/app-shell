//! What loaded extensions add to the shell's chrome, as app-shell has it.
//!
//! - Their **tabs** (`[[tabs]]`) are the app's primary destinations
//!   ([`ExtensionUi::destinations`]): the sidebar's on a desk, the pill's
//!   on a phone. The app hands them to `AppShell { destinations }` with its
//!   own and asks [`ExtensionUi::tab`] what a picked one is.
//! - Their **status items** and a phone's **bar buttons** are items of the
//!   top bar ([`ExtensionUi::bar_items`]): a status item on the left side
//!   leads, the rest trail.
//! - The **slots** they fill: `title` takes over the bar's middle and
//!   `tabs` its center ([`ExtensionUi::bar_fillers`]); `status` is a
//!   trailing item; `left`, `right`, `ribbon` and `now-playing` are the
//!   app's to place (its sidebars, its accessory), drawn by
//!   [`ExtensionUi::slot`].
//!
//! [`use_extension_bar`], called in any component under `AppShell`, keeps
//! the bar's items and fillers what the extensions say.

use std::collections::BTreeSet;

use app_shell::ui::{
    BarButton, BarFiller, BarItem, BarPlace, BarSpan, Destination, IconView, named_icon, use_shell,
};
use architect::action::{Invocation, InvocationSource};
use dioxus::prelude::*;

use crate::interface::{ExtensionUi, ExtensionView};
use crate::panes::run_action;
use architect_extensions::contrib;
use architect_extensions::host::now_ms;

/// The owner the extensions' bar fillers are put under.
const OWNER: &str = "ext";

/// Where the bar's items an extension adds go, after the app's own.
const ORDER: i32 = 100;

/// A shell slot extensions fill, by its name in a manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Slot {
    /// A desk's ribbon at the window's left edge.
    Ribbon,
    /// The left sidebar's content.
    Left,
    /// The right side's content.
    Right,
    /// The page's tabs in a desk's title row: the bar's center.
    Tabs,
    /// A desk's status bar: a trailing item of the bar.
    Status,
    /// A desk's title row: the bar's middle.
    Title,
    /// The Now Playing bar at the page's foot.
    NowPlaying,
}

impl Slot {
    /// The slot named `name` in a manifest (`now-playing`).
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Some(match name {
            "ribbon" => Self::Ribbon,
            "left" => Self::Left,
            "right" => Self::Right,
            "tabs" => Self::Tabs,
            "status" => Self::Status,
            "title" => Self::Title,
            "now-playing" | "now_playing" | "nowplaying" => Self::NowPlaying,
            _ => return None,
        })
    }
}

impl ExtensionUi {
    /// The extensions' tabs as the app's destinations, each id
    /// `<extension>/<tab>` ([`Self::tab`] says which).
    #[must_use]
    pub fn destinations(&self) -> Vec<Destination> {
        let _ = self.revision();
        self.host()
            .contributions()
            .tabs()
            .into_iter()
            .map(|by| {
                Destination::new(
                    format!("{}/{}", by.owner, by.item.id),
                    by.item.title.clone(),
                    named_icon(&by.item.icon),
                )
            })
            .collect()
    }

    /// The extensions' status items and bar buttons, and the `status`
    /// slot's view, as items of the top bar.
    #[must_use]
    pub fn bar_items(&self) -> Vec<BarItem> {
        let _ = self.revision();
        let me = *self;
        let host = self.host();
        let reg = host.contributions();
        let mut items = Vec::new();
        for (n, by) in reg.status(now_ms()).into_iter().enumerate() {
            let s = by.item;
            let id = format!("ext:{}/{}", by.owner, s.id);
            let place = if s.side == contrib::StatusSide::Left {
                BarPlace::Leading
            } else {
                BarPlace::Trailing
            };
            let order = ORDER.saturating_add(i32::try_from(n).unwrap_or(i32::MAX));
            let (text, icon, hint, action) = (s.text, s.icon, s.hint, s.action);
            items.push(BarItem::new(id, place, order, move || {
                rsx! {
                    StatusItem {
                        ui: me,
                        text: text.clone(),
                        icon: icon.clone(),
                        hint: hint.clone(),
                        action: action.clone(),
                    }
                }
            }));
        }
        for (n, by) in reg.bar().into_iter().enumerate() {
            let action = by.item.action.clone();
            let label = if by.item.title.is_empty() {
                action.clone()
            } else {
                by.item.title.clone()
            };
            let icon = named_icon(&by.item.icon);
            let order = ORDER
                .saturating_add(50)
                .saturating_add(i32::try_from(n).unwrap_or(i32::MAX));
            items.push(BarItem::new(
                format!("ext:bar:{}/{action}", by.owner),
                BarPlace::Trailing,
                order,
                move || {
                    let action = action.clone();
                    rsx! {
                        BarButton {
                            icon,
                            label: label.clone(),
                            on_press: move |()| {
                                run_action(me.host().list(), &action, Invocation::new(InvocationSource::App));
                            },
                        }
                    }
                },
            ));
        }
        for (slot, (owner, view)) in reg.slots() {
            if Slot::named(&slot) != Some(Slot::Status) {
                continue;
            }
            items.push(BarItem::new(
                format!("ext:slot:{owner}/{view}"),
                BarPlace::Trailing,
                ORDER.saturating_add(90),
                move || {
                    rsx! { ExtensionView { ui: me, extension: owner.clone(), view: view.clone(), fill: false } }
                },
            ));
        }
        items
    }

    /// The bar's spans extensions take over: the `title` slot's view the
    /// middle, the `tabs` slot's the center.
    #[must_use]
    pub fn bar_fillers(&self) -> Vec<BarFiller> {
        let _ = self.revision();
        let me = *self;
        self.host()
            .contributions()
            .slots()
            .into_iter()
            .filter_map(|(slot, (owner, view))| {
                let span = match Slot::named(&slot)? {
                    Slot::Title => BarSpan::Middle,
                    Slot::Tabs => BarSpan::Center,
                    _ => return None,
                };
                Some(BarFiller {
                    owner: OWNER.to_owned(),
                    span,
                    render: std::rc::Rc::new(move || {
                        rsx! { ExtensionView { ui: me, extension: owner.clone(), view: view.clone(), fill: false } }
                    }),
                })
            })
            .collect()
    }

    /// The view an extension fills `slot` with, drawn to its content's
    /// size: for the app to place (`left` in its sidebar, `now-playing`
    /// as its accessory). `None` when none fills it.
    #[must_use]
    pub fn slot(&self, slot: Slot) -> Option<Element> {
        let _ = self.revision();
        let me = *self;
        let (owner, view) = self
            .host()
            .contributions()
            .slots()
            .into_iter()
            .find(|(name, _)| Slot::named(name) == Some(slot))
            .map(|(_, by)| by)?;
        Some(rsx! { ExtensionView { ui: me, extension: owner, view, fill: false } })
    }
}

/// Keeps the top bar's items and fillers what the extensions say, as they
/// load and go, and hands `ui` the shell. Call it in a component under
/// `AppShell` (the app's top bar, its sidebar).
pub fn use_extension_bar(ui: ExtensionUi) {
    let shell = use_shell();
    ui.attach(shell);
    let mut put = use_hook(|| CopyValue::new(BTreeSet::<String>::new()));
    use_effect(move || {
        let items = ui.bar_items();
        let fillers = ui.bar_fillers();
        let now: BTreeSet<String> = items.iter().map(|i| i.id.clone()).collect();
        for gone in put.peek().difference(&now) {
            shell.remove_bar_item(gone);
        }
        for item in items {
            shell.put_bar_item(item);
        }
        for span in [BarSpan::Center, BarSpan::Middle] {
            shell.unfill_bar(OWNER, span);
        }
        for filler in fillers {
            shell.fill_bar(filler);
        }
        put.set(now);
    });
}

/// A status item in the bar: its icon and text, its hint as its title, a
/// press running its action.
#[component]
fn StatusItem(
    ui: ExtensionUi,
    text: String,
    icon: String,
    hint: String,
    action: String,
) -> Element {
    let pressable = !action.is_empty();
    let cursor = if pressable { "pointer" } else { "default" };
    rsx! {
        div {
            class: if pressable { "as-hov" } else { "" },
            role: if pressable { "button" } else { "status" },
            title: "{hint}",
            style: "flex: none; display: flex; align-items: center; gap: 4px; height: 24px; padding: 0 6px; border-radius: var(--as-radius); font-size: 12px; color: var(--as-muted); cursor: {cursor}; white-space: nowrap;",
            onpointerdown: move |e| e.stop_propagation(),
            onclick: move |_| {
                if pressable {
                    run_action(ui.host().list(), &action, Invocation::new(InvocationSource::App));
                }
            },
            if !icon.is_empty() {
                IconView { icon: named_icon(&icon), size: 13.0 }
            }
            span { "{text}" }
        }
    }
}
