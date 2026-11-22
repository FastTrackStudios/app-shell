//! The tray's model, as plain data the app builds and changes: the menu
//! (items, separators, submenus, checkmarks, disabled rows, keyboard
//! equivalents, a second line of status), the icon and its status badge,
//! the tooltip, and what the platform tells the app back ([`TrayEvent`]).
//! Each platform's tray (`linux.rs`, `apple.rs`) draws this; nothing
//! here touches one.

/// A row of the tray's menu.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuEntry {
    Item(MenuItem),
    Separator,
    Submenu {
        label: String,
        enabled: bool,
        items: Vec<MenuEntry>,
    },
}

/// A clickable row.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuItem {
    /// What the app hears when it is clicked ([`TrayEvent::Menu`]).
    pub id: String,
    pub label: String,
    /// A second, quieter line (status: "Synced 2 minutes ago"). Where the
    /// platform has no second line it follows the label after an em dash.
    pub detail: Option<String>,
    pub enabled: bool,
    /// `Some` makes it a checkmark row.
    pub checked: Option<bool>,
    /// A keyboard equivalent, `Mod+Q` style ([`Accelerator`]).
    pub accelerator: Option<Accelerator>,
}

impl MenuItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            detail: None,
            enabled: true,
            checked: None,
            accelerator: None,
        }
    }

    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub const fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// A keyboard equivalent written `Mod+Shift+Q` (see [`Accelerator::parse`]);
    /// one that does not parse is left off.
    #[must_use]
    pub fn accelerator(mut self, keys: &str) -> Self {
        self.accelerator = Accelerator::parse(keys);
        self
    }

    /// The label and the detail on one line, for a platform with no
    /// second line.
    #[must_use]
    pub fn one_line(&self) -> String {
        match &self.detail {
            Some(d) if !d.is_empty() => format!("{} — {d}", self.label),
            _ => self.label.clone(),
        }
    }
}

impl MenuEntry {
    /// A clickable row.
    pub fn item(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::Item(MenuItem::new(id, label))
    }

    /// A submenu of `items`.
    pub fn submenu(label: impl Into<String>, items: Vec<Self>) -> Self {
        Self::Submenu {
            label: label.into(),
            enabled: true,
            items,
        }
    }
}

/// The tray's menu.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TrayMenu {
    pub entries: Vec<MenuEntry>,
}

impl TrayMenu {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    #[must_use]
    pub fn with(mut self, entry: MenuEntry) -> Self {
        self.entries.push(entry);
        self
    }

    #[must_use]
    pub fn item(self, item: MenuItem) -> Self {
        self.with(MenuEntry::Item(item))
    }

    #[must_use]
    pub fn separator(self) -> Self {
        self.with(MenuEntry::Separator)
    }

    /// The row `id`, wherever it is (submenus included).
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&MenuItem> {
        fn walk<'a>(entries: &'a [MenuEntry], id: &str) -> Option<&'a MenuItem> {
            entries.iter().find_map(|e| match e {
                MenuEntry::Item(i) if i.id == id => Some(i),
                MenuEntry::Submenu { items, .. } => walk(items, id),
                _ => None,
            })
        }
        walk(&self.entries, id)
    }

    /// Change the row `id` in place (a checkmark flipped, a detail line
    /// updated); whether there was one.
    pub fn update(&mut self, id: &str, f: impl FnOnce(&mut MenuItem)) -> bool {
        fn walk<'a>(entries: &'a mut [MenuEntry], id: &str) -> Option<&'a mut MenuItem> {
            entries.iter_mut().find_map(|e| match e {
                MenuEntry::Item(i) if i.id == id => Some(i),
                MenuEntry::Submenu { items, .. } => walk(items, id),
                _ => None,
            })
        }
        walk(&mut self.entries, id).map(f).is_some()
    }

    /// Every row's id, depth first: what a platform's menu is diffed by.
    #[must_use]
    pub fn ids(&self) -> Vec<String> {
        fn walk(entries: &[MenuEntry], out: &mut Vec<String>) {
            for e in entries {
                match e {
                    MenuEntry::Item(i) => out.push(i.id.clone()),
                    MenuEntry::Submenu { items, .. } => walk(items, out),
                    MenuEntry::Separator => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.entries, &mut out);
        out
    }

    /// Ids used twice: a click on either would be ambiguous.
    #[must_use]
    pub fn duplicate_ids(&self) -> Vec<String> {
        let mut seen = std::collections::BTreeSet::new();
        let mut dup = std::collections::BTreeSet::new();
        for id in self.ids() {
            if !seen.insert(id.clone()) {
                dup.insert(id);
            }
        }
        dup.into_iter().collect()
    }
}

/// A keyboard equivalent: modifiers and a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accelerator {
    /// The platform's primary modifier (⌘ on a Mac, Ctrl elsewhere).
    pub primary: bool,
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
    /// The key, uppercase for letters (`Q`, `Comma`, `F5`).
    pub key: String,
}

impl Accelerator {
    /// Parse `Mod+Shift+Q`: modifiers `Mod`/`Cmd`/`CmdOrCtrl` (the primary),
    /// `Shift`, `Alt`/`Option`, `Ctrl`; a single character or a named key.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut a = Self {
            primary: false,
            shift: false,
            alt: false,
            ctrl: false,
            key: String::new(),
        };
        let parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let (key, mods) = parts.split_last()?;
        for m in mods {
            match m.to_ascii_lowercase().as_str() {
                "mod" | "cmd" | "cmdorctrl" | "commandorcontrol" | "super" => a.primary = true,
                "shift" => a.shift = true,
                "alt" | "option" => a.alt = true,
                "ctrl" | "control" => a.ctrl = true,
                _ => return None,
            }
        }
        if key.is_empty() {
            return None;
        }
        a.key = if key.chars().count() == 1 {
            key.to_uppercase()
        } else {
            let mut c = key.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect())
                .unwrap_or_default()
        };
        Some(a)
    }

    /// As muda reads one (`CmdOrCtrl+Shift+Q`).
    #[must_use]
    pub fn to_muda(&self) -> String {
        let mut parts = Vec::new();
        if self.primary {
            parts.push("CmdOrCtrl");
        }
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        let mut s = parts.join("+");
        if !s.is_empty() {
            s.push('+');
        }
        s.push_str(&self.key);
        s
    }

    /// As a person reads it on this platform (`⇧⌘Q`, `Ctrl+Shift+Q`):
    /// what a tray without native equivalents shows beside the label.
    #[must_use]
    pub fn display(&self, mac: bool) -> String {
        if mac {
            let mut s = String::new();
            if self.ctrl {
                s.push('⌃');
            }
            if self.alt {
                s.push('⌥');
            }
            if self.shift {
                s.push('⇧');
            }
            if self.primary {
                s.push('⌘');
            }
            s.push_str(&self.key);
            s
        } else {
            let mut parts: Vec<&str> = Vec::new();
            if self.primary || self.ctrl {
                parts.push("Ctrl");
            }
            if self.alt {
                parts.push("Alt");
            }
            if self.shift {
                parts.push("Shift");
            }
            parts.push(&self.key);
            parts.join("+")
        }
    }
}

/// What the app's work is doing: a badge over the icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrayStatus {
    /// All is well: no badge.
    #[default]
    Ok,
    Syncing,
    Warning,
    Error,
}

impl TrayStatus {
    /// The badge's colour (RGBA), if it has one.
    #[must_use]
    pub const fn badge(self) -> Option<[u8; 4]> {
        match self {
            Self::Ok => None,
            Self::Syncing => Some([0x3b, 0x82, 0xf6, 0xff]),
            Self::Warning => Some([0xf5, 0x9e, 0x0b, 0xff]),
            Self::Error => Some([0xef, 0x44, 0x44, 0xff]),
        }
    }

    /// Its name, for a tooltip's second line and the StatusNotifierItem
    /// status (`Active`, `NeedsAttention`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ok => "Up to date",
            Self::Syncing => "Syncing",
            Self::Warning => "Needs attention",
            Self::Error => "Error",
        }
    }
}

/// The tray's picture: RGBA pixels, square.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayIcon {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// A template image (macOS): drawn in the menu bar's own colour, light
    /// or dark, from the alpha alone. Elsewhere drawn as it is.
    pub template: bool,
}

impl TrayIcon {
    /// From RGBA pixels; `None` when the buffer is the wrong size.
    #[must_use]
    pub fn from_rgba(rgba: Vec<u8>, width: u32, height: u32) -> Option<Self> {
        let want = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?;
        (rgba.len() == want && width > 0 && height > 0).then_some(Self {
            rgba,
            width,
            height,
            template: false,
        })
    }

    /// Mark it a template image (macOS).
    #[must_use]
    pub const fn template(mut self, template: bool) -> Self {
        self.template = template;
        self
    }

    /// A plain disc in `rgba`, `size` square: a placeholder icon.
    #[must_use]
    pub fn disc(size: u32, rgba: [u8; 4]) -> Self {
        let mut px = vec![0u8; (size * size * 4) as usize];
        let c = f64::from(size) / 2.0;
        let r = c - 1.0;
        for y in 0..size {
            for x in 0..size {
                let (dx, dy) = (f64::from(x) + 0.5 - c, f64::from(y) + 0.5 - c);
                if dx.hypot(dy) <= r {
                    let i = ((y * size + x) * 4) as usize;
                    px[i..i + 4].copy_from_slice(&rgba);
                }
            }
        }
        Self {
            rgba: px,
            width: size,
            height: size,
            template: false,
        }
    }

    /// The icon with `status`'s badge, a dot at its bottom right with a
    /// clear ring around it, so it reads over any icon. A template icon
    /// with a badge is no longer a template (the badge has a colour).
    #[must_use]
    pub fn with_badge(&self, status: TrayStatus) -> Self {
        let Some(colour) = status.badge() else {
            return self.clone();
        };
        let mut out = self.clone();
        out.template = false;
        let (w, h) = (f64::from(self.width), f64::from(self.height));
        let r = w.min(h) * 0.28;
        let (cx, cy) = (w - r - 0.5, h - r - 0.5);
        let ring = r + w.min(h) * 0.08;
        for y in 0..self.height {
            for x in 0..self.width {
                let d = (f64::from(x) + 0.5 - cx).hypot(f64::from(y) + 0.5 - cy);
                let i = ((y * self.width + x) * 4) as usize;
                if d <= r {
                    out.rgba[i..i + 4].copy_from_slice(&colour);
                } else if d <= ring {
                    out.rgba[i + 3] = 0;
                }
            }
        }
        out
    }

    /// ARGB32 in network byte order, as a StatusNotifierItem carries it.
    #[must_use]
    pub fn to_argb(&self) -> Vec<u8> {
        self.rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[3], p[0], p[1], p[2]])
            .collect()
    }
}

/// What the tray tells the app.
#[derive(Debug, Clone, PartialEq)]
pub enum TrayEvent {
    /// The menu row `id` was clicked (a checkmark row: its state is the
    /// app's to flip, with [`TrayMenu::update`]).
    Menu(String),
    /// The icon itself was clicked (where the platform tells: a left click
    /// on Linux and Windows; on a Mac a click opens the menu instead
    /// unless the app has no menu), at screen point `(x, y)`.
    Activate { x: i32, y: i32 },
    /// A window opened from the tray closed (`key`).
    WindowClosed(String),
}

/// A rectangle on screen, physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Where a quick panel `size` big goes for a tray icon at `icon` on a
/// screen `screen`: centred under the icon (over it when the bar is at the
/// bottom, as on Windows and some Linux panels), kept `margin` inside the
/// screen.
#[must_use]
pub fn panel_position(
    icon: ScreenRect,
    size: (i32, i32),
    screen: ScreenRect,
    margin: i32,
) -> (i32, i32) {
    let (w, h) = size;
    let centre = icon.x + icon.w / 2;
    let mut x = centre - w / 2;
    let below = icon.y + icon.h + margin;
    let bar_at_bottom = icon.y + icon.h / 2 > screen.y + screen.h / 2;
    let mut y = if bar_at_bottom {
        icon.y - h - margin
    } else {
        below
    };
    let (min_x, max_x) = (screen.x + margin, screen.x + screen.w - w - margin);
    x = x.clamp(min_x, max_x.max(min_x));
    let (min_y, max_y) = (screen.y + margin, screen.y + screen.h - h - margin);
    y = y.clamp(min_y, max_y.max(min_y));
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu() -> TrayMenu {
        TrayMenu::new()
            .item(MenuItem::new("open", "Open Drive").accelerator("Mod+O"))
            .item(
                MenuItem::new("status", "Synced")
                    .detail("2 minutes ago")
                    .enabled(false),
            )
            .separator()
            .with(MenuEntry::submenu(
                "Pause syncing",
                vec![
                    MenuEntry::Item(MenuItem::new("pause.1h", "For an hour")),
                    MenuEntry::Item(MenuItem::new("pause.on", "Until resumed").checked(false)),
                ],
            ))
            .separator()
            .item(MenuItem::new("quit", "Quit").accelerator("Mod+Q"))
    }

    #[test]
    fn a_menu_finds_and_updates_rows_in_submenus() {
        let mut m = menu();
        assert_eq!(m.ids(), ["open", "status", "pause.1h", "pause.on", "quit"]);
        assert_eq!(m.find("pause.on").and_then(|i| i.checked), Some(false));
        assert!(m.update("pause.on", |i| i.checked = Some(true)));
        assert_eq!(m.find("pause.on").and_then(|i| i.checked), Some(true));
        assert!(!m.update("nope", |_| {}));
        assert!(m.duplicate_ids().is_empty());
        let twice = m.clone().item(MenuItem::new("open", "Again"));
        assert_eq!(twice.duplicate_ids(), ["open"]);
    }

    #[test]
    fn a_detail_line_follows_the_label_where_there_is_no_second_line() {
        let m = menu();
        assert_eq!(
            m.find("status").map(MenuItem::one_line).as_deref(),
            Some("Synced — 2 minutes ago")
        );
        assert_eq!(
            m.find("open").map(MenuItem::one_line).as_deref(),
            Some("Open Drive")
        );
    }

    #[test]
    fn accelerators_parse_and_print_per_platform() {
        let a = Accelerator::parse("Mod+Shift+q").expect("parses");
        assert!(a.primary && a.shift && !a.alt);
        assert_eq!(a.key, "Q");
        assert_eq!(a.to_muda(), "CmdOrCtrl+Shift+Q");
        assert_eq!(a.display(true), "⇧⌘Q");
        assert_eq!(a.display(false), "Ctrl+Shift+Q");
        assert_eq!(
            Accelerator::parse("Alt+comma").map(|a| a.key).as_deref(),
            Some("Comma")
        );
        assert!(Accelerator::parse("Hyper+Q").is_none());
        assert!(Accelerator::parse("").is_none());
        assert_eq!(
            Accelerator::parse("F5").map(|a| a.to_muda()).as_deref(),
            Some("F5")
        );
    }

    #[test]
    fn a_badge_marks_the_icons_corner_and_ok_leaves_it() {
        let icon = TrayIcon::disc(32, [255, 255, 255, 255]).template(true);
        assert_eq!(icon.with_badge(TrayStatus::Ok), icon);
        let err = icon.with_badge(TrayStatus::Error);
        assert!(!err.template, "a coloured badge is not a template");
        let px = |i: &TrayIcon, x: u32, y: u32| {
            let o = ((y * i.width + x) * 4) as usize;
            [i.rgba[o], i.rgba[o + 1], i.rgba[o + 2], i.rgba[o + 3]]
        };
        assert_eq!(px(&err, 26, 26), [0xef, 0x44, 0x44, 0xff], "the dot");
        assert_eq!(px(&err, 8, 8), [255, 255, 255, 255], "the rest as it was");
        assert_eq!(px(&err, 13, 22)[3], 0, "a clear ring around the dot");
        let sync = icon.with_badge(TrayStatus::Syncing);
        assert_eq!(px(&sync, 26, 26), [0x3b, 0x82, 0xf6, 0xff]);
    }

    #[test]
    fn an_icon_checks_its_size_and_converts_to_argb() {
        assert!(TrayIcon::from_rgba(vec![0; 15], 2, 2).is_none());
        let i = TrayIcon::from_rgba(vec![1, 2, 3, 4], 1, 1).expect("1x1");
        assert_eq!(i.to_argb(), vec![4, 1, 2, 3]);
    }

    #[test]
    fn a_panel_hangs_under_its_icon_inside_the_screen() {
        let screen = ScreenRect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        };
        let icon = ScreenRect {
            x: 1500,
            y: 0,
            w: 24,
            h: 24,
        };
        assert_eq!(panel_position(icon, (320, 400), screen, 6), (1352, 30));
        // At the screen's right edge it moves in.
        let edge = ScreenRect {
            x: 1900,
            y: 0,
            w: 20,
            h: 24,
        };
        assert_eq!(panel_position(edge, (320, 400), screen, 6), (1594, 30));
        // A bar at the bottom: the panel stands over the icon.
        let low = ScreenRect {
            x: 1500,
            y: 1050,
            w: 24,
            h: 30,
        };
        assert_eq!(panel_position(low, (320, 400), screen, 6), (1352, 644));
    }
}
