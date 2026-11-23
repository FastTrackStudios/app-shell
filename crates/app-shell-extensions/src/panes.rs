//! Extensions' views in app-shell's panes.
//!
//! Each view a loaded extension declares is a [`TabKind`] of the shell
//! ([`ExtensionUi::tab_kinds`]), drawn by [`ExtensionView`]. Around every
//! tab's content (an extension's or the app's own) the shell calls the
//! [`TabHooks`] [`ExtensionUi::tab_hooks`] gives: they put the pane's
//! [`PaneHostRef`] in context (what `architect_extensions_dioxus::pane`'s
//! `use_pane_state` and `use_view_state` read), a pane header's toolbars
//! above it, the extensions' rows in a tab's context menu and what they
//! set on a tab (an icon, a badge) after its title.

use std::rc::Rc;

use app_shell::ui::{MenuItem, Shell, TabCtx, TabHooks, TabKind, TabKinds, named_icon, use_shell};
use app_shell::{Pane, PaneId, Side, Target, Workspace};
use architect::action::{ActionCaller, ActionList, Invocation, InvocationSource};
use architect_views::{PaneHost, PaneHostRef};
use dioxus::prelude::*;

use crate::interface::{ExtensionUi, ExtensionView};
use architect_extensions::manifest::Home;

/// A row extensions add to a context menu: an action, ticked while a
/// toggle is on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextEntry {
    pub label: String,
    /// The action it runs.
    pub command: String,
    /// A toggle's state; `None` for an action that is not one.
    pub checked: Option<bool>,
}

/// Where a view whose manifest says `home` opens.
#[must_use]
pub const fn home_target(home: Home) -> Target {
    match home {
        Home::Main => Target::Tab,
        Home::Left => Target::Dock(Side::Left),
        Home::Right => Target::Dock(Side::Right),
        Home::Bottom => Target::Dock(Side::Bottom),
    }
}

/// The pane a view is drawn in, as the shell keeps it: what
/// `architect_extensions_dioxus::pane` reads through [`PaneHostRef`].
struct ShellPane {
    layout: Signal<Workspace>,
    ui: ExtensionUi,
    id: PaneId,
}

impl PaneHost for ShellPane {
    fn id(&self) -> architect_views::PaneId {
        architect_views::PaneId(self.id.0.clone())
    }

    fn state(&self) -> String {
        self.layout
            .read()
            .pane(&self.id)
            .map(|p| p.state.clone())
            .unwrap_or_default()
    }

    fn view_state(&self) -> String {
        self.layout
            .read()
            .pane(&self.id)
            .map(|p| p.view_state.clone())
            .unwrap_or_default()
    }

    fn set_view_state(&self, value: String) {
        let id = self.id.clone();
        self.ui.apply(move |w| w.set_view_state(&id, value));
    }
}

/// What the action of a context-menu row on `target` `id` is told: the
/// target, the pane it was opened in and that pane's view.
#[must_use]
pub fn context_invocation(target: &str, id: &str, pane: &str, view: &str) -> Invocation {
    let file = target == "file" || target.starts_with("file:");
    Invocation {
        pane: Some(pane.to_owned()),
        view: Some(view.to_owned()),
        path: file.then(|| id.to_owned()),
        ..Invocation::new(InvocationSource::ContextMenu).on(target, id)
    }
}

/// Runs `action` on `list`, told `invocation`, without waiting: a press's.
pub fn run_action(list: &ActionList, action: &str, invocation: Invocation) {
    let (list, action) = (list.clone(), action.to_owned());
    spawn(async move {
        let caller = ActionCaller::app().with_invocation(invocation);
        if let Err(e) = list.run_json_async_with(&caller, &action, b"{}").await {
            tracing::warn!(ext.action = %action, error = %e, "an extension's row did not run");
        }
    });
}

/// `entries` as rows of the shell's menus, each run told `invocation`.
fn menu_items(
    ui: ExtensionUi,
    entries: Vec<ContextEntry>,
    invocation: &Invocation,
) -> Vec<MenuItem> {
    entries
        .into_iter()
        .map(|e| {
            let label = match e.checked {
                Some(true) => format!("✓ {}", e.label),
                _ => e.label,
            };
            let (command, invocation) = (e.command, invocation.clone());
            MenuItem::call(
                label,
                Callback::new(move |()| run_action(ui.host().list(), &command, invocation.clone())),
            )
        })
        .collect()
}

impl ExtensionUi {
    /// Every loaded extension's views as the shell's tab kinds, a
    /// placeholder kind for each one seen before that is gone, and the
    /// toolbars that are panes of their own. Read in a component, it
    /// draws again as extensions load and go.
    #[must_use]
    pub fn tab_kinds(&self) -> TabKinds {
        let mut kinds = TabKinds::new();
        self.register_kinds(&mut kinds);
        kinds
    }

    /// Adds [`Self::tab_kinds`] to the app's own `kinds`.
    pub fn register_kinds(&self, kinds: &mut TabKinds) {
        let me = *self;
        for (kind, extension, decl) in self.view_decls() {
            let (e, v) = (extension, decl.id.clone());
            let mut tab = TabKind::new(kind, decl.title.clone(), move |_ctx: TabCtx| {
                rsx! { ExtensionView { ui: me, extension: e.clone(), view: v.clone() } }
            });
            if !decl.icon.is_empty() {
                tab = tab.icon(named_icon(&decl.icon));
            }
            kinds.register(tab);
        }
        for kind in crate::toolbar_ui::pane_kinds(me) {
            kinds.register(kind);
        }
    }

    /// What the shell adds to every tab for the extensions: the pane's
    /// [`PaneHostRef`] around its content, its header's toolbars, their
    /// rows in its context menu, what they set on it after its title.
    /// [`crate::interface::use_extension_ui`] provides it.
    #[must_use]
    pub fn tab_hooks(&self) -> TabHooks {
        let me = *self;
        TabHooks::new()
            .around(move |ctx: &TabCtx| {
                let shell = use_shell();
                me.attach(shell);
                let id = ctx.pane.id.clone();
                let pane = use_hook(|| {
                    PaneHostRef(Rc::new(ShellPane {
                        layout: shell.layout,
                        ui: me,
                        id,
                    }))
                });
                use_context_provider(|| pane);
            })
            .header(move |ctx: &TabCtx| crate::toolbar_ui::header_tools(me, &ctx.pane))
            .menu(move |pane: &Pane| {
                let view = pane.view.0.clone();
                let mut rows = Vec::new();
                for target in [format!("view:{view}"), "tab".to_owned()] {
                    let entries = me.context_entries(&target, &pane.id.0);
                    let invocation = context_invocation(&target, &pane.id.0, &pane.id.0, &view);
                    rows.extend(menu_items(me, entries, &invocation));
                }
                rows
            })
            .badge(move |pane: &Pane| {
                crate::host_ui_view::tab_badges(&me.host(), &pane.view.0, &pane.id.0)
            })
    }

    /// What extensions add to a context menu on `target` `id` (the
    /// toolbars' own rows first): a tab's (`tab`, `view:<kind>`), a
    /// file's, a toolbar's.
    #[must_use]
    pub fn context_entries(&self, target: &str, id: &str) -> Vec<ContextEntry> {
        let host = self.host();
        let list = host.list().clone();
        let mut rows = crate::toolbar_ui::context_rows(&host, target, id);
        rows.extend(
            host.contributions()
                .context(target, id)
                .into_iter()
                .filter_map(|by| {
                    let info = list.info(&by.item.action)?;
                    let label = if by.item.title.is_empty() {
                        info.display_name
                    } else {
                        by.item.title.clone()
                    };
                    Some(ContextEntry {
                        label,
                        command: by.item.action,
                        checked: info.checked,
                    })
                }),
        );
        rows
    }

    /// Opens the context menu of `target` `id`, from inside pane `pane`,
    /// at `(x, y)` in the window: what extensions add for it.
    pub fn context_menu(&self, shell: &Shell, target: &str, id: &str, pane: &str, at: (f64, f64)) {
        let view = shell
            .layout
            .peek()
            .pane(&PaneId::new(pane))
            .map(|p| p.view.0.clone())
            .unwrap_or_default();
        let invocation = context_invocation(target, id, pane, &view);
        let items = menu_items(*self, self.context_entries(target, id), &invocation);
        if !items.is_empty() {
            shell.menu(at.0, at.1, items);
        }
    }

    /// Hands it the shell of a window it draws in: its layout's changes
    /// then go through it (the app's `on_change`, the other windows).
    pub fn attach(&self, shell: Shell) {
        let mut held = self.shell;
        if held.peek().is_none() {
            held.set(Some(shell));
        }
    }

    /// The shell attached ([`Self::attach`]), if any: reading it
    /// subscribes, so an effect runs again once one is.
    #[must_use]
    pub fn attached(&self) -> Option<Shell> {
        (self.shell)()
    }

    /// Changes the layout with `op`: through the shell when one is
    /// attached, else on the layout signal itself.
    pub fn apply<T>(
        &self,
        op: impl FnOnce(&mut Workspace) -> Result<T, app_shell::WorkspaceError>,
    ) -> Option<T> {
        if let Some(shell) = *self.shell.peek() {
            return shell.apply(op);
        }
        let mut layout = self.layout;
        let mut next = layout.peek().clone();
        match op(&mut next) {
            Ok(out) => {
                layout.set(next);
                Some(out)
            }
            Err(error) => {
                tracing::debug!(app_shell.op_error = %error, "layout operation refused");
                None
            }
        }
    }

    /// The layout the extensions' views open in.
    #[must_use]
    pub const fn layout(&self) -> Signal<Workspace> {
        self.layout
    }
}
