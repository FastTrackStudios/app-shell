//! Tab kinds: what the app (or, later, an extension) registers to draw a
//! tab. A pane names its kind ([`crate::Pane::view`]); the shell finds the
//! kind here and calls its render function with a [`TabCtx`].

use std::fmt;
use std::rc::Rc;

use dioxus::prelude::*;

use super::icons::{FILE, Icon};
use crate::model::{Pane, WindowId};

/// What a tab kind's render function is given.
#[derive(Debug, Clone, PartialEq)]
pub struct TabCtx {
    /// The tab: its id, its state (what it shows), its title.
    pub pane: Pane,
    /// The window it is in.
    pub window: WindowId,
    /// Whether it is the active tab of the whole layout.
    pub active: bool,
}

/// Draws a tab's content.
pub type Render = Rc<dyn Fn(TabCtx) -> Element>;

/// One kind of tab.
#[derive(Clone)]
pub struct TabKind {
    /// The id a pane's `view` names (`"notes.editor"`).
    pub id: String,
    /// The kind's name, in the switcher and a placeholder.
    pub title: String,
    /// Drawn in the tab and on its switcher card.
    pub icon: Icon,
    /// Draws the content. It is called inside a component of its own, so
    /// it may use hooks; that component is keyed by the pane's id.
    pub render: Render,
}

impl TabKind {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        render: impl Fn(TabCtx) -> Element + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: FILE,
            render: Rc::new(render),
        }
    }

    #[must_use]
    pub const fn icon(mut self, icon: Icon) -> Self {
        self.icon = icon;
        self
    }
}

impl fmt::Debug for TabKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TabKind")
            .field("id", &self.id)
            .field("title", &self.title)
            .finish_non_exhaustive()
    }
}

impl PartialEq for TabKind {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.title == other.title
            && self.icon == other.icon
            && Rc::ptr_eq(&self.render, &other.render)
    }
}

/// The kinds a shell knows. A pane whose kind is not here keeps its
/// place and shows a placeholder, so a layout outlives a kind that went.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TabKinds {
    kinds: Vec<TabKind>,
}

impl TabKinds {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `kind`, replacing one with its id.
    #[must_use]
    pub fn with(mut self, kind: TabKind) -> Self {
        self.register(kind);
        self
    }

    /// Adds `kind`, replacing one with its id: what an extension system
    /// calls as it loads one.
    pub fn register(&mut self, kind: TabKind) {
        self.kinds.retain(|k| k.id != kind.id);
        self.kinds.push(kind);
    }

    /// Removes the kind `id` (its panes show the placeholder).
    pub fn unregister(&mut self, id: &str) {
        self.kinds.retain(|k| k.id != id);
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&TabKind> {
        self.kinds.iter().find(|k| k.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &TabKind> {
        self.kinds.iter()
    }
}
