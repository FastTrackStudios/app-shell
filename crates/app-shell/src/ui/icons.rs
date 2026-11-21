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
