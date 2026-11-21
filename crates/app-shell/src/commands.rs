//! What a key or a menu asks of the shell, as data: a [`Command`], run on
//! the layout by [`Workspace::run`]; and which keys ask for which, a
//! [`Keymap`] the app owns ([`DefaultKeymap`] until it says otherwise).
//!
//! No command framework: the app maps its own keys (or a palette, or a
//! menu bar) to [`Command`]s, or hears every command first through the
//! shell's `on_command` and handles the ones it wants.

use serde::{Deserialize, Serialize};

use crate::geometry::{Direction, area_rects};
use crate::model::{Area, Rect, Shown, Side, Workspace};
use crate::ops::{Dest, WorkspaceError};

/// Something the shell can do to its layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    /// Close the active tab (kept to reopen).
    CloseTab,
    /// Close every other unpinned tab in the active stack.
    CloseOthers,
    /// Reopen the tab closed last, where it was.
    ReopenClosed,
    /// The next tab of the active stack, round.
    NextTab,
    /// The previous tab of the active stack, round.
    PrevTab,
    /// Tab `n` (from zero) of the active stack; the last when past the end.
    GotoTab(usize),
    /// Move the active tab into a new stack on this side of its own.
    Split(Side),
    /// Focus the stack next to the active one in this direction.
    Focus(Direction),
    /// Show the active stack alone in its window, or the whole tree again.
    ToggleZoom,
    /// Pin or unpin the active tab.
    TogglePin,
    /// Move the active tab into a window of its own.
    MoveToNewWindow,
    /// Move the active tab back to the main window.
    MoveToMainWindow,
    /// Open or hide the dock on this side.
    ToggleDock(Side),
    /// Open the tab switcher (the UI's; the layout does not change).
    OpenSwitcher,
    /// Show or hide the sidebar (the UI's; the layout does not change).
    ToggleSidebar,
    /// Open the command palette (the UI's; the layout does not change).
    OpenPalette,
}

impl Command {
    /// Its name in a menu.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::CloseTab => "Close tab".into(),
            Self::CloseOthers => "Close other tabs".into(),
            Self::ReopenClosed => "Reopen closed tab".into(),
            Self::NextTab => "Next tab".into(),
            Self::PrevTab => "Previous tab".into(),
            Self::GotoTab(n) => format!("Go to tab {}", n.saturating_add(1)),
            Self::Split(Side::Left) => "Split left".into(),
            Self::Split(Side::Right) => "Split right".into(),
            Self::Split(Side::Top) => "Split up".into(),
            Self::Split(Side::Bottom) => "Split down".into(),
            Self::Focus(Direction::Left) => "Focus left".into(),
            Self::Focus(Direction::Right) => "Focus right".into(),
            Self::Focus(Direction::Up) => "Focus up".into(),
            Self::Focus(Direction::Down) => "Focus down".into(),
            Self::ToggleZoom => "Zoom stack".into(),
            Self::TogglePin => "Pin tab".into(),
            Self::MoveToNewWindow => "Move to new window".into(),
            Self::MoveToMainWindow => "Move to main window".into(),
            Self::ToggleDock(Side::Left) => "Toggle left dock".into(),
            Self::ToggleDock(Side::Right) => "Toggle right dock".into(),
            Self::ToggleDock(_) => "Toggle bottom dock".into(),
            Self::OpenSwitcher => "Show all tabs".into(),
            Self::ToggleSidebar => "Toggle sidebar".into(),
            Self::OpenPalette => "Open anything".into(),
        }
    }
}

/// What running a command did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ran {
    /// The layout changed.
    Changed,
    /// Nothing to do (no active tab, already there).
    Nothing,
    /// A command for the UI, not the layout ([`Command::OpenSwitcher`],
    /// [`Command::ToggleSidebar`]).
    Ui,
}

impl Workspace {
    /// Run `command` on the layout. The layout is repaired after.
    pub fn run(&mut self, command: Command) -> Result<Ran, WorkspaceError> {
        let before = self.clone();
        let active = self.active.clone();
        match command {
            Command::OpenSwitcher | Command::ToggleSidebar | Command::OpenPalette => {
                return Ok(Ran::Ui);
            }
            Command::CloseTab => {
                if let Some(a) = &active {
                    self.close(a)?;
                }
            }
            Command::CloseOthers => {
                if let Some(a) = &active {
                    self.close_others(a)?;
                }
            }
            Command::ReopenClosed => {
                self.reopen()?;
            }
            Command::NextTab => self.cycle_tab(false)?,
            Command::PrevTab => self.cycle_tab(true)?,
            Command::GotoTab(n) => self.goto_tab(n)?,
            Command::Split(side) => {
                let Some(a) = &active else {
                    return Ok(Ran::Nothing);
                };
                let Some(loc) = self.locate(a) else {
                    return Ok(Ran::Nothing);
                };
                // A lone tab has nothing to split from.
                if self.group(loc.group).is_some_and(|g| g.tabs.len() > 1) {
                    self.move_pane(
                        a,
                        Dest::Edge {
                            group: loc.group,
                            side,
                        },
                    )?;
                }
            }
            Command::Focus(direction) => {
                let Some(loc) = active.as_ref().and_then(|a| self.locate(a)) else {
                    return Ok(Ran::Nothing);
                };
                if loc.area != Area::Center {
                    return Ok(Ran::Nothing);
                }
                let Some(center) = self.windows.get(loc.window).and_then(|w| w.center.as_ref())
                else {
                    return Ok(Ran::Nothing);
                };
                let placed = area_rects(center, Rect::new(0.0, 0.0, 1000.0, 1000.0));
                let next = crate::geometry::neighbour(&placed, loc.group, direction)
                    .and_then(|g| self.group(g))
                    .and_then(|g| g.current_tab().cloned());
                if let Some(next) = next {
                    self.activate(&next)?;
                }
            }
            Command::ToggleZoom => {
                let Some(loc) = active.as_ref().and_then(|a| self.locate(a)) else {
                    return Ok(Ran::Nothing);
                };
                let zoomed = self
                    .windows
                    .get(loc.window)
                    .is_some_and(|w| w.zoomed == Some(loc.group));
                if loc.area == Area::Center {
                    self.set_zoomed(loc.group, !zoomed)?;
                    self.repair();
                }
            }
            Command::TogglePin => {
                if let Some(a) = &active {
                    let pinned = self.pane(a).is_some_and(|p| p.pinned);
                    self.set_pinned(a, !pinned)?;
                }
            }
            Command::MoveToNewWindow => {
                if let Some(a) = &active {
                    self.move_pane(a, Dest::Window(None))?;
                }
            }
            Command::MoveToMainWindow => {
                if let Some(a) = &active {
                    self.to_main_window(a)?;
                }
            }
            Command::ToggleDock(side) => {
                let shown = self.dock(side).map(|d| d.shown);
                let next = match shown {
                    Some(Shown::Open) => Shown::Hidden,
                    _ => Shown::Open,
                };
                self.set_dock_shown(side, next)?;
            }
        }
        Ok(if self.same_arrangement(&before) {
            Ran::Nothing
        } else {
            Ran::Changed
        })
    }
}

/// A key pressed with its modifiers: what a [`Keymap`] reads.
///
/// `key` is the key's name as a browser has it (`KeyboardEvent.key`),
/// letters lowercase: `"w"`, `"Tab"`, `"ArrowLeft"`, `"\\"`, `"1"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct KeyChord {
    pub key: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

impl KeyChord {
    /// `key` with no modifiers.
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: normal_key(&key.into()),
            ..Self::default()
        }
    }

    /// Parse `"Mod+Shift+T"`: modifiers `Ctrl`, `Shift`, `Alt`, `Meta`
    /// (`Cmd`), and `Mod`, the platform's primary one (`Cmd` on a Mac,
    /// `Ctrl` elsewhere: `mac` says which).
    #[must_use]
    pub fn parse(text: &str, mac: bool) -> Option<Self> {
        let mut chord = Self::default();
        let mut parts: Vec<&str> = text.split('+').collect();
        // `Mod++` names the plus key.
        if text.ends_with("++") {
            parts.retain(|p| !p.is_empty());
            parts.push("+");
        }
        let (key, mods) = parts.split_last()?;
        for m in mods {
            match m.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => chord.ctrl = true,
                "shift" => chord.shift = true,
                "alt" | "option" => chord.alt = true,
                "meta" | "cmd" | "super" => chord.meta = true,
                "mod" if mac => chord.meta = true,
                "mod" => chord.ctrl = true,
                _ => return None,
            }
        }
        if key.is_empty() {
            return None;
        }
        chord.key = normal_key(key);
        Some(chord)
    }

    /// Whether the platform's primary modifier is held alone (or with
    /// shift): `Ctrl`, or `Cmd` on a Mac.
    fn primary(&self) -> bool {
        (self.ctrl || self.meta) && !self.alt
    }
}

/// Letters lowercase (so `Shift+T` and `Mod+Shift+t` are one chord).
fn normal_key(key: &str) -> String {
    if key.chars().count() == 1 {
        key.to_lowercase()
    } else {
        key.to_owned()
    }
}

/// Which keys ask for which commands. The app implements it (or uses
/// [`Bindings`], or [`DefaultKeymap`]).
pub trait Keymap {
    /// The command `chord` asks for, if any.
    fn command(&self, chord: &KeyChord) -> Option<Command>;
}

/// The keys an app gets when it says nothing: browser-tab habits.
///
/// | keys | command |
/// |---|---|
/// | Mod+W | close tab |
/// | Mod+Shift+T | reopen closed tab |
/// | Ctrl+Tab / Ctrl+Shift+Tab | next / previous tab |
/// | Mod+1 … Mod+9 | go to tab (9: the last) |
/// | Mod+\\ / Mod+Shift+\\ | split right / down |
/// | Mod+Alt+Arrow | focus the stack that way |
/// | Mod+Shift+Enter | zoom the stack |
/// | Mod+Shift+E | show all tabs (the switcher) |
/// | Mod+Shift+L | show or hide the sidebar |
/// | Mod+K | open anything (the command palette) |
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DefaultKeymap;

impl Keymap for DefaultKeymap {
    fn command(&self, c: &KeyChord) -> Option<Command> {
        let k = c.key.as_str();
        if (c.ctrl || c.meta) && c.alt {
            return match k {
                "ArrowLeft" => Some(Command::Focus(Direction::Left)),
                "ArrowRight" => Some(Command::Focus(Direction::Right)),
                "ArrowUp" => Some(Command::Focus(Direction::Up)),
                "ArrowDown" => Some(Command::Focus(Direction::Down)),
                _ => None,
            };
        }
        if c.ctrl && k == "Tab" {
            return Some(if c.shift {
                Command::PrevTab
            } else {
                Command::NextTab
            });
        }
        if !c.primary() {
            return None;
        }
        match (k, c.shift) {
            ("w", false) => Some(Command::CloseTab),
            ("t", true) => Some(Command::ReopenClosed),
            ("\\", false) => Some(Command::Split(Side::Right)),
            ("\\" | "|", true) => Some(Command::Split(Side::Bottom)),
            ("Enter", true) => Some(Command::ToggleZoom),
            ("e", true) => Some(Command::OpenSwitcher),
            ("l", true) => Some(Command::ToggleSidebar),
            ("k", false) => Some(Command::OpenPalette),
            (digit, false) => match digit.parse::<usize>() {
                Ok(9) => Some(Command::GotoTab(usize::MAX)),
                Ok(n @ 1..=8) => Some(Command::GotoTab(n - 1)),
                _ => None,
            },
            _ => None,
        }
    }
}

/// A table of bindings the app writes (`"Mod+W" => CloseTab`), falling
/// back to another keymap for the keys it does not name.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bindings {
    pub keys: Vec<(KeyChord, Command)>,
    /// Whether [`DefaultKeymap`] answers what the table does not.
    pub with_defaults: bool,
}

impl Bindings {
    /// An empty table over the defaults.
    #[must_use]
    pub fn over_defaults() -> Self {
        Self {
            keys: Vec::new(),
            with_defaults: true,
        }
    }

    /// Bind `chord` (as [`KeyChord::parse`] reads it) to `command`.
    #[must_use]
    pub fn bind(mut self, chord: &str, mac: bool, command: Command) -> Self {
        if let Some(chord) = KeyChord::parse(chord, mac) {
            self.keys.retain(|(c, _)| *c != chord);
            self.keys.push((chord, command));
        }
        self
    }
}

impl Keymap for Bindings {
    fn command(&self, chord: &KeyChord) -> Option<Command> {
        self.keys
            .iter()
            .find(|(c, _)| c == chord)
            .map(|(_, cmd)| *cmd)
            .or_else(|| {
                self.with_defaults
                    .then(|| DefaultKeymap.command(chord))
                    .flatten()
            })
    }
}
