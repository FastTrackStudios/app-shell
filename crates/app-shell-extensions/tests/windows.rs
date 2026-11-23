//! Extensions with each other and with the app's windows
//! (`architect_extensions::interop`): the manifest's `[[windows]]`,
//! `[[points]]`, `[[contribute."<point>"]]`, `[provides]` / `[requires]`
//! and `reserved`, read and refused; a point's items checked against its
//! schema (a bad one a problem, not a failed load) and delivered to its
//! owner live as extensions load and unload; a service called across two
//! extensions (linked, Luau), with its provider missing or too old; a
//! window opened as a sheet (the shell's, from the first render) and as
//! an OS window.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use app_shell::ui::{AppShell, FormFactor, Tabs, WindowSystem, Windows};
use app_shell::{PaneId, Rect, WindowId, Workspace};
use architect::action::ActionList;
use architect_extensions::extensions::Extensions;
use architect_extensions::host::{Host, SharedSections};
use architect_extensions::install::Installs;
use architect_extensions::linked::{Builder, Extension};
use architect_extensions::{Origin, Package};
use architect_extensions_dioxus::BuilderExt as _;
use architect_settings::Registry;
use dioxus::prelude::*;
use futures::executor::block_on;
use serde_json::{Value, json};

fn host(list: &ActionList) -> Host {
    Host::new(list.clone(), SharedSections::new(Registry::new()))
}

#[allow(clippy::needless_pass_by_value)] // json!(…) at every call.
fn run(list: &ActionList, id: &str, args: Value) -> Result<Value, String> {
    block_on(list.run_json_async(id, args.to_string().as_bytes()))
        .map(|out| serde_json::from_slice(&out).unwrap_or(Value::Null))
        .map_err(|e| e.to_string())
}

// ---- windows ------------------------------------------------------------

thread_local! {
    static DRAWN: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    static OPENED: RefCell<Vec<(WindowId, String)>> = const { RefCell::new(Vec::new()) };
    static LAYOUT: RefCell<Option<Signal<Workspace>>> = const { RefCell::new(None) };
    static CLOSED: RefCell<Vec<WindowId>> = const { RefCell::new(Vec::new()) };
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
}

struct Platform;

impl WindowSystem for Platform {
    fn open(&self, id: &WindowId, title: &str, _: Option<Rect>) {
        OPENED.with(|o| o.borrow_mut().push((id.clone(), title.to_owned())));
    }
    fn close(&self, window: &WindowId) {
        CLOSED.with(|c| c.borrow_mut().push(window.clone()));
    }
}

const PANELS: &str = r#"
id = "lab.panels"
name = "Panels"
version = "0.1.0"
kind = "process"

[entry]
path = "panels"

[[actions]]
id = "LAB_PANELS_PREFS"
name = "Preferences…"

[[actions]]
id = "LAB_PANELS_MIXER"
name = "Mixer"

[[views]]
id = "prefs"
title = "Preferences"

[[views]]
id = "mixer"
title = "Mixer"

[[windows]]
id = "prefs"
view = "prefs"
title = "Preferences"
kind = "sheet"
opens = "LAB_PANELS_PREFS"

[[windows]]
id = "mixer"
view = "mixer"
title = "Mixer"
kind = "window"
opens = "LAB_PANELS_MIXER"
"#;

#[component]
fn Prefs() -> Element {
    DRAWN.with(|d| d.borrow_mut().push("prefs"));
    rsx! { "the preferences" }
}

#[component]
fn Mixer() -> Element {
    DRAWN.with(|d| d.borrow_mut().push("mixer"));
    rsx! { "the mixer" }
}

struct Panels;

impl Extension for Panels {
    fn manifest(&self) -> &'static str {
        PANELS
    }

    fn build(&self, ext: &mut Builder) {
        ext.view("prefs", Prefs).view("mixer", Mixer);
    }
}

fn window_app() -> Element {
    use_context_provider(|| Windows(Rc::new(Platform)));
    let host = HOST.with(|h| h.borrow().clone().unwrap());
    let layout = use_signal(Workspace::new);
    LAYOUT.with(|l| *l.borrow_mut() = Some(layout));
    let ui = app_shell_extensions::interface::use_extension_ui(host, layout);
    rsx! {
        AppShell {
            layout,
            kinds: ui.tab_kinds(),
            tabs: Tabs::On,
            form: FormFactor::Desktop,
            layers: rsx! { app_shell_extensions::interface::ExtensionOverlays { ui } },
        }
    }
}

/// Lets the window's futures run, then draws.
fn settle(dom: &mut VirtualDom) {
    for _ in 0..5 {
        block_on(async {
            let wait = dom.wait_for_work();
            let timeout = futures_timer::Delay::new(Duration::from_millis(50));
            futures::pin_mut!(wait);
            let _ = futures::future::select(wait, timeout).await;
        });
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
    }
}

#[test]
fn a_window_opens_as_a_sheet_and_as_an_os_window() {
    let tmp = tempfile::tempdir().unwrap();
    let list = ActionList::new();
    let ext = Extensions::new(host(&list), Installs::new(tmp.path().join("i")))
        .with_linked(&Panels)
        .unwrap();
    ext.install_contribution_actions().unwrap();
    let _ = ext.sync();
    HOST.with(|h| *h.borrow_mut() = Some(ext.host().clone()));
    let mut main = VirtualDom::new(window_app);
    main.rebuild_in_place();
    assert!(DRAWN.with(|d| d.borrow().is_empty()), "nothing open yet");

    // A sheet: drawn over the main window, in its document.
    run(&list, "LAB_PANELS_PREFS", json!({"tab": "audio"})).unwrap();
    settle(&mut main);
    assert_eq!(DRAWN.with(|d| d.borrow().clone()), ["prefs"]);
    // The shell's own sheet, with no tab drawn yet.
    let html = dioxus_ssr::render(&main);
    assert!(html.contains("as-sheet"), "{html}");
    let windows = run(&list, "EXT_WINDOWS", Value::Null).unwrap();
    let prefs = windows["windows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["id"] == "prefs")
        .unwrap();
    assert_eq!(prefs["open"], true, "{windows}");
    assert_eq!(prefs["kind"], "sheet");
    assert_eq!(prefs["state"], json!({"tab": "audio"}));
    assert!(ext.host().close_window("lab.panels", "prefs"));

    // A window: a pane of its view in a window of the layout, which the
    // shell asks the platform to open; the person closing it (its pane
    // back in the main window) closes it in the host.
    DRAWN.with(|d| d.borrow_mut().clear());
    run(&list, "LAB_PANELS_MIXER", Value::Null).unwrap();
    settle(&mut main);
    let layout = LAYOUT.with(|l| l.borrow().unwrap());
    let pane = PaneId::new("ext-window:lab.panels/mixer");
    let at = main
        .in_runtime(|| layout.peek().locate(&pane))
        .expect("its pane");
    assert_ne!(at.window, 0, "in a window of its own");
    let opened = OPENED.with(|o| o.borrow().clone());
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert_eq!(opened[0].1, "Mixer");
    let window = opened[0].0.clone();
    main.in_runtime(|| {
        let mut layout = layout;
        layout.write().to_main_window(&pane).unwrap();
    });
    settle(&mut main);
    assert!(ext.host().open_windows().is_empty());
    assert!(main.in_runtime(|| layout.peek().pane(&pane).is_none()));

    // Closed by the host: its pane and its OS window close.
    run(&list, "LAB_PANELS_MIXER", Value::Null).unwrap();
    settle(&mut main);
    assert!(main.in_runtime(|| layout.peek().pane(&pane).is_some()));
    let second = OPENED.with(|o| o.borrow().last().cloned()).unwrap().0;
    assert_ne!(second, window, "a window's id is never reused");
    let _ = run(
        &list,
        "EXT_WINDOW_CLOSE",
        json!({"extension": "lab.panels", "id": "mixer"}),
    );
    settle(&mut main);
    assert!(main.in_runtime(|| layout.peek().pane(&pane).is_none()));
    assert!(
        CLOSED.with(|c| c.borrow().contains(&second)),
        "its OS window closed"
    );
    // Unloaded: its windows close.
    run(&list, "LAB_PANELS_PREFS", Value::Null).unwrap();
    assert!(ext.host().unload("lab.panels"));
    assert!(ext.host().open_windows().is_empty());
}

thread_local! {
    /// The view kinds the window listed, at each of its renders.
    static LISTED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn views_app() -> Element {
    let host = HOST.with(|h| h.borrow().clone().unwrap());
    let ui = app_shell_extensions::interface::use_extension_ui(
        host,
        use_signal(app_shell::Workspace::new),
    );
    let kinds: Vec<String> = ui.tab_kinds().iter().map(|k| k.id.clone()).collect();
    LISTED.with(|l| *l.borrow_mut() = kinds);
    rsx! {}
}

/// An extension that adds nothing but a view, loaded after the window is
/// up (a development copy, #844): its view is a pane kind at once,
/// whatever else the window does, so the shell draws its panes.
#[test]
fn a_view_of_an_extension_loaded_later_is_listed_at_once() {
    let tmp = tempfile::tempdir().unwrap();
    let list = ActionList::new();
    let host = host(&list);
    HOST.with(|h| *h.borrow_mut() = Some(host.clone()));
    let mut main = VirtualDom::new(views_app);
    main.rebuild_in_place();
    settle(&mut main);
    assert!(LISTED.with(|l| l.borrow().is_empty()));
    let dir = tmp.path().join("boards");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("extension.toml"),
        r#"
id = "lab.boards"
name = "Boards"
version = "0.1.0"
kind = "luau"
api = "0.1"

[[views]]
id = "board"
title = "Board"
page = "board.tsx"
"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("board.tsx"),
        "export default function Board() { return <div>board</div>; }\n",
    )
    .unwrap();
    host.load(&Package::read(&dir).unwrap(), Origin::Dev)
        .unwrap();
    settle(&mut main);
    assert_eq!(
        LISTED.with(|l| l.borrow().clone()),
        ["lab.boards/board"],
        "the view isn't a pane kind"
    );
    // Saved with a second view and nothing else changed (its
    // contributions the same): reloaded, both are pane kinds.
    let manifest = std::fs::read_to_string(dir.join("extension.toml")).unwrap();
    std::fs::write(
        dir.join("extension.toml"),
        format!("{manifest}\n[[views]]\nid = \"wall\"\ntitle = \"Wall\"\npage = \"board.tsx\"\n"),
    )
    .unwrap();
    host.load(&Package::read(&dir).unwrap(), Origin::Dev)
        .unwrap();
    settle(&mut main);
    assert_eq!(
        LISTED.with(|l| l.borrow().clone()),
        ["lab.boards/board", "lab.boards/wall"],
        "the reloaded copy's new view isn't a pane kind"
    );
}
