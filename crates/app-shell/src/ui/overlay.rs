//! What opens over the page: bottom sheets (a stack of them, each with a
//! handle to drag, detents and a scrolling body), action sheets (a menu
//! on a phone), the command palette and a queue of toasts. On a desktop
//! the same sheet is a centred panel and the palette sits near the top.
//!
//! Blitz has no `position: fixed`, and a component cannot draw outside
//! its parent, so what opens is described (a [`SheetSpec`], a
//! [`ToastSpec`]) and drawn by the shell's root. A sheet's content is a
//! render callback run there: contexts provided only where it was opened
//! are not in reach, so capture what it needs.

use std::time::Duration;

use dioxus::prelude::*;

use super::foot::{bottom_pad, top_pad};
use super::icons::{self, Icon, IconView};
use super::popup::{MenuAction, MenuItem};
use super::shell::{FormFactor, Gesture, Shell, use_shell};
use crate::gestures::{Detent, Swipe, swipe_of};

/// A sheet's id, to close it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SheetId(pub u64);

/// What a sheet holds.
#[derive(Clone, PartialEq)]
pub enum SheetContent {
    /// The app's content, rendered at the shell's root.
    Custom(Callback<(), Element>),
    /// A menu: an action sheet on a phone.
    Menu(Vec<MenuItem>),
    /// The command palette: the app's entries, then what the
    /// [`super::PaletteSources`] in context give.
    Palette(Vec<PaletteItem>),
    /// Open anything (⌘O): what the openers in the
    /// [`super::PaletteSources`] find.
    QuickOpen,
    /// A palette overlay drawn by a view of the app's or an extension's
    /// ([`super::Shell::set_palette_view`]), where the palette goes.
    Overlay(Callback<(), Element>),
}

/// A sheet to open.
#[derive(Clone, PartialEq)]
pub struct SheetSpec {
    pub title: Option<String>,
    /// How tall it opens.
    pub detent: Detent,
    /// The heights a drag may settle it at (always including `detent`).
    pub detents: Vec<Detent>,
    pub content: SheetContent,
    /// Told when it closes (dragged away, the scrim, Escape, `close_sheet`).
    pub on_close: Option<Callback<()>>,
}

impl SheetSpec {
    /// A sheet of the app's own content.
    pub fn new(render: impl FnMut(()) -> Element + 'static) -> Self {
        Self {
            title: None,
            detent: Detent::Medium,
            detents: vec![Detent::Medium, Detent::Large],
            content: SheetContent::Custom(Callback::new(render)),
            on_close: None,
        }
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Opens at `detent`, settling only at the ones in `detents`.
    #[must_use]
    pub fn detents(mut self, detent: Detent, detents: &[Detent]) -> Self {
        self.detent = detent;
        self.detents = detents.to_vec();
        if !self.detents.contains(&detent) {
            self.detents.push(detent);
        }
        self
    }

    #[must_use]
    pub fn on_close(mut self, f: impl FnMut(()) + 'static) -> Self {
        self.on_close = Some(Callback::new(f));
        self
    }
}

/// A sheet on show.
#[derive(Clone, PartialEq)]
pub(crate) struct SheetEntry {
    pub id: SheetId,
    pub spec: SheetSpec,
    /// Its height now (dragged to another detent).
    pub detent: Detent,
    /// How far it is dragged now, points (down positive).
    pub drag: f64,
}

/// An entry of the command palette.
#[derive(Clone, PartialEq)]
pub struct PaletteItem {
    /// What its runs are remembered by (recently run entries come
    /// first); its label when unset.
    pub id: Option<String>,
    pub label: String,
    /// Its group (`Tabs`): found by it too.
    pub group: String,
    /// Its key, shown at the row's end (`⌘W`).
    pub shortcut: Option<String>,
    pub detail: Option<String>,
    pub icon: Option<Icon>,
    /// Words it is also found by.
    pub keywords: String,
    pub action: MenuAction,
}

impl PaletteItem {
    /// An entry calling the app.
    pub fn call(label: impl Into<String>, f: Callback<()>) -> Self {
        Self {
            id: None,
            label: label.into(),
            group: String::new(),
            shortcut: None,
            detail: None,
            icon: None,
            keywords: String::new(),
            action: MenuAction::Call(f),
        }
    }

    /// An entry running a shell command.
    pub fn command(label: impl Into<String>, command: crate::Command) -> Self {
        Self {
            id: None,
            label: label.into(),
            group: String::new(),
            shortcut: None,
            detail: None,
            icon: None,
            keywords: String::new(),
            action: MenuAction::Command {
                pane: None,
                command,
            },
        }
    }

    #[must_use]
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    #[must_use]
    pub fn keywords(mut self, words: impl Into<String>) -> Self {
        self.keywords = words.into();
        self
    }

    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    #[must_use]
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = group.into();
        self
    }

    #[must_use]
    pub fn shortcut(mut self, keys: impl Into<String>) -> Self {
        self.shortcut = Some(keys.into());
        self
    }
}

/// A toast to show.
#[derive(Clone, PartialEq)]
pub struct ToastSpec {
    pub message: String,
    /// A button on it (Undo, View).
    pub action: Option<(String, Callback<()>)>,
    /// How long it stays.
    pub duration: Duration,
}

impl ToastSpec {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            action: None,
            duration: Duration::from_millis(2600),
        }
    }

    #[must_use]
    pub fn action(mut self, label: impl Into<String>, f: impl FnMut(()) + 'static) -> Self {
        self.action = Some((label.into(), Callback::new(f)));
        self
    }

    #[must_use]
    pub const fn duration(mut self, d: Duration) -> Self {
        self.duration = d;
        self
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct ToastEntry {
    pub id: u64,
    pub spec: ToastSpec,
}

/// Opens and closes sheets for the calling component; the ones it opened
/// close when it goes (their render callbacks are its).
#[derive(Clone, Copy)]
pub struct SheetHandle {
    shell: Shell,
    mine: CopyValue<Vec<SheetId>>,
}

impl SheetHandle {
    /// Open `spec` over everything open.
    pub fn open(&self, spec: SheetSpec) -> SheetId {
        let id = self.shell.open_sheet(spec);
        let mut mine = self.mine;
        mine.write().push(id);
        id
    }

    /// Close the sheet `id` (and only that one).
    pub fn close(&self, id: SheetId) {
        self.shell.close_sheet(id);
        let mut mine = self.mine;
        mine.write().retain(|s| *s != id);
    }

    /// Close every sheet this component opened.
    pub fn close_all(&self) {
        let ids = self.mine.peek().clone();
        for id in ids {
            self.shell.close_sheet(id);
        }
        let mut mine = self.mine;
        mine.write().clear();
    }

    /// Whether the sheet `id` is open.
    #[must_use]
    pub fn is_open(&self, id: SheetId) -> bool {
        self.shell.sheets.peek().iter().any(|s| s.id == id)
    }
}

/// A [`SheetHandle`] for the calling component.
#[must_use]
pub fn use_sheet() -> SheetHandle {
    let shell = use_shell();
    let handle = use_hook(|| SheetHandle {
        shell,
        mine: CopyValue::new(Vec::new()),
    });
    use_drop(move || {
        let ids = handle.mine.peek().clone();
        for id in ids {
            handle.shell.close_sheet_quiet(id);
        }
    });
    handle
}

/// A sheet that is open while `open` is true: what a component that keeps
/// its own "is it open" state declares. Its content is `render`'s.
#[component]
pub fn Sheet(
    open: bool,
    render: Callback<(), Element>,
    title: Option<String>,
    #[props(default = Detent::Medium)] detent: Detent,
    #[props(default = vec![Detent::Medium, Detent::Large])] detents: Vec<Detent>,
    /// Told when the person closes it (drag, scrim, Escape): set `open`
    /// false.
    on_close: Option<EventHandler<()>>,
) -> Element {
    let sheet = use_sheet();
    let current = use_hook(|| CopyValue::new(None::<SheetId>));
    // The props as last rendered: what the effect opens with.
    let mut latest = use_hook(|| CopyValue::new(None::<SheetSpec>));
    latest.set(Some(SheetSpec {
        title,
        detent,
        detents,
        content: SheetContent::Custom(render),
        on_close: on_close.map(|f| Callback::new(move |()| f.call(()))),
    }));
    use_effect(use_reactive!(|open| {
        let mut current = current;
        let id_now = *current.peek();
        let live = id_now.is_some_and(|id| sheet.is_open(id));
        if open && !live {
            if let Some(spec) = latest.peek().clone() {
                current.set(Some(sheet.open(spec)));
            }
        } else if !open {
            if let Some(id) = id_now {
                sheet.shell.close_sheet_quiet(id);
            }
            current.set(None);
        }
    }));
    rsx! {}
}

/// The sheets on show, the top one under the finger: drawn by the root.
#[component]
pub(crate) fn SheetLayer() -> Element {
    let shell = use_shell();
    let sheets = shell.sheets.read().clone();
    if sheets.is_empty() {
        return rsx! {};
    }
    let phone = shell.form() == FormFactor::Phone;
    let dragging = matches!(&*shell.gesture.read(), Some(Gesture::Sheet { .. }));
    let n = sheets.len();
    rsx! {
        for (i, entry) in sheets.into_iter().enumerate() {
            SheetFrame {
                key: "{entry.id.0}",
                entry: entry.clone(),
                top: i + 1 == n,
                depth: n - 1 - i,
                phone,
                dragging: dragging && i + 1 == n,
            }
        }
    }
}

#[component]
fn SheetFrame(entry: SheetEntry, top: bool, depth: usize, phone: bool, dragging: bool) -> Element {
    let shell = use_shell();
    let id = entry.id;
    let scrim = if top {
        "var(--as-overlay)"
    } else {
        "transparent"
    };
    let events = if top { "auto" } else { "none" };
    let palette = matches!(
        entry.spec.content,
        SheetContent::Palette(_) | SheetContent::QuickOpen | SheetContent::Overlay(_)
    );
    let body = match entry.spec.content.clone() {
        SheetContent::Custom(render) => render.call(()),
        SheetContent::Menu(items) => rsx! { ActionList { items } },
        SheetContent::Palette(items) => rsx! { super::palette::PaletteView { items, phone } },
        SheetContent::QuickOpen => {
            rsx! { super::palette::PaletteView { items: Vec::new(), phone, open: true } }
        }
        SheetContent::Overlay(draw) => draw.call(()),
    };
    let title = entry.spec.title.clone();
    let header = rsx! {
        if let Some(t) = title {
            div {
                class: "as-sheet-title",
                style: "display: flex; align-items: center; gap: 8px; padding: 2px 16px 8px 16px;",
                span { style: "flex: 1 1 0px; min-width: 0; font-size: 17px; font-weight: 700; white-space: nowrap; overflow: hidden;", "{t}" }
                if !phone {
                    super::chrome::BarButton { icon: icons::X, label: "Close", on_press: move |()| shell.close_sheet(id) }
                }
            }
        }
    };
    if !phone {
        // A desktop: a centred panel (the palette near the top).
        let place = if palette {
            "left: 50%; top: 12%; transform: translateX(-50%); width: 600px; max-width: 92%; max-height: 70%;"
        } else {
            "left: 50%; top: 50%; transform: translate(-50%, -50%); width: 560px; max-width: 92%; max-height: 80%;"
        };
        return rsx! {
            div {
                class: "as-sheet-layer",
                "data-sheet": "panel",
                style: "position: absolute; left: 0; top: 0; right: 0; bottom: 0; z-index: {70 + depth_z(depth)}; background-color: {scrim}; pointer-events: {events};",
                onclick: move |_| shell.close_sheet(id),
                div {
                    class: "as-sheet",
                    role: "dialog",
                    style: "position: absolute; {place} display: flex; flex-direction: column; background-color: var(--as-surface); border: 1px solid var(--as-border); border-radius: 12px; box-shadow: 0 24px 64px rgba(0, 0, 0, 0.55); overflow: hidden; padding-top: 10px;",
                    onclick: move |event| event.stop_propagation(),
                    {header}
                    div {
                        class: "as-sheet-body",
                        style: "flex: 1 1 auto; min-height: 0; overflow-y: scroll; padding: 0 12px 12px 12px;",
                        {body}
                    }
                }
            }
        };
    }
    // A phone: a bottom sheet.
    let height = match entry.detent {
        Detent::Fit => "height: auto; max-height: 92%;",
        Detent::Medium => "height: 50%; max-height: 92%;",
        Detent::Large => "height: 92%; max-height: 92%;",
    };
    // Up past its detent only a little (rubber band).
    let dy = entry.drag.max(-40.0);
    let under = if depth > 0 {
        format!(" scale({})", 1.0 - 0.04 * depth.min(3) as f64)
    } else {
        String::new()
    };
    let transition = if dragging {
        "none"
    } else {
        "transform 0.26s cubic-bezier(0.2, 0.8, 0.2, 1), height 0.26s cubic-bezier(0.2, 0.8, 0.2, 1)"
    };
    let pad = bottom_pad(14.0, 0.0);
    rsx! {
        div {
            class: "as-sheet-layer",
            "data-sheet": "bottom",
            style: "position: absolute; left: 0; top: 0; right: 0; bottom: 0; z-index: {70 + depth_z(depth)}; background-color: {scrim}; pointer-events: {events};",
            onclick: move |_| shell.close_sheet(id),
            div {
                class: "as-sheet",
                role: "dialog",
                "data-detent": "{detent_name(entry.detent)}",
                style: "position: absolute; left: 0; right: 0; bottom: 0; {height} display: flex; flex-direction: column; box-sizing: border-box; background-color: var(--as-surface); border-top: 1px solid var(--as-border); border-radius: 20px 20px 0 0; box-shadow: 0 -16px 48px rgba(0, 0, 0, 0.5); transform: translateY({dy}px){under}; transform-origin: top center; transition: {transition}; {pad}",
                onclick: move |event| event.stop_propagation(),
                div {
                    class: "as-sheet-handle",
                    style: "flex: none; padding-top: 6px; touch-action: none; cursor: grab;",
                    onpointerdown: move |event| {
                        event.stop_propagation();
                        let p = event.client_coordinates();
                        let mut g = shell.gesture;
                        g.set(Some(Gesture::Sheet { id: id.0, start: (p.x, p.y) }));
                    },
                    div { class: "as-grab", style: "width: 36px; height: 5px; border-radius: 3px; background-color: var(--as-border); margin: 4px auto 8px auto;" }
                    {header}
                }
                div {
                    class: "as-sheet-body",
                    style: "flex: 1 1 auto; min-height: 0; overflow-y: scroll; padding: 0 12px;",
                    {body}
                }
            }
        }
    }
}

const fn depth_z(depth: usize) -> usize {
    5_usize.saturating_sub(depth)
}

pub(crate) const fn detent_name(d: Detent) -> &'static str {
    match d {
        Detent::Fit => "fit",
        Detent::Medium => "medium",
        Detent::Large => "large",
    }
}

/// A menu as an action sheet's rows: large, easy to hit.
#[component]
fn ActionList(items: Vec<MenuItem>) -> Element {
    let shell = use_shell();
    rsx! {
        div {
            class: "as-action-list",
            style: "display: flex; flex-direction: column; padding-bottom: 4px;",
            for (i, item) in items.into_iter().enumerate() {
                if item.action.is_none() && item.label.is_empty() {
                    div { key: "{i}", style: "height: 1px; margin: 4px 8px; background-color: var(--as-border);" }
                } else if item.is_text() {
                    {
                        let (size, color) = if item.heading { ("12px", "var(--as-heading)") } else { ("15px", "var(--as-muted)") };
                        rsx! {
                            div { key: "{i}", style: "padding: 10px 12px 4px; font-size: {size}; color: {color}; white-space: nowrap; overflow: hidden;", "{item.label}" }
                        }
                    }
                } else {
                    {
                        let action = item.action.clone();
                        let color = if item.danger { "var(--as-danger)" } else { "var(--as-text)" };
                        rsx! {
                            div {
                                key: "{i}",
                                class: "as-item",
                                role: "menuitem",
                                style: "display: flex; align-items: center; gap: 14px; min-height: 48px; padding: 0 12px; border-radius: 12px; color: {color}; font-size: 16px;",
                                onclick: move |_| {
                                    shell.close_top_sheet();
                                    shell.run_action(action.clone());
                                },
                                div { style: "width: 20px; flex: none; display: flex;",
                                    if let Some(icon) = item.icon { IconView { icon, size: 20.0 } }
                                }
                                span { style: "flex: 1 1 0px; min-width: 0; white-space: nowrap; overflow: hidden;", "{item.label}" }
                                if item.checked == Some(true) {
                                    IconView { icon: super::icons::CHECK, size: 18.0 }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The toast at the head of the queue: at the top of a phone (under the
/// notch), at the foot of a desktop.
#[component]
pub(crate) fn ToastLayer() -> Element {
    let shell = use_shell();
    let head = shell.toasts.read().first().cloned();
    let phone = shell.form() == FormFactor::Phone;
    // Each toast goes after its time.
    use_effect(move || {
        let Some(t) = shell.toasts.read().first().cloned() else {
            return;
        };
        spawn(async move {
            futures_timer::Delay::new(t.spec.duration).await;
            let mut toasts = shell.toasts;
            if toasts.peek().first().is_some_and(|x| x.id == t.id) {
                toasts.write().remove(0);
            }
        });
    });
    let Some(t) = head else {
        return rsx! {};
    };
    let place = if phone {
        format!("top: 0; {}", top_pad(10.0))
    } else {
        "bottom: 24px;".to_owned()
    };
    let id = t.id;
    rsx! {
        div {
            class: "as-toast-row",
            style: "position: absolute; left: 0; right: 0; {place} z-index: 90; display: flex; justify-content: center; pointer-events: none;",
            div {
                class: "as-toast",
                role: "status",
                "aria-live": "polite",
                style: "pointer-events: auto; display: flex; align-items: center; gap: 12px; max-width: 92%; padding: 10px 16px; border-radius: 20px; background-color: var(--as-raised); border: 1px solid var(--as-border); color: var(--as-text); font-weight: 550; box-shadow: 0 12px 32px rgba(0, 0, 0, 0.45);",
                span { style: "white-space: nowrap; overflow: hidden; min-width: 0;", "{t.spec.message}" }
                if let Some((label, f)) = t.spec.action.clone() {
                    span {
                        role: "button",
                        style: "flex: none; color: var(--as-accent); font-weight: 650;",
                        onclick: move |_| {
                            f.call(());
                            let mut toasts = shell.toasts;
                            toasts.write().retain(|x| x.id != id);
                        },
                        "{label}"
                    }
                }
            }
        }
    }
}

/// A content area that pages by swiping: left and right (the next and
/// the previous), down (close), with the content following the finger.
/// Quick Look's frame on a phone; a press that does not move is the
/// content's.
#[component]
pub fn Swipeable(
    on_swipe: EventHandler<Swipe>,
    #[props(default = "position: relative; flex: 1 1 0px; min-width: 0; min-height: 0; display: flex;".to_owned())]
    style: String,
    children: Element,
) -> Element {
    let mut start = use_signal(|| None::<(f64, f64)>);
    let mut delta = use_signal(|| (0.0_f64, 0.0_f64));
    let (dx, dy) = delta();
    // Follow the main axis only; down only (pulling up does nothing).
    let (tx, ty) = if dx.abs() >= dy.abs() {
        (dx, 0.0)
    } else {
        (0.0, dy.max(0.0))
    };
    let fade = (1.0 - ty / 600.0).clamp(0.4, 1.0);
    let transition = if start().is_some() {
        "none"
    } else {
        "transform 0.2s ease-out, opacity 0.2s ease-out"
    };
    rsx! {
        div {
            class: "as-swipeable",
            style: "{style} touch-action: none;",
            onpointerdown: move |e| {
                let p = e.client_coordinates();
                start.set(Some((p.x, p.y)));
                delta.set((0.0, 0.0));
            },
            onpointermove: move |e| {
                if let Some((sx, sy)) = start() {
                    let p = e.client_coordinates();
                    delta.set((p.x - sx, p.y - sy));
                }
            },
            onpointerup: move |_| {
                let (dx, dy) = delta();
                start.set(None);
                delta.set((0.0, 0.0));
                if let Some(s) = swipe_of(dx, dy) {
                    on_swipe.call(s);
                }
            },
            onpointercancel: move |_| {
                start.set(None);
                delta.set((0.0, 0.0));
            },
            div {
                style: "flex: 1 1 0px; min-width: 0; min-height: 0; display: flex; transform: translate({tx}px, {ty}px); opacity: {fade}; transition: {transition};",
                {children}
            }
        }
    }
}
