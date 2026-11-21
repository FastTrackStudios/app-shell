//! The components, rendered to HTML: what each window draws for a
//! layout, and what the shell's operations do to it.

#![cfg(feature = "ui")]
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::cell::RefCell;

use app_shell::ui::{AppShell, FormFactor, TabCtx, TabKind, TabKinds, use_shell};
use app_shell::{Command, Pane, PaneId, Rect, Side, Target, WindowId, Workspace};
use dioxus::prelude::*;

thread_local! {
    /// What `on_change` heard.
    static CHANGES: RefCell<Vec<Workspace>> = const { RefCell::new(Vec::new()) };
    /// A command the "act" kind runs as it mounts.
    static ACT: RefCell<Option<Command>> = const { RefCell::new(None) };
}

fn kinds() -> TabKinds {
    TabKinds::new()
        .with(TabKind::new("note", "Note", |ctx: TabCtx| {
            rsx! { p { class: "note-body", "note {ctx.pane.title}" } }
        }))
        .with(TabKind::new("act", "Act", |_ctx: TabCtx| {
            let shell = use_shell();
            use_hook(move || {
                if let Some(cmd) = ACT.with(|a| a.borrow_mut().take()) {
                    shell.command(cmd);
                }
            });
            rsx! { p { "acting" } }
        }))
}

#[derive(Props, Clone, PartialEq)]
struct Lab {
    layout: Workspace,
    form: FormFactor,
    #[props(default = WindowId::main())]
    window: WindowId,
}

#[allow(non_snake_case)]
fn Harness(props: Lab) -> Element {
    let layout = use_signal(|| props.layout.clone());
    rsx! {
        AppShell {
            layout,
            kinds: kinds(),
            form: props.form,
            window: props.window.clone(),
            on_change: move |w: Workspace| CHANGES.with(|c| c.borrow_mut().push(w)),
            on_new_tab: move |_| {},
        }
    }
}

fn render_lab(lab: Lab) -> String {
    let mut dom = VirtualDom::new_with_props(Harness, lab);
    dom.rebuild_in_place();
    for _ in 0..4 {
        dom.process_events();
        dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
    }
    dioxus_ssr::render(&dom)
}

fn render(layout: Workspace) -> String {
    render_lab(Lab {
        layout,
        form: FormFactor::Desktop,
        window: WindowId::main(),
    })
}

fn note(id: &str) -> Pane {
    Pane::new(id, "note", format!("Note {id}"))
}

fn count(html: &str, needle: &str) -> usize {
    html.matches(needle).count()
}

#[test]
fn a_stack_draws_its_tabs_and_only_its_current_tabs_content() {
    let mut ws = Workspace::new();
    for p in ["a", "b", "c"] {
        ws.open(note(p), Target::Tab).unwrap();
    }
    ws.activate(&PaneId::new("b")).unwrap();
    let html = render(ws);
    assert_eq!(count(&html, "class=\"as-tab as-hov\""), 3);
    assert!(html.contains("Note a") && html.contains("Note c"));
    assert!(html.contains("note Note b"), "the current tab's body");
    assert!(!html.contains("note Note a"), "only the current one");
    assert!(html.contains("data-pane=\"b\" data-current=\"true\""));
    assert!(html.contains("--as-bg: #18181b;"), "the theme's variables");
}

#[test]
fn a_split_draws_each_side_by_its_share_with_a_divider_between() {
    let mut ws = Workspace::new();
    ws.open(note("a"), Target::Tab).unwrap();
    ws.open(note("b"), Target::Split(Side::Right)).unwrap();
    ws.open(note("c"), Target::Split(Side::Bottom)).unwrap();
    let html = render(ws);
    assert_eq!(count(&html, "class=\"as-stack\""), 3);
    assert!(html.contains("data-axis=\"row\""));
    assert!(html.contains("data-axis=\"column\""));
    assert_eq!(count(&html, "class=\"as-divider\""), 2);
    assert!(html.contains("flex: 0.5 1 0px"));
}

#[test]
fn a_tab_of_an_unknown_kind_keeps_its_place_with_a_placeholder() {
    let mut ws = Workspace::new();
    ws.open(Pane::new("x", "gone.kind", "Old"), Target::Tab)
        .unwrap();
    let html = render(ws);
    assert!(html.contains("Old"));
    assert!(html.contains("Nothing draws “gone.kind” tabs here."));
}

#[test]
fn an_empty_layout_offers_a_new_tab() {
    let html = render(Workspace::new());
    assert!(html.contains("No tabs open"));
    assert!(html.contains("New tab"));
}

#[test]
fn docks_draw_at_their_size_and_a_hidden_one_does_not() {
    let mut ws = Workspace::new();
    ws.open(note("doc"), Target::Tab).unwrap();
    ws.open(note("files"), Target::Dock(Side::Left)).unwrap();
    ws.set_dock_size(Side::Left, 250.0).unwrap();
    let html = render(ws.clone());
    assert!(html.contains("data-dock=\"left\""));
    assert!(html.contains("width: 250px;"));
    ws.set_dock_shown(Side::Left, app_shell::Shown::Hidden)
        .unwrap();
    assert!(!render(ws).contains("data-dock=\"left\""));
}

#[test]
fn a_float_draws_over_the_window_where_it_was_put() {
    let mut ws = Workspace::new();
    ws.open(note("doc"), Target::Tab).unwrap();
    ws.open(
        note("f"),
        Target::Float(Rect::new(40.0, 50.0, 300.0, 200.0)),
    )
    .unwrap();
    let html = render(ws);
    assert!(html.contains("class=\"as-float\""));
    assert!(html.contains("left: 40px; top: 50px; width: 300px; height: 200px;"));
}

#[test]
fn a_pop_out_is_drawn_in_the_page_where_there_are_no_os_windows() {
    let mut ws = Workspace::new();
    ws.open(note("a"), Target::Tab).unwrap();
    ws.open(note("b"), Target::Tab).unwrap();
    ws.move_pane(&PaneId::new("b"), app_shell::Dest::Window(None))
        .unwrap();
    let html = render(ws.clone());
    assert_eq!(count(&html, "class=\"as-inpage-window\""), 1);
    assert!(html.contains("note Note b"));
    // The pop-out's own shell (an OS window's) draws only its window.
    let id = ws.windows[1].id.clone();
    let html = render_lab(Lab {
        layout: ws,
        form: FormFactor::Desktop,
        window: id,
    });
    assert!(html.contains("note Note b") && !html.contains("note Note a"));
    assert!(!html.contains("as-inpage-window"));
}

#[test]
fn a_phone_shows_the_active_tab_alone_and_counts_the_tabs() {
    let mut ws = Workspace::new();
    ws.open(note("a"), Target::Tab).unwrap();
    ws.open(note("b"), Target::Split(Side::Right)).unwrap();
    let html = render_lab(Lab {
        layout: ws,
        form: FormFactor::Phone,
        window: WindowId::main(),
    });
    assert!(html.contains("data-form=\"phone\""));
    assert!(html.contains("note Note b") && !html.contains("note Note a"));
    assert!(!html.contains("as-tabbar"), "no tab bars on a phone");
    assert!(html.contains("class=\"as-hov as-tabs-button\""));
    assert!(html.contains(">2</div>"), "the count");
}

fn acting(cmd: Command, tabs: &[&str]) -> (Workspace, String) {
    let mut ws = Workspace::new();
    for p in tabs {
        ws.open(note(p), Target::Tab).unwrap();
    }
    ws.open(Pane::new("act", "act", "Act"), Target::Tab)
        .unwrap();
    ACT.with(|a| *a.borrow_mut() = Some(cmd));
    CHANGES.with(|c| c.borrow_mut().clear());
    (ws.clone(), render(ws))
}

#[test]
fn a_command_from_inside_changes_the_layout_and_tells_the_app() {
    let (_, html) = acting(Command::Split(Side::Right), &["a"]);
    assert!(html.contains("data-axis=\"row\""), "split now");
    let changes = CHANGES.with(|c| c.borrow().clone());
    assert_eq!(changes.len(), 1);
    assert_eq!(
        changes[0].windows[0].center.as_ref().map(|n| {
            let mut g = Vec::new();
            n.groups(&mut g);
            g.len()
        }),
        Some(2)
    );
}

#[test]
fn the_switcher_shows_every_tab_of_the_place_as_a_card() {
    let (_, html) = acting(Command::OpenSwitcher, &["a", "b"]);
    assert!(html.contains("class=\"as-switcher\""));
    assert_eq!(count(&html, "class=\"as-card\""), 3);
    assert!(html.contains("3 tabs"));
    assert!(
        CHANGES.with(|c| c.borrow().is_empty()),
        "the layout is as it was"
    );
}

#[test]
fn moving_a_tab_to_a_new_window_draws_it_there() {
    let (_, html) = acting(Command::MoveToNewWindow, &["a"]);
    assert_eq!(count(&html, "class=\"as-inpage-window\""), 1);
}
