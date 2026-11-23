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

/// What an app (or an extension host) adds to every tab, whatever its
/// kind: provide it in context above [`super::AppShell`].
///
/// - `around` is called in each tab's content component, before its
///   kind's render, so it may use hooks there: it provides context the
///   content reads (an extension host's handle on the pane its view is
///   in, the view's state).
/// - `header` draws a row above the content (the pane's toolbars), or
///   nothing.
/// - `menu` adds rows to a tab's context menu, after the shell's own.
/// - `badge` draws after a tab's title (an icon, a count).
#[derive(Clone, Default)]
pub struct TabHooks {
    pub around: Option<Rc<dyn Fn(&TabCtx)>>,
    pub header: Option<Rc<dyn Fn(&TabCtx) -> Option<Element>>>,
    pub menu: Option<Rc<dyn Fn(&Pane) -> Vec<super::popup::MenuItem>>>,
    pub badge: Option<Rc<dyn Fn(&Pane) -> Option<Element>>>,
}

impl TabHooks {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn around(mut self, f: impl Fn(&TabCtx) + 'static) -> Self {
        self.around = Some(Rc::new(f));
        self
    }

    #[must_use]
    pub fn header(mut self, f: impl Fn(&TabCtx) -> Option<Element> + 'static) -> Self {
        self.header = Some(Rc::new(f));
        self
    }

    #[must_use]
    pub fn menu(mut self, f: impl Fn(&Pane) -> Vec<super::popup::MenuItem> + 'static) -> Self {
        self.menu = Some(Rc::new(f));
        self
    }

    #[must_use]
    pub fn badge(mut self, f: impl Fn(&Pane) -> Option<Element> + 'static) -> Self {
        self.badge = Some(Rc::new(f));
        self
    }
}

impl fmt::Debug for TabHooks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TabHooks")
            .field("around", &self.around.is_some())
            .field("header", &self.header.is_some())
            .field("menu", &self.menu.is_some())
            .field("badge", &self.badge.is_some())
            .finish()
    }
}

impl PartialEq for TabHooks {
    fn eq(&self, other: &Self) -> bool {
        fn same<T: ?Sized>(a: Option<&Rc<T>>, b: Option<&Rc<T>>) -> bool {
            match (a, b) {
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
        }
        same(self.around.as_ref(), other.around.as_ref())
            && same(self.header.as_ref(), other.header.as_ref())
            && same(self.menu.as_ref(), other.menu.as_ref())
            && same(self.badge.as_ref(), other.badge.as_ref())
    }
}
