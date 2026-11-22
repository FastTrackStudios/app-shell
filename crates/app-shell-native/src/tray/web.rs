//! A window showing a URL in the platform's web view (wry): WebKit on
//! macOS, WebView2 on Windows, WebKitGTK on Linux. For what Blitz cannot
//! draw: audio and video, a PDF rendered on a canvas, a site.
//!
//! On macOS and Windows the web view fills a winit window. On Linux wry's
//! web view lives in GTK: the window is a GTK window, its events pumped by
//! the tray's loop ([`pump`]) while one is open.

use dioxus_native::winit::event_loop::ActiveEventLoop;
use dioxus_native::winit::window::WindowId as OsId;

use super::{Events, WindowSpec};

/// A web window open.
pub(super) struct WebWin {
    pub key: String,
    inner: Inner,
}

#[cfg(target_os = "linux")]
struct Inner {
    window: gtk::Window,
    _view: wry::WebView,
    /// Set by the window's delete event: it was closed by its frame.
    closed: std::rc::Rc<std::cell::Cell<bool>>,
}

#[cfg(not(target_os = "linux"))]
struct Inner {
    // The view before its window: it goes first.
    view: wry::WebView,
    window: std::sync::Arc<dyn dioxus_native::winit::window::Window>,
}

impl WebWin {
    /// Open `spec`'s window showing `url`.
    #[cfg(target_os = "linux")]
    pub fn open(
        _event_loop: &dyn ActiveEventLoop,
        spec: &WindowSpec,
        url: &str,
    ) -> Result<Self, String> {
        use gtk::prelude::*;
        use wry::WebViewBuilderExtUnix;
        if !gtk::is_initialized_main_thread() {
            gtk::init().map_err(|e| format!("GTK could not start: {e}"))?;
        }
        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_title(&spec.title);
        #[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
        window.set_default_size(spec.size.0 as i32, spec.size.1 as i32);
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
        window.add(&vbox);
        let view = wry::WebViewBuilder::new()
            .with_url(url)
            .build_gtk(&vbox)
            .map_err(|e| e.to_string())?;
        let closed = std::rc::Rc::new(std::cell::Cell::new(false));
        let flag = closed.clone();
        window.connect_delete_event(move |_, _| {
            flag.set(true);
            gtk::glib::Propagation::Proceed
        });
        window.show_all();
        Ok(Self {
            key: spec.key.clone(),
            inner: Inner {
                window,
                _view: view,
                closed,
            },
        })
    }

    #[cfg(not(target_os = "linux"))]
    pub fn open(
        event_loop: &dyn ActiveEventLoop,
        spec: &WindowSpec,
        url: &str,
    ) -> Result<Self, String> {
        use dioxus_native::winit::dpi::LogicalSize;
        use dioxus_native::winit::window::WindowAttributes;
        let attributes = WindowAttributes::default()
            .with_title(spec.title.clone())
            .with_surface_size(LogicalSize::new(spec.size.0, spec.size.1));
        let window: std::sync::Arc<dyn dioxus_native::winit::window::Window> = std::sync::Arc::from(
            event_loop
                .create_window(attributes)
                .map_err(|e| e.to_string())?,
        );
        let view = wry::WebViewBuilder::new()
            .with_url(url)
            .build(&Handle(window.rwh_06_window_handle()))
            .map_err(|e| e.to_string())?;
        Ok(Self {
            key: spec.key.clone(),
            inner: Inner { view, window },
        })
    }

    /// Bring it forward.
    pub fn focus(&self) {
        #[cfg(target_os = "linux")]
        {
            use gtk::prelude::*;
            self.inner.window.present();
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.inner.window.focus_window();
        }
    }

    /// The winit window it is in (none on Linux: GTK's).
    pub fn os_id(&self) -> Option<OsId> {
        #[cfg(target_os = "linux")]
        {
            None
        }
        #[cfg(not(target_os = "linux"))]
        {
            Some(self.inner.window.id())
        }
    }

    /// Its window changed size: the view follows.
    pub fn resized(&mut self) {
        #[cfg(not(target_os = "linux"))]
        {
            let s = self.inner.window.surface_size();
            let _ = self.inner.view.set_bounds(wry::Rect {
                position: wry::dpi::PhysicalPosition::new(0, 0).into(),
                size: wry::dpi::PhysicalSize::new(s.width, s.height).into(),
            });
        }
    }
}

impl Drop for WebWin {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        {
            use gtk::prelude::*;
            if !self.inner.closed.get() {
                // SAFETY: the window is ours and no one else holds it.
                unsafe { self.inner.window.destroy() };
            }
        }
    }
}

/// A winit window's handle as wry takes one (a sized type).
#[cfg(not(target_os = "linux"))]
struct Handle<'a>(&'a dyn dioxus_native::winit::raw_window_handle::HasWindowHandle);

#[cfg(not(target_os = "linux"))]
impl dioxus_native::winit::raw_window_handle::HasWindowHandle for Handle<'_> {
    fn window_handle(
        &self,
    ) -> Result<
        dioxus_native::winit::raw_window_handle::WindowHandle<'_>,
        dioxus_native::winit::raw_window_handle::HandleError,
    > {
        self.0.window_handle()
    }
}

/// Run GTK's pending events (Linux), and tell of web windows their frame
/// closed; whether any web window is open (the loop then keeps waking).
pub(super) fn pump(webs: &mut [WebWin], events: &Events) -> bool {
    #[cfg(target_os = "linux")]
    {
        if webs.is_empty() || !gtk::is_initialized_main_thread() {
            return false;
        }
        while gtk::events_pending() {
            gtk::main_iteration_do(false);
        }
        for w in webs.iter() {
            if w.inner.closed.get() {
                events.send(super::TrayEvent::WindowClosed(w.key.clone()));
            }
        }
        !webs.is_empty()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (webs, events);
        false
    }
}
