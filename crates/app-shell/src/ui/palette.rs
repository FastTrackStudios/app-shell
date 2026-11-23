//! The command palette (Mod+K) and open-anything (⌘O), drawn by the shell
//! and ranked by [`crate::palette`].
//!
//! The palette lists the app's entries (`AppShell { palette }`), ranked as
//! commands are: by match, then how recently each ran ([`Recents`], kept
//! by the shell for the app's life). Others add to it through the
//! [`PaletteSources`] in context ([`use_palette_sources`]): a source's
//! items (an extension's `[[palette_sources]]`), each ranked its own way,
//! and openers, whose finds are the palette's rows once something is typed
//! and all of ⌘O's ([`super::Shell::open_quick_open`]). A source's id
//! typed with a colon (`notes: plan`) narrows to it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use dioxus::prelude::*;

use super::icons::{self, IconView};
use super::overlay::PaletteItem;
use super::shell::use_shell;
use crate::palette::{
    Engine, FnSource, Item, OpenIn, OpenItem, Opener, Openers, Ranked, Ranking, Recents, Source,
    rank,
};

/// What runs an item a source gave.
type Run = Rc<dyn Fn(&Item)>;

struct Inner {
    engine: Engine,
    runs: HashMap<String, Run>,
    openers: Openers,
    recents: Recents,
}

/// The palette's sources and openers, and what ran lately: provide it
/// above the shell ([`use_palette_sources`]). Cheap to clone; changes are
/// seen the next time the palette asks, or at once after [`Self::refresh`].
#[derive(Clone)]
pub struct PaletteSources {
    inner: Rc<RefCell<Inner>>,
    revision: Signal<u64>,
}

impl PartialEq for PaletteSources {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl std::fmt::Debug for PaletteSources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaletteSources")
            .field("engine", &self.inner.borrow().engine)
            .finish_non_exhaustive()
    }
}

/// Makes the window's [`PaletteSources`] and provides it: call it in the
/// app's root, above the shell.
pub fn use_palette_sources() -> PaletteSources {
    let revision = use_signal(|| 0_u64);
    use_context_provider(|| PaletteSources {
        inner: Rc::new(RefCell::new(Inner {
            engine: Engine::new(),
            runs: HashMap::new(),
            openers: Openers::new(),
            recents: Recents::default(),
        })),
        revision,
    })
}

impl PaletteSources {
    /// Adds `source` (in place of one with its id), its items picked run
    /// by `run`.
    pub fn register(&self, source: impl Source + 'static, run: impl Fn(&Item) + 'static) {
        let id = source.id().to_owned();
        let mut inner = self.inner.borrow_mut();
        inner.engine.register(source);
        inner.runs.insert(id, Rc::new(run));
    }

    /// Takes source `id` away.
    pub fn remove(&self, id: &str) {
        let mut inner = self.inner.borrow_mut();
        inner.engine.remove(id);
        inner.runs.remove(id);
    }

    /// Adds an opener (in place of one of its kind).
    pub fn register_opener(&self, opener: Opener) {
        self.inner.borrow_mut().openers.register(opener);
    }

    /// Takes the opener of `kind` away.
    pub fn remove_opener(&self, kind: &str) {
        self.inner.borrow_mut().openers.remove(kind);
    }

    /// The palette asks again (a source answered later).
    pub fn refresh(&self) {
        let mut revision = self.revision;
        let n = revision.peek().wrapping_add(1);
        revision.set(n);
    }

    /// The sources' rows for `input`, in order.
    #[must_use]
    pub fn search(&self, input: &str) -> Vec<Ranked> {
        let inner = self.inner.borrow();
        inner.engine.search(input, &inner.recents)
    }

    /// The openers' finds for `input`, in order.
    #[must_use]
    pub fn find(&self, input: &str) -> Vec<OpenItem> {
        self.inner
            .borrow()
            .openers
            .search(input)
            .into_iter()
            .map(|f| f.item)
            .collect()
    }

    /// Runs `item` with its source's run, remembering it ran.
    pub fn run(&self, item: &Item) {
        let run = {
            let mut inner = self.inner.borrow_mut();
            inner.recents.touch(&item.id);
            inner.runs.get(&item.source).cloned()
        };
        if let Some(run) = run {
            run(item);
        }
    }

    /// Opens `item` with its opener: false when none of its kind is here.
    #[must_use = "false when nothing could open it"]
    pub fn open(&self, item: &OpenItem, open_in: OpenIn) -> bool {
        self.inner.borrow().openers.open(item, open_in)
    }

    /// What ran lately, most recent first.
    #[must_use]
    pub fn recents(&self) -> Recents {
        self.inner.borrow().recents.clone()
    }

    /// Remembers that `id` ran.
    pub fn touch(&self, id: &str) {
        self.inner.borrow_mut().recents.touch(id);
    }
}

/// A command a view puts on the palette while it is mounted
/// ([`use_view_commands`]): its row and what it runs.
#[derive(Clone)]
pub struct ViewCommand {
    pub item: Item,
    pub run: Rc<dyn Fn()>,
}

impl ViewCommand {
    /// `item` on the palette, running `run` when picked.
    pub fn new(item: Item, run: impl Fn() + 'static) -> Self {
        Self {
            item,
            run: Rc::new(run),
        }
    }
}

impl std::fmt::Debug for ViewCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewCommand")
            .field("item", &self.item)
            .finish_non_exhaustive()
    }
}

/// Puts a view's `commands` on the palette while the calling component
/// is mounted: a source of its own (`id`, titled `title`), ranked as
/// commands are. Each render hands it the commands as they are now (a
/// toggle's tick, a command that cannot run now); unmounted, they go.
/// Where no [`PaletteSources`] is in context, nothing.
pub fn use_view_commands(id: &str, title: &str, commands: Vec<ViewCommand>) {
    let sources = try_use_context::<PaletteSources>();
    let own = use_hook(|| id.to_owned());
    use_drop({
        let sources = sources.clone();
        let own = own.clone();
        move || {
            if let Some(s) = sources {
                s.remove(&own);
            }
        }
    });
    let Some(sources) = sources else { return };
    let items: Vec<Item> = commands
        .iter()
        .map(|c| c.item.clone().source(own.clone()))
        .collect();
    let runs: HashMap<String, Rc<dyn Fn()>> =
        commands.into_iter().map(|c| (c.item.id, c.run)).collect();
    sources.register(
        FnSource::new(own, title, move |_| items.clone()).ranking(Ranking::Commands),
        move |item| {
            if let Some(run) = runs.get(&item.id) {
                run();
            }
        },
    );
}

/// One row of the palette.
#[derive(Debug, Clone, PartialEq)]
enum Row {
    /// The app's entry at this index.
    App(usize),
    /// A source's item.
    Source(Item),
    /// Something an opener found.
    Open(OpenItem),
}

/// The app's entries as the engine's items: their index is their id
/// here, their keywords found as their detail is.
fn app_items(items: &[PaletteItem]) -> Vec<Item> {
    items
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut item = Item::new(
                p.id.clone().unwrap_or_else(|| p.label.clone()),
                p.label.clone(),
            )
            .group(p.group.clone())
            .detail(format!(
                "{} {}",
                p.keywords,
                p.detail.clone().unwrap_or_default()
            ))
            .source(i.to_string());
            if let Some(k) = &p.shortcut {
                item = item.shortcut(k.clone());
            }
            item
        })
        .collect()
}

/// The rows for `query`: the app's entries and the sources' items (merged
/// by score once something is typed), then what the openers find; only
/// the openers' in `open` (⌘O).
fn rows(
    items: &[PaletteItem],
    sources: Option<&PaletteSources>,
    query: &str,
    open: bool,
) -> Vec<Row> {
    let text = query.trim();
    let mut ranked: Vec<(i64, Row)> = Vec::new();
    if !open {
        let recents = sources.map(PaletteSources::recents).unwrap_or_default();
        for r in rank(&app_items(items), text, &recents) {
            if let Ok(i) = r.item.source.parse::<usize>() {
                ranked.push((r.score, Row::App(i)));
            }
        }
        if let Some(s) = sources {
            ranked.extend(
                s.search(text)
                    .into_iter()
                    .map(|r| (r.score, Row::Source(r.item))),
            );
        }
        if !text.is_empty() {
            // Stable: equal scores keep the app's first.
            ranked.sort_by_key(|r| std::cmp::Reverse(r.0));
        }
    }
    let mut out: Vec<Row> = ranked.into_iter().map(|(_, r)| r).collect();
    if (open || !text.is_empty())
        && let Some(s) = sources
    {
        out.extend(s.find(text).into_iter().map(Row::Open));
    }
    out
}

/// The palette's list: the query, then the rows; Enter or a press runs
/// the chosen one and closes the palette.
#[component]
pub(crate) fn PaletteView(
    items: Vec<PaletteItem>,
    phone: bool,
    #[props(default)] open: bool,
) -> Element {
    let shell = use_shell();
    let sources = try_use_context::<PaletteSources>();
    if let Some(s) = &sources {
        // A source answering later asks again.
        let _ = (s.revision)();
    }
    let mut query = use_signal(|| shell.take_palette_query());
    let mut chosen = use_signal(|| 0usize);
    let shown = rows(&items, sources.as_ref(), &query(), open);
    let n = shown.len();
    let pick = chosen().min(n.saturating_sub(1));
    let size = if phone { "17px" } else { "15px" };
    let hint = if open {
        "Open anything"
    } else {
        "Search for anything"
    };
    let run = {
        let (items, sources) = (items.clone(), sources.clone());
        move |row: &Row| {
            shell.close_top_sheet();
            match row {
                Row::App(i) => {
                    if let Some(item) = items.get(*i) {
                        if let Some(s) = &sources {
                            s.touch(item.id.as_deref().unwrap_or(&item.label));
                        }
                        shell.run_action(Some(item.action.clone()));
                    }
                }
                Row::Source(item) => {
                    if let Some(s) = &sources {
                        s.run(item);
                    }
                }
                Row::Open(item) => {
                    if let Some(s) = &sources {
                        let _ = s.open(item, OpenIn::Here);
                    }
                }
            }
        }
    };
    let enter = shown.get(pick).cloned();
    let on_enter = run.clone();
    rsx! {
        div {
            class: "as-palette",
            style: "display: flex; flex-direction: column; gap: 4px;",
            div {
                style: "display: flex; align-items: center; gap: 10px; height: 48px; padding: 0 8px; border-bottom: 1px solid var(--as-border); color: var(--as-muted);",
                IconView { icon: icons::SEARCH, size: 18.0 }
                input {
                    r#type: "text",
                    value: "{query}",
                    placeholder: "{hint}",
                    "aria-label": "{hint}",
                    autofocus: true,
                    style: "flex: 1 1 0px; min-width: 0; border: none; outline: none; background-color: transparent; color: var(--as-text); font-family: var(--as-font); font-size: {size}; padding: 0;",
                    oninput: move |e| {
                        query.set(e.value());
                        chosen.set(0);
                    },
                    onkeydown: move |e| {
                        match e.key().to_string().as_str() {
                            "ArrowDown" => chosen.set((pick + 1).min(n.saturating_sub(1))),
                            "ArrowUp" => chosen.set(pick.saturating_sub(1)),
                            "Enter" => {
                                if let Some(row) = &enter {
                                    on_enter(row);
                                }
                            }
                            _ => {}
                        }
                    },
                }
            }
            if shown.is_empty() {
                div { style: "padding: 18px 8px; color: var(--as-muted); text-align: center;", "Nothing matches" }
            }
            for (i, row) in shown.into_iter().enumerate() {
                {
                    let (label, detail, icon, shortcut) = match &row {
                        Row::App(at) => items.get(*at).map_or_else(
                            || (String::new(), None, None, None),
                            |p| (p.label.clone(), p.detail.clone(), p.icon, p.shortcut.clone()),
                        ),
                        Row::Source(item) => (
                            item.label.clone(),
                            (!item.detail.is_empty()).then(|| item.detail.clone()),
                            None,
                            (!item.shortcut.is_empty()).then(|| item.shortcut.clone()),
                        ),
                        Row::Open(item) => (
                            item.title.clone(),
                            (!item.detail.is_empty()).then(|| item.detail.clone()),
                            Some(icons::FILE),
                            None,
                        ),
                    };
                    let bg = if i == pick && !phone { "var(--as-selected)" } else { "transparent" };
                    let run = run.clone();
                    rsx! {
                        div {
                            key: "{i}-{label}",
                            class: "as-item as-palette-row",
                            role: "option",
                            "aria-selected": "{i == pick}",
                            style: "display: flex; align-items: center; gap: 12px; min-height: 44px; padding: 4px 8px; border-radius: 10px; background-color: {bg};",
                            onclick: move |_| run(&row),
                            div { style: "width: 30px; height: 30px; flex: none; border-radius: 8px; background-color: var(--as-raised); display: flex; align-items: center; justify-content: center; color: var(--as-muted);",
                                IconView { icon: icon.unwrap_or(icons::FILE), size: 16.0 }
                            }
                            div { style: "flex: 1 1 0px; min-width: 0; display: flex; flex-direction: column; overflow: hidden;",
                                span { style: "white-space: nowrap; overflow: hidden;", "{label}" }
                                if let Some(d) = detail {
                                    span { style: "font-size: 12px; color: var(--as-muted); white-space: nowrap; overflow: hidden;", "{d}" }
                                }
                            }
                            if let Some(k) = shortcut {
                                span { style: "flex: none; font-size: 12px; color: var(--as-muted);", "{k}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::FnSource;

    fn entries() -> Vec<PaletteItem> {
        vec![
            PaletteItem::call("Upload", Callback::new(|()| {})).keywords("file add"),
            PaletteItem::call("New folder", Callback::new(|()| {})),
            PaletteItem::call("Label", Callback::new(|()| {})).keywords("show tag"),
        ]
    }

    #[test]
    fn the_apps_entries_rank_by_match_and_recency() {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        dom.in_scope(dioxus::dioxus_core::ScopeId::ROOT, || {
            let items = entries();
            assert_eq!(rows(&items, None, "", false).len(), 3);
            assert_eq!(rows(&items, None, "fold", false), vec![Row::App(1)]);
            assert_eq!(rows(&items, None, "tag", false), vec![Row::App(2)]);
        });
    }

    #[test]
    fn sources_and_openers_join_the_rows() {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        dom.in_scope(dioxus::dioxus_core::ScopeId::ROOT, || {
            let sources = PaletteSources {
                inner: Rc::new(RefCell::new(Inner {
                    engine: Engine::new(),
                    runs: HashMap::new(),
                    openers: Openers::new(),
                    recents: Recents::default(),
                })),
                revision: Signal::new(0),
            };
            let ran = Rc::new(RefCell::new(Vec::<String>::new()));
            let heard = Rc::clone(&ran);
            sources.register(
                FnSource::new("notes", "Notes", |_| vec![Item::new("n1", "Plan the week")]),
                move |item: &Item| heard.borrow_mut().push(item.id.clone()),
            );
            sources.register_opener(Opener::new(
                "file",
                "Files",
                |_| vec![OpenItem::new("file", "a.md", "plan.md")],
                |_, _| {},
            ));
            let items = entries();
            let found = rows(&items, Some(&sources), "plan", false);
            assert!(
                found
                    .iter()
                    .any(|r| matches!(r, Row::Source(i) if i.id == "n1"))
            );
            assert!(
                found
                    .iter()
                    .any(|r| matches!(r, Row::Open(o) if o.id == "a.md"))
            );
            // ⌘O: the openers' only.
            let open = rows(&items, Some(&sources), "", true);
            assert_eq!(open.len(), 1);
            let Some(Row::Source(item)) = found.into_iter().find(|r| matches!(r, Row::Source(_)))
            else {
                panic!("a source's row");
            };
            sources.run(&item);
            assert_eq!(*ran.borrow(), ["n1"]);
            assert_eq!(sources.recents().rank("n1"), Some(0));
        });
    }
}
