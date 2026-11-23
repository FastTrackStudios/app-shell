//! The shell's gestures, driven as the root's pointer handlers drive
//! them: a tab dragged to each kind of drop zone, a divider and a dock
//! edge dragged.

use std::cell::RefCell;

use dioxus::prelude::*;

use super::shell::{Gesture, Shell, Zone, use_shell};
use super::{
    AppShell, FormFactor, Sidebar, SidebarItem, SidebarState, TabCtx, TabKind, TabKinds, Tabs,
};
use crate::model::{Axis, Node, Pane, PaneId, Side, Workspace};
use crate::ops::{Dest, Target};

/// What a test does to the shell once it is mounted.
type Script = Box<dyn Fn(Shell)>;

thread_local! {
    static SCRIPT: RefCell<Option<Script>> = RefCell::new(None);
    static AFTER: RefCell<Option<Workspace>> = const { RefCell::new(None) };
    static TOLD: RefCell<usize> = const { RefCell::new(0) };
    static SIDEBAR: RefCell<Vec<SidebarState>> = const { RefCell::new(Vec::new()) };
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
            tabs: Tabs::On,
            form: FormFactor::Desktop,
            on_change: move |_| TOLD.with(|t| *t.borrow_mut() += 1),
            sidebar: rsx! { Sidebar { SidebarItem { label: "Home" } } },
            on_sidebar: move |state| SIDEBAR.with(|s| s.borrow_mut().push(state)),
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

#[test]
fn the_sidebar_edge_dragged_resizes_it_within_bounds_and_tells_the_app() {
    SIDEBAR.with(|s| s.borrow_mut().clear());
    let (_, told) = run(tabs(&["a"]), |s| {
        let mut g = s.gesture;
        g.set(Some(Gesture::Sidebar {
            start: (220.0, 10.0),
            width: 220.0,
            rail: false,
        }));
        s.moved((300.0, 10.0));
        assert!((*s.sidebar_w.peek() - 300.0).abs() < 0.01);
        s.moved((2000.0, 10.0));
        assert!(
            (*s.sidebar_w.peek() - super::SIDEBAR_MAX).abs() < 0.01,
            "kept to its widest"
        );
        s.released((2000.0, 10.0));
    });
    assert_eq!(told, 0, "the layout did not change");
    let heard = SIDEBAR.with(|s| s.borrow().clone());
    assert_eq!(
        heard,
        vec![SidebarState {
            open: true,
            width: super::SIDEBAR_MAX,
            rail: Some(false),
        }]
    );
}

#[test]
fn the_sidebar_edge_dragged_narrow_folds_it_to_the_rail_and_back() {
    SIDEBAR.with(|s| s.borrow_mut().clear());
    run(tabs(&["a"]), |s| {
        let mut g = s.gesture;
        g.set(Some(Gesture::Sidebar {
            start: (220.0, 10.0),
            width: 220.0,
            rail: false,
        }));
        s.moved((60.0, 10.0));
        assert!(s.sidebar_folded(), "past the snap it is the rail");
        assert!(
            (*s.sidebar_w.peek() - 220.0).abs() < 0.01,
            "its width kept for unfolding"
        );
        s.released((60.0, 10.0));
        g.set(Some(Gesture::Sidebar {
            start: (52.0, 10.0),
            width: 220.0,
            rail: true,
        }));
        s.moved((300.0, 10.0));
        assert!(!s.sidebar_folded());
        assert!((*s.sidebar_w.peek() - 300.0).abs() < 0.01);
        s.released((300.0, 10.0));
    });
    let heard = SIDEBAR.with(|s| s.borrow().iter().map(|x| x.rail).collect::<Vec<_>>());
    assert_eq!(heard, vec![Some(true), Some(false)]);
}

#[test]
fn a_press_on_the_edge_that_does_not_move_chooses_nothing() {
    SIDEBAR.with(|s| s.borrow_mut().clear());
    run(tabs(&["a"]), |s| {
        let mut g = s.gesture;
        g.set(Some(Gesture::Sidebar {
            start: (220.0, 10.0),
            width: 220.0,
            rail: false,
        }));
        s.moved((222.0, 10.0));
        s.released((222.0, 10.0));
        assert_eq!(*s.sidebar_rail.peek(), None, "still the window's to say");
    });
    assert!(SIDEBAR.with(|s| s.borrow().is_empty()));
}

#[test]
fn toggle_sidebar_folds_it_to_the_rail_and_back_on_a_desktop() {
    SIDEBAR.with(|s| s.borrow_mut().clear());
    run(tabs(&["a"]), |s| {
        s.command(crate::Command::ToggleSidebar);
        assert!(s.sidebar_folded() && *s.sidebar_open.peek());
        s.command(crate::Command::ToggleSidebar);
        assert!(!s.sidebar_folded());
        // Hidden by the app, the button brings it back at full width.
        s.set_sidebar(SidebarState {
            open: false,
            ..s.sidebar_state()
        });
        s.command(crate::Command::ToggleSidebar);
        assert!(*s.sidebar_open.peek() && !s.sidebar_folded());
    });
    let heard = SIDEBAR.with(|s| {
        s.borrow()
            .iter()
            .map(|x| (x.open, x.rail))
            .collect::<Vec<_>>()
    });
    assert_eq!(
        heard,
        vec![
            (true, Some(true)),
            (true, Some(false)),
            (false, Some(false)),
            (true, Some(false))
        ]
    );
}

#[derive(Props, Clone, PartialEq)]
struct PhoneProps {
    start: Workspace,
}

#[allow(non_snake_case)]
fn PhoneHarness(props: PhoneProps) -> Element {
    let layout = use_signal(|| props.start.clone());
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
    rsx! {
        AppShell {
            layout,
            kinds,
            form: FormFactor::Phone,
            sidebar: rsx! { Sidebar { SidebarItem { label: "Home" } } },
        }
    }
}

fn run_phone(script: impl Fn(Shell) + 'static) {
    SCRIPT.with(|s| *s.borrow_mut() = Some(Box::new(script)));
    let mut dom = VirtualDom::new_with_props(
        PhoneHarness,
        PhoneProps {
            start: tabs(&["a"]),
        },
    );
    dom.rebuild_in_place();
    for _ in 0..3 {
        dom.process_events();
        dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
    }
    assert!(SCRIPT.with(|s| s.borrow().is_none()), "the script ran");
}

#[test]
fn a_swipe_from_the_edge_opens_the_drawer_under_the_finger() {
    run_phone(|s| {
        let mut vp = s.viewport;
        vp.set((390.0, 844.0));
        let mut g = s.gesture;
        g.set(Some(Gesture::Drawer {
            start: (5.0, 300.0),
            from: 0.0,
        }));
        s.moved((165.0, 305.0));
        let p = s.drawer_drag.peek().expect("following the finger");
        assert!((p - 0.5).abs() < 0.01, "half of 320");
        s.released((165.0, 305.0));
        assert!(*s.drawer.peek(), "a flick past 60 points opens it");
        assert!(s.drawer_drag.peek().is_none());
        // Dragged back a little: stays; a flick left: shut.
        g.set(Some(Gesture::Drawer {
            start: (200.0, 300.0),
            from: 1.0,
        }));
        s.moved((170.0, 300.0));
        s.released((170.0, 300.0));
        assert!(*s.drawer.peek());
        g.set(Some(Gesture::Drawer {
            start: (200.0, 300.0),
            from: 1.0,
        }));
        s.moved((100.0, 300.0));
        s.released((100.0, 300.0));
        assert!(!*s.drawer.peek());
    });
}

#[test]
fn a_vertical_move_from_the_edge_is_the_pages_scroll() {
    run_phone(|s| {
        let mut vp = s.viewport;
        vp.set((390.0, 844.0));
        let mut g = s.gesture;
        g.set(Some(Gesture::Drawer {
            start: (5.0, 300.0),
            from: 0.0,
        }));
        s.moved((9.0, 360.0));
        assert!(s.gesture.peek().is_none(), "let go of");
        assert!(s.drawer_drag.peek().is_none() && !*s.drawer.peek());
    });
}

#[test]
fn a_sheet_dragged_by_its_handle_settles_or_goes() {
    run_phone(|s| {
        let mut vp = s.viewport;
        vp.set((390.0, 800.0));
        let closed = std::rc::Rc::new(std::cell::Cell::new(false));
        let seen = closed.clone();
        let id = s.open_sheet(
            super::SheetSpec::new(|()| rsx! { "x" }).on_close(move |()| seen.set(true)),
        );
        let mut g = s.gesture;
        g.set(Some(Gesture::Sheet {
            id: id.0,
            start: (100.0, 400.0),
        }));
        s.moved((100.0, 100.0));
        assert!(
            (s.sheets.peek()[0].drag + 300.0).abs() < 0.01,
            "follows the finger"
        );
        s.released((100.0, 100.0));
        assert_eq!(s.sheets.peek()[0].detent, crate::gestures::Detent::Large);
        assert_eq!(s.sheets.peek()[0].drag, 0.0);
        g.set(Some(Gesture::Sheet {
            id: id.0,
            start: (100.0, 100.0),
        }));
        s.moved((100.0, 760.0));
        s.released((100.0, 760.0));
        assert!(s.sheets.peek().is_empty(), "dragged away");
        assert!(closed.get(), "and told");
    });
}

#[test]
fn escape_closes_the_top_sheet_first() {
    run_phone(|s| {
        s.open_sheet(super::SheetSpec::new(|()| rsx! { "a" }));
        s.open_sheet(super::SheetSpec::new(|()| rsx! { "b" }));
        assert!(s.close_top_sheet());
        assert_eq!(s.sheets.peek().len(), 1);
        assert!(s.close_top_sheet());
        assert!(!s.close_top_sheet(), "none left");
    });
}

#[test]
fn the_form_follows_the_width() {
    assert_eq!(FormFactor::for_width(390.0), FormFactor::Phone);
    assert_eq!(FormFactor::for_width(599.0), FormFactor::Phone);
    assert_eq!(FormFactor::for_width(600.0), FormFactor::Desktop);
    assert_eq!(FormFactor::for_width(1280.0), FormFactor::Desktop);
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct AroundSaw(&'static str);

#[allow(non_snake_case)]
fn HooksHarness() -> Element {
    let layout = use_signal(|| tabs(&["a", "b"]));
    let kinds = TabKinds::new().with(TabKind::new("v", "V", |ctx: TabCtx| {
        // What `around` provided, read by the kind's own render.
        let saw = try_use_context::<AroundSaw>().map_or("none", |s| s.0);
        rsx! { "body {ctx.pane.id.0} saw {saw}" }
    }));
    use_context_provider(|| {
        super::TabHooks::new()
            .around(|_ctx| {
                use_context_provider(|| AroundSaw("around"));
            })
            .header(|ctx| {
                let id = ctx.pane.id.0.clone();
                Some(rsx! { "header {id}" })
            })
            .badge(|pane| (pane.id.0 == "b").then(|| rsx! { "badge-b" }))
    });
    rsx! {
        AppShell { layout, kinds, tabs: Tabs::On, form: FormFactor::Desktop }
    }
}

#[test]
fn tab_hooks_wrap_the_content_head_it_and_badge_the_tab() {
    let mut dom = VirtualDom::new(HooksHarness);
    dom.rebuild_in_place();
    for _ in 0..3 {
        dom.process_events();
        dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
    }
    let html = dioxus_ssr::render(&dom);
    assert!(html.contains("body a saw around"), "{html}");
    assert!(html.contains("header a"), "{html}");
    assert!(html.contains("badge-b"), "{html}");
}

#[test]
fn a_panes_view_state_is_kept_without_history() {
    let mut ws = tabs(&["a"]);
    let a = PaneId::new("a");
    ws.set_view_state(&a, "zoom=2".into()).unwrap();
    let p = ws.pane(&a).unwrap();
    assert_eq!(p.view_state, "zoom=2");
    assert!(p.history.back.is_empty());
    assert!(
        ws.set_view_state(&PaneId::new("nope"), String::new())
            .is_err()
    );
}

#[test]
fn bound_keys_hold_in_their_tabs_kind_before_the_keymap_and_the_windows_after() {
    use super::{BoundKey, KeyScope};
    use crate::commands::KeyChord;
    use std::rc::Rc;
    let heard: Rc<RefCell<Vec<String>>> = Rc::default();
    let h = Rc::clone(&heard);
    let (after, _) = run(tabs(&["a", "b"]), move |shell| {
        let key = |text: &str| KeyChord::parse(text, false).unwrap();
        let said = |what: &'static str, h: &Rc<RefCell<Vec<String>>>| {
            let h = Rc::clone(h);
            move |pane: Option<&Pane>| {
                let id = pane.map(|p| p.id.0.clone()).unwrap_or_default();
                h.borrow_mut().push(format!("{what} {id}"));
                true
            }
        };
        shell.bind_keys(
            "ext",
            vec![
                // The active tab's kind's ⌘W wins over the keymap's.
                BoundKey::new(key("⌘W"), KeyScope::Kind("v".into()), said("undo", &h)),
                // Another kind's keys do not hold here.
                BoundKey::new(key("⌘K"), KeyScope::Kind("other".into()), said("never", &h)),
                // The window's come after the keymap: ⇧⌘T is its.
                BoundKey::new(key("⇧⌘T"), KeyScope::Window, said("never", &h)),
                BoundKey::new(key("⌘J"), KeyScope::Window, said("window", &h)),
                // One that does not take the key lets it go on.
                BoundKey::new(key("A"), KeyScope::Kind("v".into()), |_| false),
            ],
        );
        assert!(shell.press_key(&key("⌘W")));
        assert!(shell.press_key(&key("⌘J")));
        assert!(!shell.press_key(&key("A")));
        shell.press_key(&key("⌘K"));
        shell.bind_keys("ext", Vec::new());
        assert!(!shell.press_key(&key("⌘J")));
    });
    assert_eq!(*heard.borrow(), ["undo a", "window a"]);
    // ⌘W was the binding's: no tab closed.
    assert!(after.pane(&PaneId::new("a")).is_some());
}
