//! What loaded extensions add, in an app-shell window:
//!
//! the shell's chrome (`crate::bar`: their tabs as destinations, status
//! items and bar buttons in the top bar, the slots they fill), a tab's and
//! a file's context menus, the menu bar's rows, their toolbars
//! (`crate::toolbar_ui`), their views as the shell's tab kinds
//! (`crate::panes`), where those open (tab, split, dock, float, zoomed,
//! popup, overlay), and a link's Ctrl- or Cmd-click
//! (Architect's `docs/content/architecture/extensions.md`, "Contributions").
//!
//! ```ignore
//! let layout = use_signal(Workspace::new);
//! let ui = use_extension_ui(host.clone(), layout);
//! rsx! {
//!     AppShell { layout, kinds: ui.tab_kinds(), destinations: ui.destinations(), … }
//!     ExtensionOverlays { ui }
//! }
//! // and, in a component under the shell (its top bar):
//! use_extension_bar(ui);
//! ```
//!
//! An extension's views are mounted by [`ExtensionView`]: the component
//! of its linked-in copy, or its process's remote view (feature
//! `remote`), or — once it is unloaded — a placeholder saying so (a view
//! whose manifest says `on_unload = "close"` has its panes closed).
//! An app that runs pages provides [`ViewPages`]: a view with a page file
//! (its manifest's `page`, or an override in the app's folder) is drawn
//! from that file instead (`architect-pages`). An app that hosts web views
//! provides [`WebViews`]: a view of kind `web` (#222) is drawn by it, its
//! package's `src` in a native web view or a sandboxed frame
//! (`architect_extensions::webview`).

use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use app_shell::ui::Shell;
use app_shell::{Pane, PaneId, Rect, Side, Target, Workspace};
use architect::action::{Invocation, InvocationSource};
use dioxus::prelude::*;
use futures::StreamExt as _;
use futures::channel::mpsc;
use serde_json::json;

use crate::panes::home_target;

use architect_extensions::contrib::{self, Contributions};
use architect_extensions::host::{Host, Opening, ViewOpen, now_ms};
use architect_extensions::manifest::{Home, OnUnload, ViewDecl, ViewPlacement};

/// The contributions' revision as a signal: it moves when they change,
/// and when a piece of runtime state lapses.
#[must_use]
pub fn use_contributions(contributions: &Contributions) -> Signal<u64> {
    let mut revision = use_signal(|| contributions.revision());
    let reg = contributions.clone();
    use_future(move || {
        let reg = reg.clone();
        async move {
            let (tx, mut rx) = mpsc::unbounded::<u64>();
            let _held = reg.subscribe(move |r| {
                let _ = tx.unbounded_send(r);
            });
            loop {
                // A lapse is a change too: wake for the next one.
                let lapse = reg
                    .next_expiry()
                    .map(|at| at.saturating_sub(now_ms()).max(50));
                let next = match lapse {
                    Some(ms) => {
                        let wait = futures_timer::Delay::new(std::time::Duration::from_millis(ms));
                        futures::select! {
                            r = rx.next() => r,
                            () = futures::FutureExt::fuse(wait) => Some(reg.revision()),
                        }
                    }
                    None => rx.next().await,
                };
                if next.is_none() {
                    return;
                }
                // Several at once are one redraw.
                while rx.try_recv().is_ok() {}
                let n = revision.peek().wrapping_add(1);
                revision.set(n);
            }
        }
    });
    revision
}

/// A view an extension opened over the window (`popup`, `overlay`).
#[derive(Clone, Debug, PartialEq)]
struct Layer {
    open: ViewOpen,
}

/// What loaded extensions add, for the window. Cheap to copy.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ExtensionUi {
    host: Signal<Host>,
    revision: Signal<u64>,
    layers: Signal<Vec<Layer>>,
    /// Every view an extension declared since the app started, by kind:
    /// what an unloaded one's panes show (or close by).
    seen: Signal<BTreeMap<String, (String, ViewDecl)>>,
    /// The extensions' windows open now (`crate::window_ui`).
    windows: Signal<Vec<architect_extensions::windows::OpenWindow>>,
    /// Moves when anything happens on the host's Action List (an action
    /// ran, the app's state changed): what a toggle's button re-reads.
    ticks: Signal<u64>,
    /// The toolbars that are panes of their own, by kind, with their
    /// title and place: what the views follow (a person's new toolbar).
    pub(crate) toolbar_panes: Signal<Vec<(String, String, String)>>,
    /// The layout the views open in: the app's, which its `AppShell` draws.
    pub(crate) layout: Signal<Workspace>,
    /// The shell of a window drawing that layout, once one has drawn
    /// (`ExtensionUi::attach`): changes go through it.
    pub(crate) shell: CopyValue<Option<Shell>>,
}

impl ExtensionUi {
    /// The host.
    #[must_use]
    pub fn host(&self) -> Host {
        self.host.peek().clone()
    }

    /// The contributions' revision (reading it subscribes).
    #[must_use]
    pub fn revision(&self) -> u64 {
        (self.revision)()
    }

    /// How many times something happened on the host's Action List since
    /// the window opened (reading it subscribes): a toggle may have moved.
    #[must_use]
    pub fn ticks(&self) -> u64 {
        (self.ticks)()
    }

    /// The extensions' windows open now (reading it subscribes).
    #[must_use]
    pub fn windows(&self) -> Vec<architect_extensions::windows::OpenWindow> {
        (self.windows)()
    }

    /// The extension and tab a contributed tab's id names
    /// (`lab.board/board`), when it is one.
    #[must_use]
    pub fn tab(&self, id: &str) -> Option<(String, contrib::TabDecl)> {
        let host = self.host();
        host.contributions()
            .tabs()
            .into_iter()
            .find(|by| format!("{}/{}", by.owner, by.item.id) == id)
            .map(|by| (by.owner, by.item))
    }

    /// Whether `id` is a contributed tab's.
    #[must_use]
    pub fn is_tab(&self, id: &str) -> bool {
        self.tab(id).is_some()
    }

    /// The menu bar's contributed rows for menu `menu`: each an action,
    /// ticked while a toggle is on.
    #[must_use]
    pub fn menu_rows(&self, menu: &str) -> Vec<crate::panes::ContextEntry> {
        let host = self.host();
        host.contributions()
            .menus(menu)
            .into_iter()
            .filter_map(|by| {
                let info = host.list().info(&by.item.action)?;
                Some(crate::panes::ContextEntry {
                    label: info.display_name,
                    command: by.item.action,
                    checked: info.checked,
                })
            })
            .collect()
    }

    /// Every view a loaded extension declares, and each one seen before
    /// that is gone (its panes say so, or close), by the shell's kind
    /// (`<extension>/<view>`). Reading it subscribes.
    #[must_use]
    pub(crate) fn view_decls(&self) -> Vec<(String, String, ViewDecl)> {
        let _ = (self.revision)();
        let _ = (self.toolbar_panes)();
        let mut all = self.seen.peek().clone();
        all.extend(declared(&self.host()));
        all.into_iter()
            .map(|(kind, (extension, decl))| (kind, extension, decl))
            .collect()
    }

    /// Whether a loaded extension has a view whose home is dock `side`
    /// (the navigator's on the left): a page can fold a dock nothing fills.
    /// Read again when an extension loads or unloads.
    #[must_use]
    pub fn fills_dock(&self, side: Side) -> bool {
        let _ = (self.revision)();
        declared(&self.host())
            .values()
            .any(|(_, decl)| home_target(decl.home) == Target::Dock(side))
    }

    /// Opens `url` with the extension opener that claims it, as a link's
    /// Ctrl- or Cmd-click does: false when none does.
    #[must_use = "false when nothing opens it"]
    pub fn open_link(&self, url: &str) -> bool {
        let host = self.host();
        if host.contributions().open_url(url).is_empty() {
            return false;
        }
        let url = url.to_owned();
        spawn(async move {
            let _ = host
                .open(Opening::Url(url), Invocation::new(InvocationSource::Link))
                .await;
        });
        true
    }

    /// Opens the file at `path` with the extension opener that claims it:
    /// false when none does.
    #[must_use = "false when nothing opens it"]
    pub fn open_path(&self, path: &str) -> bool {
        let host = self.host();
        if host.contributions().open_path(path).is_empty() {
            return false;
        }
        let path = path.to_owned();
        spawn(async move {
            let _ = host
                .open(Opening::Path(path), Invocation::new(InvocationSource::Open))
                .await;
        });
        true
    }

    /// Takes a popup or an overlay away.
    pub fn close_layer(&self, kind: &str) {
        let mut layers = self.layers;
        layers.write().retain(|l| l.open.kind != kind);
    }
}

/// Every loaded extension's views, by workspace kind.
fn declared(host: &Host) -> BTreeMap<String, (String, ViewDecl)> {
    host.loaded()
        .into_iter()
        .flat_map(|l| {
            let id = l.manifest.id.clone();
            // A view the palette slot draws (`[[palette_views]]`) is an
            // overlay of the shell's, not a pane.
            let slotted: Vec<String> = l
                .manifest
                .contributes
                .palette_views
                .iter()
                .map(|p| p.view.clone())
                .collect();
            l.manifest
                .views
                .into_iter()
                .filter(move |v| !slotted.contains(&v.id))
                .map(move |v| (v.workspace_kind(&id), (id.clone(), v)))
        })
        .collect()
}

/// Whether a link press is the one that opens it with an extension: a
/// press with Ctrl or Cmd held.
#[must_use]
pub fn link_press(e: &MouseEvent) -> bool {
    let m = e.modifiers();
    m.ctrl() || m.meta()
}

/// Where `open` goes in `layout`: its home's dock, a float of its size,
/// a split beside the document on show, else a tab beside it.
fn target(open: &ViewOpen) -> Target {
    match (open.home, open.placement) {
        (Home::Left | Home::Right | Home::Bottom, _) => home_target(open.home),
        (_, ViewPlacement::Float) => Target::Float(Rect {
            x: 120.0,
            y: 90.0,
            w: open.width.unwrap_or(420.0),
            h: open.height.unwrap_or(320.0),
        }),
        (_, ViewPlacement::Split) => Target::Split(Side::Right),
        _ => Target::Tab,
    }
}

/// Opens `open` in the layout: a pane of its kind, its id
/// [`ViewOpen::pane_id`] (the kind, or a document's own), so opening the
/// same thing again brings it forward; zoomed when it says so.
fn open_in_layout(ui: ExtensionUi, open: &ViewOpen) -> Option<PaneId> {
    let mut pane = Pane::new(open.pane_id(), open.kind.clone(), open.title.clone());
    pane.state = open.state.clone().unwrap_or_default();
    let zoom = open.placement == ViewPlacement::Zoomed;
    let at = target(open);
    ui.apply(move |w| {
        let id = w.open(pane, at)?;
        if zoom && let Some(group) = w.locate(&id).map(|l| l.group) {
            w.set_zoomed(group, true)?;
        }
        Ok::<_, app_shell::WorkspaceError>(id)
    })
}

/// Puts what `host`'s extensions add into the window: call it in the
/// app's root component, above the shell. `layout` is the app's layout,
/// the one its `AppShell` draws: views open in it.
///
/// It provides the shell's [`app_shell::ui::TabHooks`] (each pane's handle
/// for its view, its header's toolbars, an extension's `[[context]]` rows,
/// a tab's badges) and sets the host's view opener; read the rest from the
/// [`ExtensionUi`] it gives.
pub fn use_extension_ui(host: Host, layout: Signal<Workspace>) -> ExtensionUi {
    dioxus::prelude::use_hook(crate::install);
    let revision = use_contributions(host.contributions());
    let layers = use_signal(Vec::<Layer>::new);
    let seen = use_signal(BTreeMap::new);
    let held = use_signal(|| host.clone());
    let windows = crate::window_ui::use_open_windows(&host);
    let ticks = crate::toolbar_ui::use_ticks(&host);
    let toolbar_panes = use_signal(Vec::new);
    let shell = use_hook(|| CopyValue::new(None));
    let ui = ExtensionUi {
        host: held,
        revision,
        layers,
        seen,
        windows,
        ticks,
        toolbar_panes,
        layout,
        shell,
    };
    // Its windows: sheets of the shell's, or windows of the layout.
    crate::window_ui::use_window_panes(ui);
    crate::window_ui::set_main(ui);
    // The settings page is the settings extension's window: the app's
    // sections and values to it, `SettingsHandle::show` opening it.
    crate::window_ui::use_settings_window(&host, windows);
    // The settings page draws an extension's section from its view
    // (`[[sections]] view`), a linked one's component as it is.
    use_context_provider(move || {
        architect_settings::SectionViews(Rc::new(move |kind: &str| {
            let (extension, view) = kind.split_once('/')?;
            let host = ui.host();
            if let Some(root) = host.linked_view(extension, view) {
                return Some(rsx! { LinkedRoot { key: "{kind}", root } });
            }
            host.get(extension)?;
            Some(rsx! {
                div { style: "flex: 1; min-height: 480px; display: flex; flex-direction: column;",
                    ExtensionView { ui, extension: extension.to_owned(), view: view.to_owned() }
                }
            })
        }))
    });
    // The shell's palette: the extensions' sources (`[[palette_sources]]`)
    // in ⌘K, their openers (`[[openers]]` with a kind) in ⌘O.
    let sources = app_shell::ui::use_palette_sources();
    crate::palette::use_quick_open(host.clone(), revision, sources.clone());
    crate::palette::use_palette(host.clone(), revision, sources);
    // Around every tab: its pane's handle, its header's toolbars, the
    // extensions' rows in its menu, what they set on it.
    use_context_provider(move || ui.tab_hooks());
    // The toolbar editor's state, shared by the window's toolbars.
    let _ = crate::toolbar_editor::provide_editor();
    // A toolbar that is a pane of its own (a dock's, a float, a window)
    // opens where it goes as it appears, and closes as it goes.
    crate::toolbar_ui::use_pane_toolbars(ui);
    // Its views open in the window: a pane, or a layer over it.
    let close_host = host.clone();
    let opener = use_hook(move || {
        let (tx, rx) = mpsc::unbounded::<ViewOpen>();
        let busy = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let seen_busy = Arc::clone(&busy);
        host.set_view_opener(move |open| {
            if open.placement == ViewPlacement::Popup {
                let mut b = seen_busy
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if b.iter().any(|k| *k != open.kind) {
                    return Err(format!("busy: another popup ({}) is open", b.join(", ")));
                }
                b.push(open.kind.clone());
            }
            let placed = format!("{:?}", open.placement).to_lowercase();
            tx.unbounded_send(open)
                .map_err(|_| "the window is gone".to_owned())?;
            Ok(json!({ "placement": placed }))
        });
        Rc::new(std::cell::RefCell::new(Some((rx, busy))))
    });
    // An extension's EXT_VIEW_CLOSE takes its popup or overlay away.
    let closing = use_hook(move || {
        let (tx, rx) = mpsc::unbounded::<String>();
        close_host.set_view_closer(move |kind| {
            tx.unbounded_send(kind.to_owned())
                .map_err(|_| "the window is gone".to_owned())
        });
        Rc::new(std::cell::RefCell::new(Some(rx)))
    });
    use_future(move || {
        let taken = closing.borrow_mut().take();
        async move {
            let Some(mut rx) = taken else { return };
            let mut layers = layers;
            while let Some(kind) = rx.next().await {
                layers.write().retain(|l| l.open.kind != kind);
            }
        }
    });
    let mut layers_w = layers;
    use_future(move || {
        let taken = opener.borrow_mut().take();
        async move {
            let Some((mut rx, busy)) = taken else { return };
            while let Some(open) = rx.next().await {
                match open.placement {
                    ViewPlacement::Popup | ViewPlacement::Overlay => {
                        layers_w.write().retain(|l| l.open.kind != open.kind);
                        layers_w.write().push(Layer { open });
                    }
                    _ => {
                        if open_in_layout(ui, &open).is_none() {
                            tracing::warn!(ext.id = %open.extension, ext.view = %open.kind, "a view did not open");
                        }
                    }
                }
                busy.lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .retain(|k| layers_w.peek().iter().any(|l| l.open.kind == *k));
            }
        }
    });
    // Every view seen is remembered (an unloaded one's panes say so); an
    // unloaded extension's views that close by their manifest, close; its
    // layers go.
    let mut seen_w = seen;
    use_effect(move || {
        let _ = revision();
        let host = held.peek().clone();
        let now = declared(&host);
        if now.iter().any(|(k, v)| seen_w.peek().get(k) != Some(v)) {
            seen_w.write().extend(now.clone());
        }
        let gone: Vec<(String, ViewDecl)> = seen_w
            .peek()
            .iter()
            .filter(|(k, _)| !now.contains_key(*k))
            .map(|(k, (_, d))| (k.clone(), d.clone()))
            .collect();
        if gone.is_empty() {
            return;
        }
        layers_w
            .write()
            .retain(|l| !gone.iter().any(|(k, _)| *k == l.open.kind));
        let closing: Vec<String> = gone
            .into_iter()
            .filter(|(_, d)| d.on_unload == OnUnload::Close)
            .map(|(k, _)| k)
            .collect();
        for pane in closing {
            let pane = PaneId::new(pane);
            ui.apply(|w| w.close(&pane));
        }
    });
    ui
}

/// The popups and overlays extensions opened, over the window: mount it
/// once, last, at the root.
#[component]
pub fn ExtensionOverlays(ui: ExtensionUi) -> Element {
    let layers = (ui.layers)();
    rsx! {
        crate::window_ui::ExtensionWindows { ui }
        crate::toolbar_editor::ToolbarEditorLayer { ui }
        crate::host_ui_view::HostUiLayer { host: ui.host() }
        for layer in layers {
            {
                let open = layer.open;
                let kind = open.kind.clone();
                let popup = open.placement == ViewPlacement::Popup;
                let (w, h) = (open.width.unwrap_or(480.0), open.height.unwrap_or(360.0));
                let close = kind.clone();
                rsx! {
                    div { key: "{kind}",
                        style: if popup {
                            "position: absolute; inset: 0; z-index: 80; display: flex; align-items: center; \
                             justify-content: center; background: rgba(0,0,0,0.45);"
                        } else {
                            "position: absolute; top: 72px; right: 24px; z-index: 80;"
                        },
                        onclick: move |_| if popup { ui.close_layer(&close) },
                        div {
                            style: "position: relative; width: {w}px; height: {h}px; display: flex; flex-direction: column; \
                                    border-radius: 12px; overflow: hidden; background: var(--card); \
                                    border: 1px solid var(--border); box-shadow: 0 16px 44px rgba(0,0,0,0.5);",
                            onclick: move |e| e.stop_propagation(),
                            div { style: "display: flex; align-items: center; height: 32px; padding: 0 10px; \
                                          font-size: 12px; color: var(--muted-foreground); border-bottom: 1px solid var(--border);",
                                span { style: "flex: 1;", "{open.title}" }
                                button {
                                    style: "border: none; background: transparent; color: var(--muted-foreground);",
                                    "aria-label": "Close",
                                    onclick: {
                                        let k = kind;
                                        move |_| ui.close_layer(&k)
                                    },
                                    "✕"
                                }
                            }
                            div { style: "flex: 1; min-height: 0; display: flex; flex-direction: column;",
                                ExtensionView { ui, extension: open.extension, view: open.view }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A view's page, as [`ViewPages`] is asked for it.
#[derive(Clone)]
pub struct PageAsk {
    pub extension: String,
    pub view: String,
    /// Whether it fills its pane (false: a slot, sized to its content).
    pub fill: bool,
    /// Its manifest's `page`, inside `dir`.
    pub page: Option<String>,
    /// The extension's folder, when it has one (a linked built-in has none).
    pub dir: Option<std::path::PathBuf>,
    /// What its runs may reach: the extension's capabilities.
    pub gate: architect_extensions::host::Gate,
}

impl std::fmt::Debug for PageAsk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PageAsk")
            .field("extension", &self.extension)
            .field("view", &self.view)
            .field("page", &self.page)
            .finish_non_exhaustive()
    }
}

/// Draws an extension's view from a page file when there is one for it.
///
/// `architect-pages` provides it; `None` leaves the view as it is.
/// Reading a signal inside it re-renders the view when that moves (an
/// override saved or deleted).
#[derive(Clone)]
pub struct ViewPages(pub Rc<dyn Fn(&PageAsk) -> Option<Element>>);

/// A `web` view, as [`WebViews`] is asked for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebAsk {
    pub extension: String,
    pub view: String,
    /// Its manifest's `src`, inside `dir`.
    pub src: String,
    /// The extension's folder (its package).
    pub dir: Option<std::path::PathBuf>,
}

/// Draws an extension's `web` views (#222).
///
/// A native web view clipped to
/// the pane on a desk, a sandboxed frame in a browser
/// (`architect_extensions::webview`). An app with no web view host provides none, and
/// such a view says so in its pane.
#[derive(Clone)]
pub struct WebViews(pub Rc<dyn Fn(&WebAsk) -> Element>);

pub use architect_extensions::process_view::ProcessAsk;

pub use architect_extensions_dioxus::process::ProcessViews;

/// Why a `process` view cannot be shown in an app that provides no
/// [`ProcessViews`] on `platform`.
#[must_use]
pub fn no_process_views(platform: &str) -> String {
    architect_extensions::process_view::not_here(platform).unwrap_or_else(|| {
        "this build of the app draws no process views (feature `process-views`)".to_owned()
    })
}

/// Extension `extension`'s view `view`: its linked-in component, its
/// process's remote view, or — when it is not loaded — a placeholder.
/// It fills its parent (a pane); `fill: false` sizes it to its content
/// (a slot of the shell's: a status bar, a ribbon).
#[component]
pub fn ExtensionView(
    ui: ExtensionUi,
    extension: String,
    view: String,
    #[props(default = true)] fill: bool,
    // A linked view's component as it is, in no box of its own (a
    // window's: the window is its box).
    #[props(default)] bare: bool,
) -> Element {
    let size = if fill {
        "flex: 1; min-height: 0;"
    } else {
        "flex-shrink: 0;"
    };
    let _ = (ui.revision)();
    let host = ui.host();
    let Some(loaded) = host.get(&extension) else {
        let name = extension;
        return rsx! {
            div { style: "{size} display: flex; align-items: center; justify-content: center; padding: 24px; \
                          font-size: 13px; color: var(--muted-foreground); text-align: center;",
                "{name} is not loaded: its view comes back when it does."
            }
        };
    };
    if let Some(decl) = loaded
        .manifest
        .views
        .iter()
        .find(|v| v.id == view && v.kind == architect_views::WEB)
    {
        let ask = WebAsk {
            extension: extension.clone(),
            view: view.clone(),
            src: decl.src.clone().unwrap_or_default(),
            dir: loaded.dir.clone(),
        };
        let key = format!("{extension}/{view}");
        let body = try_consume_context::<WebViews>().map_or_else(
            || {
                rsx! {
                    div { style: "flex: 1; display: flex; align-items: center; justify-content: center; padding: 24px; \
                                  font-size: 13px; color: var(--muted-foreground); text-align: center;",
                        "This app has no web view host, so {extension}'s web view cannot be shown here."
                    }
                }
            },
            |web| (web.0)(&ask),
        );
        return rsx! {
            div { key: "{key}", style: "{size} display: flex; flex-direction: column;", {body} }
        };
    }
    if loaded
        .manifest
        .views
        .iter()
        .any(|v| v.id == view && v.kind == architect_views::PROCESS)
    {
        let ask = ProcessAsk {
            extension: extension.clone(),
            view: view.clone(),
        };
        let key = format!("{extension}/{view}");
        let body = try_consume_context::<ProcessViews>().map_or_else(
            || {
                let why = no_process_views(&host.platform());
                rsx! {
                    div { style: "flex: 1; display: flex; align-items: center; justify-content: center; padding: 24px; \
                                  font-size: 13px; color: var(--muted-foreground); text-align: center;",
                        "{extension}'s view {view} cannot be shown here: {why}."
                    }
                }
            },
            |views| (views.0)(&ask),
        );
        return rsx! {
            div { key: "{key}", style: "{size} display: flex; flex-direction: column;", {body} }
        };
    }
    if let Some(pages) = try_consume_context::<ViewPages>() {
        let ask = PageAsk {
            page: loaded
                .manifest
                .views
                .iter()
                .find(|v| v.id == view)
                .and_then(|v| v.page.clone()),
            dir: loaded.dir.clone(),
            gate: architect_extensions::host::gate(&loaded.manifest),
            extension: extension.clone(),
            view: view.clone(),
            fill,
        };
        if let Some(page) = (pages.0)(&ask) {
            // A page is a sub-document, sized by its box: a slot's is a
            // bar's height, as a process view's.
            let boxed = if fill {
                "flex: 1; min-height: 0; display: flex; flex-direction: column;"
            } else {
                "flex-shrink: 0; height: 26px; display: flex; flex-direction: column;"
            };
            return rsx! {
                div { style: "{boxed}", {page} }
            };
        }
    }
    // A compiled-in view whose scenes read an engine's values reads them
    // from the same place a process copy's do: the shared table of the
    // extension providing a service it uses (an engine in a process of its
    // own). Without it the layers those values drive (a meter, a lit mute
    // or solo) draw as if every value were full.
    #[cfg(all(feature = "remote", unix))]
    if host.linked_view(&extension, &view).is_some()
        && host
            .capabilities(&extension)
            .is_some_and(|c| c.may_use(architect_extensions::capabilities::ENGINE))
    {
        let wanted = loaded.manifest.uses.iter().chain(&loaded.manifest.requires);
        let engines = wanted
            .filter_map(|(service, req)| {
                let req = semver::VersionReq::parse(req).ok()?;
                host.provider(service, &req).map(|(id, _)| id)
            })
            .filter(|id| *id != extension)
            .collect();
        architect_scene::engine::read_from(&extension, engines);
    }
    if let Some(root) = host.linked_view(&extension, &view) {
        // A list of one, so its key is diffed: a pane that goes from one
        // of its views to another (Projects' list to a project's page)
        // mounts the new one afresh, its hooks its own.
        let key = format!("{extension}/{view}");
        if bare {
            return rsx! {
                for key in std::iter::once(key) {
                    LinkedRoot { key: "{key}", root }
                }
            };
        }
        return rsx! {
            div { style: "{size} display: flex; flex-direction: column;",
                for key in std::iter::once(key) {
                    LinkedRoot { key: "{key}", root }
                }
            }
        };
    }
    #[cfg(all(feature = "remote", unix))]
    if let Some(processes) = try_use_context::<architect_extensions::process::Processes>() {
        // Its scenes draw over the whole app only when it was granted that.
        let overlay = host
            .capabilities(&extension)
            .is_some_and(|c| c.may_use(architect_extensions::capabilities::OVERLAY));
        // Its scenes read the engine's values only when it was granted them.
        let engine = host
            .capabilities(&extension)
            .is_some_and(|c| c.may_use(architect_extensions::capabilities::ENGINE));
        // An engine in a process of its own is the provider of a service
        // the view uses: its views read that engine's shared table
        // (`architect_scene::engine::shared`).
        if engine {
            let wanted = loaded.manifest.uses.iter().chain(&loaded.manifest.requires);
            let engines = wanted
                .filter_map(|(service, req)| {
                    let req = semver::VersionReq::parse(req).ok()?;
                    host.provider(service, &req).map(|(id, _)| id)
                })
                .filter(|id| *id != extension)
                .collect();
            architect_scene::engine::read_from(&extension, engines);
        }
        // A remote document is sized by its box: a slot's is a bar's height.
        let remote = if fill {
            "flex: 1; min-height: 0; display: flex; flex-direction: column;"
        } else {
            "flex-shrink: 0; height: 26px; display: flex; flex-direction: column;"
        };
        return rsx! {
            div { style: "{remote}",
                crate::remote::RemotePaneView { processes, extension, view, overlay, engine }
            }
        };
    }
    rsx! {
        div { style: "padding: 24px; font-size: 13px; color: var(--muted-foreground);",
            "{extension}'s view {view} is drawn by its process, which this app does not show."
        }
    }
}

/// The layers extensions draw with views of their own over a view of
/// kind `view`, on `plane` (`[[overlays]] draws`, `architect_extensions::overlay`): mount
/// it in the view, filling its box, where that plane goes (as
/// `architect_canvas::ui::CanvasLayers` for Luau overlays), with the
/// view's coordinate `spaces` now. Each layer covers the view's box,
/// clipped to it, and passes the pointer through to the view.
#[component]
pub fn ExtensionViewOverlays(
    view: String,
    plane: String,
    spaces: architect_extensions::overlay::OverlaySpaces,
) -> Element {
    let Some(ui) = try_use_context::<ExtensionUi>() else {
        return rsx! {};
    };
    let _ = ui.revision();
    let layers = ui.host().view_overlays(&view, &plane);
    rsx! {
        for o in layers {
            div {
                key: "{o.extension}/{o.id}",
                class: "extension-view-overlay",
                style: "position: absolute; left: 0; top: 0; right: 0; bottom: 0; \
                        overflow: hidden; pointer-events: none;",
                OverlayLayer { ui, overlay: o, over: view.clone(), spaces: spaces.clone() }
            }
        }
    }
}

/// One overlay layer: its extension's view, linked in or remote.
#[component]
fn OverlayLayer(
    ui: ExtensionUi,
    overlay: architect_extensions::overlay::ViewOverlay,
    over: String,
    spaces: architect_extensions::overlay::OverlaySpaces,
) -> Element {
    let host = ui.host();
    if let Some(root) = host.linked_view(&overlay.extension, &overlay.draws) {
        let input = architect_extensions::contrib::EmbedInput {
            claim: architect_extensions::overlay::claim(&over),
            source: String::new(),
            props: serde_json::to_value(&spaces).unwrap_or_default(),
        };
        return rsx! { LinkedOverlay { root, input } };
    }
    #[cfg(all(feature = "remote", unix))]
    if let Some(processes) = try_use_context::<architect_extensions::process::Processes>() {
        return rsx! {
            crate::remote::RemoteOverlay {
                processes,
                extension: overlay.extension,
                view: overlay.draws,
                over,
                spaces,
            }
        };
    }
    let _ = (over, spaces);
    rsx! {}
}

/// A linked view drawn with an input of the host's: as an overlay (its
/// `use_embed` is the view's spaces) or held in a remote view
/// (`architect_extensions::embedded`).
#[component]
pub(crate) fn LinkedOverlay(
    root: fn() -> Element,
    input: architect_extensions::contrib::EmbedInput,
) -> Element {
    let mut signal = use_signal(|| input.clone());
    if *signal.peek() != input {
        signal.set(input);
    }
    use_context_provider(|| architect_extensions::linked::EmbedContext(signal));
    root()
}

/// A linked-in view's component, mounted.
#[component]
fn LinkedRoot(root: fn() -> Element) -> Element {
    root()
}
