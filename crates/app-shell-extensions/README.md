# app-shell-extensions

Architect's extensions in an app-shell window: what loaded extensions add
to the shell (tabs, toolbars, menus, status items), their windows and
sheets, their views in the shell's panes, and the app's interface for them
(notify, prompt, menu, clipboard, pickers).

The extension runtime is Architect's `architect-extensions` (it draws
nothing); `architect-extensions-dioxus` is what an extension whose views
are Dioxus components links. This crate is the chrome around them, moving
here from `architect-extensions`' `ui` feature
(FastTrackStudios/architect, branch `strip/application-framework`).

## Why a workspace of its own

Architect pins dioxus by git rev (`430cfb56`); app-shell is on the
published `0.8.0-alpha.3`. Everything this crate links must build against
one dioxus, so it is excluded from the root workspace and carries its own
`[patch]` tables: Architect's (a git dependency's patches do not reach its
users) and crates.io's `dioxus` handed the same git rev, as Studio does.
app-shell itself stays on the published alpha.

```bash
cd crates/app-shell-extensions && cargo check
```
