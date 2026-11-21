//! The shell's gestures, driven as the root's pointer handlers drive
//! them: a tab dragged to each kind of drop zone, a divider and a dock
//! edge dragged.

use std::cell::RefCell;

use dioxus::prelude::*;

use super::shell::{Gesture, Shell, Zone, use_shell};
use super::{AppShell, FormFactor, TabCtx, TabKind, TabKinds};
use crate::model::{Axis, Node, Pane, PaneId, Side, Workspace};
use crate::ops::{Dest, Target};

/// What a test does to the shell once it is mounted.
type Script = Box<dyn Fn(Shell)>;

thread_local! {
    static SCRIPT: RefCell<Option<Script>> = RefCell::new(None);
    static AFTER: RefCell<Option<Workspace>> = const { RefCell::new(None) };
    static TOLD: RefCell<usize> = const { RefCell::new(0) };
}

#[allow(non_snake_case)]
fn Harness(start: Workspace) -> Element {
    let layout = use_signal(|| start);
    let kinds = TabKinds::new().with(TabKind::new("v", "V", |ctx: TabCtx| {
        let shell = use_shell();
        let first = ctx.pane.id.0 == "a";
        use_hook(move || {
            if first && let Some(script) = SCRIPT.with(|s| s.borrow_mut().take()) {
                script(shell);
            }
        });
        rsx! { "{ctx.pane.title}" }
    }));
    use_effect(move || AFTER.with(|a| *a.borrow_mut() = Some(layout.read().clone())));
    rsx! {
        AppShell {
            layout,
            kinds,
            form: FormFactor::Desktop,
            on_change: move |_| TOLD.with(|t| *t.borrow_mut() += 1),
        }
    }
}

fn run(start: Workspace, script: impl Fn(Shell) + 'static) -> (Workspace, usize) {
    SCRIPT.with(|s| *s.borrow_mut() = Some(Box::new(script)));
    TOLD.with(|t| *t.borrow_mut() = 0);
    let mut dom = VirtualDom::new_with_props(Harness, start);
    dom.rebuild_in_place();
    for _ in 0..4 {
        dom.process_events();
        dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
    }
    let _ = dioxus_ssr::render(&dom);
    (
        AFTER.with(|a| a.borrow().clone()).expect("rendered"),
        TOLD.with(|t| *t.borrow()),
    )
}

fn tabs(names: &[&str]) -> Workspace {
    let mut ws = Workspace::new();
    for n in names {
        ws.open(Pane::new(*n, "v", n.to_uppercase()), Target::Tab)
            .unwrap();
    }
    ws.activate(&PaneId::new("a")).unwrap();
    ws
}

/// Press `pane`, move far enough, hover `zone` with `dest`, let go.
fn drag(shell: Shell, pane: &str, zone: Zone, dest: Dest) {
    let mut g = shell.gesture;
    g.set(Some(Gesture::Tab {
        pane: PaneId::new(pane),
        title: pane.to_owned(),
        start: (10.0, 10.0),
        at: (10.0, 10.0),
        live: false,
    }));
    shell.moved((40.0, 40.0));
    assert!(shell.dragging_now(), "live past the threshold");
    shell.hover(zone, dest);
    shell.moved((41.0, 41.0));
    shell.released((41.0, 41.0));
}

fn group_of(ws: &Workspace, p: &str) -> crate::model::GroupId {
    ws.locate(&PaneId::new(p)).unwrap().group
}

#[test]
fn a_tab_let_go_on_a_bar_goes_between_the_tabs_there() {
    let ws = tabs(&["a", "b", "c"]);
    let g = group_of(&ws, "a");
    let (after, told) = run(ws, move |s| {
        drag(
            s,
            "c",
            Zone::Bar(g, 0),
            Dest::Group {
                group: g,
                index: Some(0),
            },
        );
    });
    let order: Vec<&str> = after
        .group(g)
        .unwrap()
        .tabs
        .iter()
        .map(|t| t.0.as_str())
        .collect();
    assert_eq!(order, ["c", "a", "b"]);
    assert_eq!(told, 1, "the app hears it once");
}

#[test]
fn a_tab_let_go_on_a_stacks_side_splits_it() {
    let ws = tabs(&["a", "b"]);
    let g = group_of(&ws, "a");
    let (after, _) = run(ws, move |s| {
        drag(
            s,
            "b",
            Zone::Group(g, Some(Side::Bottom)),
            Dest::Edge {
                group: g,
                side: Side::Bottom,
            },
        );
    });
    let Some(Node::Split(split)) = &after.windows[0].center else {
        panic!("split")
    };
    assert_eq!(split.axis, Axis::Column);
}

#[test]
fn a_tab_let_go_on_the_new_window_target_opens_a_window_where_it_went() {
    let (after, _) = run(tabs(&["a", "b"]), |s| {
        drag(s, "b", Zone::NewWindow, Dest::Window(None));
    });
    assert_eq!(after.windows.len(), 2);
    assert!(after.windows[1].bounds.is_some(), "placed at the pointer");
}

#[test]
fn a_tab_let_go_at_the_windows_edge_docks_there() {
    let (after, _) = run(tabs(&["a", "b"]), |s| {
        drag(s, "b", Zone::Edge(Side::Left), Dest::Dock(Side::Left));
    });
    assert_eq!(
        after.locate(&PaneId::new("b")).unwrap().area,
        crate::model::Area::Dock(Side::Left)
    );
}

#[test]
fn a_tab_let_go_over_nothing_stays_and_a_press_is_not_a_drag() {
    let (after, told) = run(tabs(&["a", "b"]), |s| {
        let mut g = s.gesture;
        g.set(Some(Gesture::Tab {
            pane: PaneId::new("b"),
            title: "b".into(),
            start: (10.0, 10.0),
            at: (10.0, 10.0),
            live: false,
        }));
        s.moved((12.0, 11.0));
        assert!(!s.dragging_now(), "under the threshold");
        s.released((12.0, 11.0));
        assert!(s.gesture.peek().is_none());
    });
    assert_eq!(
        after.windows[0]
            .center
            .as_ref()
            .map(|n| matches!(n, Node::Group(_))),
        Some(true)
    );
    assert_eq!(told, 0);
}

#[test]
fn a_divider_dragged_moves_the_boundary_and_is_told_once() {
    let mut ws = tabs(&["a"]);
    ws.open(Pane::new("b", "v", "B"), Target::Split(Side::Right))
        .unwrap();
    ws.activate(&PaneId::new("a")).unwrap();
    let Some(Node::Split(split)) = ws.windows[0].center.clone() else {
        panic!()
    };
    let (after, told) = run(ws, move |s| {
        let mut g = s.gesture;
        g.set(Some(Gesture::Divider {
            split: split.id,
            index: 0,
            axis: Axis::Row,
            start: (500.0, 10.0),
            shares: vec![0.5, 0.5],
            length: Some(1000.0),
        }));
        s.moved((600.0, 10.0));
        s.moved((700.0, 10.0));
        s.released((700.0, 10.0));
    });
    let Some(Node::Split(split)) = &after.windows[0].center else {
        panic!()
    };
    assert!((split.children[0].share - 0.7).abs() < 1e-4);
    assert_eq!(told, 1, "once, at the end");
}

#[test]
fn a_dock_edge_dragged_resizes_the_dock() {
    let mut ws = tabs(&["a"]);
    ws.open(Pane::new("f", "v", "F"), Target::Dock(Side::Right))
        .unwrap();
    ws.activate(&PaneId::new("a")).unwrap();
    let (after, _) = run(ws, |s| {
        let mut g = s.gesture;
        g.set(Some(Gesture::Dock {
            side: Side::Right,
            start: (800.0, 0.0),
            size: 320.0,
        }));
        s.moved((700.0, 0.0));
        s.released((700.0, 0.0));
    });
    assert_eq!(after.windows[0].docks.right.size, 420.0);
}
