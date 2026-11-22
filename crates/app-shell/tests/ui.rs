//! The components, rendered to HTML: what each window draws for a
//! layout, and what the shell's operations do to it.

#![cfg(feature = "ui")]
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::cell::RefCell;

use app_shell::ui::{
    AppShell, FormFactor, SHELL_CSS, ScrollArea, SearchField, Sidebar, SidebarItem, SidebarMode,
    SidebarSection, SidebarState, SidebarSwitcher, TabCtx, TabKind, TabKinds, Tabs, ToolbarButton,
    ToolbarGroup, TopBar, WindowControls, icons, use_shell, use_sidebar_mode,
};
use app_shell::{Command, Pane, PaneId, Rect, Side, Target, WindowId, Workspace};
use dioxus::prelude::*;

thread_local! {
    /// What `on_change` heard.
    static CHANGES: RefCell<Vec<Workspace>> = const { RefCell::new(Vec::new()) };
    /// A command the "act" kind runs as it mounts.
    static ACT: RefCell<Option<Command>> = const { RefCell::new(None) };
}

fn kinds() -> TabKinds {
    TabKinds::new()
        .with(TabKind::new("note", "Note", |ctx: TabCtx| {
            rsx! { p { class: "note-body", "note {ctx.pane.title}" } }
        }))
        .with(TabKind::new("act", "Act", |_ctx: TabCtx| {
            let shell = use_shell();
            use_hook(move || {
                if let Some(cmd) = ACT.with(|a| a.borrow_mut().take()) {
                    shell.command(cmd);
                }
            });
            rsx! { p { "acting" } }
        }))
}

#[derive(Props, Clone, PartialEq)]
struct Lab {
    layout: Workspace,
    form: FormFactor,
    #[props(default = WindowId::main())]
    window: WindowId,
}

#[allow(non_snake_case)]
fn Harness(props: Lab) -> Element {
    let layout = use_signal(|| props.layout.clone());
    rsx! {
        AppShell {
            layout,
            kinds: kinds(),
            tabs: Tabs::On,
            form: props.form,
            window: props.window.clone(),
            on_change: move |w: Workspace| CHANGES.with(|c| c.borrow_mut().push(w)),
            on_new_tab: move |_| {},
        }
    }
}

fn render_lab(lab: Lab) -> String {
    let mut dom = VirtualDom::new_with_props(Harness, lab);
    dom.rebuild_in_place();
    for _ in 0..4 {
        dom.process_events();
        dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
    }
    dioxus_ssr::render(&dom)
}

fn render(layout: Workspace) -> String {
    render_lab(Lab {
        layout,
        form: FormFactor::Desktop,
        window: WindowId::main(),
    })
}

fn note(id: &str) -> Pane {
    Pane::new(id, "note", format!("Note {id}"))
}

fn count(html: &str, needle: &str) -> usize {
    html.matches(needle).count()
}

#[test]
fn a_stack_draws_its_tabs_and_only_its_current_tabs_content() {
    let mut ws = Workspace::new();
    for p in ["a", "b", "c"] {
        ws.open(note(p), Target::Tab).unwrap();
    }
    ws.activate(&PaneId::new("b")).unwrap();
    let html = render(ws);
    assert_eq!(count(&html, "class=\"as-tab as-hov\""), 3);
    assert!(html.contains("Note a") && html.contains("Note c"));
    assert!(html.contains("note Note b"), "the current tab's body");
    assert!(!html.contains("note Note a"), "only the current one");
    assert!(html.contains("data-pane=\"b\" data-current=\"true\""));
    assert!(html.contains("--as-bg: #18181b;"), "the theme's variables");
}

#[test]
fn a_split_draws_each_side_by_its_share_with_a_divider_between() {
    let mut ws = Workspace::new();
    ws.open(note("a"), Target::Tab).unwrap();
    ws.open(note("b"), Target::Split(Side::Right)).unwrap();
    ws.open(note("c"), Target::Split(Side::Bottom)).unwrap();
    let html = render(ws);
    assert_eq!(count(&html, "class=\"as-stack\""), 3);
    assert!(html.contains("data-axis=\"row\""));
    assert!(html.contains("data-axis=\"column\""));
    assert_eq!(count(&html, "class=\"as-divider\""), 2);
    assert!(html.contains("flex: 0.5 1 0px"));
}

#[test]
fn a_tab_of_an_unknown_kind_keeps_its_place_with_a_placeholder() {
    let mut ws = Workspace::new();
    ws.open(Pane::new("x", "gone.kind", "Old"), Target::Tab)
        .unwrap();
    let html = render(ws);
    assert!(html.contains("Old"));
    assert!(html.contains("Nothing draws “gone.kind” tabs here."));
}

#[test]
fn an_empty_layout_offers_a_new_tab() {
    let html = render(Workspace::new());
    assert!(html.contains("No tabs open"));
    assert!(html.contains("New tab"));
}

#[test]
fn docks_draw_at_their_size_and_a_hidden_one_does_not() {
    let mut ws = Workspace::new();
    ws.open(note("doc"), Target::Tab).unwrap();
    ws.open(note("files"), Target::Dock(Side::Left)).unwrap();
    ws.set_dock_size(Side::Left, 250.0).unwrap();
    let html = render(ws.clone());
    assert!(html.contains("data-dock=\"left\""));
    assert!(html.contains("width: 250px;"));
    ws.set_dock_shown(Side::Left, app_shell::Shown::Hidden)
        .unwrap();
    assert!(!render(ws).contains("data-dock=\"left\""));
}

#[test]
fn a_float_draws_over_the_window_where_it_was_put() {
    let mut ws = Workspace::new();
    ws.open(note("doc"), Target::Tab).unwrap();
    ws.open(
        note("f"),
        Target::Float(Rect::new(40.0, 50.0, 300.0, 200.0)),
    )
    .unwrap();
    let html = render(ws);
    assert!(html.contains("class=\"as-float\""));
    assert!(html.contains(
        "left: 40px; right: auto; top: 50px; bottom: auto; width: 300px; height: 200px;"
    ));
}

#[test]
fn a_pop_out_is_drawn_in_the_page_where_there_are_no_os_windows() {
    let mut ws = Workspace::new();
    ws.open(note("a"), Target::Tab).unwrap();
    ws.open(note("b"), Target::Tab).unwrap();
    ws.move_pane(&PaneId::new("b"), app_shell::Dest::Window(None))
        .unwrap();
    let html = render(ws.clone());
    assert_eq!(count(&html, "class=\"as-inpage-window\""), 1);
    assert!(html.contains("note Note b"));
    // The pop-out's own shell (an OS window's) draws only its window.
    let id = ws.windows[1].id.clone();
    let html = render_lab(Lab {
        layout: ws,
        form: FormFactor::Desktop,
        window: id,
    });
    assert!(html.contains("note Note b") && !html.contains("note Note a"));
    assert!(!html.contains("as-inpage-window"));
}

#[test]
fn a_phone_shows_the_active_tab_alone_and_counts_the_tabs() {
    let mut ws = Workspace::new();
    ws.open(note("a"), Target::Tab).unwrap();
    ws.open(note("b"), Target::Split(Side::Right)).unwrap();
    let html = render_lab(Lab {
        layout: ws,
        form: FormFactor::Phone,
        window: WindowId::main(),
    });
    assert!(html.contains("data-form=\"phone\""));
    assert!(html.contains("note Note b") && !html.contains("note Note a"));
    assert!(!html.contains("as-tabbar"), "no tab bars on a phone");
    assert!(html.contains("class=\"as-hov as-tabs-button\""));
    assert!(html.contains(">2</div>"), "the count");
    assert!(
        html.contains("Note b</span>"),
        "the bar titled with the tab on show"
    );
}

fn acting(cmd: Command, tabs: &[&str]) -> (Workspace, String) {
    let mut ws = Workspace::new();
    for p in tabs {
        ws.open(note(p), Target::Tab).unwrap();
    }
    ws.open(Pane::new("act", "act", "Act"), Target::Tab)
        .unwrap();
    ACT.with(|a| *a.borrow_mut() = Some(cmd));
    CHANGES.with(|c| c.borrow_mut().clear());
    (ws.clone(), render(ws))
}

#[test]
fn a_command_from_inside_changes_the_layout_and_tells_the_app() {
    let (_, html) = acting(Command::Split(Side::Right), &["a"]);
    assert!(html.contains("data-axis=\"row\""), "split now");
    let changes = CHANGES.with(|c| c.borrow().clone());
    assert_eq!(changes.len(), 1);
    assert_eq!(
        changes[0].windows[0].center.as_ref().map(|n| {
            let mut g = Vec::new();
            n.groups(&mut g);
            g.len()
        }),
        Some(2)
    );
}

#[test]
fn the_switcher_shows_every_tab_of_the_place_as_a_card() {
    let (_, html) = acting(Command::OpenSwitcher, &["a", "b"]);
    assert!(html.contains("class=\"as-switcher\""));
    assert_eq!(count(&html, "class=\"as-card\""), 3);
    assert!(html.contains("3 tabs"));
    assert!(
        CHANGES.with(|c| c.borrow().is_empty()),
        "the layout is as it was"
    );
}

#[test]
fn moving_a_tab_to_a_new_window_draws_it_there() {
    let (_, html) = acting(Command::MoveToNewWindow, &["a"]);
    assert_eq!(count(&html, "class=\"as-inpage-window\""), 1);
}

// Tabs are opt-in, the chrome and the sidebar.

fn render_app(app: fn() -> Element) -> String {
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    for _ in 0..4 {
        dom.process_events();
        dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
    }
    dioxus_ssr::render(&dom)
}

#[test]
fn by_default_the_shell_is_one_content_area_with_nothing_about_tabs() {
    let html = render_app(|| {
        rsx! {
            AppShell { title: "Notes", form: FormFactor::Desktop,
                p { class: "page", "hello" }
            }
        }
    });
    assert!(html.contains("<p class=\"page\">hello</p>"));
    assert!(html.contains("class=\"as-topbar-title\""));
    assert!(html.contains(">Notes</span>"));
    assert!(html.contains("data-tabs=\"false\""));
    for tabby in [
        "as-tabbar",
        "as-stack",
        "as-tab ",
        "as-switcher-button",
        "as-drop-zones",
        "as-tabs-button",
    ] {
        assert!(!html.contains(tabby), "{tabby} drawn without tabs");
    }
    assert!(
        !html.contains("as-lights"),
        "no window buttons in a browser"
    );
    assert!(!html.contains("data-sidebar"), "no sidebar unless given");
}

#[test]
fn tabs_off_shows_the_layouts_active_tab_alone() {
    let mut ws = Workspace::new();
    for p in ["a", "b", "c"] {
        ws.open(note(p), Target::Tab).unwrap();
    }
    ws.activate(&PaneId::new("b")).unwrap();
    #[component]
    fn Off(start: Workspace) -> Element {
        let layout = use_signal(|| start.clone());
        rsx! { AppShell { layout, kinds: kinds(), form: FormFactor::Desktop } }
    }
    let mut dom = VirtualDom::new_with_props(Off, OffProps { start: ws });
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    assert!(html.contains("note Note b"));
    assert!(!html.contains("note Note a") && !html.contains("as-tabbar"));
}

#[component]
fn AutoShell(start: Workspace) -> Element {
    let layout = use_signal(|| start.clone());
    rsx! { AppShell { layout, kinds: kinds(), tabs: Tabs::Auto, form: FormFactor::Desktop } }
}

fn render_auto(ws: Workspace) -> String {
    let mut dom = VirtualDom::new_with_props(AutoShell, AutoShellProps { start: ws });
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn tabs_auto_appear_with_a_second_tab_and_not_before() {
    let mut ws = Workspace::new();
    ws.open(note("a"), Target::Tab).unwrap();
    let one = render_auto(ws.clone());
    assert!(one.contains("note Note a") && !one.contains("as-tabbar"));
    assert!(one.contains("data-tabs=\"false\""));
    ws.open(note("b"), Target::Tab).unwrap();
    let two = render_auto(ws);
    assert!(two.contains("as-tabbar") && two.contains("data-tabs=\"true\""));
    assert!(two.contains("as-switcher-button"));
}

fn controls(focused: bool, system: Option<f32>) -> WindowControls {
    WindowControls {
        minimize: Some(Callback::new(|()| {})),
        toggle_maximize: Some(Callback::new(|()| {})),
        close: Some(Callback::new(|()| {})),
        drag: Some(Callback::new(|()| {})),
        focused: Some(Signal::new(focused)),
        system_lights: system,
    }
}

#[derive(Props, Clone, PartialEq)]
struct Framed {
    focused: bool,
    system: Option<f32>,
    with_sidebar: bool,
}

#[allow(non_snake_case)]
fn FramedShell(props: Framed) -> Element {
    let c = use_hook(|| controls(props.focused, props.system));
    use_context_provider(|| c);
    let sidebar = props.with_sidebar.then(|| {
        rsx! {
            Sidebar {
                header: rsx! { SidebarSwitcher { title: "Library", detail: "3 locations" } },
                footer: rsx! { SidebarItem { label: "Settings", icon: icons::SETTINGS } },
                SidebarSection { title: "Library",
                    SidebarItem { label: "Overview", icon: icons::HOME, selected: true }
                    SidebarItem { label: "Recents", icon: icons::FOLDER, badge: "12" }
                }
                SidebarSection { title: "Tags", open: false,
                    SidebarItem { label: "Hidden tag" }
                }
            }
        }
    });
    rsx! {
        AppShell { title: "Files", form: FormFactor::Desktop, sidebar,
            p { "body" }
        }
    }
}

fn render_framed(focused: bool, system: Option<f32>, with_sidebar: bool) -> String {
    let mut dom = VirtualDom::new_with_props(
        FramedShell,
        Framed {
            focused,
            system,
            with_sidebar,
        },
    );
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn a_framed_window_draws_traffic_lights_grey_when_not_focused() {
    let html = render_framed(true, None, false);
    assert!(html.contains("class=\"as-lights\""));
    for b in ["Close", "Minimise", "Zoom"] {
        assert!(html.contains(&format!("aria-label=\"{b}\"")), "{b}");
    }
    assert!(html.contains("var(--as-light-close)") && html.contains("data-focused=\"true\""));
    let idle = render_framed(false, None, false);
    assert!(idle.contains("data-focused=\"false\"") && !idle.contains("var(--as-light-close)"));
    assert!(idle.contains("var(--as-light-off)"));
}

#[test]
fn where_the_system_draws_the_lights_the_shell_keeps_their_room() {
    let html = render_framed(true, Some(76.0), false);
    assert!(html.contains("class=\"as-lights-room\""));
    assert!(html.contains("width: 76px;"));
    assert!(!html.contains("class=\"as-lights\""));
}

#[test]
fn a_sidebar_docks_at_the_left_with_the_lights_at_its_top() {
    let html = render_framed(true, None, true);
    assert!(html.contains("data-sidebar=\"docked\""));
    assert!(html.contains("width: 220px;"));
    let sidebar_at = html.find("data-sidebar").unwrap();
    let lights_at = html.find("class=\"as-lights\"").unwrap();
    let bar_at = html.find("class=\"as-topbar\"").unwrap();
    assert!(
        sidebar_at < lights_at && lights_at < bar_at,
        "the lights in the sidebar, not the bar"
    );
    assert_eq!(html.matches("class=\"as-lights\"").count(), 1);
    assert!(html.contains("text-transform: uppercase") && html.contains(">Library</span>"));
    assert!(html.contains("aria-current=\"page\"") && html.contains(">Overview</span>"));
    assert!(html.contains(">12</span>"), "the badge");
    assert!(
        !html.contains("Hidden tag"),
        "a folded section hides its rows"
    );
    assert!(html.contains("3 locations") && html.contains("class=\"as-sidebar-footer\""));
    assert!(
        html.contains("aria-label=\"Sidebar\""),
        "the button that hides it"
    );
}

#[derive(Props, Clone, PartialEq)]
struct Railed {
    rail: bool,
}

#[allow(non_snake_case)]
fn RailShell(props: Railed) -> Element {
    let c = use_hook(|| controls(true, None));
    use_context_provider(|| c);
    rsx! {
        AppShell {
            title: "Files",
            form: FormFactor::Desktop,
            sidebar_state: SidebarState { rail: Some(props.rail), ..SidebarState::default() },
            sidebar: rsx! {
                Sidebar {
                    header: rsx! { SidebarSwitcher { title: "Library", detail: "3 locations" } },
                    footer: rsx! { ModeProbe {} },
                    SidebarSection { title: "Library", count: "2",
                        action: rsx! { button { class: "add", "+" } },
                        SidebarItem { label: "Overview", icon: icons::HOME, selected: true }
                        SidebarItem { label: "Projects", icon: icons::FOLDER, badge: "3", expanded: true,
                            SidebarItem { label: "Nested child", icon: icons::FOLDER }
                        }
                        SidebarItem { label: "Shut branch", expanded: false,
                            SidebarItem { label: "Unseen child" }
                        }
                    }
                }
            },
            p { "body" }
        }
    }
}

#[component]
fn ModeProbe() -> Element {
    let mode = match use_sidebar_mode() {
        SidebarMode::Docked => "docked",
        SidebarMode::Rail => "rail",
        SidebarMode::Drawer => "drawer",
    };
    rsx! { span { class: "probe", "mode {mode}" } }
}

fn render_rail(rail: bool) -> String {
    let mut dom = VirtualDom::new_with_props(RailShell, Railed { rail });
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn rows_nest_under_a_chevron_with_a_guide_and_take_the_keyboard() {
    let html = render_rail(false);
    assert!(html.contains("data-sidebar=\"docked\""));
    assert!(html.contains("mode docked"), "the content knows its mode");
    assert!(
        html.contains(">Nested child</span>"),
        "an open branch shows its rows"
    );
    assert!(html.contains("aria-level=\"2\""), "one level in");
    assert!(html.contains("class=\"as-indent-guide\""));
    assert!(!html.contains("Unseen child"), "a shut branch hides them");
    assert_eq!(
        html.matches("aria-expanded=\"true\"").count(),
        2,
        "the section and the open branch"
    );
    assert!(html.contains("aria-label=\"Collapse\"") && html.contains("aria-label=\"Expand\""));
    assert!(html.contains("tabindex=\"0\""), "rows take the focus");
    assert!(html.contains("role=\"tree\"") && html.contains("role=\"treeitem\""));
    assert!(
        html.contains("class=\"as-section-count\"") && html.contains("class=\"as-section-action\"")
    );
    assert!(
        html.contains("class=\"as-sidebar-edge\""),
        "an edge to drag"
    );
}

#[test]
fn folded_to_the_rail_the_sidebar_is_icons_under_a_top_bar_across_the_window() {
    let html = render_rail(true);
    assert!(html.contains("data-sidebar=\"rail\"") && html.contains("width: 52px;"));
    assert!(html.contains("mode rail"));
    let bar_at = html.find("class=\"as-topbar\"").unwrap();
    let lights_at = html.find("class=\"as-lights\"").unwrap();
    let rail_at = html.find("data-sidebar=\"rail\"").unwrap();
    assert!(
        bar_at < lights_at && lights_at < rail_at,
        "the lights in the top bar, the rail under it"
    );
    assert!(
        html.contains("aria-label=\"Overview\""),
        "its name for a reader and the tip"
    );
    assert!(!html.contains(">Overview</span>"), "no label drawn");
    assert!(
        !html.contains("Nested child"),
        "nested rows hide on the rail"
    );
    assert!(
        !html.contains(">Library</span>"),
        "no headings, a rule instead"
    );
    assert!(html.contains("class=\"as-rail-rule\""));
    assert!(html.contains("class=\"as-rail-dot\""), "a badge is a dot");
}

#[test]
fn the_scrollbars_are_thin_rounded_and_themed_in_every_engine() {
    for needle in [
        "::-webkit-scrollbar-thumb",
        "border-radius: 999px",
        "var(--as-scrollbar)",
        "scrollbar-width: thin",
        "scrollbar-color: var(--as-scrollbar) transparent",
        "@supports not selector(::-webkit-scrollbar)",
    ] {
        assert!(SHELL_CSS.contains(needle), "{needle}");
    }
    let mut dom = VirtualDom::new(|| rsx! { ScrollArea { p { "x" } } });
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    assert!(html.contains("as-scroll") && html.contains("overflow-y: scroll"));
    assert!(
        !html.contains("scrollbar-width"),
        "inline, it would switch Blink's parts off"
    );
}

#[test]
fn on_a_phone_the_sidebar_is_a_drawer_behind_a_menu_button() {
    let html = render_app(|| {
        rsx! {
            AppShell { title: "Files", form: FormFactor::Phone,
                sidebar: rsx! { Sidebar { SidebarItem { label: "Overview" } } },
                p { "body" }
            }
        }
    });
    assert!(
        html.contains("data-open=\"false\""),
        "shut until the menu or a swipe opens it"
    );
    assert!(html.contains("data-sidebar=\"drawer\"") && !html.contains("data-sidebar=\"docked\""));
    assert!(html.contains("aria-label=\"Menu\""));
    assert!(html.contains("data-form=\"phone\""));
}

#[test]
fn the_top_bar_holds_the_apps_slots() {
    let html = render_app(|| {
        rsx! {
            AppShell {
                form: FormFactor::Desktop,
                top_bar: rsx! {
                    TopBar {
                        lead: rsx! { span { class: "crumb", "Home / Photos" } },
                        search: rsx! { SearchField { placeholder: "Search files" } },
                        actions: rsx! {
                            ToolbarGroup {
                                ToolbarButton { icon: icons::GRID, label: "Grid", active: true, on_press: |_| {} }
                                ToolbarButton { icon: icons::LIST, label: "List", on_press: |_| {} }
                            }
                        },
                        on_back: |_| {},
                        on_forward: |_| {},
                        can_forward: false,
                    }
                },
                p { "body" }
            }
        }
    });
    assert!(html.contains("Home / Photos") && html.contains("placeholder=\"Search files\""));
    assert!(html.contains("aria-pressed=\"true\"") && html.contains("aria-label=\"List\""));
    assert!(
        html.contains("aria-label=\"Back\"")
            && html.contains("aria-label=\"Forward\" aria-disabled=\"true\"")
    );
    assert!(
        html.contains("grid-template-columns: minmax(0, 1fr) minmax(0, auto) minmax(0, 1fr)"),
        "the search centred"
    );
}

// The phone layout: the foot, the drawer, sheets, menus, the palette, toasts.

use app_shell::ui::{Destination, PaletteItem, PrimaryAction, Sheet, SheetSpec, use_sheet};

fn destinations() -> Vec<Destination> {
    vec![
        Destination::new("drive", "Drive", icons::FOLDER),
        Destination::new("search", "Search", icons::SEARCH),
        Destination::new("tags", "Tags", icons::TAG).badge("3"),
    ]
}

#[derive(Props, Clone, PartialEq)]
struct PhoneLab {
    form: FormFactor,
    #[props(default)]
    with_accessory: bool,
}

#[allow(non_snake_case)]
fn PhoneShell(props: PhoneLab) -> Element {
    rsx! {
        AppShell {
            title: "Files",
            form: props.form,
            destinations: destinations(),
            destination: "search",
            action: PrimaryAction::new("Upload", icons::PLUS, |_| {}),
            accessory: props.with_accessory.then(|| rsx! { div { class: "uploads", "Uploading 2 files" } }),
            sidebar: rsx! { Sidebar { SidebarItem { label: "Overview" } } },
            p { "body" }
        }
    }
}

fn render_phone(form: FormFactor, with_accessory: bool) -> String {
    let mut dom = VirtualDom::new_with_props(
        PhoneShell,
        PhoneLab {
            form,
            with_accessory,
        },
    );
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn a_phone_has_a_tab_pill_of_destinations_and_a_round_button() {
    let html = render_phone(FormFactor::Phone, false);
    assert!(html.contains("class=\"as-pill\""));
    assert_eq!(html.matches("class=\"as-pill-tab\"").count(), 3);
    assert!(html.contains("aria-label=\"Search\" style=\"position: relative; flex: 1 1 0px; min-width: 0; height: 46px; border-radius: 23px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 2px; background-color: var(--as-selected)"), "the current one lit");
    assert!(html.contains("aria-selected=\"true\""));
    assert!(html.contains(">3</span>"), "a badge");
    assert!(html.contains("class=\"as-round\"") && html.contains("aria-label=\"Upload\""));
    assert!(
        html.contains("env(safe-area-inset-bottom, 0px)"),
        "clear of the home indicator"
    );
    assert!(
        html.contains("padding-bottom: 12px; padding-bottom: max(12px"),
        "a fallback first for Blitz"
    );
    assert!(!html.contains("class=\"as-accessory\""));
}

#[test]
fn the_accessory_docks_over_the_pill_on_a_phone_and_under_the_content_on_a_desktop() {
    let phone = render_phone(FormFactor::Phone, true);
    let acc = phone.find("class=\"as-accessory\"").unwrap();
    assert!(
        acc < phone.find("class=\"as-pill\"").unwrap(),
        "above the pill"
    );
    assert!(phone.contains("Uploading 2 files"));
    let desk = render_phone(FormFactor::Desktop, true);
    assert!(desk.contains("class=\"as-accessory\"") && desk.contains("Uploading 2 files"));
    assert!(
        !desk.contains("as-pill") && !desk.contains("as-round"),
        "the sidebar holds the destinations"
    );
}

#[test]
fn on_a_phone_the_drawer_is_mounted_shut_off_screen() {
    let html = render_phone(FormFactor::Phone, false);
    assert!(html.contains("class=\"as-drawer\"") && html.contains("data-open=\"false\""));
    assert!(html.contains("transform: translateX(-"), "off to the left");
    assert!(
        html.contains("touch-action: pan-y"),
        "a horizontal swipe is the shell's"
    );
}

/// What a test does to the shell once it is mounted.
type ShellScript = Box<dyn Fn(app_shell::ui::Shell)>;

thread_local! {
    static SHEET_ACT: RefCell<Option<ShellScript>> = RefCell::new(None);
}

#[derive(Props, Clone, PartialEq)]
struct SheetLab {
    form: FormFactor,
}

#[component]
fn Opener() -> Element {
    let shell = use_shell();
    let sheet = use_sheet();
    use_hook(move || {
        if let Some(f) = SHEET_ACT.with(|a| a.borrow_mut().take()) {
            f(shell);
        }
        let _ = sheet;
    });
    rsx! {}
}

#[allow(non_snake_case)]
fn SheetShell(props: SheetLab) -> Element {
    rsx! {
        AppShell {
            title: "Files",
            form: props.form,
            palette: Callback::new(|()| vec![
                PaletteItem::call("Upload files", Callback::new(|()| {})).keywords("new add"),
                PaletteItem::call("New folder", Callback::new(|()| {})),
            ]),
            Opener {}
        }
    }
}

fn render_sheets(form: FormFactor, act: impl Fn(app_shell::ui::Shell) + 'static) -> String {
    SHEET_ACT.with(|a| *a.borrow_mut() = Some(Box::new(act)));
    let mut dom = VirtualDom::new_with_props(SheetShell, SheetLab { form });
    dom.rebuild_in_place();
    for _ in 0..4 {
        dom.process_events();
        dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
    }
    dioxus_ssr::render(&dom)
}

#[test]
fn a_sheet_is_a_bottom_sheet_with_a_handle_on_a_phone_and_a_panel_on_a_desktop() {
    let open = |s: app_shell::ui::Shell| {
        s.open_sheet(
            SheetSpec::new(|()| rsx! { p { class: "info", "About this file" } }).title("Info"),
        );
    };
    let phone = render_sheets(FormFactor::Phone, open);
    assert!(phone.contains("data-sheet=\"bottom\"") && phone.contains("data-detent=\"medium\""));
    assert!(
        phone.contains("class=\"as-grab\"")
            && phone.contains("About this file")
            && phone.contains(">Info</span>")
    );
    assert!(phone.contains("overflow-y: scroll"), "its body scrolls");
    let desk = render_sheets(FormFactor::Desktop, open);
    assert!(desk.contains("data-sheet=\"panel\"") && desk.contains("translate(-50%, -50%)"));
    assert!(!desk.contains("as-grab") && desk.contains("aria-label=\"Close\""));
}

#[test]
fn sheets_stack_the_lower_ones_set_back() {
    let html = render_sheets(FormFactor::Phone, |s| {
        s.open_sheet(SheetSpec::new(|()| rsx! { "first" }));
        s.open_sheet(SheetSpec::new(|()| rsx! { "second" }).detents(
            app_shell::gestures::Detent::Large,
            &[app_shell::gestures::Detent::Large],
        ));
    });
    assert_eq!(html.matches("data-sheet=\"bottom\"").count(), 2);
    assert!(html.contains("scale(0.96)"), "the one under set back");
    assert!(html.contains("data-detent=\"large\""));
}

#[test]
fn a_menu_is_an_action_sheet_on_a_phone_and_a_popup_on_a_desktop() {
    let items = || {
        vec![
            app_shell::ui::MenuItem::call("Rename", Callback::new(|()| {})),
            app_shell::ui::MenuItem::call("Delete", Callback::new(|()| {})).danger(),
        ]
    };
    let phone = render_sheets(FormFactor::Phone, move |s| s.menu(10.0, 10.0, items()));
    assert!(phone.contains("class=\"as-action-list\"") && phone.contains("data-detent=\"fit\""));
    assert!(phone.contains("min-height: 48px") && phone.contains(">Delete</span>"));
    assert!(!phone.contains("class=\"as-popup\""));
    let desk = render_sheets(FormFactor::Desktop, move |s| s.menu(10.0, 10.0, items()));
    assert!(desk.contains("class=\"as-popup\"") && !desk.contains("as-action-list"));
}

#[test]
fn the_palette_is_a_sheet_on_a_phone_and_near_the_top_on_a_desktop() {
    let phone = render_sheets(FormFactor::Phone, |s| s.open_palette());
    assert!(phone.contains("class=\"as-palette\"") && phone.contains("data-detent=\"large\""));
    assert!(phone.contains(">Upload files</span>") && phone.contains(">New folder</span>"));
    let desk = render_sheets(FormFactor::Desktop, |s| {
        s.command(app_shell::Command::OpenPalette)
    });
    assert!(desk.contains("class=\"as-palette\"") && desk.contains("top: 12%"));
}

#[test]
fn toasts_queue_and_show_one_at_a_time() {
    let html = render_sheets(FormFactor::Phone, |s| {
        s.toast("Uploaded 3 files");
        s.show_toast(app_shell::ui::ToastSpec::new("Moved to Trash").action("Undo", |_| {}));
    });
    assert!(
        html.contains("Uploaded 3 files") && !html.contains("Moved to Trash"),
        "the first first"
    );
    assert!(
        html.contains("role=\"status\"") && html.contains("env(safe-area-inset-top"),
        "under the notch on a phone"
    );
}

#[test]
fn a_declared_sheet_opens_while_its_flag_is_set() {
    #[component]
    fn Declared(open: bool) -> Element {
        rsx! {
            AppShell { form: FormFactor::Phone,
                Sheet { open, title: "Inspector", render: Callback::new(|()| rsx! { "details" }) }
            }
        }
    }
    let render = |open| {
        let mut dom = VirtualDom::new_with_props(Declared, DeclaredProps { open });
        dom.rebuild_in_place();
        for _ in 0..4 {
            dom.process_events();
            dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
        }
        dioxus_ssr::render(&dom)
    };
    assert!(render(true).contains(">Inspector</span>"));
    assert!(!render(false).contains("Inspector"));
}
