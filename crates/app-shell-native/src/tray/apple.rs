//! The tray on macOS (the menu bar) and Windows (the notification area),
//! through tray-icon and its menus (muda). A template icon on macOS takes
//! the menu bar's colour, light or dark. A left click opens the menu
//! (macOS's way) unless the app has a quick panel, which it then opens.

use tray_icon::menu::{
    CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem as MudaItem, PredefinedMenuItem, Submenu,
};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use super::model::{MenuEntry, MenuItem};
use super::{Backend, Events, TrayEvent, TrayState};

/// A muda accelerator from the model's, where it parses.
fn accelerator(item: &MenuItem) -> Option<tray_icon::menu::accelerator::Accelerator> {
    item.accelerator
        .as_ref()
        .and_then(|a| a.to_muda().parse().ok())
}

fn build(entries: &[MenuEntry]) -> Vec<Box<dyn IsMenuItem>> {
    entries
        .iter()
        .map(|e| -> Box<dyn IsMenuItem> {
            match e {
                MenuEntry::Separator => Box::new(PredefinedMenuItem::separator()),
                MenuEntry::Submenu {
                    label,
                    enabled,
                    items,
                } => {
                    let sub = Submenu::new(label, *enabled);
                    for child in build(items) {
                        let _ = sub.append(child.as_ref());
                    }
                    Box::new(sub)
                }
                // muda has no second line: the detail follows the label.
                MenuEntry::Item(item) => match item.checked {
                    Some(checked) => Box::new(CheckMenuItem::with_id(
                        item.id.clone(),
                        item.one_line(),
                        item.enabled,
                        checked,
                        accelerator(item),
                    )),
                    None => Box::new(MudaItem::with_id(
                        item.id.clone(),
                        item.one_line(),
                        item.enabled,
                        accelerator(item),
                    )),
                },
            }
        })
        .collect()
}

fn menu(state: &TrayState) -> Menu {
    let m = Menu::new();
    for item in build(&state.menu.entries) {
        let _ = m.append(item.as_ref());
    }
    m
}

fn icon(state: &TrayState) -> Option<Icon> {
    let i = state.drawn_icon();
    Icon::from_rgba(i.rgba, i.width, i.height).ok()
}

fn tooltip(state: &TrayState) -> String {
    let (title, second) = state.tooltip_lines();
    format!("{title}\n{second}")
}

struct Tray {
    icon: TrayIcon,
}

impl Backend for Tray {
    fn update(&mut self, state: &TrayState) {
        self.icon.set_menu(Some(Box::new(menu(state))));
        // A badge has a colour: then no longer a template.
        let _ = if state.drawn_icon().template {
            self.icon.set_icon_templated(icon(state))
        } else {
            self.icon.set_icon(icon(state))
        };
        let _ = self.icon.set_tooltip(Some(tooltip(state)));
    }
}

/// The tray, its clicks and its menu's sent to the loop.
pub(super) fn start(state: &TrayState, events: Events, panel: bool) -> Option<Box<dyn Backend>> {
    let menu_events = events.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        menu_events.send(TrayEvent::Menu(e.id().0.clone()));
    }));
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if let TrayIconEvent::Click {
            rect,
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = e
        {
            #[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
            let (x, y) = (
                (rect.position.x + f64::from(rect.size.width) / 2.0) as i32,
                (rect.position.y + f64::from(rect.size.height)) as i32,
            );
            events.send(TrayEvent::Activate { x, y });
        }
    }));
    let built = TrayIconBuilder::new()
        .with_id(state.id.clone())
        .with_menu(Box::new(menu(state)))
        .with_tooltip(tooltip(state))
        // A left click opens the menu, macOS's way, unless the app has a
        // quick panel: then the click is the panel's.
        .with_menu_on_left_click(!panel);
    let built = match icon(state) {
        Some(i) if state.drawn_icon().template => built.with_icon_templated(i),
        Some(i) => built.with_icon(i),
        None => built,
    };
    match built.build() {
        Ok(icon) => Some(Box::new(Tray { icon })),
        Err(error) => {
            tracing::warn!(app_shell.tray_error = %error, "the tray could not start");
            None
        }
    }
}
