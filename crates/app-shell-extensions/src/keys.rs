//! The keys loaded extensions bind (`[[keys]]`), in app-shell's keymap.
//!
//! Each binding is one of the shell's [`BoundKey`]s, kept what the loaded
//! extensions say as they load and go: a `when = "view:<view>"` one
//! holds while a tab of that view is active (before the app's keymap: a
//! chart's plain letters, its own ⌘Z), the others everywhere (after it).
//! A view an opener shows a file in edits that document: ⌘Z and ⇧⌘Z
//! there are its own undo (`DOC_UNDO`, `DOC_REDO`, told the pane's path),
//! under the keys the extension binds there itself.
//!
//! A plain key (no ⌘ or ⌃) bound in a view is not taken while a text
//! field has the focus ([`crate::field_focus`]): a letter typed into a
//! field is the field's.

use app_shell::ui::{BoundKey, KeyScope, Shell};
use app_shell::{KeyChord, Pane};
use architect::action::{ActionCaller, ActionList, Invocation, InvocationSource};
use architect_extensions::contrib::Contributions;
use dioxus::prelude::*;
use serde_json::json;

use crate::interface::ExtensionUi;

/// Who binds the extensions' keys, to the shell.
const OWNER: &str = "extensions";

/// The document actions a view an opener shows a file in gets, and their
/// chords.
const DOC_KEYS: [(&str, &str); 2] = [("DOC_UNDO", "⌘Z"), ("DOC_REDO", "⇧⌘Z")];

/// One key the extensions bind, as data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionKey {
    /// As commands show theirs (`⌘K`, `A`).
    pub chord: String,
    /// The action it runs.
    pub action: String,
    /// Where it holds: a tab kind (`<extension>/<view>`) or the window.
    pub scope: KeyScope,
    /// Whether the action is told the active pane's document
    /// (`{path}`: `DOC_UNDO`'s).
    pub document: bool,
}

/// Every key the loaded extensions bind: their `[[keys]]` (the first
/// loaded winning a chord), then ⌘Z and ⇧⌘Z in each view an opener shows
/// a file in, unless the extension binds those there itself.
#[must_use]
pub fn extension_keys(contributions: &Contributions) -> Vec<ExtensionKey> {
    let mut keys: Vec<ExtensionKey> = contributions
        .key_bindings()
        .into_iter()
        .map(|by| {
            let context = by.item.context(&by.owner);
            let scope = context
                .strip_prefix("view:")
                .map_or(KeyScope::Window, |kind| KeyScope::Kind(kind.to_owned()));
            ExtensionKey {
                chord: by.item.chord,
                action: by.item.action,
                scope,
                document: false,
            }
        })
        .collect();
    for by in contributions.openers() {
        let Some(view) = by
            .item
            .view
            .as_deref()
            .filter(|_| !by.item.files.is_empty() || !by.item.doc_kinds.is_empty())
        else {
            continue;
        };
        let scope = KeyScope::Kind(format!("{}/{view}", by.owner));
        for (action, chord) in DOC_KEYS {
            let taken = keys.iter().any(|k| k.scope == scope && k.chord == chord);
            if !taken {
                keys.push(ExtensionKey {
                    chord: chord.to_owned(),
                    action: action.to_owned(),
                    scope: scope.clone(),
                    document: true,
                });
            }
        }
    }
    keys
}

/// Whether the platform's primary modifier is ⌘.
const MAC: bool = cfg!(any(target_os = "macos", target_os = "ios"));

/// `keys` as the shell's bound keys, each running its action on `list`.
fn bound(list: &ActionList, keys: Vec<ExtensionKey>) -> Vec<BoundKey> {
    keys.into_iter()
        .filter_map(|k| {
            let chord = KeyChord::parse(&k.chord, MAC)?;
            let plain_in_view = chord.is_plain() && matches!(k.scope, KeyScope::Kind(_));
            let list = list.clone();
            Some(BoundKey::new(chord, k.scope, move |pane: Option<&Pane>| {
                if plain_in_view && crate::field_focus::field_focused() {
                    return false;
                }
                let args = match pane {
                    Some(p) if k.document && !p.state.is_empty() => json!({ "path": p.state }),
                    _ => json!({}),
                };
                run_key(&list, &k.action, args.to_string(), pane);
                true
            }))
        })
        .collect()
}

/// Runs `action` as a key does, told the pane it was pressed in.
fn run_key(list: &ActionList, action: &str, args: String, pane: Option<&Pane>) {
    let mut invocation = Invocation::new(InvocationSource::Key);
    if let Some(p) = pane {
        invocation.pane = Some(p.id.0.clone());
        invocation.view = Some(p.view.0.clone());
    }
    let (list, action) = (list.clone(), action.to_owned());
    spawn(async move {
        let caller = ActionCaller::app().with_invocation(invocation);
        if let Err(e) = list
            .run_json_async_with(&caller, &action, args.as_bytes())
            .await
        {
            tracing::warn!(ext.action = %action, error = %e, "an extension's key did not run");
        }
    });
}

/// Binds the extensions' keys in `shell`: what they are now.
pub fn bind(ui: ExtensionUi, shell: Shell) {
    let host = ui.host();
    shell.bind_keys(
        OWNER,
        bound(host.list(), extension_keys(host.contributions())),
    );
}

/// Keeps the keys bound in the window's shell what the loaded extensions
/// say, from when the shell is attached ([`ExtensionUi::attach`]).
pub(crate) fn use_keys(ui: ExtensionUi) {
    use_effect(move || {
        let _ = ui.revision();
        if let Some(shell) = ui.attached() {
            bind(ui, shell);
        }
    });
}
