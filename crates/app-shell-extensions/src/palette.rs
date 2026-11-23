//! The extensions in the shell's palette (#381): their sources
//! (`[[palette_sources]]`) give ⌘K rows, ranked by app-shell's engine;
//! their openers (`[[openers]]` with a `kind`) give ⌘O's. Both are kept
//! what the loaded extensions say in the window's
//! [`app_shell::ui::PaletteSources`], as they load and go.
//!
//! An extension may draw an overlay itself (`[[palette_views]]`): its view
//! is then the shell's palette view for it
//! ([`app_shell::ui::Shell::set_palette_view`]), the first loaded winning
//! an overlay. `COMMANDS_SHOW` (`{overlay, query?}`: `palette`,
//! `quick-open`, or `none` to close) opens one from an action.

use std::collections::BTreeMap;
use std::rc::Rc;

use app_shell::palette::{FnSource, Item, OpenIn, OpenItem, Opener, Ranking};
use app_shell::ui::{PaletteOverlay, PaletteSources, Shell};
use architect::action::{
    ActionCaller, ActionError, ActionList, ActionMeta, Effect, Invocation, InvocationSource,
};
use dioxus::prelude::*;
use futures::StreamExt as _;
use futures::channel::mpsc;
use serde_json::{Value, json};

use architect_extensions::contrib;
use architect_extensions::host::Host;

use crate::interface::{ExtensionUi, ExtensionView};

/// What a row picked runs: on `list`, `action`, with `args`.
type Ask = (ActionList, String, String);

/// Runs what the palette's rows ask, on the window's runtime: the palette
/// has closed by then.
fn use_runner(what: &'static str) -> mpsc::UnboundedSender<Ask> {
    use_coroutine(move |mut asks: mpsc::UnboundedReceiver<Ask>| async move {
        while let Some((list, action, args)) = asks.next().await {
            let caller =
                ActionCaller::app().with_invocation(Invocation::new(InvocationSource::Palette));
            if let Err(e) = list
                .run_json_async_with(&caller, &action, args.as_bytes())
                .await
            {
                tracing::warn!(ext.action = %action, ext.what = what, error = %e, "an extension's palette row did not run");
            }
        }
    })
    .tx()
}

/// Keeps ⌘O's openers what the extensions' `[[openers]]` (the ones with a
/// `kind`, not the claims) say, as they load and go.
pub(crate) fn use_quick_open(host: Host, revision: Signal<u64>, sources: PaletteSources) {
    let mine = use_hook(|| Rc::new(std::cell::RefCell::new(Vec::<String>::new())));
    let opens = use_runner("opener");
    use_effect(move || {
        let _ = revision();
        let now = host.contributions().quick_open();
        let kinds: Vec<String> = now.iter().map(|o| o.item.kind.clone()).collect();
        for gone in mine.borrow().iter().filter(|k| !kinds.contains(k)) {
            sources.remove_opener(gone);
        }
        for by in now {
            sources.register_opener(quick_opener(
                host.list().clone(),
                &sources,
                opens.clone(),
                &by.item,
            ));
        }
        *mine.borrow_mut() = kinds;
    });
}

/// Keeps the palette's extension sources what the loaded extensions say
/// (`[[palette_sources]]`), as they load and go.
pub(crate) fn use_palette(host: Host, revision: Signal<u64>, sources: PaletteSources) {
    let mine = use_hook(|| Rc::new(std::cell::RefCell::new(Vec::<String>::new())));
    let runs = use_runner("source");
    use_effect(move || {
        let _ = revision();
        let now = host.contributions().palette_sources();
        let ids: Vec<String> = now.iter().map(|p| p.item.id.clone()).collect();
        for gone in mine.borrow().iter().filter(|id| !ids.contains(id)) {
            sources.remove(gone);
        }
        for by in now {
            let decl = by.item;
            let ranking = if decl.ranking == "commands" {
                Ranking::Commands
            } else {
                Ranking::Open
            };
            let id = decl.id.clone();
            let items = asked(
                host.list().clone(),
                &sources,
                decl.action.clone(),
                move |out| palette_items(&id, out),
            );
            let source = FnSource::new(decl.id.clone(), decl.title.clone(), items).ranking(ranking);
            let (run, by_id) = if decl.run.is_empty() {
                (decl.action.clone(), false)
            } else {
                (decl.run.clone(), true)
            };
            let (list, tx) = (host.list().clone(), runs.clone());
            sources.register(source, move |item: &Item| {
                let key = if by_id { "id" } else { "run" };
                let args = json!({ key: item.id }).to_string();
                let _ = tx.unbounded_send((list.clone(), run.clone(), args));
            });
        }
        *mine.borrow_mut() = ids;
    });
}

/// A palette source action's answer (`{items: [{id, label, detail?,
/// group?, shortcut?, recency?}]}`; `title` for `label` too) as the
/// engine's items of source `source`.
fn palette_items(source: &str, out: Option<Vec<u8>>) -> Vec<Item> {
    let answer: Value = out
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null);
    let list = answer
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    list.iter()
        .filter_map(|v| {
            let text = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default();
            let id = text("id");
            let label = [text("label"), text("title"), id]
                .into_iter()
                .find(|s| !s.is_empty())?;
            (!id.is_empty()).then(|| {
                Item::new(id, label)
                    .source(source)
                    .group(text("group"))
                    .detail(text("detail"))
                    .shortcut(text("shortcut"))
                    .recency(v.get("recency").and_then(Value::as_u64).unwrap_or(0))
            })
        })
        .collect()
}

/// What an extension's finding action (`{query}` in) answers for a
/// query, as `parse` reads it, synchronously for the engine.
///
/// A linked extension's action answers at once; a process's later, its
/// answer kept a moment ([`ASKED_AGAIN`]) and the palette told to look
/// again ([`PaletteSources::refresh`]), the last answer shown meanwhile.
fn asked<T: Clone + 'static>(
    list: ActionList,
    sources: &PaletteSources,
    action: String,
    parse: impl Fn(Option<Vec<u8>>) -> Vec<T> + 'static,
) -> impl Fn(&str) -> Vec<T> + 'static {
    type Kept<T> = Rc<std::cell::RefCell<BTreeMap<String, (web_time::Instant, Vec<T>)>>>;
    let answers: Kept<T> = Rc::default();
    let pending: Rc<std::cell::RefCell<std::collections::BTreeSet<String>>> = Rc::default();
    let shown_last: Rc<std::cell::RefCell<Vec<T>>> = Rc::default();
    let parse = Rc::new(parse);
    let sources = sources.clone();
    move |query: &str| -> Vec<T> {
        if let Some((at, items)) = answers.borrow().get(query)
            && at.elapsed() < ASKED_AGAIN
        {
            return items.clone();
        }
        if pending.borrow().contains(query) {
            return shown_last.borrow().clone();
        }
        let (list, action, args) = (
            list.clone(),
            action.clone(),
            json!({ "query": query }).to_string(),
        );
        let mut run = Box::pin(async move {
            let caller =
                ActionCaller::app().with_invocation(Invocation::new(InvocationSource::Palette));
            list.run_json_async_with(&caller, &action, args.as_bytes())
                .await
        });
        let mut cx = std::task::Context::from_waker(futures::task::noop_waker_ref());
        if let std::task::Poll::Ready(out) = run.as_mut().poll(&mut cx) {
            let items = parse(out.ok());
            shown_last.borrow_mut().clone_from(&items);
            return items;
        }
        pending.borrow_mut().insert(query.to_owned());
        let (kept, asked, shown, parse, query, sources) = (
            Rc::clone(&answers),
            Rc::clone(&pending),
            Rc::clone(&shown_last),
            Rc::clone(&parse),
            query.to_owned(),
            sources.clone(),
        );
        spawn(async move {
            let items = parse(run.await.ok());
            asked.borrow_mut().remove(&query);
            shown.borrow_mut().clone_from(&items);
            kept.borrow_mut()
                .insert(query, (web_time::Instant::now(), items));
            sources.refresh();
        });
        shown_last.borrow().clone()
    }
}

/// How long an answer an extension's opener gave later is shown before it
/// is asked again.
const ASKED_AGAIN: std::time::Duration = std::time::Duration::from_secs(1);

/// ⌘O's opener for an extension's `decl`: a query runs its `action`
/// (`{query}` in, `{items}` out, as [`asked`] gets it); an item picked
/// runs its `open`.
fn quick_opener(
    list: ActionList,
    sources: &PaletteSources,
    opens: mpsc::UnboundedSender<Ask>,
    decl: &contrib::OpenerDecl,
) -> Opener {
    let kind = decl.kind.clone();
    let parse_kind = kind.clone();
    let items = asked(list.clone(), sources, decl.action.clone(), move |out| {
        open_items(&parse_kind, out)
    });
    let (open, by_id) = if decl.open.is_empty() {
        (decl.action.clone(), false)
    } else {
        (decl.open.clone(), true)
    };
    let opened = move |item: &OpenItem, open_in: OpenIn| {
        let place = match open_in {
            OpenIn::Here => "here",
            OpenIn::Tab => "tab",
            OpenIn::Split => "split",
        };
        let key = if by_id { "id" } else { "open" };
        let mut args = serde_json::Map::new();
        args.insert(key.to_owned(), json!(item.id));
        // Where it goes, to an action that takes it (`in` in its params).
        let takes_in = list
            .info(&open)
            .and_then(|i| i.params_schema)
            .is_some_and(|p| p.contains("\"in\""));
        if takes_in {
            args.insert("in".to_owned(), json!(place));
        }
        let args = Value::Object(args).to_string();
        let _ = opens.unbounded_send((list.clone(), open.clone(), args));
    };
    Opener::new(kind, decl.title.clone(), items, opened)
}

/// An opener action's answer (`{items: [{id, title, detail?, recency?}]}`)
/// as ⌘O's items: the palette sorts them by its match, then `recency`
/// (higher first), then title.
fn open_items(kind: &str, out: Option<Vec<u8>>) -> Vec<OpenItem> {
    let answer: Value = out
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null);
    let list = answer
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    list.iter()
        .filter_map(|v| {
            let text = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default();
            let (id, title) = (text("id"), text("title"));
            (!id.is_empty()).then(|| {
                OpenItem::new(kind, id, if title.is_empty() { id } else { title })
                    .detail(text("detail"))
                    .recency(v.get("recency").and_then(Value::as_u64).unwrap_or(0))
            })
        })
        .collect()
}

/// Keeps the shell's palette overlays drawn by the views the loaded
/// extensions say (`[[palette_views]]`), from when the shell is attached;
/// an overlay none draws any more is the shell's own again.
pub(crate) fn use_palette_views(ui: ExtensionUi) {
    let mine = use_hook(|| Rc::new(std::cell::RefCell::new(Vec::<PaletteOverlay>::new())));
    use_effect(move || {
        let _ = ui.revision();
        let Some(shell) = ui.attached() else { return };
        let mut now = Vec::new();
        for by in ui.host().contributions().palette_views() {
            let Some(overlay) = PaletteOverlay::parse(&by.item.overlay) else {
                tracing::debug!(ext.id = %by.owner, ext.overlay = %by.item.overlay, "no such palette overlay");
                continue;
            };
            let (extension, view) = (by.owner, by.item.view);
            let draw = Callback::new(move |()| palette_view(ui, extension.clone(), view.clone()));
            shell.set_palette_view(overlay, Some(draw));
            now.push(overlay);
        }
        for gone in mine.borrow().iter().filter(|o| !now.contains(o)) {
            shell.set_palette_view(*gone, None);
        }
        *mine.borrow_mut() = now;
    });
}

/// Extension `extension`'s view `view` where the palette goes: a linked
/// one's component as it is, a process's (a development copy) in a box
/// of the palette's height.
fn palette_view(ui: ExtensionUi, extension: String, view: String) -> Element {
    if ui.host().linked_view(&extension, &view).is_some() {
        return rsx! {
            ExtensionView { ui, extension, view, bare: true }
        };
    }
    rsx! {
        div { style: "height: 56vh; min-height: 320px; display: flex; flex-direction: column;",
            ExtensionView { ui, extension, view }
        }
    }
}

/// `COMMANDS_SHOW`: opens a palette overlay from an action.
static COMMANDS_SHOW: ActionMeta = ActionMeta {
    id: "COMMANDS_SHOW",
    trait_name: "Commands",
    method_name: "show",
    display_name: "Show the palette",
    description: "Opens the palette's overlay `overlay` (`palette`: ⌘K, `quick-open`: ⌘O; `none` closes it), its field holding `query` when given (`theme:` asks one opener).",
    category: "Help",
    group: "Help",
    shortcut: "",
    toggleable: false,
    undo: false,
};

/// What `COMMANDS_SHOW` asks: an overlay (none: close), and its query.
type Show = (Option<PaletteOverlay>, String);

/// `COMMANDS_SHOW`'s arguments.
#[derive(Debug, Default, facet::Facet)]
struct ShowArgs {
    /// `palette` (the default), `quick-open`, or `none` to close.
    overlay: Option<String>,
    /// What its field holds as it opens.
    query: Option<String>,
}

/// `COMMANDS_SHOW`'s arguments, read.
fn show_args(args: &[u8]) -> Result<Show, String> {
    let v: Value = if args.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(args).map_err(|e| e.to_string())?
    };
    let text = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default();
    let overlay = match text("overlay") {
        "" | "palette" => Some(PaletteOverlay::Palette),
        "none" => None,
        other => Some(
            PaletteOverlay::parse(other)
                .ok_or_else(|| format!("no overlay {other:?}: palette, quick-open or none"))?,
        ),
    };
    Ok((overlay, text("query").to_owned()))
}

/// Registers `COMMANDS_SHOW` on `list` (in place of an earlier window's),
/// what it asks done in the attached shell.
pub(crate) fn use_show_action(ui: ExtensionUi) {
    let shows = use_coroutine(move |mut asks: mpsc::UnboundedReceiver<Show>| async move {
        while let Some((overlay, query)) = asks.next().await {
            let Some(shell) = *ui.shell.peek() else {
                continue;
            };
            show(shell, overlay, &query);
        }
    });
    use_hook(move || {
        let list = ui.host().list().clone();
        let tx = shows.tx();
        list.unregister(COMMANDS_SHOW.id);
        let handler = std::sync::Arc::new(move |args: &[u8]| {
            let fail = |message: String| ActionError::Failed {
                id: COMMANDS_SHOW.id.to_owned(),
                message,
            };
            let asked = show_args(args).map_err(fail)?;
            tx.unbounded_send(asked)
                .map_err(|_| fail("the window is gone".to_owned()))?;
            Ok(b"null".to_vec())
        });
        let params = Some(<ShowArgs as facet::Facet>::SHAPE);
        if let Err(e) =
            list.register_dynamic(&COMMANDS_SHOW, Effect::ReadOnly, params, None, handler)
        {
            tracing::warn!(error = %e, "COMMANDS_SHOW was not registered");
        }
    });
}

/// Does what `COMMANDS_SHOW` asked in `shell`.
fn show(shell: Shell, overlay: Option<PaletteOverlay>, query: &str) {
    match overlay {
        Some(o) => shell.open_overlay(o, query),
        None => shell.close_palette(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_shows_arguments_name_an_overlay_or_none() {
        assert_eq!(
            show_args(br#"{"overlay":"quick-open","query":"theme:"}"#),
            Ok((Some(PaletteOverlay::QuickOpen), "theme:".to_owned()))
        );
        assert_eq!(
            show_args(b""),
            Ok((Some(PaletteOverlay::Palette), String::new()))
        );
        assert_eq!(
            show_args(br#"{"overlay":"none"}"#),
            Ok((None, String::new()))
        );
        assert!(show_args(br#"{"overlay":"which-key"}"#).is_err());
    }
}
