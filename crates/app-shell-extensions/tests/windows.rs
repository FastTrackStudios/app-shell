//! Extensions with each other and with the app's windows
//! (`architect_extensions::interop`): the manifest's `[[windows]]`,
//! `[[points]]`, `[[contribute."<point>"]]`, `[provides]` / `[requires]`
//! and `reserved`, read and refused; a point's items checked against its
//! schema (a bad one a problem, not a failed load) and delivered to its
//! owner live as extensions load and unload; a service called across two
//! extensions (linked, Luau), with its provider missing or too old; a
//! window opened as a sheet and as an OS window.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use architect::action::ActionList;
use architect_extensions::extensions::Extensions;
use architect_extensions::host::{Event, Host, LoadError, SharedSections};
use architect_extensions::hostcall::{HostError, HostRequest};
use architect_extensions::install::Installs;
use architect_extensions::interop::WindowKind;
use architect_extensions::linked::{Builder, Extension, Handle};
use architect_extensions::{Manifest, ManifestError, Origin, Package};
use architect_extensions_dioxus::BuilderExt as _;
use architect_settings::Registry;
use architect_workspace::{Popout, WindowId, WindowSystem, Windows};
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

fn until(what: &str, ok: impl Fn() -> bool) {
    let start = Instant::now();
    while !ok() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "waited for {what}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

// ---- windows ------------------------------------------------------------

thread_local! {
    static DRAWN: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    static OPENED: RefCell<Vec<Popout>> = const { RefCell::new(Vec::new()) };
    static CLOSED: RefCell<Vec<WindowId>> = const { RefCell::new(Vec::new()) };
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
}

struct Platform;

impl WindowSystem for Platform {
    fn open(&self, popout: Popout) {
        OPENED.with(|o| o.borrow_mut().push(popout));
    }
    fn close(&self, window: &WindowId) {
        CLOSED.with(|c| c.borrow_mut().push(window.clone()));
    }
    fn origin(&self, _: &WindowId) -> Option<(f32, f32)> {
        None
    }
    fn window_at(&self, _: f32, _: f32) -> Option<WindowId> {
        None
    }
    fn on_release_outside(&self, _: &WindowId, _: Rc<dyn Fn(f32, f32)>) {}
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
    use_context_provider(|| Windows::new(Platform));
    let host = HOST.with(|h| h.borrow().clone().unwrap());
    let ui = app_shell_extensions::interface::use_extension_ui(host, Arc::new(ActionList::new()));
    rsx! { app_shell_extensions::interface::ExtensionOverlays { ui } }
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

    // An OS window: the platform is asked for one, whose own document
    // draws the view; the person closing it closes it in the host.
    DRAWN.with(|d| d.borrow_mut().clear());
    run(&list, "LAB_PANELS_MIXER", Value::Null).unwrap();
    settle(&mut main);
    assert!(
        DRAWN.with(|d| d.borrow().is_empty()),
        "not in the main window"
    );
    let popout = OPENED
        .with(|o| o.borrow().first().cloned())
        .expect("an OS window");
    assert_eq!(popout.title(), "Mixer");
    assert_eq!(popout.id().0, "ext-lab.panels-mixer");
    let mut window = popout.dom();
    window.rebuild_in_place();
    assert_eq!(DRAWN.with(|d| d.borrow().clone()), ["mixer"]);
    window.in_runtime(|| popout.closed());
    drop(window);
    assert!(ext.host().open_windows().is_empty());
    settle(&mut main);

    // Closed by the host: its OS window closes.
    run(&list, "LAB_PANELS_MIXER", Value::Null).unwrap();
    settle(&mut main);
    assert_eq!(OPENED.with(|o| o.borrow().len()), 2);
    let _ = run(
        &list,
        "EXT_WINDOW_CLOSE",
        json!({"extension": "lab.panels", "id": "mixer"}),
    );
    settle(&mut main);
    assert!(
        CLOSED.with(|c| c.borrow().iter().any(|w| w.0 == "ext-lab.panels-mixer")),
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
    let ui = app_shell_extensions::interface::use_extension_ui(host, Arc::new(ActionList::new()));
    let kinds: Vec<String> = ui.views().into_iter().map(|v| v.kind.0).collect();
    LISTED.with(|l| *l.borrow_mut() = kinds);
    rsx! {}
}

/// An extension that adds nothing but a view, loaded after the window is
/// up (a development copy, #844): its view is a pane kind at once,
/// whatever else the window does, so `WORKSPACE_OPEN` finds it.
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
