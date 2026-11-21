//! More than one window: the seam a platform fills.
//!
//! In a browser a pop-out window of the layout is drawn in the page, over
//! the main window (a frame with a title bar you can move). Where the
//! platform has real windows (`app-shell-native`), it puts a
//! [`Windows`] in context: the main window's shell then asks it to open
//! an OS window for each pop-out and to close it when the pop-out goes,
//! and every window's shell keeps its copy of the layout in step through
//! a [`LayoutHub`].
//!
//! Each window has a `VirtualDom` of its own, so they do not share
//! signals: a change made in one window is published on the hub, and each
//! other window's shell, listening on a channel in its own runtime, takes
//! it into its own `Signal<Workspace>`.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};

use crate::model::{Rect, WindowId, Workspace};

/// A platform's windows, as the main window's shell drives them.
pub trait WindowSystem {
    /// Open an OS window for the pop-out `id` (focus it when it is open).
    fn open(&self, id: &WindowId, title: &str, bounds: Option<Rect>);
    /// Close the OS window of the pop-out `id`.
    fn close(&self, id: &WindowId);
}

/// The platform's [`WindowSystem`], in context (compared by identity).
#[derive(Clone)]
pub struct Windows(pub Rc<dyn WindowSystem>);

impl PartialEq for Windows {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// What travels between windows.
#[derive(Debug, Clone, PartialEq)]
pub enum HubMessage {
    /// The layout as it is now, after a change in `from`.
    Layout { from: WindowId, layout: Workspace },
    /// The platform closed the pop-out `0` (its close button): its tabs
    /// go back to the main window. Heard by the main window's shell.
    Closed(WindowId),
    /// The platform moved or resized the pop-out to these bounds.
    Moved(WindowId, Rect),
}

#[derive(Default)]
struct HubInner {
    latest: Option<Workspace>,
    listeners: Vec<(WindowId, UnboundedSender<HubMessage>)>,
}

/// Keeps every window's copy of the layout in step.
///
/// One per app, in every window's root context (`app-shell-native` puts
/// it there). Single-threaded: every window's dom runs on the event
/// loop's thread.
#[derive(Clone, Default)]
pub struct LayoutHub(Rc<RefCell<HubInner>>);

impl PartialEq for LayoutHub {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl LayoutHub {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The layout as last published (a new window starts from it).
    #[must_use]
    pub fn latest(&self) -> Option<Workspace> {
        self.0.borrow().latest.clone()
    }

    /// Hear what other windows publish, as `window`.
    pub fn subscribe(&self, window: &WindowId) -> UnboundedReceiver<HubMessage> {
        let (tx, rx) = unbounded();
        let mut inner = self.0.borrow_mut();
        inner
            .listeners
            .retain(|(w, s)| w != window && !s.is_closed());
        inner.listeners.push((window.clone(), tx));
        rx
    }

    /// `from` changed the layout to `layout`: every other window hears it.
    pub fn publish(&self, from: &WindowId, layout: &Workspace) {
        let mut inner = self.0.borrow_mut();
        inner.latest = Some(layout.clone());
        inner.listeners.retain(|(_, s)| !s.is_closed());
        for (w, s) in &inner.listeners {
            if w != from {
                let _ = s.unbounded_send(HubMessage::Layout {
                    from: from.clone(),
                    layout: layout.clone(),
                });
            }
        }
    }

    /// Tell the main window's shell something the platform did to a
    /// pop-out ([`HubMessage::Closed`], [`HubMessage::Moved`]).
    pub fn tell_main(&self, message: HubMessage) {
        let inner = self.0.borrow();
        for (w, s) in &inner.listeners {
            if w.is_main() {
                let _ = s.unbounded_send(message.clone());
            }
        }
    }
}

/// What a window's own frame can do: what the platform provides for a
/// window without system decorations, drawn by the shell's title bar
/// ([`super::TitleBar`]). In context, from `app-shell-native`.
#[derive(Clone, Copy, PartialEq)]
pub struct WindowControls {
    pub minimize: Option<Callback<()>>,
    pub toggle_maximize: Option<Callback<()>>,
    pub close: Option<Callback<()>>,
    /// Start moving the window with the pointer (a press on the title).
    pub drag: Option<Callback<()>>,
}
