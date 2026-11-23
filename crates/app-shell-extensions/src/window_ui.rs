//! Extensions' windows in the app's window (feature `ui`): the window
//! machinery the core keeps (`docs/content/architecture/extensions.md`,
//! "Windows, points and services").
//!
//! Each open `[[windows]]` (`architect_extensions::windows`) is drawn by
//! app-shell: a `sheet` as the shell's sheet ([`ExtensionWindows`]: a
//! card over the window on a desk, from the foot on a phone) holding the
//! extension's view; a `window` as a window of the layout
//! ([`use_window_panes`]): a pane of its view, popped out, which
//! app-shell opens as an OS window where the platform has them
//! (`app_shell::ui::Windows`, app-shell-native) and draws in the page
//! where it has not. The pop-out's root is the app's, so it draws the view
//! with the main window's tab kinds ([`ExtensionPopout`] does it all).
//! Either way the view is drawn as any of its views is ([`ExtensionView`]:
//! linked in, a process's or a WASI component's remote document, a page).
//!
//! [`use_settings_window`] joins the app's settings (`crate::settings`'s
//! [`Settings`](crate::settings::Settings)) to the settings extension's window: the app's own
//! sections as its items of `settings.sections`, `Settings::show`
//! opening the window that `APP_SETTINGS` opens, its closing closing the
//! page, and the values for a sheet drawn out of process
//! (`APP_SETTINGS_VALUES`, `APP_SETTINGS_SET`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

use crate::settings::{
    Owner, json_of as setting_json, try_use_settings, value_of as setting_value,
};
use app_shell::ui::{SheetSpec, use_shell};
use app_shell::{Pane, PaneId, Rect, Target};
use architect::action::{ActionCaller, ActionError, ActionFuture, ActionInfo, Effect};
use dioxus::prelude::*;
use futures::StreamExt as _;
use futures::channel::mpsc;
use serde_json::{Value, json};

use crate::interface::{ExtensionUi, ExtensionView};
use architect_extensions::host::Host;
use architect_extensions::interop::{SETTINGS_SECTIONS, WindowKind};
use architect_extensions::windows::OpenWindow;

/// The action that opens the settings page.
pub const APP_SETTINGS: &str = "APP_SETTINGS";

/// The open windows, as a signal that moves when one opens or closes.
pub(crate) fn use_open_windows(host: &Host) -> Signal<Vec<OpenWindow>> {
    let mut open = use_signal(|| host.open_windows());
    let h = host.clone();
    use_future(move || {
        let h = h.clone();
        async move {
            let (tx, mut rx) = mpsc::unbounded::<()>();
            h.on_windows(move || {
                let _ = tx.unbounded_send(());
            });
            // What opened before it listened.
            let now = h.open_windows();
            if *open.peek() != now {
                open.set(now);
            }
            while rx.next().await.is_some() {
                while rx.try_recv().is_ok() {}
                open.set(h.open_windows());
            }
        }
    });
    open
}

/// The prefix of the pane an extension's OS window is (`ext-window:`
/// then its key).
pub const WINDOW_PANE: &str = "ext-window:";

/// The pane extension window `key` is in, when it is a window.
#[must_use]
pub fn window_pane(key: &str) -> PaneId {
    PaneId::new(format!("{WINDOW_PANE}{key}"))
}

/// Every open extension sheet, as the shell's sheets: mount it once under
/// the shell (`ExtensionOverlays` does, in `AppShell`'s `layers`). Its windows of kind `window` are
/// windows of the layout ([`use_window_panes`]), not drawn here.
#[component]
pub fn ExtensionWindows(ui: ExtensionUi) -> Element {
    let open = ui.windows();
    rsx! {
        for w in open {
            if w.window.kind != WindowKind::Window {
                ExtensionSheet { key: "{w.key()}", ui, open: w }
            }
        }
    }
}

/// A sheet: the shell's own (a card over the window on a desk, a sheet
/// from the foot on a phone) holding the extension's view, open while the
/// window is; the person closing it closes it in the host.
#[component]
fn ExtensionSheet(ui: ExtensionUi, open: OpenWindow) -> Element {
    let shell = use_shell();
    let sheet = use_hook(|| CopyValue::new(None::<app_shell::ui::SheetId>));
    use_hook(move || {
        let (ext, view) = (open.extension.clone(), open.window.view.clone());
        let (close_ext, close_id) = (open.extension.clone(), open.window.id.clone());
        let title = if open.window.title.is_empty() {
            open.window.id.clone()
        } else {
            open.window.title.clone()
        };
        let spec = SheetSpec::new(move |()| {
            rsx! {
                div { style: "flex: 1; min-height: 0; display: flex; flex-direction: column;",
                    ExtensionView { ui, extension: ext.clone(), view: view.clone(), bare: true }
                }
            }
        })
        .title(title)
        .on_close(move |()| {
            ui.host().close_window(&close_ext, &close_id);
        });
        let mut sheet = sheet;
        sheet.set(Some(shell.open_sheet(spec)));
    });
    use_drop(move || {
        if let Some(id) = *sheet.peek() {
            shell.close_sheet(id);
        }
    });
    rsx! {}
}

/// Keeps the extensions' windows of kind `window` windows of the layout:
/// each one open is a pane of its view ([`window_pane`]) in a window of
/// its own (an OS window where the platform has them, else one drawn in
/// the page, as app-shell does every pop-out); the host closing it closes
/// its pane, and its pane closed, or back in the main window (the person
/// closed its OS window), closes it in the host.
pub(crate) fn use_window_panes(ui: ExtensionUi) {
    let placed = use_hook(|| CopyValue::new(BTreeSet::<String>::new()));
    use_effect(move || {
        let open: Vec<OpenWindow> = ui
            .windows()
            .into_iter()
            .filter(|w| w.window.kind == WindowKind::Window)
            .collect();
        let layout = ui.layout();
        let layout = layout.read();
        let keys: BTreeSet<String> = open.iter().map(OpenWindow::key).collect();
        let mut placed = placed;
        // Gone from the host: its pane closes.
        let gone: Vec<String> = placed.peek().difference(&keys).cloned().collect();
        // Gone from its window: it closes in the host.
        let left: Vec<&OpenWindow> = open
            .iter()
            .filter(|w| placed.peek().contains(&w.key()))
            .filter(|w| {
                layout
                    .locate(&window_pane(&w.key()))
                    .is_none_or(|l| l.window == 0)
            })
            .collect();
        let new: Vec<&OpenWindow> = open
            .iter()
            .filter(|w| !placed.peek().contains(&w.key()))
            .collect();
        drop(layout);
        for key in gone {
            placed.write().remove(&key);
            let pane = window_pane(&key);
            ui.apply(|w| w.close(&pane));
        }
        for w in left {
            placed.write().remove(&w.key());
            let pane = window_pane(&w.key());
            ui.apply(|l| l.close(&pane));
            ui.host().close_window(&w.extension, &w.window.id);
        }
        for w in new {
            placed.write().insert(w.key());
            let title = if w.window.title.is_empty() {
                w.window.id.clone()
            } else {
                w.window.title.clone()
            };
            let mut pane = Pane::new(
                window_pane(&w.key()).0,
                format!("{}/{}", w.extension, w.window.view),
                title,
            );
            if !w.state.is_null() {
                pane.state = w.state.to_string();
            }
            // Placed by the platform, at its size (720 × 560 unless it says).
            let bounds = (w.window.width.is_some() || w.window.height.is_some()).then(|| Rect {
                x: 120.0,
                y: 90.0,
                w: w.window.width.unwrap_or(720.0),
                h: w.window.height.unwrap_or(560.0),
            });
            ui.apply(|l| l.open(pane, Target::Window(bounds)));
        }
    });
}

// ---- the settings page's window ---------------------------------------

/// What a settings sheet out of process reads and writes: every setting's
/// value now, and the writes it asked for.
#[derive(Default)]
struct Shared {
    now: Mutex<BTreeMap<String, Value>>,
    writes: Mutex<Vec<(String, Option<Value>)>>,
}

fn info(id: &str, name: &str, effect: Effect, description: &str, params: &str) -> ActionInfo {
    ActionInfo {
        id: id.to_owned(),
        display_name: name.to_owned(),
        description: description.to_owned(),
        category: "App".to_owned(),
        group: "Settings".to_owned(),
        shortcut: String::new(),
        effect,
        params_schema: (!params.is_empty()).then(|| params.to_owned()),
        result_schema: None,
        enabled: true,
        disabled_reason: None,
        checked: None,
    }
}

/// Joins the app's settings to the settings extension's window: call it
/// where the app's [`Settings`](crate::settings::Settings) and `host` are (`use_extension_ui`
/// does). Without a settings handle in context it does nothing.
pub(crate) fn use_settings_window(host: &Host, open: Signal<Vec<OpenWindow>>) {
    let handle = try_use_settings();
    let shared = use_hook(|| Arc::new(Shared::default()));
    // The app's own sections, as its items of `settings.sections` (an
    // extension's come from its manifest); a page of the app's own
    // (Appearance) is named so the sheet draws it with the app's page.
    {
        let host = host.clone();
        use_effect(move || {
            let Some(h) = handle else { return };
            let registry = h.registry.read();
            let pages = h.pages.read();
            let items: Vec<Value> = registry
                .sections()
                .iter()
                .filter(|s| s.owner == Owner::App)
                .filter_map(|s| {
                    serde_json::to_value(architect_extensions::manifest::decl_of(
                        s,
                        pages.contains(&s.id),
                    ))
                    .ok()
                })
                .collect();
            drop((registry, pages));
            host.contribute_app(SETTINGS_SECTIONS, items);
        });
    }
    // The values, for a sheet out of process.
    {
        let shared = Arc::clone(&shared);
        use_effect(move || {
            let Some(h) = handle else { return };
            let registry = h.registry.read();
            let now = h.values.read();
            let mut out = BTreeMap::new();
            for section in registry.sections() {
                for setting in section.settings() {
                    if let Some(v) = now.get(&registry, &setting.key) {
                        out.insert(setting.key.clone(), setting_json(&v));
                    }
                }
            }
            *shared.now.lock().unwrap_or_else(PoisonError::into_inner) = out;
        });
    }
    {
        let shared = Arc::clone(&shared);
        use_future(move || {
            let shared = Arc::clone(&shared);
            async move {
                let Some(h) = handle else { return };
                loop {
                    futures_timer::Delay::new(std::time::Duration::from_millis(100)).await;
                    let queued: Vec<_> = std::mem::take(
                        &mut *shared.writes.lock().unwrap_or_else(PoisonError::into_inner),
                    );
                    for (key, value) in queued {
                        match value.as_ref().and_then(setting_value) {
                            Some(v) => h.set(&key, v),
                            None => h.reset(&key),
                        }
                    }
                }
            }
        });
    }
    {
        let (list, shared) = (host.list().clone(), Arc::clone(&shared));
        use_hook(move || {
            if handle.is_none() {
                return;
            }
            let now = Arc::clone(&shared);
            let _ = list.register_forwarded_with(
                &info(
                    "APP_SETTINGS_VALUES",
                    "Settings: every value",
                    Effect::ReadOnly,
                    "Every setting's value now, by key (its default when not set): what a settings \
                     sheet drawn out of process shows.",
                    "",
                ),
                Arc::new(move |_: &ActionCaller, _: &[u8]| {
                    let values = now
                        .now
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .clone();
                    let out: ActionFuture = Box::pin(async move {
                        Ok(json!({ "values": values }).to_string().into_bytes())
                    });
                    Ok(out)
                }),
            );
            let writes = Arc::clone(&shared);
            let _ = list.register_forwarded_with(
                &info(
                    "APP_SETTINGS_SET",
                    "Settings: set a value",
                    Effect::Mutating,
                    "Sets setting `key` to `value` in its scope's store, as the settings page does \
                     (a value it does not take, or one the org locked, is let go); no `value` puts \
                     its default back.",
                    r#"{"type":"object","properties":{"key":{"type":"string"},"value":{}},"required":["key"]}"#,
                ),
                Arc::new(move |_: &ActionCaller, args: &[u8]| {
                    let a: Value = serde_json::from_slice(args).map_err(|e| ActionError::BadArgs {
                        id: "APP_SETTINGS_SET".to_owned(),
                        message: e.to_string(),
                        expected: None,
                    })?;
                    let key = a.get("key").and_then(Value::as_str).unwrap_or_default().to_owned();
                    let value = a.get("value").cloned().filter(|v| !v.is_null());
                    writes
                        .writes
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push((key, value));
                    let out: ActionFuture = Box::pin(async move { Ok(b"null".to_vec()) });
                    Ok(out)
                }),
            );
        });
    }
    // `Settings::show` (a search hit, the manager's MANAGER_SHOW)
    // opens the window `APP_SETTINGS` opens; closing it closes the page.
    {
        let host = host.clone();
        let mut wanted = use_signal(|| false);
        use_effect(move || {
            let Some(h) = handle else { return };
            let want = (h.open)().is_some();
            let window =
                settings_decl(&host).filter(|_| settings_window(&host, &open.read()).is_some());
            let before = *wanted.peek();
            match (want, window.is_some(), before) {
                // Shown while closed: open the window.
                (true, false, false) => {
                    if let Some((ext, id)) = settings_decl(&host) {
                        let _ = host.open_window(&ext, &id, Value::Null);
                    }
                }
                // The window closed (the person, its extension): the page
                // closes.
                (true, false, true) => {
                    h.close();
                    wanted.set(false);
                    return;
                }
                // The page closed (its own close): the window closes.
                (false, true, true) => {
                    if let Some((ext, id)) = window {
                        host.close_window(&ext, &id);
                    }
                }
                _ => {}
            }
            wanted.set(want);
        });
    }
    // The window opened (⌘,, the palette, an agent): the page shows the
    // section its state names, or stays where it was, or the first.
    {
        let host = host.clone();
        let mut last = use_signal(|| 0_u64);
        use_effect(move || {
            let Some(h) = handle else { return };
            let Some(w) = settings_window(&host, &open.read()) else {
                return;
            };
            if *last.peek() == w.opened {
                return;
            }
            last.set(w.opened);
            let section = w
                .state
                .get("section")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let setting = w
                .state
                .get("setting")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if let Some(key) = setting {
                h.show_setting(&key);
            } else if section.is_some() || h.open.peek().is_none() {
                h.show(section.as_deref());
            }
        });
    }
}

/// The window `APP_SETTINGS` opens: its extension and id.
fn settings_decl(host: &Host) -> Option<(String, String)> {
    host.window_decls()
        .into_iter()
        .find(|(_, w)| w.opens == APP_SETTINGS)
        .map(|(ext, w)| (ext, w.id))
}

/// That window, when it is open.
fn settings_window(host: &Host, open: &[OpenWindow]) -> Option<OpenWindow> {
    let (ext, id) = settings_decl(host)?;
    open.iter()
        .find(|w| w.extension == ext && w.window.id == id)
        .cloned()
}

thread_local! {
    /// The main window's [`ExtensionUi`]: what a pop-out's root draws the
    /// extensions' views with (its own `VirtualDom`, on the same thread).
    static MAIN: std::cell::Cell<Option<ExtensionUi>> = const { std::cell::Cell::new(None) };
}

/// Keeps `ui` as the main window's, for the pop-outs.
pub(crate) fn set_main(ui: ExtensionUi) {
    MAIN.with(|m| m.set(Some(ui)));
}

/// The main window's [`ExtensionUi`], from a pop-out window's root.
#[must_use]
pub fn main_ui() -> Option<ExtensionUi> {
    MAIN.with(std::cell::Cell::get)
}

/// A pop-out window's shell, drawing the extensions' views as the main
/// window does: what an app hands `app_shell_native::launch` as its
/// `popout` (or mounts in its own), `kinds` its own tab kinds.
#[component]
pub fn ExtensionPopout(
    window: app_shell::WindowId,
    #[props(default)] kinds: app_shell::ui::TabKinds,
) -> Element {
    let layout = app_shell::ui::use_window_layout();
    let ui = main_ui();
    use_context_provider(move || ui.map(|ui| ui.tab_hooks()).unwrap_or_default());
    let mut kinds = kinds;
    if let Some(ui) = ui {
        ui.register_kinds(&mut kinds);
    }
    rsx! {
        app_shell::ui::AppShell { layout, kinds, window }
    }
}
