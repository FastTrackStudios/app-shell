//! The settings page, a built-in extension (#272): it owns the window ⌘,
//! opens (`APP_SETTINGS`, an app id the app allows it), a sheet over the
//! app, and the point `settings.sections`, whose items are the page's
//! sections: the app's own (Workspace, Appearance, …), and every
//! extension's (`[[sections]]`, sugar for an item of the point; the
//! extensions manager's among them).
//!
//! One [`SettingsSheet`] value runs it linked into an app
//! (`Extensions::with_linked`) or as its own process (`src/settings_main.rs`,
//! feature `guest`: a development copy made by `EXT_CLONE {"id":
//! "settings"}`). Either way its sections arrive through the host
//! (`SETTINGS_SECTIONS`, the point's delivery) and it draws them with
//! [`SettingsPage`] on app-shell's theme: linked in over the app's
//! [`Settings`] and its extensions' section views ([`SectionViews`]); out
//! of process over the values the host service gives it
//! (`APP_SETTINGS_VALUES`, `APP_SETTINGS_SET`), a section drawn by
//! another's view named, not drawn.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use architect_extensions::hostcall::HostRequest;
use architect_extensions::linked::{Builder, Extension, Handle};
use architect_extensions::manifest::{SectionDecl, section_of};
use architect_extensions_dioxus::BuilderExt as _;
use architect_settings::{Control, Setting};
use dioxus::prelude::*;
use serde_json::{Value as Json, json};

use crate::kit::{NativeSelect, NativeSelectOption, SegmentedControl, Switch};
use crate::settings::{
    Registry, ScopedValues, SectionViews, Settings, SettingsChange, Value, json_of,
    try_use_settings, value_of,
};

/// Its id.
pub const ID: &str = "settings";

/// Its manifest.
pub const MANIFEST: &str = include_str!("../extensions/settings.toml");

/// Its window's id.
pub const WINDOW: &str = "settings";

/// The settings page, as a `linked::Extension`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SettingsSheet;

/// The point's items as last delivered: its revision, and each item with
/// who gave it.
struct Delivered {
    revision: u64,
    items: Vec<(String, Json)>,
}

static DELIVERED: Mutex<Delivered> = Mutex::new(Delivered {
    revision: 0,
    items: Vec::new(),
});

/// Moves on every delivery kept: what its view looks at.
static CHANGES: AtomicU64 = AtomicU64::new(0);

static HANDLE: Mutex<Option<Handle>> = Mutex::new(None);

fn delivered() -> MutexGuard<'static, Delivered> {
    DELIVERED.lock().unwrap_or_else(PoisonError::into_inner)
}

fn handle() -> Option<Handle> {
    HANDLE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// Keeps a delivery (`{point, revision, items: [{from, value}]}`) unless
/// a newer one was kept: whether it was.
pub fn keep(delivery: &Json) -> bool {
    let revision = delivery
        .get("revision")
        .and_then(Json::as_u64)
        .unwrap_or_default();
    let items: Vec<(String, Json)> = delivery
        .get("items")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .filter_map(|i| Some((i.get("from")?.as_str()?.to_owned(), i.get("value")?.clone())))
        .collect();
    let mut d = delivered();
    if revision < d.revision {
        return false;
    }
    d.revision = revision;
    d.items = items;
    drop(d);
    CHANGES.fetch_add(1, Ordering::Relaxed);
    true
}

/// The sections `items` (who gave each, and the item) make, in order.
///
/// An item that is not a section, or whose id another took first, is left
/// out. The app's own section drawn by a page of the app's (`view =
/// "page"`) is drawn by its rows.
#[must_use]
pub fn registry(items: &[(String, Json)]) -> Registry {
    let mut r = Registry::new();
    for (from, value) in items {
        let Ok(mut decl) = serde_json::from_value::<SectionDecl>(value.clone()) else {
            continue;
        };
        if from == "app" && decl.view.as_deref() == Some("page") {
            decl.view = None;
        }
        let _ = r.register(section_of(from, &decl));
    }
    r
}

/// What the page lists: each section's id, title, group and who gave it.
#[must_use]
pub fn listed(items: &[(String, Json)]) -> Json {
    let r = registry(items);
    let from = |id: &str| {
        items
            .iter()
            .find(|(_, v)| v.get("id").and_then(Json::as_str) == Some(id))
            .map(|(f, _)| f.clone())
            .unwrap_or_default()
    };
    let sections: Vec<Json> = r
        .sections()
        .iter()
        .map(|s| {
            json!({
                "id": s.id,
                "title": s.title,
                "group": s.group,
                "from": from(&s.id),
                "view": s.view,
                "settings": s.settings().count(),
            })
        })
        .collect();
    json!({ "sections": sections })
}

impl Extension for SettingsSheet {
    fn manifest(&self) -> &'static str {
        MANIFEST
    }

    fn build(&self, ext: &mut Builder) {
        *HANDLE.lock().unwrap_or_else(PoisonError::into_inner) = Some(ext.handle());
        ext.view("sheet", SheetView)
            .action("SETTINGS_SECTIONS", |delivery| async move {
                Ok(json!({ "kept": keep(&delivery) }))
            })
            .action("SETTINGS_LIST", |_| async move {
                let items = delivered().items.clone();
                Ok(listed(&items))
            });
    }
}

/// Redraws the calling view whenever a delivery is kept.
fn use_changes() -> u64 {
    let mut seen = use_signal(|| CHANGES.load(Ordering::Relaxed));
    use_future(move || async move {
        loop {
            futures_timer::Delay::new(Duration::from_millis(150)).await;
            let now = CHANGES.load(Ordering::Relaxed);
            if *seen.peek() != now {
                seen.set(now);
            }
        }
    });
    seen()
}

/// Runs app action `action` through the host service.
async fn run(action: &str, args: Json) -> Result<Json, String> {
    let Some(host) = handle() else {
        return Err("not running".to_owned());
    };
    host.host(HostRequest::Run {
        action: action.to_owned(),
        args,
        ticket: None,
    })
    .await
    .map_err(|e| format!("{action}: {e}"))
}

/// The settings page in its window: the sections delivered, over the
/// app's settings (linked in) or its own copy of their values (out of
/// process).
#[component]
fn SheetView() -> Element {
    let changes = use_changes();
    let mut sections = use_signal(|| registry(&delivered().items));
    use_effect(use_reactive!(|changes| {
        let _ = changes;
        sections.set(registry(&delivered().items));
    }));
    let app = try_use_settings();
    let own = app.is_none();
    let settings = use_hook(move || {
        let base = app.unwrap_or_else(|| {
            Settings::detached(
                Registry::new(),
                ScopedValues::default(),
                Some(Callback::new(|change: SettingsChange| {
                    for key in change.keys {
                        let value = change.values.set.get(&key).map(json_of);
                        spawn(async move {
                            let _ = run("APP_SETTINGS_SET", json!({ "key": key, "value": value }))
                                .await;
                        });
                    }
                })),
            )
        });
        base.with_registry(sections)
    });
    // Out of process: the values the app has now.
    use_future(move || async move {
        if !own {
            return;
        }
        loop {
            if let Ok(answer) = run("APP_SETTINGS_VALUES", Json::Null).await {
                let mut scoped = ScopedValues::default();
                let registry = sections.peek().clone();
                if let Some(values) = answer.get("values").and_then(Json::as_object) {
                    for (key, v) in values {
                        if let Some(v) = value_of(v) {
                            let _ = scoped.set(&registry, key, v);
                        }
                    }
                }
                let mut values = settings.values;
                if *values.peek() != scoped {
                    values.set(scoped);
                }
            }
            futures_timer::Delay::new(Duration::from_millis(500)).await;
        }
    });
    // Nothing open: the first section. Closed (its own close): the
    // window closes.
    let mut was_open = use_signal(|| false);
    use_effect(move || {
        let open = (settings.open)().is_some();
        let has = !sections.read().sections().is_empty();
        if !open && !*was_open.peek() && has {
            settings.show(None);
        }
        if !open
            && *was_open.peek()
            && let Some(host) = handle()
        {
            spawn(async move {
                let _ = host.window(WINDOW, false, Json::Null).await;
            });
        }
        was_open.set(open);
    });
    rsx! {
        div {
            style: "position: relative; flex: 1 1 0px; min-height: 420px; display: flex; overflow: hidden; \
                    background-color: var(--as-bg); color: var(--as-text); font-family: var(--as-font); font-size: 13px;",
            SettingsPage { settings }
        }
    }
}

/// The settings page: the sections down the side, grouped; the one open
/// on the right, its settings' rows (or its view, an extension's section
/// drawn by one of its views through the [`SectionViews`] in context).
#[component]
pub fn SettingsPage(settings: Settings) -> Element {
    let registry = (settings.registry)();
    let open = (settings.open)();
    let shown = open
        .as_deref()
        .and_then(|id| registry.sections().iter().find(|s| s.id == id))
        .or_else(|| registry.sections().first())
        .cloned();
    let mut groups: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for s in registry.sections() {
        let group = s.group.clone().unwrap_or_default();
        let row = (s.id.clone(), s.title.clone());
        match groups.iter_mut().find(|(g, _)| *g == group) {
            Some((_, rows)) => rows.push(row),
            None => groups.push((group, vec![row])),
        }
    }
    let current = shown.as_ref().map(|s| s.id.clone()).unwrap_or_default();
    rsx! {
        div {
            class: "as-settings-nav",
            style: "flex: none; width: 200px; display: flex; flex-direction: column; gap: 2px; padding: 12px 8px; \
                    overflow-y: scroll; background-color: var(--as-surface); border-right: 1px solid var(--as-border);",
            for (group, rows) in groups {
                if !group.is_empty() {
                    div { key: "g-{group}", style: "padding: 10px 8px 4px 8px; font-size: 11px; font-weight: 600; text-transform: uppercase; color: var(--as-heading);",
                        "{group}"
                    }
                }
                for (id, title) in rows {
                    {
                        let on = id == current;
                        let bg = if on { "var(--as-selected)" } else { "transparent" };
                        rsx! {
                            div {
                                key: "{id}",
                                class: "as-hov",
                                role: "button",
                                "aria-current": "{on}",
                                style: "height: 30px; padding: 0 10px; display: flex; align-items: center; border-radius: var(--as-radius); \
                                        background-color: {bg}; cursor: pointer; white-space: nowrap; overflow: hidden;",
                                onclick: move |_| settings.show(Some(&id)),
                                "{title}"
                            }
                        }
                    }
                }
            }
        }
        div { style: "flex: 1 1 0px; min-width: 0; display: flex; flex-direction: column;",
            div { style: "flex: none; display: flex; align-items: center; height: 44px; padding: 0 16px; border-bottom: 1px solid var(--as-border);",
                span { style: "flex: 1 1 0px; font-size: 15px; font-weight: 600;",
                    {shown.as_ref().map(|s| s.title.clone()).unwrap_or_else(|| "Settings".to_owned())}
                }
                div {
                    class: "as-hov",
                    role: "button",
                    "aria-label": "Close",
                    style: "width: 28px; height: 28px; display: flex; align-items: center; justify-content: center; border-radius: var(--as-radius); color: var(--as-muted); cursor: pointer;",
                    onclick: move |_| settings.close(),
                    "✕"
                }
            }
            div { style: "flex: 1 1 0px; min-height: 0; overflow-y: scroll; padding: 8px 16px 24px 16px;",
                if let Some(section) = shown {
                    SectionBody { settings, section }
                } else {
                    div { style: "padding: 24px; color: var(--as-muted);", "Nothing to set yet." }
                }
            }
        }
    }
}

/// One section's body: its view, or its blocks of rows.
#[component]
fn SectionBody(settings: Settings, section: architect_settings::Section) -> Element {
    if let Some(kind) = section.view.clone() {
        let drawn = try_use_context::<SectionViews>().and_then(|v| (v.0)(&kind));
        return drawn.unwrap_or_else(|| {
            rsx! {
                div { style: "padding: 24px 0; color: var(--as-muted);",
                    "{section.title} is drawn by {kind}, which this window does not show."
                }
            }
        });
    }
    rsx! {
        for (i, block) in section.blocks.iter().enumerate() {
            div { key: "{i}", style: "display: flex; flex-direction: column; margin-top: 12px;",
                if let Some(title) = &block.title {
                    div { style: "padding: 4px 0; font-size: 12px; font-weight: 600; color: var(--as-heading);", "{title}" }
                }
                div { style: "display: flex; flex-direction: column; border: 1px solid var(--as-border); border-radius: 10px; background-color: var(--as-surface);",
                    for setting in block.settings.iter().cloned() {
                        SettingRow { key: "{setting.key}", settings, setting }
                    }
                }
            }
        }
    }
}

/// A setting's row: its title and description, its control at the end.
#[component]
fn SettingRow(settings: Settings, setting: Setting) -> Element {
    let now = {
        let registry = settings.registry.read();
        (settings.values)()
            .get(&registry, &setting.key)
            .unwrap_or_else(|| setting.default.clone())
    };
    let locked = settings.values.read().is_locked(&setting.key);
    let focused = settings
        .focus
        .read()
        .as_ref()
        .is_some_and(|(k, _)| *k == setting.key);
    let bg = if focused {
        "var(--as-accent-soft)"
    } else {
        "transparent"
    };
    let events = if locked { "none" } else { "auto" };
    let key = setting.key.clone();
    let set = Callback::new(move |v: Value| settings.set(&key, v));
    rsx! {
        div {
            "data-setting": "{setting.key}",
            style: "display: flex; align-items: center; gap: 16px; min-height: 44px; padding: 8px 12px; \
                    border-bottom: 1px solid var(--as-border); background-color: {bg};",
            div { style: "flex: 1 1 0px; min-width: 0; display: flex; flex-direction: column; gap: 2px;",
                span { "{setting.title}" }
                if !setting.description.is_empty() {
                    span { style: "font-size: 12px; color: var(--as-muted);", "{setting.description}" }
                }
                if locked {
                    span { style: "font-size: 11px; color: var(--as-muted);", "Set by your organisation" }
                }
            }
            div { style: "flex: none; display: flex; align-items: center; pointer-events: {events};",
                {control(&setting.control, &now, set)}
            }
        }
    }
}

/// The control of `control` showing `now`, telling `set` a new value.
fn control(control: &Control, now: &Value, set: Callback<Value>) -> Element {
    match control {
        Control::Switch => {
            let on = matches!(now, Value::Bool(true));
            rsx! { BoolSwitch { on, set } }
        }
        Control::Select(choices) => {
            let at = choices.iter().position(|c| c.value == *now).unwrap_or(0);
            let values: Vec<Value> = choices.iter().map(|c| c.value.clone()).collect();
            let labels: Vec<String> = choices.iter().map(|c| c.label.clone()).collect();
            rsx! { Choose { at, values, labels, set } }
        }
        Control::Segmented(choices) | Control::Cards(choices) => {
            let current = choices
                .iter()
                .position(|c| c.value == *now)
                .map(|i| i.to_string())
                .unwrap_or_default();
            let options = choices
                .iter()
                .enumerate()
                .map(|(i, c)| (i.to_string(), c.label.clone()))
                .collect();
            let values: Vec<Value> = choices.iter().map(|c| c.value.clone()).collect();
            rsx! {
                SegmentedControl {
                    value: current,
                    options,
                    on_change: move |i: String| {
                        if let Some(v) = i.parse::<usize>().ok().and_then(|i| values.get(i)) {
                            set.call(v.clone());
                        }
                    },
                }
            }
        }
        Control::Slider {
            min,
            max,
            step,
            unit,
        } => {
            let v = match now {
                Value::Float(f) => *f,
                #[allow(clippy::cast_precision_loss)]
                Value::Int(i) => *i as f64,
                _ => *min,
            };
            let (min, max, step, unit) = (*min, *max, *step, unit.clone());
            rsx! {
                input {
                    r#type: "range",
                    min: "{min}",
                    max: "{max}",
                    step: "{step}",
                    value: "{v}",
                    style: "width: 160px;",
                    oninput: move |e| {
                        if let Ok(f) = e.value().parse::<f64>() {
                            set.call(Value::Float(f));
                        }
                    },
                }
                span { style: "min-width: 48px; text-align: right; color: var(--as-muted);", "{v}{unit}" }
            }
        }
        Control::Stepper {
            min,
            max,
            step,
            unit,
        } => {
            let v = match now {
                Value::Int(i) => *i,
                _ => *min,
            };
            let (min, max, step, unit) = (*min, *max, *step, unit.clone());
            let button = "width: 26px; height: 26px; display: flex; align-items: center; justify-content: center; \
                          border-radius: var(--as-radius); background-color: var(--as-raised); cursor: pointer;";
            rsx! {
                div { style: "display: flex; align-items: center; gap: 8px;",
                    div { role: "button", "aria-label": "Less", style: "{button}",
                        onclick: move |_| set.call(Value::Int(v.saturating_sub(step).max(min))),
                        "−"
                    }
                    span { style: "min-width: 40px; text-align: center;", "{v}{unit}" }
                    div { role: "button", "aria-label": "More", style: "{button}",
                        onclick: move |_| set.call(Value::Int(v.saturating_add(step).min(max))),
                        "+"
                    }
                }
            }
        }
        Control::Text { placeholder } => {
            let text = now.as_text().unwrap_or_default().to_owned();
            let placeholder = placeholder.clone();
            rsx! { TextField { text, placeholder, set } }
        }
        Control::Shortcut => {
            let text = now.as_text().unwrap_or_default().to_owned();
            rsx! { TextField { text, placeholder: "⌘K".to_owned(), set } }
        }
    }
}

/// A switch for a bool setting.
#[component]
fn BoolSwitch(on: bool, set: Callback<Value>) -> Element {
    let mut checked = use_signal(|| on);
    if *checked.peek() != on {
        checked.set(on);
    }
    rsx! {
        Switch { checked, on_change: move |b: bool| set.call(Value::Bool(b)) }
    }
}

/// A select over `labels`, the `at`th chosen.
#[component]
fn Choose(at: usize, values: Vec<Value>, labels: Vec<String>, set: Callback<Value>) -> Element {
    let mut chosen = use_signal(|| at.to_string());
    if *chosen.peek() != at.to_string() {
        chosen.set(at.to_string());
    }
    rsx! {
        NativeSelect {
            value: chosen,
            on_change: move |e: FormEvent| {
                if let Some(v) = e.value().parse::<usize>().ok().and_then(|i| values.get(i)) {
                    set.call(v.clone());
                }
            },
            for (i, label) in labels.iter().enumerate() {
                NativeSelectOption { key: "{i}", value: "{i}", "{label}" }
            }
        }
    }
}

/// A one-line field, set as it changes.
#[component]
fn TextField(text: String, placeholder: String, set: Callback<Value>) -> Element {
    rsx! {
        input {
            r#type: "text",
            value: "{text}",
            "aria-label": "{placeholder}",
            style: "width: 180px; height: 28px; padding: 0 8px; border: 1px solid var(--as-border); border-radius: var(--as-radius); \
                    background-color: var(--as-bg); color: var(--as-text); font-family: var(--as-font); font-size: 13px; outline: none;",
            onchange: move |e| set.call(Value::Text(e.value())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use architect_settings::{Choice, Section};

    fn page() -> Element {
        let settings = use_hook(|| {
            let mut r = Registry::new();
            let _ = r.register(
                Section::new("audio", "Audio").group("App").block(
                    None,
                    vec![
                        Setting::new("app.click", "Click", Control::Switch, Value::Bool(true))
                            .describe("A click on every beat"),
                        Setting::new(
                            "app.device",
                            "Device",
                            Control::Select(vec![
                                Choice::text("a", "Built-in"),
                                Choice::text("b", "Interface"),
                            ]),
                            Value::Text("b".into()),
                        ),
                    ],
                ),
            );
            let _ = r.register(Section::new("lab", "Lab").view("lab.ext/prefs"));
            Settings::detached(r, ScopedValues::default(), None)
        });
        rsx! { SettingsPage { settings } }
    }

    #[test]
    fn the_page_lists_the_sections_and_draws_the_open_ones_rows() {
        let mut dom = VirtualDom::new(page);
        dom.rebuild_in_place();
        let html = dioxus_ssr::render(&dom);
        for want in [
            "Audio",
            "Lab",
            "Click",
            "A click on every beat",
            "Interface",
            "App",
        ] {
            assert!(html.contains(want), "{want}: {html}");
        }
        assert!(html.contains("aria-checked=\"true\""), "{html}");
    }

    #[test]
    fn a_delivery_makes_the_sections_and_an_older_one_is_dropped() {
        let items = vec![
            (
                "app".to_owned(),
                json!({"id": "workspace", "title": "Workspace", "view": "page"}),
            ),
            (
                "fts.metronome".to_owned(),
                json!({"id": "metronome", "title": "Metronome"}),
            ),
            ("x".to_owned(), json!({"title": "no id"})),
        ];
        let r = registry(&items);
        let ids: Vec<&str> = r.sections().iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["workspace", "metronome"]);
        // The app's own page is drawn by its rows here.
        assert_eq!(r.sections()[0].view, None);
        assert_eq!(listed(&items)["sections"][1]["from"], "fts.metronome");
    }
}
