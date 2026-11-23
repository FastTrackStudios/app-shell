//! The extensions in the shell's palette (#381): their sources
//! (`[[palette_sources]]`) give ⌘K rows, ranked by app-shell's engine;
//! their openers (`[[openers]]` with a `kind`) give ⌘O's. Both are kept
//! what the loaded extensions say in the window's
//! [`app_shell::ui::PaletteSources`], as they load and go.

use std::collections::BTreeMap;
use std::rc::Rc;

use app_shell::palette::{FnSource, Item, OpenIn, OpenItem, Opener, Ranking};
use app_shell::ui::PaletteSources;
use architect::action::{ActionCaller, ActionList, Invocation, InvocationSource};
use dioxus::prelude::*;
use futures::StreamExt as _;
use futures::channel::mpsc;
use serde_json::{Value, json};

use architect_extensions::contrib;
use architect_extensions::host::Host;

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
