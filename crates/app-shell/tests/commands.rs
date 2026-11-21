//! Commands run on the layout, and the keys that ask for them.

#![allow(clippy::unwrap_used)]

use app_shell::{
    Bindings, Command, DefaultKeymap, Direction, KeyChord, Keymap, Pane, PaneId, Ran, Shown, Side,
    Target, Workspace,
};

fn id(s: &str) -> PaneId {
    PaneId::new(s)
}

fn three_tabs() -> Workspace {
    let mut ws = Workspace::new();
    for p in ["a", "b", "c"] {
        ws.open(Pane::new(p, "test.view", p), Target::Tab).unwrap();
    }
    ws
}

#[test]
fn close_and_reopen_bring_the_tab_back_where_it_was() {
    let mut ws = three_tabs();
    assert_eq!(ws.run(Command::CloseTab).unwrap(), Ran::Changed);
    assert!(ws.pane(&id("c")).is_none());
    assert_eq!(ws.run(Command::ReopenClosed).unwrap(), Ran::Changed);
    assert_eq!(ws.active, Some(id("c")));
    let g = ws.locate(&id("c")).unwrap();
    assert_eq!(g.index, 2);
}

#[test]
fn tabs_cycle_and_go_to_by_number_with_nine_the_last() {
    let mut ws = three_tabs();
    ws.run(Command::NextTab).unwrap();
    assert_eq!(ws.active, Some(id("a")), "round from the last");
    ws.run(Command::PrevTab).unwrap();
    assert_eq!(ws.active, Some(id("c")));
    ws.run(Command::GotoTab(1)).unwrap();
    assert_eq!(ws.active, Some(id("b")));
    ws.run(Command::GotoTab(usize::MAX)).unwrap();
    assert_eq!(ws.active, Some(id("c")));
}

#[test]
fn split_moves_the_active_tab_beside_its_stack_and_focus_crosses_back() {
    let mut ws = three_tabs();
    assert_eq!(ws.run(Command::Split(Side::Right)).unwrap(), Ran::Changed);
    let (b, c) = (ws.locate(&id("b")).unwrap(), ws.locate(&id("c")).unwrap());
    assert_ne!(b.group, c.group);
    assert_eq!(ws.active, Some(id("c")));
    ws.run(Command::Focus(Direction::Left)).unwrap();
    assert_eq!(
        ws.locate(ws.active.as_ref().unwrap()).unwrap().group,
        b.group
    );
    ws.run(Command::Focus(Direction::Left)).unwrap();
    assert_eq!(
        ws.run(Command::Focus(Direction::Left)).unwrap(),
        Ran::Nothing,
        "nothing further left"
    );
}

#[test]
fn a_lone_tab_does_not_split_from_itself() {
    let mut ws = Workspace::new();
    ws.open(Pane::new("a", "v", "A"), Target::Tab).unwrap();
    assert_eq!(ws.run(Command::Split(Side::Right)).unwrap(), Ran::Nothing);
}

#[test]
fn zoom_needs_a_split_and_toggles() {
    let mut ws = three_tabs();
    assert_eq!(ws.run(Command::ToggleZoom).unwrap(), Ran::Nothing);
    ws.run(Command::Split(Side::Bottom)).unwrap();
    ws.run(Command::ToggleZoom).unwrap();
    assert!(ws.windows[0].zoomed.is_some());
    ws.run(Command::ToggleZoom).unwrap();
    assert!(ws.windows[0].zoomed.is_none());
}

#[test]
fn a_tab_goes_to_a_window_of_its_own_and_back() {
    let mut ws = three_tabs();
    ws.run(Command::MoveToNewWindow).unwrap();
    assert_eq!(ws.windows.len(), 2);
    assert_eq!(ws.locate(&id("c")).unwrap().window, 1);
    ws.run(Command::MoveToMainWindow).unwrap();
    assert_eq!(ws.windows.len(), 1, "the empty pop-out closes");
}

#[test]
fn pin_toggles_and_close_others_keeps_pinned_tabs() {
    let mut ws = three_tabs();
    ws.activate(&id("a")).unwrap();
    ws.run(Command::TogglePin).unwrap();
    ws.activate(&id("c")).unwrap();
    ws.run(Command::CloseOthers).unwrap();
    assert!(ws.pane(&id("a")).is_some());
    assert!(ws.pane(&id("b")).is_none());
}

#[test]
fn docks_toggle_and_the_switcher_is_the_uis() {
    let mut ws = three_tabs();
    ws.open(Pane::new("files", "v", "Files"), Target::Dock(Side::Left))
        .unwrap();
    ws.run(Command::ToggleDock(Side::Left)).unwrap();
    assert_eq!(ws.dock(Side::Left).unwrap().shown, Shown::Hidden);
    ws.run(Command::ToggleDock(Side::Left)).unwrap();
    assert_eq!(ws.dock(Side::Left).unwrap().shown, Shown::Open);
    assert_eq!(ws.run(Command::OpenSwitcher).unwrap(), Ran::Ui);
}

#[test]
fn every_command_on_an_empty_layout_does_nothing_and_does_not_fail() {
    let all = [
        Command::CloseTab,
        Command::CloseOthers,
        Command::ReopenClosed,
        Command::NextTab,
        Command::PrevTab,
        Command::GotoTab(3),
        Command::Split(Side::Left),
        Command::Focus(Direction::Up),
        Command::ToggleZoom,
        Command::TogglePin,
        Command::MoveToNewWindow,
        Command::MoveToMainWindow,
    ];
    for cmd in all {
        let mut ws = Workspace::new();
        assert_eq!(ws.run(cmd).unwrap(), Ran::Nothing, "{cmd:?}");
        assert!(!cmd.label().is_empty());
    }
}

fn chord(text: &str) -> KeyChord {
    KeyChord::parse(text, false).unwrap()
}

#[test]
fn chords_parse_with_the_platforms_primary_modifier() {
    let mac = KeyChord::parse("Mod+Shift+T", true).unwrap();
    assert!(mac.meta && mac.shift && !mac.ctrl);
    assert_eq!(mac.key, "t");
    let pc = KeyChord::parse("Mod+W", false).unwrap();
    assert!(pc.ctrl && !pc.meta);
    assert_eq!(KeyChord::parse("Mod++", false).unwrap().key, "+");
    assert!(KeyChord::parse("Hyper+W", false).is_none());
    assert!(KeyChord::parse("", false).is_none());
}

#[test]
fn the_default_keys_are_browser_tab_habits() {
    let k = DefaultKeymap;
    assert_eq!(k.command(&chord("Ctrl+W")), Some(Command::CloseTab));
    assert_eq!(k.command(&chord("Meta+W")), Some(Command::CloseTab));
    assert_eq!(
        k.command(&chord("Ctrl+Shift+T")),
        Some(Command::ReopenClosed)
    );
    assert_eq!(k.command(&chord("Ctrl+Tab")), Some(Command::NextTab));
    assert_eq!(k.command(&chord("Ctrl+Shift+Tab")), Some(Command::PrevTab));
    assert_eq!(k.command(&chord("Ctrl+1")), Some(Command::GotoTab(0)));
    assert_eq!(
        k.command(&chord("Ctrl+9")),
        Some(Command::GotoTab(usize::MAX))
    );
    assert_eq!(
        k.command(&chord("Ctrl+\\")),
        Some(Command::Split(Side::Right))
    );
    assert_eq!(
        k.command(&chord("Ctrl+Alt+ArrowDown")),
        Some(Command::Focus(Direction::Down))
    );
    assert_eq!(
        k.command(&chord("Ctrl+Shift+E")),
        Some(Command::OpenSwitcher)
    );
    assert_eq!(k.command(&chord("W")), None);
    assert_eq!(k.command(&chord("Ctrl+0")), None);
}

#[test]
fn an_apps_bindings_win_and_fall_back_to_the_defaults() {
    let b = Bindings::over_defaults()
        .bind("Mod+K", false, Command::OpenSwitcher)
        .bind("Mod+W", false, Command::TogglePin);
    assert_eq!(b.command(&chord("Ctrl+K")), Some(Command::OpenSwitcher));
    assert_eq!(b.command(&chord("Ctrl+W")), Some(Command::TogglePin));
    assert_eq!(b.command(&chord("Ctrl+Tab")), Some(Command::NextTab));
    let only = Bindings::default().bind("Mod+K", false, Command::OpenSwitcher);
    assert_eq!(only.command(&chord("Ctrl+Tab")), None);
}

#[test]
fn a_command_round_trips_through_serde() {
    for cmd in [
        Command::Split(Side::Top),
        Command::GotoTab(4),
        Command::Focus(Direction::Right),
    ] {
        let json = serde_json::to_string(&cmd).unwrap();
        assert_eq!(serde_json::from_str::<Command>(&json).unwrap(), cmd);
    }
}
