//! An extension's keys (`[[keys]]`) in app-shell's keymap: one bound
//! everywhere runs its action from any tab; one bound in a view's context
//! runs only while a tab of that view is active; a view an opener shows a
//! file in gets ⌘Z and ⇧⌘Z as its document's undo (`DOC_UNDO`,
//! `DOC_REDO`), unless the extension binds those there itself.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::cell::RefCell;
use std::time::Duration;

use app_shell::ui::{AppShell, FormFactor, KeyScope, Tabs};
use app_shell::{KeyChord, Workspace};
use app_shell_extensions::keys::{ExtensionKey, extension_keys};
use app_shell_extensions::{ExtensionUi, use_extension_ui};
use architect::action::{ActionList, Invocation, InvocationSource};
use architect_extensions::extensions::Extensions;
use architect_extensions::host::{Host, SharedSections};
use architect_extensions::install::Installs;
use architect_extensions::linked::{Builder, Extension};
use architect_extensions_dioxus::BuilderExt as _;
use architect_settings::Registry;
use dioxus::prelude::*;
use futures::executor::block_on;

const CHARTS: &str = r#"
id = "lab.charts"
name = "Charts"
version = "0.1.0"
kind = "process"

[entry]
path = "charts"

[[views]]
id = "chart"
title = "Chart"

[[views]]
id = "sheet"
title = "Sheet"

[[actions]]
id = "LAB_CHARTS_FIND"
name = "Find"

[[actions]]
id = "LAB_CHARTS_NOTE"
name = "Note A"

[[actions]]
id = "LAB_CHARTS_UNDO"
name = "Undo the sheet"

[[keys]]
chord = "⌘J"
action = "LAB_CHARTS_FIND"

[[keys]]
chord = "A"
action = "LAB_CHARTS_NOTE"
when = "view:chart"

[[keys]]
chord = "⌘Z"
action = "LAB_CHARTS_UNDO"
when = "view:sheet"

[[openers]]
id = "charts"
title = "Charts"
files = ["kf"]
view = "chart"

[[openers]]
id = "sheets"
title = "Sheets"
files = ["sheet"]
view = "sheet"
"#;

#[component]
fn Chart() -> Element {
    rsx! { "chart" }
}

struct Charts;

impl Extension for Charts {
    fn manifest(&self) -> &'static str {
        CHARTS
    }

    fn build(&self, ext: &mut Builder) {
        let heard = |id: &'static str| {
            move |_| async move {
                HEARD.with(|h| h.borrow_mut().push(id.to_owned()));
                Ok(serde_json::Value::Null)
            }
        };
        ext.view("chart", Chart)
            .view("sheet", Chart)
            .action("LAB_CHARTS_FIND", heard("find"))
            .action("LAB_CHARTS_NOTE", heard("note"))
            .action("LAB_CHARTS_UNDO", heard("undo"));
    }
}

thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
    static UI: RefCell<Option<ExtensionUi>> = const { RefCell::new(None) };
    static HEARD: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn app() -> Element {
    let host = HOST.with(|h| h.borrow().clone().unwrap());
    let layout = use_signal(Workspace::new);
    let ui = use_extension_ui(host, layout);
    UI.with(|u| *u.borrow_mut() = Some(ui));
    rsx! {
        AppShell { layout, kinds: ui.tab_kinds(), tabs: Tabs::On, form: FormFactor::Desktop }
    }
}

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

fn key(scope: KeyScope, chord: &str, action: &str, document: bool) -> ExtensionKey {
    ExtensionKey {
        chord: chord.to_owned(),
        action: action.to_owned(),
        scope,
        document,
    }
}

#[test]
fn an_extensions_keys_are_the_shells_in_their_views_and_everywhere() {
    let tmp = tempfile::tempdir().unwrap();
    let host = Host::new(ActionList::new(), SharedSections::new(Registry::new()));
    let ext = Extensions::new(host, Installs::new(tmp.path().join("i")))
        .with_linked(&Charts)
        .unwrap();
    let _ = ext.sync();

    let chart = KeyScope::Kind("lab.charts/chart".into());
    let sheet = KeyScope::Kind("lab.charts/sheet".into());
    assert_eq!(
        extension_keys(ext.host().contributions()),
        [
            key(KeyScope::Window, "⌘J", "LAB_CHARTS_FIND", false),
            key(chart.clone(), "A", "LAB_CHARTS_NOTE", false),
            key(sheet.clone(), "⌘Z", "LAB_CHARTS_UNDO", false),
            key(chart.clone(), "⌘Z", "DOC_UNDO", true),
            key(chart, "⇧⌘Z", "DOC_REDO", true),
            // The sheet binds ⌘Z itself: only ⇧⌘Z is the document's.
            key(sheet, "⇧⌘Z", "DOC_REDO", true),
        ]
    );

    HOST.with(|h| *h.borrow_mut() = Some(ext.host().clone()));
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    settle(&mut dom);
    ext.host()
        .open_view(
            "lab.charts",
            "chart",
            Some("song.kf".to_owned()),
            None,
            Invocation::new(InvocationSource::App),
        )
        .unwrap();
    settle(&mut dom);

    let ui = UI.with(|u| u.borrow().unwrap());
    let shell = dom
        .in_runtime(|| ui.attached())
        .expect("the shell attached");
    let mac = cfg!(any(target_os = "macos", target_os = "ios"));
    let press = |dom: &mut VirtualDom, text: &str| {
        let chord = KeyChord::parse(text, mac).unwrap();
        let took = dom.in_scope(ScopeId::ROOT, || shell.press_key(&chord));
        settle(dom);
        took
    };
    // The chart is active: its A, and ⌘J from anywhere.
    assert!(press(&mut dom, "A"));
    assert!(press(&mut dom, "⌘J"));
    // A letter no view binds goes on to the view.
    assert!(!press(&mut dom, "B"));
    assert_eq!(HEARD.with(|h| h.borrow().clone()), ["note", "find"]);

    // In the sheet, A is not bound; its own ⌘Z is.
    ext.host()
        .open_view(
            "lab.charts",
            "sheet",
            Some("a.sheet".to_owned()),
            None,
            Invocation::new(InvocationSource::App),
        )
        .unwrap();
    settle(&mut dom);
    assert!(!press(&mut dom, "A"));
    assert!(press(&mut dom, "⌘Z"));
    assert_eq!(HEARD.with(|h| h.borrow().clone()), ["note", "find", "undo"]);
}
