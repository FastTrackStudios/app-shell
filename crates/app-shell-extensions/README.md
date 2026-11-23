# app-shell-extensions

Architect's extensions in an app-shell window: what loaded extensions add
to the shell (tabs, toolbars, menus, status items), their windows and
sheets, their views in the shell's panes, and the app's interface for them
(notify, prompt, menu, clipboard, pickers).

The extension runtime is Architect's `architect-extensions` (it draws
nothing); `architect-extensions-dioxus` is what an extension whose views
are Dioxus components links. This crate is the chrome around them, moved
here as it was from `architect-extensions`' `ui`, `page`, `pickers` and
`remote` features (FastTrackStudios/architect, branch
`strip/application-framework`): what loaded extensions add to the window,
their windows and sheets, the app's interface for them, process and WASI
views as remote Blitz documents (`remote`, the default), the extensions
page (`page`) and the platform's pickers (`pickers`).

Extensions' views are app-shell tab kinds (`ExtensionUi::tab_kinds`), drawn
in the shell's panes; app-shell's `TabHooks` put each pane's
`architect_views::PaneHostRef` around its content (what a view's
`use_pane_state` and `use_view_state` read), the pane header's toolbars
above it, the extensions' context rows in a tab's menu and their badges
after its title. Their keys (`[[keys]]`, and ⌘Z / ⇧⌘Z as `DOC_UNDO` /
`DOC_REDO` in a view an opener shows a file in) are the shell's bound keys
(`keys`); a view an extension draws a palette overlay with
(`[[palette_views]]`) is the shell's palette view for it, and
`COMMANDS_SHOW` opens one (`palette`). Their tabs are the app's destinations, their status items
and bar buttons the top bar's items and the `title` and `tabs` slots its
fillers (`use_extension_bar`). The phone's Typing Toolbar is `TypingBar`,
which the app places over the keyboard. `tests/panes.rs` is the whole
path, end to end.
`install()` (called by `interface::use_extension_ui`) hands the runtime
what the window does for it: process views' `set_view_state`, `EXT_CHECK`'s
frame renderer, what goes when an extension's session ends.

## Why a workspace of its own

Architect pins dioxus by git rev (`430cfb56`); app-shell is on the
published `0.8.0-alpha.3`. Everything this crate links must build against
one dioxus, so it is excluded from the root workspace and carries its own
`[patch]` tables: Architect's (a git dependency's patches do not reach its
users) and crates.io's `dioxus` handed the same git rev, as Studio does.
app-shell itself stays on the published alpha.

```bash
cd crates/app-shell-extensions && cargo check --all-features && cargo nextest run
```

## What it still takes from Architect

The runtime (`architect`, `architect-extensions`,
`architect-extensions-dioxus`, `architect-scene`), the settings model
(`architect-settings`, no UI) and the views' contract (`architect-views`:
`PaneHostRef`, `Meters`). Not Architect's shell, workspace, commands or UI
kit: the palette is app-shell's (`app_shell::palette`, `PaletteSources`),
the panes, windows and sheets app-shell's, the remote views' frame pacing
and field focus here (`pacing`, `field_focus`), the extensions page's
controls here (`kit`). With feature `page`, `architect-extensions/manager`
still brings `architect-workspace` (its layouts kind reads
`architect_workspace::home`).

