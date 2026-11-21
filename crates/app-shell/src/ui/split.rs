//! A tree of splits, drawn as nested flex boxes by their shares; the
//! dividers between children are zero-width, so the shares are exact.

use std::rc::Rc;

use dioxus::prelude::*;

use super::shell::{Gesture, use_shell};
use super::stack::StackView;
use crate::model::{Axis, Node, Split, WindowId};

/// A node of a window's tree: a split or a stack.
#[component]
pub fn NodeView(node: Node, window: WindowId) -> Element {
    match node {
        Node::Group(group) => rsx! { StackView { group, window } },
        Node::Split(split) => rsx! { SplitView { split, window } },
    }
}

#[component]
fn SplitView(split: Split, window: WindowId) -> Element {
    let mut mounted = use_signal(|| None::<Rc<MountedData>>);
    let dir = match split.axis {
        Axis::Row => "row",
        Axis::Column => "column",
    };
    let count = split.children.len();
    rsx! {
        div {
            class: "as-split",
            "data-split": "{split.id.0}",
            "data-axis": "{dir}",
            style: "position: relative; flex: 1 1 0px; min-width: 0; min-height: 0; display: flex; flex-direction: {dir};",
            onmounted: move |event| mounted.set(Some(event.data())),
            for (i, child) in split.children.iter().enumerate() {
                div {
                    key: "{i}",
                    style: "position: relative; flex: {child.share} 1 0px; min-width: 0; min-height: 0; display: flex;",
                    NodeView { node: child.node.clone(), window: window.clone() }
                }
                if i + 1 < count {
                    Divider {
                        key: "d{i}",
                        split: split.clone(),
                        index: i,
                        mounted,
                    }
                }
            }
        }
    }
}

/// The boundary after child `index`: drag it to move the boundary,
/// double-press it to even the split.
#[component]
fn Divider(split: Split, index: usize, mounted: Signal<Option<Rc<MountedData>>>) -> Element {
    let shell = use_shell();
    let id = split.id;
    let (hit, cursor) = match split.axis {
        Axis::Row => ("left: -3px; top: 0; bottom: 0; width: 6px;", "col-resize"),
        Axis::Column => ("top: -3px; left: 0; right: 0; height: 6px;", "row-resize"),
    };
    let line = match split.axis {
        Axis::Row => "left: 0; top: 0; bottom: 0; width: 1px;",
        Axis::Column => "top: 0; left: 0; right: 0; height: 1px;",
    };
    rsx! {
        div {
            style: "position: relative; flex: none; width: 0; height: 0; align-self: stretch; z-index: 10;",
            div { style: "position: absolute; {line} background-color: var(--as-border);" }
            div {
                class: "as-divider",
                "data-divider": "{id.0}:{index}",
                style: "position: absolute; {hit} cursor: {cursor};",
                onpointerdown: move |event| {
                    event.stop_propagation();
                    let p = event.client_coordinates();
                    let mut gesture = shell.gesture;
                    gesture.set(Some(Gesture::Divider {
                        split: id,
                        index,
                        axis: split.axis,
                        start: (p.x, p.y),
                        shares: split.children.iter().map(|c| c.share).collect(),
                        length: None,
                    }));
                    // Measured off the event (the document is busy in a
                    // handler), then the drag uses it.
                    if let Some(m) = mounted.peek().clone() {
                        let axis = split.axis;
                        spawn(async move {
                            if let Ok(rect) = m.get_client_rect().await {
                                let len = match axis {
                                    Axis::Row => rect.size.width,
                                    Axis::Column => rect.size.height,
                                };
                                let mut gesture = gesture;
                                if let Some(Gesture::Divider { length, .. }) = &mut *gesture.write() {
                                    *length = Some(len);
                                }
                            }
                        });
                    }
                },
                ondoubleclick: move |_| {
                    shell.apply(|w| w.even(id));
                },
            }
        }
    }
}
