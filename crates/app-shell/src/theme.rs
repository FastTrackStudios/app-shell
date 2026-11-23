//! The shell's colours and sizes, as CSS custom properties.
//!
//! Every component reads `var(--as-…)`; the shell's root sets them from a
//! [`Theme`] (dark by default). An app restyles the shell by passing its
//! own [`Theme`], or by setting the same variables on an ancestor and
//! passing [`Theme::inherit`]. No stylesheet is needed: Blitz and the
//! browser read the variables from inline styles alike.

/// The variables, with the dark defaults.
pub const VARIABLES: &[(&str, &str)] = &[
    // Surfaces.
    ("--as-bg", "#18181b"),
    ("--as-surface", "#1f1f23"),
    ("--as-raised", "#27272a"),
    ("--as-overlay", "rgba(0, 0, 0, 0.55)"),
    // Lines and text.
    ("--as-border", "#3f3f46"),
    ("--as-text", "#e4e4e7"),
    ("--as-muted", "#a1a1aa"),
    // The accent: the active tab's mark, a drop's highlight.
    ("--as-accent", "#6366f1"),
    ("--as-accent-soft", "rgba(99, 102, 241, 0.28)"),
    // Text on the accent (a badge, a primary button).
    ("--as-accent-text", "#ffffff"),
    ("--as-danger", "#ef4444"),
    // What lifts off the page: a button over it, and what floats over
    // everything (a menu, a popover, a floating pane).
    ("--as-shadow-lift", "0 6px 20px rgba(0, 0, 0, 0.35)"),
    ("--as-shadow-float", "0 16px 44px rgba(0, 0, 0, 0.55)"),
    // Sizes.
    (
        "--as-font",
        "system-ui, -apple-system, 'Segoe UI', sans-serif",
    ),
    ("--as-font-size", "13px"),
    ("--as-tab-h", "34px"),
    ("--as-radius", "6px"),
    // The top bar: the window's title strip, the traffic lights centred
    // in it (`TOP_BAR_H` in code: the native side places macOS's own
    // lights in a row this tall).
    ("--as-topbar-h", "44px"),
    ("--as-topbar-bg", "#18181b"),
    // The sidebar: a panel a shade apart, translucent where the renderer
    // blurs what is behind it (a browser; Blitz draws the colour).
    ("--as-sidebar-bg", "rgba(28, 28, 32, 0.92)"),
    ("--as-sidebar-blur", "blur(24px) saturate(160%)"),
    ("--as-selected", "rgba(255, 255, 255, 0.08)"),
    ("--as-selected-text", "#ffffff"),
    ("--as-heading", "#71717a"),
    // The traffic lights, and their colour in a window that is not the
    // key window.
    ("--as-light-close", "#ff5f57"),
    ("--as-light-min", "#febc2e"),
    ("--as-light-zoom", "#28c840"),
    ("--as-light-off", "#4a4a50"),
    ("--as-search-bg", "rgba(255, 255, 255, 0.06)"),
    // A row under the pointer, pressed, and the keyboard's ring.
    ("--as-hover", "rgba(255, 255, 255, 0.05)"),
    ("--as-pressed", "rgba(255, 255, 255, 0.1)"),
    ("--as-focus", "var(--as-accent)"),
    // The scrollbars' thumb, and under the pointer.
    ("--as-scrollbar", "rgba(255, 255, 255, 0.16)"),
    ("--as-scrollbar-hover", "rgba(255, 255, 255, 0.3)"),
];

/// The light theme's colours (the sizes are the defaults').
pub const LIGHT: &[(&str, &str)] = &[
    ("--as-bg", "#fafafa"),
    ("--as-surface", "#ffffff"),
    ("--as-raised", "#f4f4f5"),
    ("--as-overlay", "rgba(24, 24, 27, 0.35)"),
    ("--as-border", "#d4d4d8"),
    ("--as-text", "#18181b"),
    ("--as-muted", "#71717a"),
    ("--as-accent", "#4f46e5"),
    ("--as-accent-soft", "rgba(79, 70, 229, 0.18)"),
    ("--as-danger", "#dc2626"),
    ("--as-shadow-lift", "0 6px 20px rgba(0, 0, 0, 0.12)"),
    ("--as-shadow-float", "0 16px 44px rgba(0, 0, 0, 0.18)"),
    ("--as-topbar-bg", "#fafafa"),
    ("--as-sidebar-bg", "rgba(244, 244, 245, 0.92)"),
    ("--as-selected", "rgba(0, 0, 0, 0.07)"),
    ("--as-selected-text", "#09090b"),
    ("--as-heading", "#a1a1aa"),
    ("--as-light-off", "#d4d4d8"),
    ("--as-search-bg", "rgba(0, 0, 0, 0.05)"),
    ("--as-hover", "rgba(0, 0, 0, 0.04)"),
    ("--as-pressed", "rgba(0, 0, 0, 0.08)"),
    ("--as-scrollbar", "rgba(0, 0, 0, 0.2)"),
    ("--as-scrollbar-hover", "rgba(0, 0, 0, 0.35)"),
];

/// The values the shell's root sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    vars: Vec<(String, String)>,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// The defaults ([`VARIABLES`]).
    #[must_use]
    pub fn dark() -> Self {
        Self {
            vars: VARIABLES
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        }
    }

    /// The defaults with [`LIGHT`]'s colours.
    #[must_use]
    pub fn light() -> Self {
        LIGHT.iter().fold(Self::dark(), |t, (k, v)| t.set(k, *v))
    }

    /// Sets nothing: the variables come from an ancestor the app styles.
    #[must_use]
    pub const fn inherit() -> Self {
        Self { vars: Vec::new() }
    }

    /// `name` (`"--as-accent"`) set to `value`.
    #[must_use]
    pub fn set(mut self, name: &str, value: impl Into<String>) -> Self {
        let value = value.into();
        match self.vars.iter_mut().find(|(k, _)| k == name) {
            Some(slot) => slot.1 = value,
            None => self.vars.push((name.to_owned(), value)),
        }
        self
    }

    /// The value of `name`, if this theme sets it.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.vars
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// As declarations for a `style` attribute: `--as-bg: #18181b; …`.
    #[must_use]
    pub fn style(&self) -> String {
        self.vars
            .iter()
            .map(|(k, v)| format!("{k}: {v};"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_theme_writes_its_variables_as_a_style() {
        let t = Theme::dark().set("--as-accent", "red");
        assert!(t.style().contains("--as-accent: red;"));
        assert!(t.style().contains("--as-bg: #18181b;"));
        assert_eq!(Theme::inherit().style(), "");
    }

    #[test]
    fn light_keeps_the_sizes_and_changes_the_colours() {
        let t = Theme::light();
        assert_eq!(t.get("--as-bg"), Some("#fafafa"));
        assert_eq!(t.get("--as-tab-h"), Some("34px"));
    }
}
