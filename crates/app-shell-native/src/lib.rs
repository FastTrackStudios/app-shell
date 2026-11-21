//! app-shell's native side: an app's windows on dioxus-native (Blitz).
//!
//! dioxus-native opens one window, at start. [`launch`] runs its
//! application inside one of its own that opens more while the app runs:
//! each pop-out window of the layout is an OS window with a Blitz view of
//! its own (a document, a `VirtualDom`, a renderer), its root the app's
//! `popout` component. Every window's root gets, in context:
//!
//! - the [`LayoutHub`] all windows share, so a change in one reaches the
//!   others (each dom takes it into its own `Signal<Workspace>`);
//! - [`WindowControls`] for its OS window (minimise, maximise, close,
//!   move), so the shell's title bar draws the window's buttons where the
//!   window has no system frame;
//! - in the main window, [`Windows`]: the main window's shell opens and
//!   closes the pop-outs' OS windows through it.
//!
//! What the main window gets from dioxus-native is untouched (its window
//! events, its document head, hot reload where enabled).
//!
//! This crate uses the published dioxus-native and blitz-shell only; see
//! `docs/migration.md` for the forks' fixes an app may want patched in.

// The roots take the app's component as a fn pointer prop: a stale compare
// only redraws.
#![allow(unpredictable_function_pointer_comparisons)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use app_shell::ui::{HubMessage, LIGHTS_WIDTH, LayoutHub, WindowControls, WindowSystem, Windows};
use app_shell::{Rect, WindowId};
use blitz_shell::{BlitzShellEvent, BlitzShellProxy, View, WindowConfig};
use dioxus::core::ScopeId;
use dioxus::prelude::*;
use dioxus_native::winit::application::ApplicationHandler;
use dioxus_native::winit::dpi::{LogicalPosition, LogicalSize};
use dioxus_native::winit::event::{StartCause, WindowEvent};
use dioxus_native::winit::event_loop::ActiveEventLoop;
use dioxus_native::winit::window::{Window as OsWindow, WindowAttributes, WindowId as OsId};
use dioxus_native::{
    DioxusDocument, DioxusNativeApplication, DioxusNativeEvent, DioxusNativeWindowRenderer,
    DocumentConfig,
};
use futures_channel::mpsc::{UnboundedSender, unbounded};
use futures_util::StreamExt;

#[cfg(target_os = "macos")]
use dioxus_native::winit::platform::macos::ApplicationHandlerExtMacOS;

pub use dioxus_native::winit;

/// How the main window opens.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub title: String,
    /// Its content's size, logical points.
    pub size: (f64, f64),
    /// Whether the system draws the windows' frames. Off (the default):
    /// the shell's title bar is the frame, its buttons the window's.
    pub system_decorations: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            title: "App".to_owned(),
            size: (1280.0, 800.0),
            system_decorations: false,
        }
    }
}

/// Run `main` in a window, each pop-out of the layout opening as an OS
/// window drawn by `popout` (given the pop-out's id).
///
/// `main` and `popout` each mount an `app_shell::ui::AppShell`: the
/// main one with the app's layout signal, a pop-out's with
/// `app_shell::ui::use_window_layout()` and its `window`.
pub fn launch(main: fn() -> Element, popout: fn(WindowId) -> Element, config: Config) {
    let event_loop = blitz_shell::create_default_event_loop();
    let winit_proxy = event_loop.create_proxy();
    // The main window's events go to dioxus-native's queue; the pop-outs'
    // to this one, so neither drops the other's.
    let (proxy, queue) = BlitzShellProxy::new(winit_proxy.clone());
    let (own, own_queue) = BlitzShellProxy::new(winit_proxy);
    let desk = Rc::new(Desk {
        wake: own.clone(),
        requests: RefCell::new(Vec::new()),
        main: RefCell::new(None),
        open: RefCell::new(Vec::new()),
        hub: LayoutHub::new(),
        decorated: config.system_decorations,
        focus: RefCell::new(Vec::new()),
    });
    let vdom = VirtualDom::new_with_props(
        MainRoot,
        MainRootProps {
            app: main,
            desk: DeskHandle(desk.clone()),
        },
    );
    let doc = DioxusDocument::new(vdom, document_config());
    let attributes = frame(
        WindowAttributes::default()
            .with_title(config.title.clone())
            .with_surface_size(LogicalSize::new(config.size.0, config.size.1)),
        config.system_decorations,
    );
    let window =
        WindowConfig::with_attributes(Box::new(doc), DioxusNativeWindowRenderer::new(), attributes);
    let app = App {
        inner: DioxusNativeApplication::new(proxy, queue, window),
        desk,
        popout,
        popouts: HashMap::new(),
        own,
        own_queue,
    };
    if let Err(error) = event_loop.run_app(app) {
        tracing::error!(app_shell.event_loop = %error, "the event loop could not run");
    }
}

/// What a window's document is made with: Blitz's HTML parser, so
/// `dangerous_inner_html` makes nodes.
fn document_config() -> DocumentConfig {
    DocumentConfig {
        html_parser_provider: Some(Arc::new(blitz_html::HtmlProvider)),
        ..Default::default()
    }
}

/// What the shells asked of the event loop.
enum Ask {
    Open {
        id: WindowId,
        title: String,
        bounds: Option<Rect>,
    },
    Close(WindowId),
    /// End the event loop: the app quits.
    Quit,
}

/// The app's windows, as the shells see them.
struct Desk {
    wake: BlitzShellProxy,
    requests: RefCell<Vec<Ask>>,
    main: RefCell<Option<Arc<dyn OsWindow>>>,
    /// The pop-outs open now, by id.
    open: RefCell<Vec<(WindowId, Arc<dyn OsWindow>)>>,
    hub: LayoutHub,
    decorated: bool,
    /// Each window's root, told when its window gains or loses the focus.
    focus: RefCell<Vec<(OsId, UnboundedSender<bool>)>>,
}

impl Desk {
    /// The OS window `os` gained or lost the focus.
    fn focused(&self, os: OsId, on: bool) {
        self.focus.borrow_mut().retain(|(_, s)| !s.is_closed());
        for (w, s) in self.focus.borrow().iter() {
            if *w == os {
                let _ = s.unbounded_send(on);
            }
        }
    }

    fn ask(&self, ask: Ask) {
        self.requests.borrow_mut().push(ask);
        self.wake.wake_up();
    }
}

impl WindowSystem for Desk {
    fn open(&self, id: &WindowId, title: &str, bounds: Option<Rect>) {
        self.ask(Ask::Open {
            id: id.clone(),
            title: title.to_owned(),
            bounds,
        });
    }

    fn close(&self, id: &WindowId) {
        self.ask(Ask::Close(id.clone()));
    }
}

/// The desk, as a prop (compared by identity).
#[derive(Clone)]
struct DeskHandle(Rc<Desk>);

impl PartialEq for DeskHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// A window's frame: the system's (`decorated`), or the shell's top bar.
/// On macOS the system's traffic lights stay, over a transparent title
/// bar the content runs under (placed in the top bar's row by
/// [`fit_lights`]); elsewhere the window has no system frame and the
/// shell draws the lights.
fn frame(attributes: WindowAttributes, decorated: bool) -> WindowAttributes {
    if decorated {
        return attributes.with_decorations(true);
    }
    #[cfg(target_os = "macos")]
    {
        attributes.with_platform_attributes(Box::new(
            dioxus_native::winit::platform::macos::WindowAttributesMacOS::default()
                .with_titlebar_transparent(true)
                .with_fullsize_content_view(true)
                .with_title_hidden(true),
        ))
    }
    #[cfg(not(target_os = "macos"))]
    {
        attributes.with_decorations(false)
    }
}

/// Puts macOS's traffic lights in the top bar's row; nothing elsewhere.
/// `AppKit` lays its title bar out again on a resize, so this runs after
/// each.
fn fit_lights(window: &dyn OsWindow, decorated: bool) {
    #[cfg(target_os = "macos")]
    if !decorated && window.fullscreen().is_none() {
        mac::fit_traffic_lights(
            window.rwh_06_window_handle(),
            Some(app_shell::ui::TOP_BAR_H),
            app_shell::ui::LIGHTS_LEFT,
        );
    }
    let _ = (window, decorated);
}

/// The controls of `window`, closing it with `close`, its lights lit
/// while `focused`.
fn controls(
    window: &Arc<dyn OsWindow>,
    close: Callback<()>,
    focused: Signal<bool>,
) -> WindowControls {
    let (mini, zoom, drag) = (window.clone(), window.clone(), window.clone());
    let system = cfg!(target_os = "macos");
    WindowControls {
        // On macOS the system's own lights do these.
        minimize: (!system).then(|| Callback::new(move |()| mini.set_minimized(true))),
        toggle_maximize: Some(Callback::new(move |()| {
            zoom.set_maximized(!zoom.is_maximized());
        })),
        close: (!system).then_some(close),
        drag: Some(Callback::new(move |()| {
            if let Err(error) = drag.drag_window() {
                tracing::debug!(app_shell.drag = %error, "the platform refused a window drag");
            }
        })),
        focused: Some(focused),
        #[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
        system_lights: system.then_some(LIGHTS_WIDTH as f32),
    }
}

/// A window root's frame: its focus heard from the event loop, and its
/// controls in context (unless the system draws the frame).
fn use_frame(desk: &Rc<Desk>, window: &Arc<dyn OsWindow>, close: Callback<()>) {
    let focused = use_signal(|| true);
    let desk = desk.clone();
    let window = window.clone();
    use_hook(move || {
        let (tx, mut rx) = unbounded::<bool>();
        desk.focus.borrow_mut().push((window.id(), tx));
        spawn(async move {
            let mut focused = focused;
            while let Some(on) = rx.next().await {
                if *focused.peek() != on {
                    focused.set(on);
                }
            }
        });
        if !desk.decorated {
            provide_context(controls(&window, close, focused));
        }
    });
}

#[derive(Props, Clone, PartialEq)]
struct MainRootProps {
    app: fn() -> Element,
    desk: DeskHandle,
}

/// The main window's root: the windows, the hub and the window's
/// controls in context, then the app.
#[allow(non_snake_case, clippy::needless_pass_by_value)]
fn MainRoot(props: MainRootProps) -> Element {
    let desk = props.desk.0.clone();
    use_context_provider(|| Windows(desk.clone() as Rc<dyn WindowSystem>));
    use_context_provider(|| desk.hub.clone());
    let window = dioxus_native::use_window();
    let quit = desk.clone();
    let seen = desk.clone();
    let main = window.clone();
    use_hook(move || {
        *seen.main.borrow_mut() = Some(main.clone());
        fit_lights(main.as_ref(), seen.decorated);
    });
    use_frame(&desk, &window, Callback::new(move |()| quit.ask(Ask::Quit)));
    (props.app)()
}

#[derive(Props, Clone, PartialEq)]
struct PopoutRootProps {
    app: fn(WindowId) -> Element,
    id: WindowId,
    desk: DeskHandle,
}

/// A pop-out's root: the hub and the OS window are put in by
/// [`App::open`]; its frame here; then the app's pop-out component.
#[allow(non_snake_case, clippy::needless_pass_by_value)]
fn PopoutRoot(props: PopoutRootProps) -> Element {
    let desk = props.desk.0.clone();
    let window = use_context::<Arc<dyn OsWindow>>();
    // Closing a pop-out gives its tabs back: the main window's shell hears
    // it, and closes this window.
    let hub = desk.hub.clone();
    let closing = props.id.clone();
    use_frame(
        &desk,
        &window,
        Callback::new(move |()| hub.tell_main(HubMessage::Closed(closing.clone()))),
    );
    (props.app)(props.id.clone())
}

/// A pop-out on screen.
struct Open {
    id: WindowId,
    view: View<DioxusNativeWindowRenderer>,
}

/// dioxus-native's application, and the pop-outs beside its window.
struct App {
    inner: DioxusNativeApplication,
    desk: Rc<Desk>,
    popout: fn(WindowId) -> Element,
    popouts: HashMap<OsId, Open>,
    own: BlitzShellProxy,
    own_queue: Receiver<BlitzShellEvent>,
}

/// Where `window`'s content is on screen, logical points.
fn content_rect(window: &dyn OsWindow) -> Option<Rect> {
    let scale = window.scale_factor();
    let outer = window.outer_position().ok()?;
    let inset = window.surface_position();
    let size = window.surface_size();
    let to = |v: f64| narrow(v / scale);
    Some(Rect::new(
        to(f64::from(outer.x) + f64::from(inset.x)),
        to(f64::from(outer.y) + f64::from(inset.y)),
        to(f64::from(size.width)),
        to(f64::from(size.height)),
    ))
}

impl App {
    /// Do what the shells asked.
    fn asks(&mut self, event_loop: &dyn ActiveEventLoop) {
        let asks = std::mem::take(&mut *self.desk.requests.borrow_mut());
        for ask in asks {
            match ask {
                Ask::Open { id, title, bounds } => self.open(event_loop, id, &title, bounds),
                Ask::Close(id) => {
                    let os = self
                        .popouts
                        .iter()
                        .find(|(_, o)| o.id == id)
                        .map(|(os, _)| *os);
                    if let Some(os) = os {
                        self.forget(os);
                    }
                }
                Ask::Quit => event_loop.exit(),
            }
        }
    }

    fn open(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        id: WindowId,
        title: &str,
        bounds: Option<Rect>,
    ) {
        if let Some(open) = self.popouts.values().find(|o| o.id == id) {
            open.view.window.focus_window();
            return;
        }
        let (w, h) = bounds.map_or((640.0, 480.0), |b| (b.w, b.h));
        let mut attributes = frame(
            WindowAttributes::default()
                .with_title(title.to_owned())
                .with_surface_size(LogicalSize::new(f64::from(w), f64::from(h))),
            self.desk.decorated,
        );
        if let Some(b) = bounds {
            attributes =
                attributes.with_position(LogicalPosition::new(f64::from(b.x), f64::from(b.y)));
        }
        let vdom = VirtualDom::new_with_props(
            PopoutRoot,
            PopoutRootProps {
                app: self.popout,
                id: id.clone(),
                desk: DeskHandle(self.desk.clone()),
            },
        );
        let doc = DioxusDocument::new(vdom, document_config());
        let config = WindowConfig::with_attributes(
            Box::new(doc),
            DioxusNativeWindowRenderer::new(),
            attributes,
        );
        let mut view = View::init(config, event_loop, &self.own);
        let window = Arc::clone(&view.window);
        let doc = view.downcast_doc_mut::<DioxusDocument>();
        let shell = doc.inner.borrow().shell_provider.clone();
        let head: Rc<dyn dioxus::document::Document> = Rc::new(Head {
            window: Arc::clone(&window),
            os: window.id(),
            proxy: self.own.clone(),
        });
        let hub = self.desk.hub.clone();
        let os_window = Arc::clone(&window);
        doc.vdom.in_scope(ScopeId::ROOT, move || {
            provide_context(head);
            provide_context(shell);
            provide_context(hub);
            provide_context(os_window);
        });
        fit_lights(window.as_ref(), self.desk.decorated);
        doc.initial_build();
        view.resume();
        view.request_redraw();
        self.desk
            .open
            .borrow_mut()
            .push((id.clone(), Arc::clone(&window)));
        self.popouts.insert(view.window_id(), Open { id, view });
    }

    /// Drop the pop-out's window (its document goes with it).
    fn forget(&mut self, os: OsId) {
        if let Some(open) = self.popouts.remove(&os) {
            self.desk.open.borrow_mut().retain(|(w, _)| *w != open.id);
            drop(open);
        }
    }

    /// The pop-out `os` has an event from the platform.
    fn popout_event(&mut self, os: OsId, event: WindowEvent) {
        let Some(open) = self.popouts.get_mut(&os) else {
            return;
        };
        match &event {
            WindowEvent::CloseRequested => {
                // The main window's shell gives its tabs back and asks
                // for the window to close.
                self.desk.hub.tell_main(HubMessage::Closed(open.id.clone()));
                return;
            }
            WindowEvent::Moved(_) | WindowEvent::SurfaceResized(_) => {
                if let Some(bounds) = content_rect(open.view.window.as_ref()) {
                    self.desk
                        .hub
                        .tell_main(HubMessage::Moved(open.id.clone(), bounds));
                }
            }
            _ => {}
        }
        open.view.handle_winit_event(event);
        self.own.send_event(BlitzShellEvent::Poll { window_id: os });
    }

    /// What the pop-outs' documents asked of the loop.
    fn own_events(&mut self) {
        while let Ok(event) = self.own_queue.try_recv() {
            match event {
                BlitzShellEvent::Poll { window_id } => {
                    if let Some(open) = self.popouts.get_mut(&window_id) {
                        open.view.poll();
                    }
                }
                BlitzShellEvent::RequestRedraw { doc_id } => {
                    if let Some(open) = self
                        .popouts
                        .values_mut()
                        .find(|o| o.view.doc.id() == doc_id)
                    {
                        open.view.request_redraw();
                    }
                }
                BlitzShellEvent::ResumeReady { window_id } => {
                    if let Some(open) = self.popouts.get_mut(&window_id) {
                        let _ = open.view.complete_resume();
                    }
                }
                BlitzShellEvent::CloseWindow { window_id } => {
                    self.popout_event(window_id, WindowEvent::CloseRequested);
                }
                BlitzShellEvent::Embedder(event) => {
                    if let Some(DioxusNativeEvent::CreateHeadElement {
                        window,
                        name,
                        attributes,
                        contents,
                    }) = event.downcast_ref::<DioxusNativeEvent>()
                        && let Some(open) = self.popouts.get_mut(window)
                    {
                        let doc = open.view.downcast_doc_mut::<DioxusDocument>();
                        doc.create_head_element(name, attributes, contents);
                        open.view.poll();
                    }
                }
                _ => {}
            }
        }
    }
}

impl ApplicationHandler for App {
    #[cfg(target_os = "macos")]
    fn macos_handler(&mut self) -> Option<&mut dyn ApplicationHandlerExtMacOS> {
        self.inner.macos_handler()
    }

    fn resumed(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.resumed(event_loop);
    }

    fn suspended(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.suspended(event_loop);
    }

    fn destroy_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        for open in self.popouts.values_mut() {
            open.view.suspend();
        }
        self.inner.destroy_surfaces(event_loop);
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.about_to_wait(event_loop);
    }

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.can_create_surfaces(event_loop);
        for open in self.popouts.values_mut() {
            open.view.resume();
        }
        self.asks(event_loop);
    }

    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        self.inner.new_events(event_loop, cause);
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, window: OsId, event: WindowEvent) {
        match &event {
            WindowEvent::Focused(on) => self.desk.focused(window, *on),
            // AppKit lays its title bar out again on a resize.
            WindowEvent::SurfaceResized(_) => {
                let os = self
                    .popouts
                    .get(&window)
                    .map(|o| Arc::clone(&o.view.window))
                    .or_else(|| self.desk.main.borrow().clone());
                if let Some(os) = os {
                    fit_lights(os.as_ref(), self.desk.decorated);
                }
            }
            _ => {}
        }
        if self.popouts.contains_key(&window) {
            self.popout_event(window, event);
            self.asks(event_loop);
        } else {
            self.inner.window_event(event_loop, window, event);
            self.asks(event_loop);
        }
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.own_events();
        self.asks(event_loop);
        self.inner.proxy_wake_up(event_loop);
        // The main window's turn may have asked for more.
        self.asks(event_loop);
    }
}

/// A pop-out's document head: its title is the OS window's; its styles
/// go into its document on the loop's next turn.
struct Head {
    window: Arc<dyn OsWindow>,
    os: OsId,
    proxy: BlitzShellProxy,
}

impl dioxus::document::Document for Head {
    fn eval(&self, js: String) -> dioxus::document::Eval {
        dioxus::document::NoOpDocument.eval(js)
    }

    fn set_title(&self, title: String) {
        self.window.set_title(&title);
    }

    fn create_head_element(
        &self,
        name: &str,
        attributes: &[(&str, String)],
        contents: Option<String>,
    ) {
        if name == "title" {
            if let Some(title) = &contents {
                self.window.set_title(title);
            }
            return;
        }
        self.proxy.send_event(BlitzShellEvent::embedder_event(
            DioxusNativeEvent::CreateHeadElement {
                window: self.os,
                name: name.to_owned(),
                attributes: attributes
                    .iter()
                    .map(|(k, v)| ((*k).to_owned(), v.clone()))
                    .collect(),
                contents,
            },
        ));
    }

    fn create_head_component(&self) -> bool {
        true
    }
}

#[cfg(target_os = "macos")]
mod mac;

/// A screen length as the model's points (screens fit an `f32`).
#[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
const fn narrow(v: f64) -> f32 {
    v as f32
}
