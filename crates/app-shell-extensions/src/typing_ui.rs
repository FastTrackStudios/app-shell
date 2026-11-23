//! A phone's Typing Toolbar as toolbars (#316).
//!
//! The bar over the keyboard ([`TypingBar`]), filled with the toolbars of
//! place `typing` whose `when` is in force (Architect's
//! `docs/content/architecture/app-shell.md`, "The action bar and the
//! keyboard bar"). The app says what has the keyboard ([`Typing`]: the
//! focused editor, its own actions, the pages and views the focus is in)
//! and puts the bar where it goes (app-shell's accessory, over the
//! keyboard).
//!
//! - The focused editor's own items are the built-in toolbar of its
//!   context, `typing/<context>`
//!   ([`architect_extensions::toolbars::Toolbars::provide_editor`]): a person's copy
//!   reorders, hides or adds to them, and they keep the editor's icons.
//! - An extension's, from its manifest or at run time, follow: shown
//!   while the page or view their `when` names has the focus.
//! - With the toolbar editor on, their strips show above the bar, edited
//!   in place as any toolbar is.
//!
//! A press keeps the field's focus and runs as a toolbar's (source
//! `toolbar`, target `typing`, id the toolbar's key, the view the focus is
//! in); an editor's own action runs as the editor says.

use app_shell::ui::{Icon, IconView, named_icon};
use architect::action::{Invocation, InvocationSource};
use dioxus::prelude::*;

use crate::interface::{ExtensionUi, ExtensionView};
use crate::toolbar_ui::ToolbarStrip;
use architect_extensions::toolbar::{MenuItem, MenuKind, TYPING, ToolItem, ToolKind};
use architect_extensions::toolbars::EDITOR;

/// One of the focused editor's own editing actions: the bar's built-in
/// items.
#[derive(Clone, PartialEq)]
pub struct EditorAction {
    /// Its action's id (`md.bold`).
    pub id: String,
    pub label: String,
    pub icon: Option<Icon>,
    /// A count or a dot's text over it.
    pub badge: Option<String>,
    /// What a press does; `None` runs the action on the host's list.
    pub on_press: Option<Callback<()>>,
}

impl EditorAction {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            badge: None,
            on_press: None,
        }
    }
}

/// What has the keyboard, as the app says.
#[derive(Clone, PartialEq, Default)]
pub struct Typing {
    /// The editor whose field has the focus: its context (`markdown`).
    pub editor: Option<String>,
    /// Its editing actions, in its order.
    pub actions: Vec<EditorAction>,
    /// The pages and views the focus is in, outermost first
    /// (`view:note`, `view:lab.board/chart`).
    pub scopes: Vec<String>,
}

impl Typing {
    /// Every context in force: `editor:<context>` for the editor, then the
    /// scopes, each once.
    #[must_use]
    pub fn contexts(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for c in self
            .editor
            .iter()
            .map(|e| format!("editor:{e}"))
            .chain(self.scopes.iter().cloned())
        {
            if !out.contains(&c) {
                out.push(c);
            }
        }
        out
    }
}

/// One item of the bar.
#[derive(Clone, PartialEq)]
enum TypingItem {
    /// A button; a toggle shows on while it is.
    Button {
        key: String,
        label: String,
        icon: Option<Icon>,
        badge: Option<String>,
        checked: Option<bool>,
        on_press: Callback<()>,
    },
    Separator,
    /// A button opening rows above the bar.
    Menu {
        key: String,
        label: String,
        icon: Option<Icon>,
        cx: Cx,
        rows: Vec<MenuItem>,
    },
    /// A live control: an extension's view.
    Widget {
        key: String,
        width: f64,
        extension: String,
        view: String,
    },
}

/// The bar over the keyboard for `typing`: the toolbars of place
/// `typing` in force, each one's items after the last's, a rule between;
/// a menu's rows above it while open; with the toolbar editor on, their
/// strips above it. Nothing when none is in force.
#[component]
pub fn TypingBar(ui: ExtensionUi, typing: Typing) -> Element {
    let mut open = use_signal(|| None::<String>);
    let t = crate::theme_ui::shell_theme();
    let (items, editing) = fill(ui, &typing);
    if items.is_empty() && editing.is_empty() {
        return rsx! {};
    }
    let menu = open().and_then(|key| {
        items.iter().find_map(|i| match i {
            TypingItem::Menu {
                key: k, cx, rows, ..
            } if *k == key => Some((cx.clone(), rows.clone())),
            _ => None,
        })
    });
    let close = Callback::new(move |()| open.set(None));
    rsx! {
        div {
            class: "ext-typing-bar",
            style: "display:flex; flex-direction:column; min-width:0; background:{t.sheet}; border-top:1px solid {t.rule};",
            for key in editing {
                ToolbarStrip { key: "{key}", ui, toolbar: key }
            }
            if let Some((cx, rows)) = menu {
                div { style: "display:flex; flex-direction:column; padding:4px; max-height:40vh; overflow-y:scroll; border-bottom:1px solid {t.rule};",
                    TypingRows { cx, rows, depth: 0, close }
                }
            }
            div {
                role: "toolbar",
                style: "display:flex; align-items:center; gap:2px; height:44px; padding:0 6px; overflow-x:scroll; overflow-y:hidden;",
                for (n, item) in items.into_iter().enumerate() {
                    match item {
                        TypingItem::Separator => rsx! {
                            div { key: "{n}", style: "flex:none; width:1px; height:24px; margin:0 4px; background:{t.rule};" }
                        },
                        TypingItem::Button { key, label, icon, badge, checked, on_press } => {
                            let on = checked == Some(true);
                            let bg = if on { t.active } else { "transparent" };
                            rsx! {
                                div { key: "{n}-{key}",
                                    role: "button",
                                    title: "{label}",
                                    "aria-label": "{label}",
                                    "aria-pressed": checked.map(|c| c.to_string()),
                                    "data-tool": "{key}",
                                    style: "position:relative; flex:none; display:flex; align-items:center; justify-content:center; min-width:40px; height:36px; padding:0 6px; box-sizing:border-box; border-radius:8px; background:{bg}; color:{t.ink};",
                                    onmousedown: move |e| e.prevent_default(),
                                    onclick: move |_| on_press.call(()),
                                    match icon {
                                        Some(icon) => rsx! { IconView { icon, size: 18.0 } },
                                        None => rsx! { span { style: "font-size:14px;", "{label}" } },
                                    }
                                    if let Some(badge) = badge {
                                        span { style: "position:absolute; top:2px; right:2px; font-size:9px; color:{t.accent};", "{badge}" }
                                    }
                                }
                            }
                        }
                        TypingItem::Menu { key, label, icon, .. } => {
                            let on = open() == Some(key.clone());
                            let bg = if on { t.active } else { "transparent" };
                            rsx! {
                                div { key: "{n}-{key}",
                                    role: "button",
                                    title: "{label}",
                                    "aria-label": "{label}",
                                    "aria-expanded": "{on}",
                                    style: "flex:none; display:flex; align-items:center; gap:4px; height:36px; padding:0 8px; border-radius:8px; background:{bg}; color:{t.ink};",
                                    onmousedown: move |e| e.prevent_default(),
                                    onclick: move |_| open.set((!on).then(|| key.clone())),
                                    match icon {
                                        Some(icon) => rsx! { IconView { icon, size: 18.0 } },
                                        None => rsx! { span { style: "font-size:14px;", "{label}" } },
                                    }
                                    span { style: "font-size:10px;", if on { "▾" } else { "▴" } }
                                }
                            }
                        }
                        TypingItem::Widget { key, width, extension, view } => rsx! {
                            div { key: "{n}-{key}", style: "flex:none; width:{width}px; height:36px; display:flex; flex-direction:column;",
                                ExtensionView { ui, extension, view, fill: true }
                            }
                        },
                    }
                }
            }
        }
    }
}

/// The bar's items for `typing`, each toolbar's in turn, a separator
/// between toolbars; and the toolbars' keys whose strips show above it
/// while they are edited.
fn fill(ui: ExtensionUi, typing: &Typing) -> (Vec<TypingItem>, Vec<String>) {
    let (_, _) = (ui.revision(), ui.ticks());
    let host = ui.host();
    let bars = host.toolbars();
    // The editor's own items, kept as its context's built-in toolbar (a
    // change is heard as the toolbars', and drawn again).
    if let Some(editor) = &typing.editor {
        let own: Vec<(String, String)> = typing
            .actions
            .iter()
            .map(|a| (a.id.clone(), a.label.clone()))
            .collect();
        let _ = bars.provide_editor(editor, &own);
    }
    let shown = bars.typing(
        host.contributions(),
        typing.editor.as_deref(),
        &typing.contexts(),
    );
    let view = typing
        .scopes
        .iter()
        .rev()
        .find_map(|s| s.strip_prefix("view:"))
        .map(str::to_owned);
    let mut items = Vec::new();
    for t in &shown {
        let cx = Cx {
            ui,
            key: t.key.clone(),
            owner: t.owner.clone(),
            view: view.clone(),
        };
        let mut mine = Vec::new();
        for (n, item) in t.decl.items.iter().enumerate() {
            push(&cx, item, &n.to_string(), typing, &mut mine);
        }
        if !mine.is_empty() && !items.is_empty() {
            items.push(TypingItem::Separator);
        }
        items.extend(mine);
    }
    let editing = if bars.editing() {
        shown.into_iter().map(|t| t.key).collect()
    } else {
        Vec::new()
    };
    (items, editing)
}

/// What a toolbar's items are drawn and run with.
#[derive(Clone, PartialEq)]
struct Cx {
    ui: ExtensionUi,
    /// The toolbar's key and owner.
    key: String,
    owner: String,
    /// The view the focus is in (`lab.board/chart`).
    view: Option<String>,
}

impl Cx {
    fn invocation(&self) -> Invocation {
        let mut i = Invocation::new(InvocationSource::Toolbar).on(TYPING, self.key.clone());
        i.view.clone_from(&self.view);
        i
    }

    /// Runs `action` as pressed on this toolbar.
    fn run(&self, action: &str) {
        crate::panes::run_action(self.ui.host().list(), action, self.invocation());
    }
}

/// `item`, at `path` in its toolbar, as the bar's items.
fn push(cx: &Cx, item: &ToolItem, path: &str, typing: &Typing, out: &mut Vec<TypingItem>) {
    match item.kind {
        ToolKind::Separator => out.push(TypingItem::Separator),
        // A group's buttons, one after another: the bar is one row.
        ToolKind::Group => {
            for (n, inner) in item.items.iter().enumerate() {
                push(cx, inner, &format!("{path}.{n}"), typing, out);
            }
        }
        ToolKind::Widget => {
            let (extension, view) = item.view.split_once('/').map_or_else(
                || (cx.owner.clone(), item.view.clone()),
                |(e, v)| (e.to_owned(), v.to_owned()),
            );
            out.push(TypingItem::Widget {
                key: format!("{extension}/{view}"),
                width: f64::from(item.width.unwrap_or(120)),
                extension,
                view,
            });
        }
        ToolKind::Button if item.action.is_empty() => {
            let icon = (!item.icon.is_empty()).then(|| named_icon(&item.icon));
            let label = if item.label.is_empty() {
                "More".to_owned()
            } else {
                item.label.clone()
            };
            out.push(TypingItem::Menu {
                key: format!("{}#{path}", cx.key),
                label,
                icon,
                cx: cx.clone(),
                rows: item.menu.clone(),
            });
        }
        ToolKind::Button => out.push(button(cx, item, typing)),
    }
}

/// A button: its icon (its own, else the editor's for its action), its
/// name, a toggle's state from the app's list.
fn button(cx: &Cx, item: &ToolItem, typing: &Typing) -> TypingItem {
    let host = cx.ui.host();
    let info = host.list().info(&item.action);
    let builtin = typing.actions.iter().find(|a| a.id == item.action);
    let icon: Option<Icon> = if item.icon.is_empty() {
        builtin.and_then(|a| a.icon)
    } else {
        Some(named_icon(&item.icon))
    };
    let label = [
        Some(item.label.clone()).filter(|l| !l.is_empty()),
        builtin.map(|a| a.label.clone()),
        info.as_ref().map(|i| i.display_name.clone()),
    ]
    .into_iter()
    .flatten()
    .next()
    .unwrap_or_else(|| item.action.clone());
    // An editor's own run as the editor says; an extension's as pressed
    // on its toolbar.
    let own = builtin
        .filter(|_| cx.owner == EDITOR)
        .and_then(|b| b.on_press);
    let on_press = own.unwrap_or_else(|| {
        let (cx, id) = (cx.clone(), item.action.clone());
        Callback::new(move |()| cx.run(&id))
    });
    TypingItem::Button {
        key: item.action.clone(),
        label,
        icon,
        badge: builtin.and_then(|b| b.badge.clone()),
        checked: info.and_then(|i| i.checked),
        on_press,
    }
}

/// A dropdown's rows, above the bar: actions (ticked while a toggle is
/// on), headings, separators, and submenus that open in place. A row
/// pressed runs and closes it; the shell keeps the field's focus.
#[component]
fn TypingRows(cx: Cx, rows: Vec<MenuItem>, depth: usize, close: Callback<()>) -> Element {
    let (_, _) = (cx.ui.revision(), cx.ui.ticks());
    let t = crate::theme_ui::shell_theme();
    let mut expanded = use_signal(|| None::<usize>);
    let host = cx.ui.host();
    let indent = depth.saturating_mul(12);
    rsx! {
        for (n, row) in rows.into_iter().enumerate() {
            match row.kind {
                MenuKind::Heading => rsx! {
                    div { key: "{n}", style: "padding:8px 10px 2px {10 + indent}px; font-size:12px; font-weight:600; color:{t.ink_3};", "{row.label}" }
                },
                MenuKind::Separator => rsx! {
                    div { key: "{n}", role: "separator", style: "height:1px; margin:4px 8px; background:{t.rule};" }
                },
                MenuKind::Submenu => {
                    let on = expanded() == Some(n);
                    rsx! {
                        button { key: "{n}",
                            tabindex: "-1",
                            role: "menuitem",
                            "aria-expanded": "{on}",
                            style: "display:flex; justify-content:space-between; align-items:center; padding:0 10px 0 {10 + indent}px; \
                                    min-height:40px; border:none; border-radius:8px; background:transparent; color:{t.ink}; font-size:15px;",
                            onclick: move |_| expanded.set((!on).then_some(n)),
                            span { "{row.label}" }
                            span { if on { "▾" } else { "›" } }
                        }
                        if on {
                            TypingRows { key: "{n}-in", cx: cx.clone(), rows: row.items, depth: depth.saturating_add(1), close }
                        }
                    }
                }
                MenuKind::Action => {
                    let info = host.list().info(&row.action);
                    let checked = info.as_ref().and_then(|i| i.checked);
                    let label = if row.label.is_empty() {
                        info.map_or_else(|| row.action.clone(), |i| i.display_name)
                    } else {
                        row.label.clone()
                    };
                    let tick = if checked == Some(true) { "✓" } else { "" };
                    let (run, action) = (cx.clone(), row.action.clone());
                    rsx! {
                        button { key: "{n}",
                            tabindex: "-1",
                            role: if checked.is_some() { "menuitemcheckbox" } else { "menuitem" },
                            "aria-checked": checked.map(|on| on.to_string()),
                            "data-tool": "{row.action}",
                            style: "display:flex; align-items:center; gap:8px; padding:0 10px 0 {indent}px; min-height:40px; \
                                    border:none; border-radius:8px; background:transparent; color:{t.ink}; font-size:15px; text-align:left;",
                            onclick: move |e: MouseEvent| {
                                e.prevent_default();
                                close.call(());
                                run.run(&action);
                            },
                            span { style: "flex:0 0 auto; width:18px; text-align:center; color:{t.accent};", "{tick}" }
                            span { "{label}" }
                        }
                    }
                }
            }
        }
    }
}
