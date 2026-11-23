//! An extension's view in app-shell's panes, end to end: a linked
//! extension's view opened through the host (`Host::open_view`, what
//! `EXT_VIEW_OPEN` runs) lands as a pane of the app's layout, the shell
//! draws it from the kind `ExtensionUi::tab_kinds` registers, and the view
//! reads its pane (id, state, view state) through the `PaneHostRef` the
//! shell's `TabHooks` put around it; the view's own `ViewState::set` is
//! kept on the pane. The extension's tabs, status items and slots reach
//! the shell's chrome as destinations and bar items, its palette source
//! the shell's palette rows.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::cell::RefCell;
use std::time::Duration;

use app_shell::ui::{AppShell, FormFactor, PaletteSources, Tabs};
use app_shell::{PaneId, Workspace};
use app_shell_extensions::{ExtensionOverlays, ExtensionUi, use_extension_ui};
use architect::action::{ActionList, Invocation, InvocationSource};
use architect_extensions::extensions::Extensions;
use architect_extensions::host::{Host, SharedSections};
use architect_extensions::install::Installs;
use architect_extensions::linked::{Builder, Extension};
use architect_extensions_dioxus::BuilderExt as _;
use architect_extensions_dioxus::pane::{use_pane_state, use_view_state};
use architect_settings::Registry;
use dioxus::prelude::*;
use futures::executor::block_on;

const NOTES: &str = r#"
id = "lab.notes"
name = "Notes"
version = "0.1.0"
kind = "process"

[entry]
path = "notes"

[[views]]
id = "editor"
title = "Editor"
icon = "notebook-pen"

[[tabs]]
id = "notes"
title = "Notes"
icon = "notebook"
view = "editor"

[[status]]
id = "words"
text = "42 words"

[[actions]]
id = "LAB_NOTES_FIND"
name = "Find notes"
params = '{"type":"object","properties":{"query":{"type":"string"}}}'

[[actions]]
id = "LAB_NOTES_GO"
name = "Go to a note"
params = '{"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}'

[[views]]
id = "finder"
title = "Find a note"

[[palette_views]]
overlay = "palette"
view = "finder"

[[palette_sources]]
id = "notes"
title = "Notes"
action = "LAB_NOTES_FIND"
run = "LAB_NOTES_GO"
"#;

/// The view: what its pane says, and its view state kept on first draw.
#[component]
fn Editor() -> Element {
    let pane = use_pane_state();
    let kept = use_view_state();
    if let Some(v) = &kept
        && v.value.is_empty()
    {
        v.set("scroll=7");
    }
    let (id, state) = pane.unwrap_or_default();
    let view_state = kept.map(|v| v.value).unwrap_or_default();
    rsx! { "editor in {id} showing {state} at {view_state}" }
}

/// Its palette: ⌘K drawn by the extension.
#[component]
fn Finder() -> Element {
    rsx! { "notes finder" }
}

struct Notes;

impl Extension for Notes {
    fn manifest(&self) -> &'static str {
        NOTES
    }

    fn build(&self, ext: &mut Builder) {
        ext.view("editor", Editor)
            .view("finder", Finder)
            .action("LAB_NOTES_FIND", |_| async {
                Ok(serde_json::json!({ "items": [
                    { "id": "n1", "label": "Plan the week" },
                    { "id": "n2", "label": "Groceries" },
                ] }))
            })
            .action("LAB_NOTES_GO", |args| async move {
                WENT.with(|w| {
                    w.borrow_mut()
                        .push(args["id"].as_str().unwrap_or_default().to_owned())
                });
                Ok(serde_json::Value::Null)
            });
    }
}

thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
    static UI: RefCell<Option<ExtensionUi>> = const { RefCell::new(None) };
    static LAYOUT: RefCell<Option<Signal<Workspace>>> = const { RefCell::new(None) };
    static SOURCES: RefCell<Option<PaletteSources>> = const { RefCell::new(None) };
    static WENT: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn app() -> Element {
    let host = HOST.with(|h| h.borrow().clone().unwrap());
    let layout = use_signal(Workspace::new);
    let ui = use_extension_ui(host, layout);
    UI.with(|u| *u.borrow_mut() = Some(ui));
    LAYOUT.with(|l| *l.borrow_mut() = Some(layout));
    let sources = use_context::<PaletteSources>();
    SOURCES.with(|s| *s.borrow_mut() = Some(sources));
    rsx! {
        AppShell {
            layout,
            kinds: ui.tab_kinds(),
            tabs: Tabs::On,
            form: FormFactor::Desktop,
            destinations: ui.destinations(),
            layers: rsx! { ExtensionOverlays { ui } },
        }
    }
}

/// Lets the window's futures run, then draws.
fn settle(dom: &mut VirtualDom) {
    for _ in 0..6 {
        block_on(async {
            let wait = dom.wait_for_work();
            let timeout = futures_timer::Delay::new(Duration::from_millis(30));
            futures::pin_mut!(wait);
            let _ = futures::future::select(wait, timeout).await;
        });
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
    }
}

#[test]
fn an_extensions_view_opens_in_a_pane_and_reads_its_pane() {
    let tmp = tempfile::tempdir().unwrap();
    let list = ActionList::new();
    let host = Host::new(list.clone(), SharedSections::new(Registry::new()));
    let ext = Extensions::new(host, Installs::new(tmp.path().join("i")))
        .with_linked(&Notes)
        .unwrap();
    let _ = ext.sync();
    HOST.with(|h| *h.borrow_mut() = Some(ext.host().clone()));
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    settle(&mut dom);

    let ui = UI.with(|u| u.borrow().unwrap());
    let kinds: Vec<String> =
        dom.in_runtime(|| ui.tab_kinds().iter().map(|k| k.id.clone()).collect());
    assert!(kinds.contains(&"lab.notes/editor".to_owned()), "{kinds:?}");
    let destinations: Vec<String> =
        dom.in_runtime(|| ui.destinations().into_iter().map(|d| d.id).collect());
    assert_eq!(destinations, ["lab.notes/notes"]);
    let bar: Vec<String> = dom.in_runtime(|| ui.bar_items().into_iter().map(|i| i.id).collect());
    assert_eq!(bar, ["ext:lab.notes/words"]);

    ext.host()
        .open_view(
            "lab.notes",
            "editor",
            Some("todo.md".to_owned()),
            None,
            Invocation::new(InvocationSource::App),
        )
        .unwrap();
    settle(&mut dom);

    let layout = LAYOUT.with(|l| l.borrow().unwrap());
    let pane = dom
        .in_runtime(|| {
            layout
                .peek()
                .pane(&PaneId::new("lab.notes/editor"))
                .cloned()
        })
        .expect("a pane of the view's kind");
    assert_eq!(pane.state, "todo.md");
    assert_eq!(pane.view.0, "lab.notes/editor");
    // Its view kept where it is on the pane.
    assert_eq!(pane.view_state, "scroll=7");

    let html = dioxus_ssr::render(&dom);
    assert!(
        html.contains("editor in lab.notes/editor showing todo.md at scroll=7"),
        "{html}"
    );

    // Opened again: the same pane comes forward, no second one.
    ext.host()
        .open_view(
            "lab.notes",
            "editor",
            Some("todo.md".to_owned()),
            None,
            Invocation::new(InvocationSource::App),
        )
        .unwrap();
    settle(&mut dom);
    let panes = dom.in_runtime(|| layout.peek().panes.len());
    assert_eq!(panes, 1);
}

#[test]
fn an_extensions_palette_source_gives_the_palettes_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let list = ActionList::new();
    let host = Host::new(list.clone(), SharedSections::new(Registry::new()));
    let ext = Extensions::new(host, Installs::new(tmp.path().join("i")))
        .with_linked(&Notes)
        .unwrap();
    let _ = ext.sync();
    HOST.with(|h| *h.borrow_mut() = Some(ext.host().clone()));
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    settle(&mut dom);
    let sources = SOURCES.with(|s| s.borrow().clone()).unwrap();
    // A linked action may answer later: the palette is told to ask again.
    let _ = dom.in_scope(ScopeId::ROOT, || sources.search("plan"));
    settle(&mut dom);
    let rows = dom.in_scope(ScopeId::ROOT, || sources.search("plan"));
    let first = rows.first().expect("a row").item.clone();
    assert_eq!(
        (first.id.as_str(), first.label.as_str()),
        ("n1", "Plan the week")
    );
    assert_eq!(first.source, "notes");
    dom.in_scope(ScopeId::ROOT, || sources.run(&first));
    settle(&mut dom);
    assert_eq!(WENT.with(|w| w.borrow().clone()), ["n1"]);
}

#[test]
fn an_extensions_palette_view_draws_the_palette_and_commands_show_opens_it() {
    let tmp = tempfile::tempdir().unwrap();
    let list = ActionList::new();
    let host = Host::new(list.clone(), SharedSections::new(Registry::new()));
    let ext = Extensions::new(host, Installs::new(tmp.path().join("i")))
        .with_linked(&Notes)
        .unwrap();
    let _ = ext.sync();
    HOST.with(|h| *h.borrow_mut() = Some(ext.host().clone()));
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    settle(&mut dom);
    let ui = UI.with(|u| u.borrow().unwrap());
    // A view the palette draws is not a tab kind.
    let kinds: Vec<String> =
        dom.in_runtime(|| ui.tab_kinds().iter().map(|k| k.id.clone()).collect());
    assert!(!kinds.contains(&"lab.notes/finder".to_owned()), "{kinds:?}");
    // A tab drawn hands the extensions the shell.
    ext.host()
        .open_view(
            "lab.notes",
            "editor",
            None,
            None,
            Invocation::new(InvocationSource::App),
        )
        .unwrap();
    settle(&mut dom);

    let run = |dom: &mut VirtualDom, args: &str| {
        let out = block_on(list.run_json_async("COMMANDS_SHOW", args.as_bytes()));
        assert!(out.is_ok(), "{out:?}");
        settle(dom);
        dioxus_ssr::render(dom)
    };
    let html = run(&mut dom, r#"{"overlay":"palette"}"#);
    assert!(html.contains("notes finder"), "{html}");
    // ⌘O is still the shell's own, its field holding the query.
    let html = run(&mut dom, r#"{"overlay":"quick-open","query":"plan"}"#);
    assert!(!html.contains("notes finder"), "{html}");
    assert!(html.contains("value=\"plan\""), "{html}");
    let html = run(&mut dom, r#"{"overlay":"none"}"#);
    assert!(!html.contains("Open anything"), "{html}");
}
