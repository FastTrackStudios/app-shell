//! What fills the top bar besides the shell's own controls.
//!
//! The shell always owns the window's controls (its traffic lights or the
//! system's), the two sidebar toggles, back and forward. Everything else
//! in the bar is a **place** an app, a page or an extension contributes
//! to, without drawing the bar itself:
//!
//! | place | where | by default |
//! |---|---|---|
//! | [`BarPlace::Leading`] | after back and forward | the title (or the `TopBar`'s `lead`) |
//! | [`BarPlace::Center`] | centred | the `TopBar`'s `search` |
//! | [`BarPlace::Trailing`] | before the right sidebar's toggle | the `TopBar`'s `actions` |
//!
//! An item ([`BarItem`]) goes into a place after the app's own content,
//! by its `order` (then its id). A **span** ([`BarSpan`]) can be taken
//! over whole by one [`BarFiller`]: the center (in place of the search),
//! or the middle (the title, the center and the trailing actions: a
//! song's tabs across the bar). The last filler put wins; removing it
//! brings back what was there.
//!
//! In a component, [`use_top_bar_item`] and [`use_top_bar_span`] put
//! theirs while the component is mounted. Anything else (an extension
//! host turning an extension's declared toolbar items into bar items)
//! calls [`super::Shell::put_bar_item`] and friends. A render function is
//! called inside the bar, so what it reads (signals) re-renders it there.

use std::rc::Rc;

use dioxus::prelude::*;

use super::shell::use_shell;

/// A place in the top bar items go into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BarPlace {
    Leading,
    Center,
    Trailing,
}

/// A stretch of the top bar one filler can take over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BarSpan {
    /// The center: in place of the search and the center's items.
    Center,
    /// The title, the center and the trailing items: everything but the
    /// shell's own controls and the tab tools.
    Middle,
}

/// Draws an item or a filler; called inside the bar.
pub type BarRender = Rc<dyn Fn() -> Element>;

/// One contribution to a place.
#[derive(Clone)]
pub struct BarItem {
    /// Unique among the bar's items (`studio/now-playing`,
    /// `ext:lab.board/sync`): putting an item with an id that is there
    /// replaces it.
    pub id: String,
    pub place: BarPlace,
    /// Lower first.
    pub order: i32,
    pub render: BarRender,
}

impl BarItem {
    pub fn new(
        id: impl Into<String>,
        place: BarPlace,
        order: i32,
        render: impl Fn() -> Element + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            place,
            order,
            render: Rc::new(render),
        }
    }
}

/// One span taken over.
#[derive(Clone)]
pub struct BarFiller {
    /// Who put it (`studio/song-tabs`): only its owner removes it.
    pub owner: String,
    pub span: BarSpan,
    pub render: BarRender,
}

/// Everything put into the bar, in the shell's state.
#[derive(Clone, Default)]
pub struct BarContributions {
    items: Vec<BarItem>,
    fillers: Vec<BarFiller>,
}

impl BarContributions {
    /// Put `item`, in place of one with its id.
    pub fn put(&mut self, item: BarItem) {
        match self.items.iter_mut().find(|i| i.id == item.id) {
            Some(i) => *i = item,
            None => self.items.push(item),
        }
    }

    /// Whether an item has this id.
    #[must_use]
    pub fn items_with_id(&self, id: &str) -> bool {
        self.items.iter().any(|i| i.id == id)
    }

    pub fn remove(&mut self, id: &str) {
        self.items.retain(|i| i.id != id);
    }

    /// Take over `filler.span` (in place of any filler of the same owner
    /// and span).
    pub fn fill(&mut self, filler: BarFiller) {
        self.fillers
            .retain(|f| !(f.owner == filler.owner && f.span == filler.span));
        self.fillers.push(filler);
    }

    /// Give back what `owner` took over.
    pub fn unfill(&mut self, owner: &str, span: BarSpan) {
        self.fillers
            .retain(|f| !(f.owner == owner && f.span == span));
    }

    /// A place's items in order.
    #[must_use]
    pub fn items(&self, place: BarPlace) -> Vec<BarItem> {
        let mut out: Vec<BarItem> = self
            .items
            .iter()
            .filter(|i| i.place == place)
            .cloned()
            .collect();
        out.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
        out
    }

    /// The filler of `span` on show: the last put.
    #[must_use]
    pub fn filler(&self, span: BarSpan) -> Option<BarFiller> {
        self.fillers.iter().rev().find(|f| f.span == span).cloned()
    }

    /// How many items there are (all places).
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Put an item in the top bar while the calling component is mounted.
/// `render` is called inside the bar (what it reads re-renders it there).
pub fn use_top_bar_item(
    id: &str,
    place: BarPlace,
    order: i32,
    render: impl Fn() -> Element + 'static,
) {
    let shell = use_shell();
    let id = id.to_owned();
    use_hook({
        let id = id.clone();
        move || shell.put_bar_item(BarItem::new(id, place, order, render))
    });
    use_drop(move || shell.remove_bar_item(&id));
}

/// Take over a span of the top bar while the calling component is
/// mounted (`owner` names it: `studio/song-tabs`).
pub fn use_top_bar_span(owner: &str, span: BarSpan, render: impl Fn() -> Element + 'static) {
    let shell = use_shell();
    let owner = owner.to_owned();
    use_hook({
        let owner = owner.clone();
        move || {
            shell.fill_bar(BarFiller {
                owner,
                span,
                render: Rc::new(render),
            });
        }
    });
    use_drop(move || shell.unfill_bar(&owner, span));
}

/// A place's contributed items, drawn in a row (the bar's own).
#[component]
pub(crate) fn BarItems(place: BarPlace) -> Element {
    let shell = use_shell();
    let items = shell.bar.read().items(place);
    rsx! {
        for item in items {
            div {
                key: "{item.id}",
                class: "as-bar-item",
                "data-bar-item": "{item.id}",
                style: "flex: none; display: flex; align-items: center; min-width: 0;",
                onpointerdown: move |event| event.stop_propagation(),
                ondoubleclick: move |event| event.stop_propagation(),
                {(item.render)()}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, place: BarPlace, order: i32) -> BarItem {
        BarItem::new(id, place, order, || rsx! {})
    }

    #[test]
    fn items_come_in_order_and_an_id_is_put_once() {
        let mut bar = BarContributions::default();
        bar.put(item("b", BarPlace::Trailing, 10));
        bar.put(item("a", BarPlace::Trailing, 10));
        bar.put(item("first", BarPlace::Trailing, 0));
        bar.put(item("lead", BarPlace::Leading, 0));
        bar.put(item("a", BarPlace::Trailing, 5));
        let ids: Vec<String> = bar
            .items(BarPlace::Trailing)
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(ids, ["first", "a", "b"]);
        assert_eq!(bar.len(), 4);
        bar.remove("a");
        assert_eq!(bar.items(BarPlace::Trailing).len(), 2);
    }

    #[test]
    fn the_last_filler_wins_and_its_owner_gives_it_back() {
        let mut bar = BarContributions::default();
        let filler = |owner: &str| BarFiller {
            owner: owner.into(),
            span: BarSpan::Middle,
            render: Rc::new(|| rsx! {}),
        };
        assert!(bar.filler(BarSpan::Middle).is_none());
        bar.fill(filler("set"));
        bar.fill(filler("song"));
        assert_eq!(
            bar.filler(BarSpan::Middle).map(|f| f.owner).as_deref(),
            Some("song")
        );
        bar.unfill("song", BarSpan::Middle);
        assert_eq!(
            bar.filler(BarSpan::Middle).map(|f| f.owner).as_deref(),
            Some("set")
        );
        bar.unfill("set", BarSpan::Center);
        assert!(bar.filler(BarSpan::Middle).is_some());
        assert!(bar.filler(BarSpan::Center).is_none());
    }
}
