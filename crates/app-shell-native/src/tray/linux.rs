//! The tray on Linux: a StatusNotifierItem over D-Bus (ksni), its menu a
//! `com.canonical.dbusmenu`. KDE Plasma shows it natively; GNOME with the
//! AppIndicator extension; most panels (waybar, xfce4-panel, …).
//!
//! ksni runs on a thread of its own; a click is sent to the event loop
//! with a wake.

use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{CheckmarkItem, StandardItem, SubMenu};

use super::model::{MenuEntry, MenuItem, TrayStatus};
use super::{Backend, Events, TrayState};
use crate::tray::TrayEvent;

struct Sni {
    state: TrayState,
    events: Events,
}

/// A menu row's keyboard equivalent as dbusmenu carries it
/// (`[["Control", "Shift", "q"]]`).
fn shortcut(item: &MenuItem) -> Vec<Vec<String>> {
    let Some(a) = &item.accelerator else {
        return Vec::new();
    };
    let mut keys = Vec::new();
    if a.primary || a.ctrl {
        keys.push("Control".to_owned());
    }
    if a.alt {
        keys.push("Alt".to_owned());
    }
    if a.shift {
        keys.push("Shift".to_owned());
    }
    keys.push(a.key.to_lowercase());
    vec![keys]
}

fn entries(list: &[MenuEntry]) -> Vec<ksni::MenuItem<Sni>> {
    list.iter()
        .map(|e| match e {
            MenuEntry::Separator => ksni::MenuItem::Separator,
            MenuEntry::Submenu {
                label,
                enabled,
                items,
            } => SubMenu {
                label: label.clone(),
                enabled: *enabled,
                submenu: entries(items),
                ..Default::default()
            }
            .into(),
            MenuEntry::Item(item) => {
                let id = item.id.clone();
                // dbusmenu has no second line: the detail follows the label.
                let label = item.one_line();
                match item.checked {
                    Some(checked) => CheckmarkItem {
                        label,
                        enabled: item.enabled,
                        checked,
                        shortcut: shortcut(item),
                        activate: Box::new(move |t: &mut Sni| {
                            t.events.send(TrayEvent::Menu(id.clone()))
                        }),
                        ..Default::default()
                    }
                    .into(),
                    None => StandardItem {
                        label,
                        enabled: item.enabled,
                        shortcut: shortcut(item),
                        activate: Box::new(move |t: &mut Sni| {
                            t.events.send(TrayEvent::Menu(id.clone()))
                        }),
                        ..Default::default()
                    }
                    .into(),
                }
            }
        })
        .collect()
}

impl ksni::Tray for Sni {
    // A click on the icon is the app's (its panel, or an event), not the menu.
    const MENU_ON_ACTIVATE: bool = false;

    fn id(&self) -> String {
        self.state.id.clone()
    }

    fn title(&self) -> String {
        self.state.title.clone()
    }

    fn status(&self) -> ksni::Status {
        match self.state.status {
            TrayStatus::Warning | TrayStatus::Error => ksni::Status::NeedsAttention,
            _ => ksni::Status::Active,
        }
    }

    #[allow(clippy::cast_possible_wrap, clippy::as_conversions)]
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let icon = self.state.drawn_icon();
        vec![ksni::Icon {
            width: icon.width as i32,
            height: icon.height as i32,
            data: icon.to_argb(),
        }]
    }

    // A panel that shows the attention icon in NeedsAttention shows the
    // same picture, badge and all.
    fn attention_icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icon_pixmap()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let (title, description) = self.state.tooltip_lines();
        ksni::ToolTip {
            title,
            description,
            ..Default::default()
        }
    }

    fn activate(&mut self, x: i32, y: i32) {
        self.events.send(TrayEvent::Activate { x, y });
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        entries(&self.state.menu.entries)
    }

    fn watcher_offline(&self, reason: ksni::OfflineReason) -> bool {
        tracing::warn!(app_shell.tray_offline = ?reason, "the StatusNotifierWatcher went away; waiting for it");
        true
    }
}

struct SniBackend(Handle<Sni>);

impl Backend for SniBackend {
    fn update(&mut self, state: &TrayState) {
        let state = state.clone();
        self.0.update(move |t| t.state = state);
    }
}

/// The tray, or `None` where the session has no StatusNotifierWatcher yet
/// and will not get one (it waits for one that comes later).
pub(super) fn start(state: &TrayState, events: Events, _panel: bool) -> Option<Box<dyn Backend>> {
    let sni = Sni {
        state: state.clone(),
        events,
    };
    match sni.assume_sni_available(true).spawn() {
        Ok(handle) => Some(Box::new(SniBackend(handle))),
        Err(error) => {
            tracing::warn!(app_shell.tray_error = %error, "the tray could not start");
            None
        }
    }
}
