//! The theme the extensions' chrome draws with: app-shell's
//! ([`app_shell::Theme`]), as the CSS variables its root sets
//! (`var(--as-…)`), so the chrome follows whatever theme the shell has
//! without being told of it.

/// The chrome's colours and face, each a `var(--as-…)` of app-shell's
/// theme, by the names the chrome's components draw with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChromeTheme {
    /// Under everything.
    pub desk: &'static str,
    /// The sidebars, the drawers, the menus.
    pub sheet: &'static str,
    /// A window's title bar.
    pub title_bar: &'static str,
    /// A pill, a raised control.
    pub pill: &'static str,
    /// An edge.
    pub rule: &'static str,
    /// The current tab's fill, a pressed row's.
    pub active: &'static str,
    pub ink: &'static str,
    pub ink_2: &'static str,
    /// The quietest text.
    pub ink_3: &'static str,
    pub accent: &'static str,
    /// Text on the accent.
    pub accent_ink: &'static str,
    pub danger: &'static str,
    /// Over the page while a drawer or a menu is open.
    pub scrim: &'static str,
    /// The accent, faint.
    pub accent_soft: &'static str,
    /// A pill or a button over the page.
    pub shadow_lift: &'static str,
    /// What floats over everything.
    pub shadow_float: &'static str,
    /// The face, a CSS stack.
    pub font: &'static str,
}

/// app-shell's theme, as the chrome reads it.
pub const THEME: ChromeTheme = ChromeTheme {
    desk: "var(--as-bg)",
    sheet: "var(--as-surface)",
    title_bar: "var(--as-topbar-bg)",
    pill: "var(--as-raised)",
    rule: "var(--as-border)",
    active: "var(--as-selected)",
    ink: "var(--as-text)",
    ink_2: "var(--as-muted)",
    ink_3: "var(--as-heading)",
    accent: "var(--as-accent)",
    accent_ink: "var(--as-accent-text)",
    danger: "var(--as-danger)",
    scrim: "var(--as-overlay)",
    accent_soft: "var(--as-accent-soft)",
    shadow_lift: "var(--as-shadow-lift)",
    shadow_float: "var(--as-shadow-float)",
    font: "var(--as-font)",
};

/// The chrome's theme: app-shell's variables, which the shell's root sets.
#[must_use]
pub const fn shell_theme() -> ChromeTheme {
    THEME
}

/// A theme token's name as an extension writes it (`primary`, `accent`,
/// `destructive`, `muted-foreground`) as app-shell's CSS variable;
/// `None` for one app-shell has no counterpart of.
#[must_use]
pub fn token_var(token: &str) -> Option<&'static str> {
    Some(match token {
        "primary" | "accent" | "ring" => "var(--as-accent)",
        "primary-foreground" | "accent-foreground" | "destructive-foreground" => {
            "var(--as-accent-text)"
        }
        "secondary" | "muted" => "var(--as-raised)",
        "secondary-foreground" | "foreground" | "card-foreground" | "popover-foreground" => {
            "var(--as-text)"
        }
        "muted-foreground" => "var(--as-muted)",
        "destructive" | "danger" => "var(--as-danger)",
        "border" | "input" => "var(--as-border)",
        "background" => "var(--as-bg)",
        "card" | "popover" => "var(--as-surface)",
        _ => return None,
    })
}
