# app-shell

An app's shell for [Dioxus](https://dioxuslabs.com): a macOS-style top bar
with traffic lights, a sidebar, and one content area. When the app asks for
them, it also draws tabs, splits, docks, floating stacks, pop-out windows and
the tab switcher. It renders the same on **dioxus-web** and on
**dioxus-native** (Blitz).

**Tabs are opt-in.** By default a window is one content area: no tab bar, no
drop zones, no switcher. An app that wants tabs gives the shell a layout and
sets `tabs`.

The layout is plain data that your app owns. app-shell draws it and turns
pointer drags and keys into operations on it. It does not store the layout,
run a command framework, or theme anything beyond its own CSS variables.

It was extracted from Architect's `architect-workspace` crate, without
Architect, CRDTs, storage or the Action List.

## Crates

| crate | what it is | depends on |
|---|---|---|
| `app-shell` | the layout model (no features), and the Dioxus components (feature `ui`, on by default) | `serde`, `thiserror`; with `ui`: `dioxus`, `futures-channel`, `futures-util`, `tracing` |
| `app-shell-native` | pop-out OS windows and window decorations on dioxus-native | `app-shell`, `dioxus-native`, `blitz-shell`, `blitz-html` |

A browser build depends only on `app-shell`, so it never pulls in Blitz or wgpu.

## The layout

```text
Workspace
├── windows: Vec<Window>     the first is the main window; the rest are pop-outs
│   ├── center: Node         a tree of Split { axis, children: [(Node, share)] } ending in
│   │                        Group (a tab stack: tabs, current, chrome)
│   ├── docks: left / right / bottom, each a Node with a size and shown state
│   └── floats: stacks floating over the window (snap, pin, minimise)
├── panes: Vec<Pane>         every tab: id, view kind, state, title, pinned, history
├── active, recent, closed   focus, most recent first, and closed tabs to reopen
└── next_id, aliases         ids are never reused; ids of stacks that went still resolve
```

Every gesture and every key comes down to an operation: `open`, `move_pane`
(to a stack, beside a stack, a dock, a float or a new window), `close`,
`close_others`, `reopen`, `activate`, `cycle_tab`, `goto_tab`, `set_shares`,
`even`, `set_dock_shown`, `set_dock_size`, `pop_out_dock`, `dock_back`,
`close_window`, `to_main_window`, `set_zoomed`, `snap`, `pin`, `minimise`,
`navigate` and `go`. After each one the layout is **repaired** to its
invariants: every tab is in exactly one stack, empty stacks go, one-child
splits unwrap, shares are positive and sum to one, and so on (`repair.rs`).
A layout read back from storage goes through `Workspace::repaired()`, so a
layout written by an older or newer app, or edited by hand, still opens.

## The window: top bar, traffic lights, sidebar

- **`TopBar`**: one strip that is both the window's title bar and its toolbar,
  like a macOS unified toolbar. From the left it holds the traffic lights
  (when no sidebar holds them), the sidebar button, back and forward, and the
  title or the app's `lead` (a breadcrumb). The app's `search` sits in the
  centre and its `actions` at the right (`ToolbarButton`, `ToolbarGroup` for
  a view-mode switch, `SearchField`). Every empty part moves the window, and
  a double press zooms it. The shell draws a default one titled with its
  `title`; pass your own as `top_bar` to fill the slots.
- **Traffic lights** (`WindowLights`): close, minimise and zoom in red,
  yellow and green at the top-left, 12 pt, 8 pt apart, 14 pt in. Their glyphs
  show on hover, and they turn grey when the window isn't the key window.
  - **Linux and Windows:** the window has no system frame and the shell draws
    the lights.
  - **macOS:** the system's own lights stay, over a transparent, full-size
    title bar, placed in the top bar's 44 pt row. The shell only keeps their
    room clear.
  - **Browser:** no window buttons (the browser has its own), but the same
    top bar.
- **`Sidebar`**: a full-height panel at the left, a shade apart and
  translucent (`--as-sidebar-bg`, blurred where the renderer can). The
  traffic lights sit at its top. It has a `header` slot (`SidebarSwitcher`,
  a library or organisation switcher), sections with small uppercase
  headings that fold (`SidebarSection`), rows with an icon, a label, an
  optional count and a selected highlight (`SidebarItem`), and a `footer`
  slot.
  - **Desktop:** docked, resizable by its edge (double press resets the
    width) and collapsible (the top bar's button, Mod+Shift+L); `on_sidebar`
    hears each change so the app can keep it.
  - **Phone:** a drawer behind a menu button. Picking a row closes it.

## On a phone

Below 600 points wide (a browser at 390 px, a phone running the native
build) the shell takes the phone layout by itself. In a browser it follows
`onresize`; natively it follows the window's size from `app-shell-native`.

- **The foot.** `destinations` (the app's primary places, each a
  `Destination`: an icon over a label, with an optional badge) make a
  floating tab pill. `destination` lights the current one, and
  `on_destination` hears a tap. `action` (a `PrimaryAction`) is the round
  button beside it (Upload, New, Play). `accessory` docks over them: Now
  Playing, a transport, an upload's progress. On a desktop only the
  accessory shows, docked under the content, because the sidebar holds the
  destinations.
- **Safe areas.** The foot and the top bar keep clear of the notch and the
  home indicator with `env(safe-area-inset-*)`. Each is written after a
  plain value, so a renderer that can't read `env()` keeps the plain one.
  Blitz already keeps its viewport inside the safe area. A web app needs
  `viewport-fit=cover` in its viewport meta for the browser to report the
  insets.
- **The drawer.** The same `sidebar` that docks on a desktop is a drawer
  on a phone. It slides in under a finger from the left edge (or the top
  bar's menu button) and is dragged back shut. It settles by a flick or by
  where it is let go, and a tap on the scrim closes it. A vertical move
  from the edge stays the page's scroll.
- **Sheets.** `Shell::open_sheet(SheetSpec::new(|()| rsx! { … }))`,
  `use_sheet()` (sheets that close with the component that opened them),
  or the declarative `Sheet { open, render, on_close }`.
  - On a phone each is a bottom sheet with a handle. It can be dragged
    between detents (`Detent::Fit`, `Medium`, `Large`) or down and away,
    and its body scrolls. Sheets stack, the lower ones set back.
  - On a desktop the same sheet is a centred panel.
  - A sheet's content renders at the shell's root: context provided only
    where it was opened isn't there, so capture what it needs.
- **Menus** (`Shell::menu`) are popups on a desktop and action sheets on a
  phone.
- **The command palette.** The `palette` prop lists its entries
  (`PaletteItem`). Mod+K, `Command::OpenPalette` or `Shell::open_palette`
  opens it: a large sheet on a phone, near the top on a desktop. It ranks
  label prefixes, then word starts, then keywords.
- **Toasts.** `Shell::toast("Uploaded")` or `Shell::show_toast(ToastSpec)`
  (with an action such as Undo) queue one at a time: under the notch on a
  phone, at the foot on a desktop.
- **The top bar** is compact on a phone: the menu button, back, the title,
  and `phone_actions` (one or two) in place of `actions`. The search gets
  a row of its own.
- **Tabs**, when the app turns them on, keep the switcher on a phone.
- **Swipeable** pages its content: left and right (next and previous) and
  down (close), the content following the finger. It's what Quick Look
  needs.

The arithmetic behind all this (`app_shell::gestures`: the layout
threshold, the drawer's width, progress and settling, sheet detents,
swipe direction, palette ranking) is plain functions with tests.

## The seams your app owns

- **Storage.** You pass a `Signal<Workspace>` and an `on_change`. Save the
  value wherever you like: a file, a CRDT, a server. It is `serde`
  `Serialize`/`Deserialize`.
- **Keys and actions.** A `Command` enum (close tab, reopen, next/previous,
  go to tab *n*, split, focus in a direction, zoom, pin, move to a new or the
  main window, toggle a dock, open the switcher) and `Workspace::run(command)`.
  Keys are mapped by a `Keymap` you implement, or by `Bindings`
  (`"Mod+Shift+T" => ReopenClosed`), or by `DefaultKeymap` (browser-tab
  habits). `on_command` hears every command first and can take it.
- **Theme.** CSS custom properties (`--as-bg`, `--as-accent`, `--as-tab-h`,
  …) set on the shell's root from a `Theme` (`Theme::dark()`,
  `Theme::light()`, `.set(name, value)`). With `Theme::inherit()` they come
  from an ancestor you style.
- **Tabs.** `tabs: Tabs::Off` (the default) draws one content area: the
  shell's `children`, or the layout's active tab alone. `Tabs::Auto` draws
  one area while the layout is simple (`Workspace::is_simple`: one tab, no
  split, dock, float or pop-out) and the tab chrome once the app opens a
  second tab. `Tabs::On` always draws it. Without tabs, tab keys do nothing.
- **Tab content.** You register `TabKind`s (an id, a title, an icon and a
  render function) in a `TabKinds`. A pane names its kind. A kind that is not
  registered shows a placeholder and keeps its place. An extension system can
  `register` and `unregister` kinds through the same API.

## A minimal app

One content area, a top bar and nothing else:

```rust
use app_shell::ui::AppShell;
use dioxus::prelude::*;

fn app() -> Element {
    rsx! {
        div { style: "position: absolute; inset: 0; display: flex;",
            AppShell { title: "Notes",
                p { "Hello" }
            }
        }
    }
}

fn main() {
    dioxus::launch(app);
}
```

With a sidebar, a filled top bar, and tabs once a second one opens:

```rust
use app_shell::ui::{
    AppShell, SearchField, Sidebar, SidebarItem, SidebarSection, SidebarSwitcher, TabCtx, TabKind,
    TabKinds, Tabs, ToolbarButton, ToolbarGroup, TopBar, icons,
};
use app_shell::{Pane, Target, Workspace};
use dioxus::prelude::*;

fn kinds() -> TabKinds {
    TabKinds::new().with(TabKind::new("folder", "Folder", |ctx: TabCtx| rsx! {
        div { style: "padding: 16px;", "{ctx.pane.title}" }
    }))
}

fn app() -> Element {
    let layout = use_signal(|| {
        let mut ws = Workspace::new();
        let _ = ws.open(Pane::new("home", "folder", "Overview"), Target::Tab);
        ws
    });
    rsx! {
        div { style: "position: absolute; inset: 0; display: flex;",
            AppShell {
                layout,
                kinds: kinds(),
                tabs: Tabs::Auto,
                on_change: move |ws: Workspace| {
                    // Save it: serde_json::to_string(&ws), a CRDT, a server …
                    let _ = ws;
                },
                sidebar: rsx! {
                    Sidebar {
                        header: rsx! { SidebarSwitcher { title: "My Library", detail: "4 locations" } },
                        footer: rsx! { SidebarItem { label: "Settings", icon: icons::SETTINGS } },
                        SidebarSection { title: "Library",
                            SidebarItem { label: "Overview", icon: icons::HOME, selected: true }
                            SidebarItem { label: "Recents", icon: icons::LAYERS, badge: "24" }
                        }
                    }
                },
                top_bar: rsx! {
                    TopBar {
                        title: "Overview",
                        search: rsx! { SearchField {} },
                        actions: rsx! {
                            ToolbarGroup {
                                ToolbarButton { icon: icons::GRID, label: "Grid", active: true, on_press: |_| {} }
                                ToolbarButton { icon: icons::LIST, label: "List", on_press: |_| {} }
                            }
                        },
                        on_back: |_| {},
                        on_forward: |_| {},
                    }
                },
            }
        }
    }
}
```

Natively, `app_shell_native::launch(main, popout, Config::default())` takes
your main component and a pop-out component. Each pop-out OS window renders
`AppShell { layout: use_window_layout(), window: id, tabs: Tabs::On, … }`,
and all windows stay in sync through a `LayoutHub`. Without system
decorations (the default), the shell's top bar is the window's frame. It
reaches close, minimise, zoom, drag and the window's focus through
`WindowControls`.

Components inside the shell reach the layout through `use_shell()`:
`shell.open(pane, target)`, `shell.command(cmd)`,
`shell.menu(x, y, items)`, `shell.apply(|ws| ws.move_pane(…))`.

## How it renders on both

The components follow Blitz's rules, so the same tree lays out in a browser
and in Blitz:

- inline styles only, reading the theme's variables; the small hover sheet is
  additive
- no `position: fixed` or `sticky`: overlays (drop zones, switcher, popups)
  mount at the shell's root with `position: absolute`
- no `overflow: auto`: `overflow: hidden`, or `overflow-y: scroll` where a
  list scrolls
- every flex child that must shrink has `min-width: 0` or `min-height: 0`
- pointer events for every drag (no HTML drag and drop)
- nothing measured synchronously inside an event handler (a split's length
  is measured in a spawned task)

Drop targets are elements of their own (a stack's middle and its four side
bands, each tab's halves, the window's edges, a "New window" target). The
element under the pointer decides where the tab lands, so the drag needs no
geometry measured.

A browser has no OS windows: there the main window's shell draws pop-outs in
the page, as frames you can move and close (their tabs go back).

## Dioxus version

app-shell uses the **published** `dioxus = "=0.8.0-alpha.3"` (and the
matching `dioxus-native` and `blitz-shell` `0.3.0-beta.4`), so a crate on
crates.io's dioxus can depend on it (FastTrackStudios/drive does).
**Architect pins dioxus by git rev `430cfb56`**. To adopt app-shell it would
have to move to the published alpha, patching in its forks' fixes; see
[docs/migration.md](docs/migration.md).

An app built for the web with `dioxus` and `default-features = false` hits
missing `web-sys` features in `dioxus-web` 0.8.0-alpha.3. Keep dioxus's
default features in the app crate.

## Developing

```bash
devenv shell -- cargo test --workspace            # model, commands, repair, UI (dioxus-ssr), gestures
devenv shell -- cargo clippy --workspace --all-targets
devenv shell -- cargo check -p app-shell --no-default-features   # the model alone
```

The devenv environment pins Rust 1.99.1 and includes the libraries
app-shell-native needs on Linux (fontconfig, wayland, xkbcommon, vulkan).

## What it is not

- Not storage. No files, CRDTs, sync or screensets: keep the `Workspace`
  where you like, and keep several of them yourself if you want named
  layouts.
- Not a command framework. No palette, Action List or menu bar: map your
  commands onto `Command` and call `shell.command`.
- Not a design system. Only the shell's own chrome, themed by CSS variables.
- Not an extension host. `TabKinds` is the API an extension host would call.

## Licence

MIT OR Apache-2.0, as the Architect code it comes from (see `NOTICE`). Icons
are Lucide's path data (ISC).
