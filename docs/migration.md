# Moving architect-workspace and Studio onto app-shell

app-shell is the layout half of `architect-workspace`: the model, its
operations and repair, drag and drop, tab bars, splits, docks, floats,
pop-outs, the switcher, popups and window decorations. It has no Architect,
CRDT, storage or Action List inside. This note says how Architect's workspace
and Studio's views would move onto it, in order, and what can go wrong.
Nothing here has been done yet: Studio still mounts into
`architect-workspace`.

## What moved, what did not

| architect-workspace | app-shell |
|---|---|
| `model.rs`, `ops.rs`, `repair.rs`, `layout.rs`, `drop.rs`, `switcher.rs` (the model) | ported almost line for line: `model.rs`, `ops.rs`, `repair.rs`, `geometry.rs`, `drop.rs`, `switcher.rs`. `facet` became `serde`. |
| `Ribbon` (a dock's views and pinned Action List commands) | dropped: a dock is just its tree. An app draws a ribbon as a tab kind, or beside the shell. |
| `Workspace::to_json` / `from_json` (facet JSON) | `serde` derives plus `Workspace::repaired()`. The JSON shape differs from facet's (see the risks below). |
| `store.rs`, `home.rs`, `screenset.rs`, `deck.rs` (storage, interface files, screensets, scope tabs) | not ported. The app owns storage (`Signal<Workspace>` + `on_change`). Screensets and decks are app features built on several `Workspace` values. |
| `control/*` (every operation as an Action List action), `host/commands.rs`, `host/actions.rs` | `Command` + `Workspace::run` + `Keymap`/`Bindings`/`DefaultKeymap`, and `on_command`. An Action List adapter maps its actions onto `Command`, or calls `Workspace` operations directly. |
| `kinds.rs`, `ctx.rs`, `paint/*`, `pane_space.rs`, `presence.rs`, `meter.rs`, `frames.rs`, `pacing.rs`, `shot.rs` (view kinds, the `ViewCtx` contract, paint views, cursors, frame meters, `APP_SHOT`) | not ported. These are the **view** contract, not the shell. They stay in Architect (or a crate of their own) and plug in as `TabKind`s. |
| `host/*` (`WorkspaceView`, `Views`, tab bars, docks, overlays, the switcher, ~11k lines) | rewritten, smaller: `ui::AppShell`, `StackView`, `TabBar`, `NodeView`, `WindowBody`, `SoloView`, `Switcher`, `PopupLayer`, `TopBar`. Drop zones are elements rather than measured rectangles. |
| `native.rs` (pop-out OS windows, frame meters, `APP_FRAMES` driving) | `app-shell-native::launch`: pop-outs and decorations only. No meters, no input driving, no dma-buf renderer option. |
| `architect-theme` | CSS variables (`--as-*`) from a `Theme`. |
| `architect-shell` (bars, page, keyboard, favourites, motion) | `TopBar` (the title row and toolbar in one strip), `WindowLights`, `Sidebar` and its rows, the phone drawer, `Switcher`. The rest (favourites, the keyboard inset, status, motion) stays in Architect's shell. |
| `architect-shell`'s `WindowChrome` / `WindowControls` / `fit_traffic_lights` (macOS lights in a 40 pt row, Mac-style lights drawn elsewhere) | `WindowControls` (close, minimise, zoom, drag, `focused`, `system_lights`), `WindowLights`, and `app-shell-native`'s `mac::fit_traffic_lights` placing the system's lights in the 44 pt top bar (`TOP_BAR_H`). Same colours, sizes and inset as Architect's. |

## Step by step

1. **Move Architect to the published dioxus alpha.** Architect pins dioxus by
   git rev `430cfb56`. app-shell is built on crates.io `0.8.0-alpha.3`. Two
   copies of dioxus in one build do not share types, so this comes first.
   Patch the forks' fixes in as described under *The forks* below.
2. **Depend on app-shell beside architect-workspace.** Add the crates as git
   dependencies (pinned by tag) and build nothing on them yet. Check that the
   lockfile holds one dioxus.
3. **Convert stored layouts.** Write a one-time converter from facet JSON
   (`architect_workspace::Workspace::to_json`) to app-shell's serde JSON,
   field by field: the types match one to one except `Dock::ribbon`. Run it
   where Architect reads a layout (the vault's interface files, `home.rs`),
   and keep the old file beside the new one until the switch has shipped.
4. **Make the view contract a tab kind.** Write one adapter: an app-shell
   `TabKind` per Architect view kind whose render function builds the view's
   `ViewCtx` (doc, history, theme tokens from the `--as-*` variables or the
   Architect theme) and mounts the view as `WorkspaceView` does today. Paint
   views (`PaintPane`) and process views mount the same way, inside
   `TabBody`. Studio's extensions (`ViewCtx`, `paint`, `ViewHost`,
   `Placement`, `KindId`, `Geometry`, `FrameClock`, `ViewInput`, in 14 files)
   keep compiling unchanged, because they speak the view contract, not the
   layout.
5. **Route commands.** Map Architect's workspace actions (`WORKSPACE_*`,
   `SCOPE_*`) onto `Command` or straight onto `Workspace` operations, in an
   `on_command` / Action List adapter. Keys move from `WindowKeys` to a
   `Bindings` table, or to an Architect `Keymap` implementation that reads the
   Action List's bindings.
6. **Swap the host.** Replace `WorkspaceView` with `AppShell { tabs: Tabs::On, … }`
   in Architect's app root (tabs are off by default), behind a feature flag, with the layout in a `Signal<Workspace>`
   and `on_change` writing it where `WorkspaceStore` wrote it. Natively,
   replace `architect_workspace::native::launch` with
   `app_shell_native::launch`, keeping Architect's own launcher bits (fonts,
   the `RESTORE_WAYLAND_DISPLAY` dance, `MainWindow::on_close`, frame meters)
   in a wrapper, or adding them to app-shell-native (issues filed).
7. **Port what Studio still needs.** Screensets (⌘1–9 layouts per form
   class), scope tabs (decks), presence cursors and `APP_SHOT` either stay as
   Architect features layered on app-shell or become app-shell issues. Decide
   for each before removing the old host.
8. **Delete the old host.** When Studio's desktop and web apps run on
   `AppShell` and their smoke screenshots at desktop width and 390 px match,
   remove `architect-workspace`'s `host/`, `native.rs`, `bar.rs` and the
   ported model. Keep the view contract (`ctx`, `kinds`, `paint`).

## The forks

FastTrackStudios carries forks with real fixes (FastTrackStudios/vendor, tag
v0.7.1, plus a `dioxus-native-dom` rev). app-shell does not depend on them
and does not rely on behaviour only they have. A consumer that wants the
fixes patches the published crates with the forks **rebased onto the
published 0.8.0-alpha.3**:

```toml
[patch.crates-io]
dioxus-native     = { git = "https://github.com/FastTrackStudios/vendor", tag = "<rebased tag>" }
dioxus-native-dom = { git = "https://github.com/FastTrackStudios/vendor", tag = "<rebased tag>" }
dioxus-core       = { git = "https://github.com/FastTrackStudios/vendor", tag = "<rebased tag>" }
```

A `[patch]` cannot rename a crate, so each fork must keep its crates.io name
and version `0.8.0-alpha.3`.

What app-shell's native side would hit without each fix:

| fork | fix | where app-shell meets it without the fix |
|---|---|---|
| dioxus-native | the renderer pick (Vello vs hybrid vs CPU per platform) | app-shell-native turns on `vello` (GPU compute). A machine whose GPU cannot run it gets no window; the fork falls back. |
| dioxus-native | iOS simulator on vello-hybrid | no simulator build. app-shell has no iOS target yet. |
| dioxus-native | winit beta.4 to match Blitz | the published alpha uses winit 0.31.0-beta.3. app-shell-native builds against it; mixing it with a crate on beta.4 gives two winits and type mismatches. |
| dioxus-native-dom | `text_input_focused` | a keyboard shortcut typed into a text field in a tab can reach the shell's `onkeydown` (Mod+W closes the tab while typing). app-shell only reacts to modifier chords, but a kind with its own single-key bindings needs this. |
| dioxus-native-dom | focus and measurements that never panic on a busy document | `MountedData::get_client_rect` borrows the document. app-shell calls it only from spawned tasks (the shell's width at mount, a split's length when a divider is pressed), never synchronously in a handler. A tab kind that measures inside a handler would panic. With the fork that is harmless. |
| dioxus-core (dioxus-vdom) | a dropped handler's owner is not a panic | closing a tab from its own × or a menu row unmounts the component whose handler is running. The published core can panic when that handler's owner scope is gone. app-shell defers nothing yet; see the issues. |
| dioxus-core | DioxusLabs/dioxus#5668, the suspense fix | a tab kind that suspends (an async resource) inside `TabBody` can hit it. app-shell itself does not suspend. |

## Risks

- **Two dioxus versions.** Until Architect moves to the published alpha,
  app-shell and Architect's crates cannot share a build. Step 1 blocks
  everything else.
- **Stored layouts.** facet JSON and serde JSON differ in how they encode
  enums (`Node`, `Snap`) and newtypes. An unconverted layout fails to read
  and the user gets a fresh one: their arrangement is lost. Convert it, and
  keep the old file.
- **Measurement on native.** The published dioxus-native-dom panics when a
  handler measures synchronously. Every new measurement must go through
  `spawn`.
- **Styles patched property by property.** dioxus-web patches a changed
  `style` string property by property, so a property you drop from the string
  lingers. Write every geometry property each time (app-shell does in its
  drop highlights and floats).
- **Multi-window signals.** Each OS window is its own `VirtualDom`. The
  layout is synced by value through `LayoutHub`, not shared as a signal. A
  tab kind that reads another window's signals sees values but no
  re-renders. Architect's pop-outs read the main window's
  `Signal<Workspace>` directly today. Code relying on that must move to the
  hub, or to its own state per window.
- **Features not ported.** Screensets, scope tabs, presence cursors, frame
  meters, `APP_SHOT`, dock ribbons, stacked (sliding) tabs drawn as a strip
  (the model's `stacked` flag and `slides()` geometry are there, the drawing
  is not), minimised-float chips and cross-window tab dragging. Studio uses
  some of these. Each is an issue on this repo or stays in Architect.
- **The title row's height.** Architect's Mac row is 40 pt; app-shell's top
  bar is 44 pt (`--as-topbar-h`, `TOP_BAR_H`). An app that changes the CSS
  variable must change the constant the native side places macOS's lights
  by, or the lights sit off centre.
- **macOS is untested here.** The macOS path (transparent title bar,
  `fit_traffic_lights`) is ported from Architect but was built and run only
  on Linux: there is no macOS target on the dev machine. Check it on a Mac
  before relying on it.
- **Touch.** A browser gives a touch pointer implicit capture, so a tab drag
  started with a finger does not reach the drop zones. Phones use the
  switcher; tablets need a fix (issue filed).
