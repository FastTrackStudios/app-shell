//! The app's settings, as the extensions' chrome joins them: the
//! sections (Architect's `architect-settings` model: a [`Registry`] of
//! [`Section`]s, their values in [`ScopedValues`]) and which one is on
//! show, in the window's signals ([`use_settings`]).
//!
//! What draws the page is not here: the settings extension's window
//! (`APP_SETTINGS`, `crate::window_ui`; the extension is
//! `crate::settings_sheet`), which reads the app's sections
//! (`settings.sections`) and draws an extension's own section with its
//! view ([`SectionViews`]). Loaded extensions' sections come and go with
//! them (`crate::ui::use_extension_sections`); their values are the
//! page's (`crate::ui::use_page_setting_values`).

use std::collections::BTreeSet;
use std::rc::Rc;

pub use architect_settings::{Owner, Registry, Scope, ScopedValues, Section, Value, Values};
use dioxus::prelude::*;

/// What changed, for the store of its scope to keep.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsChange {
    pub scope: Scope,
    /// Everything that scope now holds.
    pub values: Values,
    /// The keys that changed.
    pub keys: Vec<String>,
}

/// The app's settings in the window. Cheap to copy.
#[derive(Clone, Copy, PartialEq)]
pub struct Settings {
    pub registry: Signal<Registry>,
    pub values: Signal<ScopedValues>,
    /// The section on show; `None` when the page is closed.
    pub open: Signal<Option<String>>,
    /// The setting a search hit went to, and how many times one has (so
    /// the same one pulses again).
    pub focus: Signal<Option<(String, u32)>>,
    /// The sections drawn by a page of the app's own (Appearance), by id.
    pub pages: Signal<BTreeSet<String>>,
    on_change: CopyValue<Option<Callback<SettingsChange>>>,
}

/// Puts the app's settings in context: `registry` its sections, `values`
/// what they hold; `on_change` keeps each change in its scope's store.
pub fn use_settings(
    registry: Registry,
    values: ScopedValues,
    on_change: Option<Callback<SettingsChange>>,
) -> Settings {
    let registry = use_signal(|| registry);
    let values = use_signal(|| values);
    let open = use_signal(|| None);
    let focus = use_signal(|| None);
    let pages = use_signal(BTreeSet::new);
    let on_change = use_hook(|| CopyValue::new(on_change));
    use_context_provider(|| Settings {
        registry,
        values,
        open,
        focus,
        pages,
        on_change,
    })
}

/// The app's settings above, if it put them there.
#[must_use]
pub fn try_use_settings() -> Option<Settings> {
    try_use_context()
}

impl Settings {
    /// Settings of their own, in no context: what a settings page drawn
    /// out of process keeps (its values the app's, asked for). Call it
    /// where a hook may make signals (`use_hook`).
    #[must_use]
    pub fn detached(
        registry: Registry,
        values: ScopedValues,
        on_change: Option<Callback<SettingsChange>>,
    ) -> Self {
        Self {
            registry: Signal::new(registry),
            values: Signal::new(values),
            open: Signal::new(None),
            focus: Signal::new(None),
            pages: Signal::new(BTreeSet::new()),
            on_change: CopyValue::new(on_change),
        }
    }

    /// The same settings over the sections `registry` has: the page's,
    /// the app's and every extension's as its point delivers them.
    #[must_use]
    pub const fn with_registry(self, registry: Signal<Registry>) -> Self {
        Self { registry, ..self }
    }

    /// Sets `key` to `value` in its scope (a value it does not take, or
    /// one the org locked, is let go).
    pub fn set(&self, key: &str, value: Value) {
        let mut values = self.values;
        let set = {
            let registry = self.registry.peek();
            values.write().set(&registry, key, value).ok()
        };
        if let Some(scope) = set {
            self.changed(scope, vec![key.to_owned()]);
        }
    }

    /// Puts `key`'s default back.
    pub fn reset(&self, key: &str) {
        let mut values = self.values;
        let scope = {
            let registry = self.registry.peek();
            values.write().reset(&registry, key)
        };
        if let Some(scope) = scope {
            self.changed(scope, vec![key.to_owned()]);
        }
    }

    /// Opens the page at section `id` (the first when `None`).
    pub fn show(&self, id: Option<&str>) {
        let first = self
            .registry
            .peek()
            .sections()
            .first()
            .map(|s| s.id.clone());
        let mut open = self.open;
        open.set(id.map(str::to_owned).or(first));
    }

    /// Opens the page at setting `key`'s section, `key` in focus.
    pub fn show_setting(&self, key: &str) {
        let section = self.registry.peek().setting(key).map(|(s, _)| s.id.clone());
        let Some(section) = section else { return };
        let n = self
            .focus
            .peek()
            .as_ref()
            .map_or(0, |(_, n)| n.wrapping_add(1));
        let mut focus = self.focus;
        focus.set(Some((key.to_owned(), n)));
        let mut open = self.open;
        open.set(Some(section));
    }

    /// Closes the page.
    pub fn close(&self) {
        let mut open = self.open;
        open.set(None);
    }

    /// Takes extension `id`'s sections away.
    pub fn remove_extension(&self, id: &str) {
        let mut registry = self.registry;
        registry
            .write()
            .remove_owner(&Owner::Extension(id.to_owned()));
    }

    fn changed(&self, scope: Scope, keys: Vec<String>) {
        if let Some(cb) = *self.on_change.peek() {
            cb.call(SettingsChange {
                scope,
                values: self.values.peek().of(scope).clone(),
                keys,
            });
        }
    }
}

/// Draws a section of the settings page from a view: an extension's
/// section (`[[sections]] view`), by its kind (`<extension>/<view>`).
/// `None` when it cannot. `use_extension_ui` provides it.
#[derive(Clone)]
pub struct SectionViews(pub Rc<dyn Fn(&str) -> Option<Element>>);

impl std::fmt::Debug for SectionViews {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SectionViews")
    }
}

/// A JSON value as a setting's.
#[must_use]
pub fn value_of(v: &serde_json::Value) -> Option<Value> {
    match v {
        serde_json::Value::Bool(b) => Some(Value::Bool(*b)),
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(Value::Int)
            .or_else(|| n.as_f64().map(Value::Float)),
        serde_json::Value::String(t) => Some(Value::Text(t.clone())),
        _ => None,
    }
}

/// A setting's value as JSON.
#[must_use]
pub fn json_of(v: &Value) -> serde_json::Value {
    match v {
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Int(i) => serde_json::json!(i),
        Value::Float(f) => serde_json::json!(f),
        Value::Text(t) => serde_json::Value::String(t.clone()),
    }
}
