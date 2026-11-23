//! app-shell's gallery: a window with a sidebar that has everything the
//! sidebar does (a switcher, sections with counts and an action, rows
//! nested two deep, a folded section, enough rows to scroll, a footer
//! that draws itself small on the rail) and a content area that scrolls.
//!
//! The top bar runs across the window with both sidebars' toggles; the
//! right sidebar holds an inspector; a page puts an item in the bar's
//! trailing place (every page) and takes over its center ("Sessions":
//! a session's song tabs in place of the search).
//!
//! In a browser: `dx serve -p app-shell-gallery --platform web --features web`.
//! `#rail` starts the sidebar folded to the rail, `#focus` puts the
//! keyboard on its first row, `#sessions` opens on the page that takes
//! over the center.

use app_shell::ui::{
    AppShell, BarPlace, BarSpan, Destination, ScrollArea, SearchField, Sidebar, SidebarItem,
    SidebarMode, SidebarSection, SidebarState, SidebarSwitcher, ToolbarButton, ToolbarGroup,
    TopBar, icons, use_shell, use_sidebar_mode, use_top_bar_item, use_top_bar_span,
};
use dioxus::prelude::*;

fn main() {
    dioxus::launch(app);
}

const PAGE_CSS: &str = "html, body { margin: 0; height: 100%; background: #18181b; }";

fn app() -> Element {
    let mut page = use_signal(|| "overview".to_owned());
    let at = move |id: &'static str| move |()| page.set(id.to_owned());
    let on = |id: &str| page.read().as_str() == id;
    rsx! {
        document::Style { {PAGE_CSS} }
        div { style: "position: absolute; inset: 0; display: flex;",
            AppShell {
                title: "Gallery",
                destinations: vec![
                    Destination::new("overview", "Overview", icons::HOME),
                    Destination::new("files", "Files", icons::FOLDER),
                    Destination::new("tags", "Tags", icons::TAG),
                    Destination::new("settings", "Settings", icons::SETTINGS),
                ],
                destination: page(),
                on_destination: move |d: String| page.set(d),
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
                sidebar: rsx! {
                    Sidebar {
                        header: rsx! { SidebarSwitcher { title: "Studio Library", detail: "4 locations" } },
                        footer: rsx! { Account {} },
                        SidebarSection { title: "Library",
                            SidebarItem { label: "Overview", icon: icons::HOME, selected: on("overview"), on_press: at("overview") }
                            SidebarItem { label: "Recents", icon: icons::LAYERS, badge: "24", selected: on("recents"), on_press: at("recents") }
                            SidebarItem { label: "Pinned", icon: icons::PIN, selected: on("pinned"), on_press: at("pinned") }
                        }
                        SidebarSection { title: "Projects", count: "3",
                            action: rsx! { AddButton {} },
                            SidebarItem { label: "Albums", icon: icons::FOLDER, expanded: true, selected: on("albums"), on_press: at("albums"),
                                SidebarItem { label: "Night Drive", icon: icons::FOLDER, expanded: true, on_press: at("night"),
                                    SidebarItem { label: "Stems", icon: icons::FILE, badge: "48", selected: on("stems"), on_press: at("stems") }
                                    SidebarItem { label: "Mixes", icon: icons::FILE, on_press: at("mixes") }
                                }
                                SidebarItem { label: "Live at the Hall", icon: icons::FOLDER, expanded: false,
                                    SidebarItem { label: "Set one", icon: icons::FILE }
                                }
                            }
                            SidebarItem { label: "Singles", icon: icons::FOLDER, badge: "7", on_press: at("singles") }
                            SidebarItem { label: "Sessions", icon: icons::FOLDER, on_press: at("sessions") }
                        }
                        SidebarSection { title: "Tags", open: false,
                            SidebarItem { label: "Rough", icon: icons::TAG }
                            SidebarItem { label: "Final", icon: icons::TAG }
                        }
                        SidebarSection { title: "Locations",
                            for (i, name) in ["Workstation", "Laptop", "Studio NAS", "Offsite", "Phone", "Archive A", "Archive B", "Archive C"].iter().enumerate() {
                                SidebarItem { key: "{i}", label: *name, icon: icons::COLUMNS, badge: format!("{}", (i + 2) * 31) }
                            }
                        }
                    }
                },
                right_sidebar: rsx! { Inspector { page: page() } },
                StartFromHash { page }
                Synced {}
                if page() == "sessions" {
                    SessionTabs {}
                }
                Content { page: page() }
            }
        }
    }
}

/// A section's "+": shown on hover, its own press.
#[component]
fn AddButton() -> Element {
    rsx! {
        div {
            class: "as-hov",
            role: "button",
            "aria-label": "New project",
            style: "width: 18px; height: 18px; border-radius: 4px; display: flex; align-items: center; justify-content: center; color: var(--as-muted);",
            app_shell::ui::IconView { icon: icons::PLUS, size: 12.0 }
        }
    }
}

/// The footer: who is signed in, small on the rail.
#[component]
fn Account() -> Element {
    let rail = use_sidebar_mode() == SidebarMode::Rail;
    let mark = rsx! {
        div { style: "flex: none; width: 28px; height: 28px; border-radius: 14px; background-color: #0d9488; color: #fff; display: flex; align-items: center; justify-content: center; font-weight: 700; font-size: 12px;", "AS" }
    };
    if rail {
        return rsx! { div { style: "display: flex; justify-content: center; padding: 4px 0;", {mark} } };
    }
    rsx! {
        div { style: "display: flex; align-items: center; gap: 8px; padding: 4px;",
            {mark}
            div { style: "display: flex; flex-direction: column; min-width: 0;",
                span { style: "font-weight: 600;", "A. Sample" }
                span { style: "font-size: 11px; color: var(--as-muted);", "Signed in" }
            }
        }
    }
}

/// `#rail` folds the sidebar; `#focus` puts the keyboard on its first row.
#[component]
fn StartFromHash(page: Signal<String>) -> Element {
    let mut page = page;
    let shell = use_shell();
    use_future(move || async move {
        let hash = document::eval("return window.location.hash")
            .await
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        if hash.contains("rail") {
            shell.set_sidebar(SidebarState {
                rail: Some(true),
                ..shell.sidebar_state()
            });
        }
        if hash.contains("sessions") {
            page.set("sessions".to_owned());
        }
        if hash.contains("focus") {
            let _ = document::eval(
                "setTimeout(() => document.querySelector('[role=treeitem]')?.focus(), 300)",
            )
            .await;
        }
    });
    rsx! {}
}

/// A content area long enough to scroll.
#[component]
fn Content(page: String) -> Element {
    rsx! {
        ScrollArea { style: "padding: 20px 24px;".to_owned(),
            h2 { style: "margin: 0 0 12px 0; font-size: 18px;", "{page}" }
            for i in 0..60 {
                div {
                    key: "{i}",
                    style: "height: 40px; display: flex; align-items: center; padding: 0 12px; margin-bottom: 6px; border-radius: 8px; background-color: var(--as-surface); border: 1px solid var(--as-border);",
                    "Row {i + 1}"
                }
            }
        }
    }
}

/// Every page: a sync status in the bar's trailing place.
#[component]
fn Synced() -> Element {
    use_top_bar_item("gallery/sync", BarPlace::Trailing, 0, || {
        rsx! {
            span { style: "font-size: 12px; color: var(--as-muted); padding: 0 8px;", "Synced" }
        }
    });
    rsx! {}
}

/// "Sessions": the bar's center is the session's song tabs.
#[component]
fn SessionTabs() -> Element {
    use_top_bar_span("gallery/session-tabs", BarSpan::Center, || {
        rsx! {
            div { style: "display: flex; gap: 4px;",
                for (i, song) in ["Intro", "Know Me", "Fire", "Hey!"].iter().enumerate() {
                    span {
                        key: "{i}",
                        style: if i == 1 { "padding: 4px 10px; border-radius: 6px; background-color: var(--as-selected); font-weight: 600;" } else { "padding: 4px 10px; border-radius: 6px; color: var(--as-muted);" },
                        "{song}"
                    }
                }
            }
        }
    });
    rsx! {}
}

/// The right sidebar: what is selected.
#[component]
fn Inspector(page: String) -> Element {
    rsx! {
        ScrollArea { style: "padding: 8px 14px;".to_owned(),
            div { style: "font-size: 11px; text-transform: uppercase; color: var(--as-muted); margin-bottom: 8px;", "Inspector" }
            div { style: "font-weight: 600; margin-bottom: 6px;", "{page}" }
            for (k, v) in [("Kind", "Folder"), ("Items", "48"), ("Modified", "Today")] {
                div { key: "{k}", style: "display: flex; justify-content: space-between; padding: 4px 0; color: var(--as-muted);",
                    span { "{k}" }
                    span { style: "color: var(--as-text);", "{v}" }
                }
            }
        }
    }
}
