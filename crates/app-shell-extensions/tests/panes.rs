//! An extension's view in app-shell's panes, end to end: a linked
//! extension's view opened through the host (`Host::open_view`, what
//! `EXT_VIEW_OPEN` runs) lands as a pane of the app's layout, the shell
//! draws it from the kind `ExtensionUi::tab_kinds` registers, and the view
//! reads its pane (id, state, view state) through the `PaneHostRef` the
//! shell's `TabHooks` put around it; the view's own `ViewState::set` is
//! kept on the pane. The extension's tabs, status items and slots reach
//! the shell's chrome as destinations and bar items.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::cell::RefCell;
use std::time::Duration;

use app_shell::ui::{AppShell, FormFactor, Tabs};
use app_shell::{PaneId, Workspace};
use app_shell_extensions::{ExtensionUi, use_extension_ui};
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

struct Notes;

impl Extension for Notes {
    fn manifest(&self) -> &'static str {
        NOTES
    }

    fn build(&self, ext: &mut Builder) {
        ext.view("editor", Editor);
    }
}

thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
    static UI: RefCell<Option<ExtensionUi>> = const { RefCell::new(None) };
    static LAYOUT: RefCell<Option<Signal<Workspace>>> = const { RefCell::new(None) };
}

fn app() -> Element {
    let host = HOST.with(|h| h.borrow().clone().unwrap());
    let layout = use_signal(Workspace::new);
    let ui = use_extension_ui(host, layout);
    UI.with(|u| *u.borrow_mut() = Some(ui));
    LAYOUT.with(|l| *l.borrow_mut() = Some(layout));
    rsx! {
        AppShell {
            layout,
            kinds: ui.tab_kinds(),
            tabs: Tabs::On,
            form: FormFactor::Desktop,
            destinations: ui.destinations(),
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
