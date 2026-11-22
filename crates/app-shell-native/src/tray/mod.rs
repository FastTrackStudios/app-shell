//! Menu bar and system tray apps (feature `tray`).
//!
//! [`run`] starts an app whose home is a tray icon: the menu bar on macOS
//! (activation policy Accessory: no Dock icon until a window opens), a
//! StatusNotifierItem on Linux (KDE, GNOME with the AppIndicator
//! extension, most panels), the notification area on Windows. It runs
//! with no window at all; closing the last window keeps the process (and
//! the app's background work) alive; Quit ([`Tray::quit`]) ends it.
//!
//! The app builds the tray's menu as data ([`TrayMenu`]) and changes it
//! live with its status badge and tooltip ([`Tray::set_menu`],
//! [`Tray::set_status`], …); clicks come back as [`TrayEvent`]s. From the
//! tray it opens windows ([`Tray::open_window`]): an app-shell window
//! (Blitz) or a web view showing a URL (wry), for what Blitz cannot draw
//! (audio and video, a PDF on a canvas). A window already open with that
//! key comes to the front instead. A quick panel ([`PanelSpec`]) hangs
//! under the icon and closes when it loses the focus. Start at login is
//! opt-in ([`autostart`]).

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod apple;
pub mod autostart;
#[cfg(target_os = "linux")]
mod linux;
mod model;
mod web;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use app_shell::ui::LayoutHub;
use blitz_shell::{BlitzShellEvent, BlitzShellProxy, View, WindowConfig};
use dioxus::core::ScopeId;
use dioxus::prelude::*;
use dioxus_native::winit::application::ApplicationHandler;
use dioxus_native::winit::dpi::{LogicalSize, PhysicalPosition};
use dioxus_native::winit::event::{StartCause, WindowEvent};
use dioxus_native::winit::event_loop::{ActiveEventLoop, ControlFlow};
use dioxus_native::winit::window::{
    Window as OsWindow, WindowAttributes, WindowId as OsId, WindowLevel,
};
use dioxus_native::{DioxusDocument, DioxusNativeEvent, DioxusNativeWindowRenderer};

pub use model::{
    Accelerator, MenuEntry, MenuItem, ScreenRect, TrayEvent, TrayIcon, TrayMenu, TrayStatus,
    panel_position,
};

use crate::{Desk, Head, document_config, fit_lights, frame, logical_size, use_frame};

/// A window the tray opens.
#[derive(Clone)]
pub struct WindowSpec {
    /// Its key: opening a key that is open brings that window forward.
    pub key: String,
    pub title: String,
    /// Its content size, logical points.
    pub size: (f64, f64),
    pub kind: WindowKind,
}

/// What a window shows.
#[derive(Clone)]
pub enum WindowKind {
    /// An app-shell window: the component (its root mounts an
    /// `AppShell`), drawn by Blitz. [`use_tray`] reaches the tray from it.
    Shell(fn() -> Element),
    /// A web view showing this URL (the platform's: WebKit on macOS and
    /// Linux, WebView2 on Windows), for what Blitz cannot draw.
    Web(String),
}

impl WindowSpec {
    pub fn shell(key: impl Into<String>, title: impl Into<String>, root: fn() -> Element) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            size: (960.0, 640.0),
            kind: WindowKind::Shell(root),
        }
    }

    pub fn web(key: impl Into<String>, title: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            size: (960.0, 640.0),
            kind: WindowKind::Web(url.into()),
        }
    }

    #[must_use]
    pub const fn size(mut self, w: f64, h: f64) -> Self {
        self.size = (w, h);
        self
    }
}

/// The quick panel: a small window under the tray icon, opened by a click
/// on the icon, closed when it loses the focus.
#[derive(Clone)]
pub struct PanelSpec {
    pub size: (f64, f64),
    pub root: fn() -> Element,
}

/// The panel's window key.
pub const PANEL_KEY: &str = "app-shell.panel";

/// What hears the tray's events.
pub type OnEvent = Box<dyn FnMut(TrayEvent, &Tray)>;

/// The tray app to run.
pub struct TrayApp {
    /// A reverse-DNS id (`studio.fasttrack.drive`): the tray's id, the
    /// autostart file's name.
    pub id: String,
    /// The app's name: the tooltip's title.
    pub name: String,
    pub icon: TrayIcon,
    pub menu: TrayMenu,
    /// Opened at start (`None`: the tray alone).
    pub start_window: Option<WindowSpec>,
    /// Opened by a click on the icon (where the platform tells of one).
    pub panel: Option<PanelSpec>,
    /// Hears every [`TrayEvent`], on the event loop's thread.
    pub on_event: OnEvent,
}

/// The tray's state, as the app last set it.
#[derive(Debug, Clone, PartialEq)]
pub struct TrayState {
    pub id: String,
    pub title: String,
    pub tooltip: String,
    pub icon: TrayIcon,
    pub status: TrayStatus,
    pub menu: TrayMenu,
}

impl TrayState {
    /// The icon as drawn: with the status's badge.
    #[must_use]
    pub fn drawn_icon(&self) -> TrayIcon {
        self.icon.with_badge(self.status)
    }

    /// The tooltip's lines: the app's name, then the tooltip or the
    /// status's name.
    #[must_use]
    pub fn tooltip_lines(&self) -> (String, String) {
        let second = if self.tooltip.is_empty() {
            self.status.name().to_owned()
        } else {
            self.tooltip.clone()
        };
        (self.title.clone(), second)
    }
}

enum TrayAsk {
    Refresh,
    Open(WindowSpec),
    Close(String),
    /// Show (`Some(true)`), hide or toggle (`None`) the panel, near a point.
    Panel(Option<bool>, Option<(i32, i32)>),
    Quit,
}

struct TrayInner {
    state: RefCell<TrayState>,
    asks: RefCell<Vec<TrayAsk>>,
    wake: BlitzShellProxy,
    /// The keys of the windows open now.
    open: RefCell<Vec<String>>,
}

/// The tray, for the app: change it, open windows from it, quit. In
/// context in every window the tray opens ([`use_tray`]) and handed to
/// [`TrayApp::on_event`]. Main-thread only.
#[derive(Clone)]
pub struct Tray(Rc<TrayInner>);

impl PartialEq for Tray {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Tray {
    fn ask(&self, ask: TrayAsk) {
        self.0.asks.borrow_mut().push(ask);
        self.0.wake.wake_up();
    }

    fn edit(&self, f: impl FnOnce(&mut TrayState)) {
        f(&mut self.0.state.borrow_mut());
        self.ask(TrayAsk::Refresh);
    }

    /// Replace the menu.
    pub fn set_menu(&self, menu: TrayMenu) {
        self.edit(|s| s.menu = menu);
    }

    /// Change one row of the menu (a checkmark, a detail line).
    pub fn update_item(&self, id: &str, f: impl FnOnce(&mut MenuItem)) {
        self.edit(|s| {
            s.menu.update(id, f);
        });
    }

    /// The status badge over the icon.
    pub fn set_status(&self, status: TrayStatus) {
        self.edit(|s| s.status = status);
    }

    /// The tooltip's second line (`""`: the status's name).
    pub fn set_tooltip(&self, tooltip: impl Into<String>) {
        let t = tooltip.into();
        self.edit(|s| s.tooltip = t);
    }

    pub fn set_icon(&self, icon: TrayIcon) {
        self.edit(|s| s.icon = icon);
    }

    /// The tray as the app last set it.
    #[must_use]
    pub fn state(&self) -> TrayState {
        self.0.state.borrow().clone()
    }

    /// Open `spec`'s window, or bring the one open with its key forward.
    pub fn open_window(&self, spec: WindowSpec) {
        self.ask(TrayAsk::Open(spec));
    }

    /// Close the window `key`.
    pub fn close_window(&self, key: &str) {
        self.ask(TrayAsk::Close(key.to_owned()));
    }

    /// Whether the window `key` is open.
    #[must_use]
    pub fn is_open(&self, key: &str) -> bool {
        self.0.open.borrow().iter().any(|k| k == key)
    }

    /// Show the quick panel (near the icon, where the platform places it).
    pub fn show_panel(&self) {
        self.ask(TrayAsk::Panel(Some(true), None));
    }

    pub fn hide_panel(&self) {
        self.ask(TrayAsk::Panel(Some(false), None));
    }

    /// End the app: every window closes, the tray goes, the process ends.
    pub fn quit(&self) {
        self.ask(TrayAsk::Quit);
    }
}

/// The tray, from a window it opened.
#[must_use]
pub fn use_tray() -> Tray {
    use_context::<Tray>()
}

/// What a platform's tray does with the state.
trait Backend {
    fn update(&mut self, state: &TrayState);
}

/// Where the platform tray sends what it hears: a channel, and a wake.
#[derive(Clone)]
struct Events {
    tx: Sender<TrayEvent>,
    wake: BlitzShellProxy,
}

impl Events {
    fn send(&self, event: TrayEvent) {
        if self.tx.send(event).is_ok() {
            self.wake.wake_up();
        }
    }
}

/// Run the tray app until it quits. On the main thread; does not return
/// until [`Tray::quit`].
pub fn run(app: TrayApp) {
    let event_loop = blitz_shell::create_default_event_loop();
    let (proxy, queue) = BlitzShellProxy::new(event_loop.create_proxy());
    let (tx, rx) = channel();
    let state = TrayState {
        id: app.id.clone(),
        title: app.name.clone(),
        tooltip: String::new(),
        icon: app.icon,
        status: TrayStatus::Ok,
        menu: app.menu,
    };
    let tray = Tray(Rc::new(TrayInner {
        state: RefCell::new(state),
        asks: RefCell::new(Vec::new()),
        wake: proxy.clone(),
        open: RefCell::new(Vec::new()),
    }));
    let desk = Rc::new(Desk {
        wake: proxy.clone(),
        requests: RefCell::new(Vec::new()),
        main: RefCell::new(None),
        open: RefCell::new(Vec::new()),
        hub: LayoutHub::new(),
        decorated: false,
        focus: RefCell::new(Vec::new()),
        sizes: RefCell::new(Vec::new()),
    });
    if let Some(spec) = app.start_window {
        tray.open_window(spec);
    }
    let mut handler = TrayLoop {
        tray,
        desk,
        events: Events {
            tx,
            wake: proxy.clone(),
        },
        event_rx: rx,
        proxy,
        queue,
        on_event: app.on_event,
        panel: app.panel,
        backend: None,
        windows: HashMap::new(),
        webs: Vec::new(),
        started: false,
        last_click: None,
    };
    let _ = &mut handler;
    if let Err(error) = event_loop.run_app(handler) {
        tracing::error!(app_shell.event_loop = %error, "the tray's event loop could not run");
    }
}

/// An app-shell window the tray opened.
struct ShellWin {
    key: String,
    view: View<DioxusNativeWindowRenderer>,
    panel: bool,
}

struct TrayLoop {
    tray: Tray,
    desk: Rc<Desk>,
    events: Events,
    event_rx: Receiver<TrayEvent>,
    proxy: BlitzShellProxy,
    queue: Receiver<BlitzShellEvent>,
    on_event: OnEvent,
    panel: Option<PanelSpec>,
    backend: Option<Box<dyn Backend>>,
    windows: HashMap<OsId, ShellWin>,
    webs: Vec<web::WebWin>,
    started: bool,
    /// Where the icon was last clicked (screen), for the panel.
    last_click: Option<(i32, i32)>,
}

impl TrayLoop {
    fn start(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.started {
            return;
        }
        self.started = true;
        accessory(event_loop, true);
        let state = self.tray.state();
        #[cfg(target_os = "linux")]
        {
            self.backend = linux::start(&state, self.events.clone(), self.panel.is_some());
        }
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            self.backend = apple::start(&state, self.events.clone(), self.panel.is_some());
        }
        if self.backend.is_none() {
            tracing::warn!(
                app_shell.tray = "none",
                "no tray on this desktop: the app runs without one"
            );
        }
        self.asks(event_loop);
    }

    fn asks(&mut self, event_loop: &dyn ActiveEventLoop) {
        let asks = std::mem::take(&mut *self.tray.0.asks.borrow_mut());
        let mut refresh = false;
        for ask in asks {
            match ask {
                TrayAsk::Refresh => refresh = true,
                TrayAsk::Open(spec) => self.open(event_loop, spec, false),
                TrayAsk::Close(key) => self.close(&key),
                TrayAsk::Panel(show, at) => self.panel(event_loop, show, at),
                TrayAsk::Quit => {
                    self.windows.clear();
                    self.webs.clear();
                    self.backend = None;
                    event_loop.exit();
                    return;
                }
            }
        }
        if refresh && let Some(b) = self.backend.as_mut() {
            b.update(&self.tray.state());
        }
        self.keys_changed(event_loop);
    }

    /// The window keys open now, and (macOS) the Dock icon while any
    /// window but the panel is open.
    fn keys_changed(&self, event_loop: &dyn ActiveEventLoop) {
        let mut keys: Vec<String> = self.windows.values().map(|w| w.key.clone()).collect();
        keys.extend(self.webs.iter().map(|w| w.key.clone()));
        let windowed = self.windows.values().any(|w| !w.panel) || !self.webs.is_empty();
        *self.tray.0.open.borrow_mut() = keys;
        accessory(event_loop, !windowed);
    }

    fn focus(&self, key: &str) -> bool {
        if let Some(w) = self.windows.values().find(|w| w.key == key) {
            w.view.window.focus_window();
            return true;
        }
        if let Some(w) = self.webs.iter().find(|w| w.key == key) {
            w.focus();
            return true;
        }
        false
    }

    fn open(&mut self, event_loop: &dyn ActiveEventLoop, spec: WindowSpec, panel: bool) {
        if self.focus(&spec.key) {
            return;
        }
        match spec.kind.clone() {
            WindowKind::Web(url) => match web::WebWin::open(event_loop, &spec, &url) {
                Ok(w) => self.webs.push(w),
                Err(error) => {
                    tracing::warn!(app_shell.web = %error, "a web window could not open");
                }
            },
            WindowKind::Shell(root) => self.open_shell(event_loop, &spec, root, panel),
        }
    }

    fn open_shell(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        spec: &WindowSpec,
        root: fn() -> Element,
        panel: bool,
    ) {
        let (w, h) = spec.size;
        let mut attributes = WindowAttributes::default()
            .with_title(spec.title.clone())
            .with_surface_size(LogicalSize::new(w, h));
        attributes = if panel {
            attributes
                .with_decorations(false)
                .with_resizable(false)
                .with_window_level(WindowLevel::AlwaysOnTop)
        } else {
            frame(attributes, false)
        };
        if panel && let Some((x, y)) = self.panel_origin((w, h)) {
            attributes = attributes.with_position(PhysicalPosition::new(x, y));
        }
        let vdom = VirtualDom::new_with_props(
            TrayRoot,
            TrayRootProps {
                root,
                window_key: spec.key.clone(),
                panel,
            },
        );
        let doc = DioxusDocument::new(vdom, document_config());
        let config = WindowConfig::with_attributes(
            Box::new(doc),
            DioxusNativeWindowRenderer::new(),
            attributes,
        );
        let mut view = View::init(config, event_loop, &self.proxy);
        let window = Arc::clone(&view.window);
        let doc = view.downcast_doc_mut::<DioxusDocument>();
        let shell = doc.inner.borrow().shell_provider.clone();
        let head: Rc<dyn dioxus::document::Document> = Rc::new(Head {
            window: Arc::clone(&window),
            os: window.id(),
            proxy: self.proxy.clone(),
        });
        let (tray, desk, hub) = (
            self.tray.clone(),
            DeskRef(self.desk.clone()),
            self.desk.hub.clone(),
        );
        let os_window = Arc::clone(&window);
        doc.vdom.in_scope(ScopeId::ROOT, move || {
            provide_context(head);
            provide_context(shell);
            provide_context(hub);
            provide_context(tray);
            provide_context(desk);
            provide_context(os_window);
        });
        if !panel {
            fit_lights(window.as_ref(), false);
        }
        doc.initial_build();
        view.resume();
        view.request_redraw();
        self.windows.insert(
            view.window_id(),
            ShellWin {
                key: spec.key.clone(),
                view,
                panel,
            },
        );
    }

    /// Where the panel goes: under the icon where it was clicked, on the
    /// screen the click was on. `None`: where the platform puts it (on
    /// Wayland a window cannot place itself).
    fn panel_origin(&self, size: (f64, f64)) -> Option<(i32, i32)> {
        let (x, y) = self.last_click?;
        let icon = ScreenRect {
            x: x - 12,
            y: y - 12,
            w: 24,
            h: 24,
        };
        // The screen: the primary monitor's, where known.
        let screen = ScreenRect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        };
        #[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
        let size = (size.0 as i32, size.1 as i32);
        Some(panel_position(icon, size, screen, 6))
    }

    fn panel(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        show: Option<bool>,
        at: Option<(i32, i32)>,
    ) {
        let Some(spec) = self.panel.clone() else {
            return;
        };
        if at.is_some() {
            self.last_click = at;
        }
        let open = self.windows.values().any(|w| w.panel);
        let want = show.unwrap_or(!open);
        if want && !open {
            let w = WindowSpec {
                key: PANEL_KEY.to_owned(),
                title: self.tray.state().title,
                size: spec.size,
                kind: WindowKind::Shell(spec.root),
            };
            self.open(event_loop, w, true);
        } else if !want && open {
            self.close(PANEL_KEY);
        }
    }

    fn close(&mut self, key: &str) {
        let os: Vec<OsId> = self
            .windows
            .iter()
            .filter(|(_, w)| w.key == key)
            .map(|(id, _)| *id)
            .collect();
        for id in os {
            self.windows.remove(&id);
        }
        self.webs.retain(|w| w.key != key);
        self.tray_event(TrayEvent::WindowClosed(key.to_owned()));
    }

    fn tray_event(&mut self, event: TrayEvent) {
        let tray = self.tray.clone();
        (self.on_event)(event, &tray);
    }

    /// What the platform's tray and the windows' documents said.
    fn drain(&mut self, event_loop: &dyn ActiveEventLoop) {
        while let Ok(event) = self.event_rx.try_recv() {
            match &event {
                TrayEvent::Activate { x, y } if self.panel.is_some() => {
                    self.panel(event_loop, None, Some((*x, *y)));
                }
                TrayEvent::WindowClosed(key) => {
                    // A web window closed by its own frame.
                    self.webs.retain(|w| &w.key != key);
                }
                _ => {}
            }
            self.tray_event(event);
        }
        while let Ok(event) = self.queue.try_recv() {
            match event {
                BlitzShellEvent::Poll { window_id } => {
                    if let Some(w) = self.windows.get_mut(&window_id) {
                        w.view.poll();
                    }
                }
                BlitzShellEvent::RequestRedraw { doc_id } => {
                    if let Some(w) = self
                        .windows
                        .values_mut()
                        .find(|w| w.view.doc.id() == doc_id)
                    {
                        w.view.request_redraw();
                    }
                }
                BlitzShellEvent::ResumeReady { window_id } => {
                    if let Some(w) = self.windows.get_mut(&window_id) {
                        let _ = w.view.complete_resume();
                    }
                }
                BlitzShellEvent::CloseWindow { window_id } => {
                    if let Some(key) = self.windows.get(&window_id).map(|w| w.key.clone()) {
                        self.close(&key);
                    }
                }
                BlitzShellEvent::Embedder(event) => {
                    if let Some(DioxusNativeEvent::CreateHeadElement {
                        window,
                        name,
                        attributes,
                        contents,
                    }) = event.downcast_ref::<DioxusNativeEvent>()
                        && let Some(w) = self.windows.get_mut(window)
                    {
                        let doc = w.view.downcast_doc_mut::<DioxusDocument>();
                        doc.create_head_element(name, attributes, contents);
                        w.view.poll();
                    }
                }
                _ => {}
            }
        }
        self.asks(event_loop);
    }
}

impl ApplicationHandler for TrayLoop {
    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::Init) {
            self.start(event_loop);
        }
    }

    fn resumed(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.start(event_loop);
    }

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.start(event_loop);
        for w in self.windows.values_mut() {
            w.view.resume();
        }
    }

    fn destroy_surfaces(&mut self, _event_loop: &dyn ActiveEventLoop) {
        for w in self.windows.values_mut() {
            w.view.suspend();
        }
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, window: OsId, event: WindowEvent) {
        match &event {
            WindowEvent::Focused(on) => {
                self.desk.focused(window, *on);
                // The panel goes when it loses the focus.
                if !on && self.windows.get(&window).is_some_and(|w| w.panel) {
                    self.close(PANEL_KEY);
                    self.asks(event_loop);
                    return;
                }
            }
            WindowEvent::CloseRequested => {
                let web = self
                    .webs
                    .iter()
                    .find(|w| w.os_id() == Some(window))
                    .map(|w| w.key.clone());
                if let Some(key) = self.windows.get(&window).map(|w| w.key.clone()).or(web) {
                    self.close(&key);
                }
                // The last window closing keeps the app: its home is the tray.
                self.asks(event_loop);
                return;
            }
            WindowEvent::SurfaceResized(_) => {
                if let Some(w) = self.windows.get(&window) {
                    let os = Arc::clone(&w.view.window);
                    if !w.panel {
                        fit_lights(os.as_ref(), false);
                    }
                    self.desk.resized(window, logical_size(os.as_ref()));
                }
                if let Some(w) = self.webs.iter_mut().find(|w| w.os_id() == Some(window)) {
                    w.resized();
                }
            }
            _ => {}
        }
        if let Some(w) = self.windows.get_mut(&window) {
            w.view.handle_winit_event(event);
            self.proxy
                .send_event(BlitzShellEvent::Poll { window_id: window });
        }
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.drain(event_loop);
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        // Web views on Linux live in GTK: its events are pumped here, so
        // the loop wakes while one is open.
        if web::pump(&mut self.webs, &self.events) {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                std::time::Instant::now() + std::time::Duration::from_millis(16),
            ));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
        self.drain(event_loop);
    }
}

/// The desk, in a tray window's context (for its frame).
#[derive(Clone)]
struct DeskRef(Rc<Desk>);

#[derive(Props, Clone, PartialEq)]
struct TrayRootProps {
    root: fn() -> Element,
    window_key: String,
    panel: bool,
}

/// A tray window's root: its frame (the shell's top bar, closing the
/// window through the tray), then the app's component.
#[allow(non_snake_case, clippy::needless_pass_by_value)]
fn TrayRoot(props: TrayRootProps) -> Element {
    if props.panel {
        // A panel has no frame: no traffic lights in a dropdown.
        return (props.root)();
    }
    rsx! { FramedRoot { root: props.root, window_key: props.window_key.clone() } }
}

/// A window's frame (its controls, focus and size), then the app.
#[component]
fn FramedRoot(root: fn() -> Element, window_key: String) -> Element {
    let desk = use_context::<DeskRef>().0;
    let window = use_context::<Arc<dyn OsWindow>>();
    let tray = use_tray();
    use_frame(
        &desk,
        &window,
        Callback::new(move |()| tray.close_window(&window_key)),
    );
    root()
}

/// macOS: Accessory (no Dock icon, no menu bar of its own) while only the
/// tray is up, Regular while a window is open. Nothing elsewhere.
#[allow(clippy::missing_const_for_fn)]
fn accessory(event_loop: &dyn ActiveEventLoop, on: bool) {
    #[cfg(target_os = "macos")]
    crate::mac::set_accessory(on);
    let _ = (event_loop, on);
}
