//! Repair: any layout read back opens, and holds its invariants.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use app_shell::{
    Child, Group, GroupId, Node, Pane, PaneId, Rect, Side, Split, SplitId, Target, Window,
    WindowId, Workspace,
};

fn id(s: &str) -> PaneId {
    PaneId::new(s)
}

fn group(n: u64, tabs: &[&str]) -> Node {
    let mut g = Group::new(GroupId(n));
    g.tabs = tabs.iter().map(|t| id(t)).collect();
    Node::Group(g)
}

fn panes(names: &[&str]) -> Vec<Pane> {
    names.iter().map(|n| Pane::new(*n, "v", *n)).collect()
}

#[test]
fn an_empty_layout_becomes_a_main_window_with_one_group() {
    let mut ws = Workspace::new();
    ws.windows.clear();
    ws.repair();
    assert_eq!(ws.windows.len(), 1);
    assert_eq!(ws.windows[0].id, WindowId::main());
    assert!(matches!(ws.windows[0].center, Some(Node::Group(_))));
}

#[test]
fn shares_are_made_positive_and_summing_to_one() {
    let mut ws = Workspace::new();
    ws.panes = panes(&["a", "b", "c"]);
    ws.windows[0].center = Some(Node::Split(Split {
        id: SplitId(10),
        axis: app_shell::Axis::Row,
        children: vec![
            Child {
                node: group(11, &["a"]),
                share: f32::NAN,
            },
            Child {
                node: group(12, &["b"]),
                share: -3.0,
            },
            Child {
                node: group(13, &["c"]),
                share: 0.0,
            },
        ],
    }));
    ws.repair();
    let Some(Node::Split(s)) = &ws.windows[0].center else {
        panic!("still split")
    };
    let sum: f32 = s.children.iter().map(|c| c.share).sum();
    assert!((sum - 1.0).abs() < 1e-5);
    assert!(s.children.iter().all(|c| c.share > 0.0));
}

#[test]
fn a_split_in_a_split_on_the_same_axis_merges_and_one_child_unwraps() {
    let mut ws = Workspace::new();
    ws.panes = panes(&["a", "b", "c"]);
    let inner = Node::Split(Split {
        id: SplitId(20),
        axis: app_shell::Axis::Row,
        children: vec![
            Child {
                node: group(21, &["b"]),
                share: 0.5,
            },
            Child {
                node: group(22, &["c"]),
                share: 0.5,
            },
        ],
    });
    ws.windows[0].center = Some(Node::Split(Split {
        id: SplitId(10),
        axis: app_shell::Axis::Row,
        children: vec![
            Child {
                node: group(11, &["a"]),
                share: 0.5,
            },
            Child {
                node: inner,
                share: 0.5,
            },
            Child {
                node: group(12, &[]),
                share: 0.2,
            },
        ],
    }));
    ws.repair();
    let Some(Node::Split(s)) = &ws.windows[0].center else {
        panic!("split")
    };
    assert_eq!(s.children.len(), 3, "merged, the empty group gone");
    let shares: Vec<f32> = s.children.iter().map(|c| c.share).collect();
    assert!((shares[0] - 0.5).abs() < 1e-5 && (shares[1] - 0.25).abs() < 1e-5);
}

#[test]
fn a_pop_out_with_nothing_left_closes_and_the_main_window_stays() {
    let mut ws = Workspace::new();
    ws.panes = panes(&["a"]);
    ws.windows[0].center = Some(group(1, &["a"]));
    ws.windows
        .push(Window::new(WindowId::new("w"), group(5, &["ghost"])));
    ws.repair();
    assert_eq!(ws.windows.len(), 1);
}

#[test]
fn ids_handed_out_next_stay_past_every_id_in_use() {
    let mut ws = Workspace::new();
    ws.panes = panes(&["a"]);
    ws.windows[0].center = Some(group(500, &["a"]));
    ws.next_id = 3;
    ws.repair();
    assert!(ws.next_id > 500);
    ws.open(Pane::new("b", "v", "b"), Target::Split(Side::Right))
        .unwrap();
    assert!(ws.locate(&id("b")).unwrap().group.0 > 500);
}

#[test]
fn active_and_recent_name_open_panes_and_current_is_clamped() {
    let mut ws = Workspace::new();
    ws.panes = panes(&["a", "b"]);
    let mut g = Group::new(GroupId(1));
    g.tabs = vec![id("a"), id("b")];
    g.current = 9;
    ws.windows[0].center = Some(Node::Group(g));
    ws.active = Some(id("gone"));
    ws.recent = vec![id("gone"), id("b"), id("b"), id("a")];
    ws.repair();
    assert_eq!(ws.recent, vec![id("b"), id("a")]);
    assert_eq!(ws.active, Some(id("b")));
    assert_eq!(ws.group(GroupId(1)).unwrap().current, 1);
}

#[test]
fn a_zoom_on_a_lone_group_or_a_group_that_went_is_dropped() {
    let mut ws = Workspace::new();
    ws.open(Pane::new("a", "v", "a"), Target::Tab).unwrap();
    let g = ws.locate(&id("a")).unwrap().group;
    ws.windows[0].zoomed = Some(g);
    ws.repair();
    assert_eq!(ws.windows[0].zoomed, None);
}

#[test]
fn a_float_with_no_tabs_goes_and_a_tiny_dock_grows() {
    let mut ws = Workspace::new();
    ws.open(
        Pane::new("f", "v", "f"),
        Target::Float(Rect::new(0.0, 0.0, 10.0, 10.0)),
    )
    .unwrap();
    ws.windows[0].docks.left.size = 3.0;
    ws.panes.clear();
    ws.repair();
    assert!(ws.windows[0].floats.is_empty());
    assert_eq!(ws.windows[0].docks.left.size, 120.0);
}

#[test]
fn a_layout_with_unknown_fields_and_missing_defaults_still_reads() {
    let mut ws = Workspace::new();
    ws.open(Pane::new("a", "v", "A"), Target::Tab).unwrap();
    let mut json: serde_json::Value = serde_json::to_value(&ws).unwrap();
    // A newer app's field, and fields an older app did not write.
    json["future_field"] = serde_json::json!(true);
    json.as_object_mut().unwrap().remove("aliases");
    json["windows"][0].as_object_mut().unwrap().remove("zoomed");
    json["panes"][0]
        .as_object_mut()
        .unwrap()
        .remove("view_state");
    let back: Workspace = serde_json::from_value(json).unwrap();
    assert_eq!(back.repaired(), ws);
}
