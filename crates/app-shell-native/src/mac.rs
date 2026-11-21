//! A Mac window's own title bar: its traffic lights placed in the shell's
//! top bar row, however tall the row is (Electron's
//! `trafficLightPosition`). `AppKit` lays its title bar out again on a
//! resize and on entering or leaving full screen, so this runs after each.
//!
//! Ported from Architect's `architect-shell` (MIT OR Apache-2.0).

use dioxus_native::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use objc2_app_kit::{NSView, NSWindowButton};
use objc2_foundation::NSPoint;

/// Centres the window's buttons in a title row `row` points tall, the
/// first `left` points in.
///
/// A `row` of `None` hides them. Does nothing off `AppKit` or before the
/// window has its buttons.
pub fn fit_traffic_lights(window: &dyn HasWindowHandle, row: Option<f64>, left: f64) {
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    // SAFETY: an AppKit window handle's `ns_view` is the window's content
    // view, alive while the window is; this runs on the main thread (from
    // the window's own event handling).
    let view: &NSView = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    let Some(ns_window) = view.window() else {
        return;
    };
    let buttons = [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ]
    .map(|b| ns_window.standardWindowButton(b));
    let [Some(close), Some(mini), Some(zoom)] = buttons else {
        return;
    };
    for button in [&close, &mini, &zoom] {
        button.setHidden(row.is_none());
    }
    let Some(row) = row else {
        return;
    };
    // The buttons' superview is the title bar view, its superview the
    // container AppKit sizes to its own title bar's height.
    // SAFETY: plain view-hierarchy reads on the main thread.
    let Some(container) = (unsafe { close.superview() }).and_then(|v| unsafe { v.superview() })
    else {
        return;
    };
    // AppKit moves the window for a drag that starts anywhere in its title
    // bar, over the page or not; the row's empty space moves the window
    // itself (the shell's `DragArea`, `drag_window`).
    ns_window.setMovable(false);
    let mut rect = container.frame();
    rect.size.height = row;
    rect.origin.y = ns_window.frame().size.height - row;
    container.setFrame(rect);
    let step = mini.frame().origin.x - close.frame().origin.x;
    let mut x = left;
    for button in [&close, &mini, &zoom] {
        let y = (row - button.frame().size.height) / 2.0;
        button.setFrameOrigin(NSPoint::new(x, y));
        x += step;
    }
}
