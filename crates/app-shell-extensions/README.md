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
fillers (`use_extension_bar`). `ExtensionOverlays` goes in `AppShell`'s `layers`: drawn in
the shell's context from the first render, it hands the extensions the
shell (their keys, sheets and palette views) before any tab draws. The
phone's Typing Toolbar is `TypingBar`,
which the app places over the keyboard. `tests/panes.rs` is the whole
path, end to end.
`install()` (called by `interface::use_extension_ui`) hands the runtime
what the window does for it: process views' `set_view_state`, `EXT_CHECK`'s
frame renderer, what goes when an extension's session ends.

## The settings page

`settings_sheet` is the settings extension (`SettingsSheet`, manifest
`extensions/settings.toml`), moved from Architect's
`architect-settings-sheet`: ⌘, (`APP_SETTINGS`, an app id the app allows
it) opens its window, a sheet over the app, its sections what the app and
every extension contribute to `settings.sections`, drawn by
`SettingsPage` over the window's `Settings` (an extension's own section by
its view, `SectionViews`). Link it in (`Extensions::with_linked`), or run
it as a process: `app-shell-settings`, feature `guest`.

## The built-in extensions moved from Architect

Members of this crate's workspace (one lockfile, one target, the same
patches): `theme-picker` (`app-shell-theme-picker`, id `theme.picker`:
⌘K's "Choose a theme…" and "Light or dark", ⌘O's `theme:`; it reaches
the theme store only through the host service, so it needs no UI of its
own), linked in or run as its process.

## Why a workspace of its own

Architect pins dioxus by git rev (`430cfb56`); app-shell is on the
published `0.8.0-alpha.3`. Everything this crate links must build against
one dioxus, so it is excluded from the root workspace and carries its own
`[patch]` tables: Architect's (a git dependency's patches do not reach its
users) and crates.io's `dioxus` handed the same git rev, as Studio does.
app-shell itself stays on the published alpha.

```bash
cd crates/app-shell-extensions && cargo check --workspace --all-features && cargo nextest run --workspace --all-features
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

