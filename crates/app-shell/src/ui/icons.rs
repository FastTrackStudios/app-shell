//! Icons as path data: Lucide's 24×24 stroked outlines (ISC licence,
//! <https://lucide.dev>), drawn with SVG presentation attributes so Blitz
//! colours them as a browser does. An app's tab kinds bring their own
//! [`Icon`]s the same way.

use dioxus::prelude::*;

/// An icon: the `d` of each path in a 24×24 box, stroked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Icon(pub &'static [&'static str]);

pub const X: Icon = Icon(&["M18 6 6 18", "m6 6 12 12"]);
pub const PLUS: Icon = Icon(&["M5 12h14", "M12 5v14"]);
pub const MINUS: Icon = Icon(&["M5 12h14"]);
pub const SQUARE: Icon =
    Icon(&["M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z"]);
pub const LAYERS: Icon = Icon(&[
    "m12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z",
    "m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65",
    "m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65",
]);
pub const COLUMNS: Icon = Icon(&[
    "M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z",
    "M12 3v18",
]);
pub const ROWS: Icon = Icon(&[
    "M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z",
    "M3 12h18",
]);
pub const EXTERNAL: Icon = Icon(&[
    "M15 3h6v6",
    "M10 14 21 3",
    "M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6",
]);
pub const FILE: Icon = Icon(&[
    "M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z",
    "M14 2v4a2 2 0 0 0 2 2h4",
]);
pub const PIN: Icon = Icon(&["M12 17v5", "M9 3h6l-1 7 3 4H7l3-4z"]);
pub const MAXIMIZE: Icon = Icon(&[
    "M8 3H5a2 2 0 0 0-2 2v3",
    "M21 8V5a2 2 0 0 0-2-2h-3",
    "M3 16v3a2 2 0 0 0 2 2h3",
    "M16 21h3a2 2 0 0 0 2-2v-3",
]);
pub const PANEL_LEFT: Icon = Icon(&[
    "M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z",
    "M9 3v18",
]);
pub const PANEL_RIGHT: Icon = Icon(&[
    "M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z",
    "M15 3v18",
]);
pub const PANEL_BOTTOM: Icon = Icon(&[
    "M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z",
    "M3 15h18",
]);
pub const MORE: Icon = Icon(&["M5 12h.01", "M12 12h.01", "M19 12h.01"]);

/// `icon` drawn `size` points square in the current text colour (or
/// `color`).
#[component]
pub fn IconView(icon: Icon, #[props(default = 14.0)] size: f32, color: Option<String>) -> Element {
    let stroke = color.unwrap_or_else(|| "currentColor".to_owned());
    // Sized in style too: a stylesheet's `svg { height: auto }` beats the
    // attributes.
    rsx! {
        svg {
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "{stroke}",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            style: "width: {size}px; height: {size}px; flex: none; display: block;",
            for d in icon.0.iter() {
                path { d: "{d}" }
            }
        }
    }
}

pub const CHEVRON_DOWN: Icon = Icon(&["m6 9 6 6 6-6"]);
pub const CHEVRON_RIGHT: Icon = Icon(&["m9 18 6-6-6-6"]);
pub const CHEVRONS_UP_DOWN: Icon = Icon(&["m7 15 5 5 5-5", "m7 9 5-5 5 5"]);
pub const ARROW_LEFT: Icon = Icon(&["m12 19-7-7 7-7", "M19 12H5"]);
pub const ARROW_RIGHT: Icon = Icon(&["M5 12h14", "m12 5 7 7-7 7"]);
pub const MENU: Icon = Icon(&["M4 12h16", "M4 6h16", "M4 18h16"]);
pub const SEARCH: Icon = Icon(&["M19 11a8 8 0 1 1-16 0 8 8 0 0 1 16 0z", "m21 21-4.3-4.3"]);
pub const GRID: Icon = Icon(&[
    "M3 3h7v7H3z",
    "M14 3h7v7h-7z",
    "M14 14h7v7h-7z",
    "M3 14h7v7H3z",
]);
pub const LIST: Icon = Icon(&[
    "M8 6h13",
    "M8 12h13",
    "M8 18h13",
    "M3 6h.01",
    "M3 12h.01",
    "M3 18h.01",
]);
pub const HOME: Icon = Icon(&[
    "M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8",
    "M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
]);
pub const FOLDER: Icon = Icon(&[
    "M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z",
]);
pub const TAG: Icon = Icon(&[
    "M12.586 2.586A2 2 0 0 0 11.172 2H4a2 2 0 0 0-2 2v7.172a2 2 0 0 0 .586 1.414l8.704 8.704a2.426 2.426 0 0 0 3.42 0l6.58-6.58a2.426 2.426 0 0 0 0-3.42z",
    "M7.5 7.5h.01",
]);
pub const SETTINGS: Icon = Icon(&[
    "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z",
    "M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0z",
]);

/// What [`named_icon`] draws for a name it does not know: a puzzle piece.
pub const PUZZLE: Icon = Icon(&[
    "M15.39 4.39a1 1 0 0 0 1.68-.474 2.5 2.5 0 1 1 3.014 3.015 1 1 0 0 0-.474 1.68l1.683 1.682a2.414 2.414 0 0 1 0 3.414L19.61 15.39a1 1 0 0 1-1.68-.474 2.5 2.5 0 1 0-3.014 3.015 1 1 0 0 1 .474 1.68l-1.683 1.682a2.414 2.414 0 0 1-3.414 0L8.61 19.61a1 1 0 0 0-1.68.474 2.5 2.5 0 1 1-3.014-3.015 1 1 0 0 0 .474-1.68l-1.683-1.682a2.414 2.414 0 0 1 0-3.414L4.39 8.61a1 1 0 0 1 1.68.474 2.5 2.5 0 1 0 3.014-3.015 1 1 0 0 1-.474-1.68l1.683-1.682a2.414 2.414 0 0 1 3.414 0z",
]);

/// The icon called `name`: Lucide's names (`"file-text"`, `"house"`,
/// `"git-pull-request"`), all of them with feature `icons`, else the
/// shell's own few (`"x"`, `"plus"`, `"search"`, `"settings"`, …). `None`
/// for a name it does not have. For a name from data (a manifest, a
/// layout file).
#[must_use]
pub fn named(name: &str) -> Option<Icon> {
    let name = match name {
        "home" => "house",
        "indent-decrease" => "list-indent-decrease",
        "indent-increase" => "list-indent-increase",
        "external" => "external-link",
        other => other,
    };
    #[cfg(feature = "icons")]
    if let Some(icon) = super::lucide::lucide(name) {
        return Some(icon);
    }
    Some(match name {
        "x" => X,
        "plus" => PLUS,
        "minus" => MINUS,
        "square" => SQUARE,
        "layers" => LAYERS,
        "columns" | "columns-2" => COLUMNS,
        "rows" | "rows-2" => ROWS,
        "external-link" => EXTERNAL,
        "file" => FILE,
        "pin" => PIN,
        "maximize" => MAXIMIZE,
        "panel-left" => PANEL_LEFT,
        "panel-right" => PANEL_RIGHT,
        "panel-bottom" => PANEL_BOTTOM,
        "ellipsis" | "more" => MORE,
        "chevron-down" => CHEVRON_DOWN,
        "chevron-right" => CHEVRON_RIGHT,
        "chevrons-up-down" => CHEVRONS_UP_DOWN,
        "arrow-left" => ARROW_LEFT,
        "arrow-right" => ARROW_RIGHT,
        "menu" => MENU,
        "search" => SEARCH,
        "grid" | "layout-grid" => GRID,
        "list" => LIST,
        "house" => HOME,
        "folder" => FOLDER,
        "tag" => TAG,
        "settings" => SETTINGS,
        "puzzle" => PUZZLE,
        _ => return None,
    })
}

/// [`named`], or a puzzle piece for a name it does not have.
#[must_use]
pub fn named_icon(name: &str) -> Icon {
    named(name).unwrap_or(PUZZLE)
}

#[cfg(test)]
mod named_tests {
    use super::*;

    #[test]
    fn a_name_it_has_is_its_icon_and_one_it_lacks_a_puzzle() {
        assert_ne!(named_icon("search"), PUZZLE);
        assert_eq!(named_icon("home"), named_icon("house"));
        assert_eq!(named_icon("no-such-icon"), PUZZLE);
    }

    #[cfg(feature = "icons")]
    #[test]
    fn with_icons_every_lucide_name_is_there_its_shapes_as_paths() {
        let check = named("circle-check").expect("lucide's circle-check");
        assert_eq!(check.0.len(), 2);
        assert!(check.0[0].starts_with('M'), "a circle as a path: {check:?}");
        assert!(named("git-pull-request").is_some());
        assert!(named("audio-lines").is_some());
    }
}
