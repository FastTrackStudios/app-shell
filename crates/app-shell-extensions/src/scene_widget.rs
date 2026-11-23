//! A remote view's scenes in the host's Blitz document: the widget that
//! draws an element's scene (`architect_extensions::scene`), its layers and
//! its overlays.

#![allow(unused_imports)]

use std::sync::Arc;

use architect_extensions::scene::*;
use peniko::kurbo::Affine;

impl Painted {
    /// Draws it into `scene` at `at`: its layers, placed by their
    /// animations at the clocks now, or its one scene.
    // kurbo's transforms compose with `*`: no overflow to guard.
    #[allow(clippy::arithmetic_side_effects)]
    pub(crate) fn draw(&mut self, scene: &mut Scene, at: Affine) {
        match &self.layers {
            Some(_) => {
                let now = self.clocks.read();
                // A flicked value coasts on: the host's, as a drag is.
                if let Some(receiver) = self.layers.as_mut() {
                    self.interaction.coast(&now, receiver.parts_mut().1);
                }
                let Some(layers) = &self.layers else { return };
                let lit = self.interaction.lit();
                layers
                    .layers()
                    .compose_lit(scene, at * self.transform, &now, layers.values(), lit);
                self.drawn = now;
                self.drawn_lit = lit;
                self.drawn_values = layers.values().revision();
                if let Some(alarm) = &self.alarm {
                    let _ = alarm.drawn(layers.layers().next(&now));
                }
            }
            None => scene.append_shared(&self.commands, at * self.transform),
        }
    }

    /// A pointer event at `at` (the element's CSS pixels) for its
    /// layers' regions: what it did ([`architect_scene::region::Outcome`]).
    // kurbo's transforms compose with `*`: no overflow to guard.
    #[allow(clippy::arithmetic_side_effects)]
    pub(crate) fn pointer(
        &mut self,
        kind: PointerKind,
        at: peniko::kurbo::Point,
        mods: architect_scene::region::Mods,
    ) -> architect_scene::region::Outcome {
        let Some(receiver) = self.layers.as_mut() else {
            return architect_scene::region::Outcome::default();
        };
        if self.transform.determinant().abs() < 1e-12 {
            return architect_scene::region::Outcome::default();
        }
        let at = self.transform.inverse() * at;
        let now = self.clocks.read();
        let (layers, values) = receiver.parts_mut();
        let i = &mut self.interaction;
        match kind {
            PointerKind::Move => i.moved(layers, &now, values, at),
            PointerKind::Down => i.pressed_with(layers, &now, values, at, mods),
            PointerKind::Up => i.released_with(layers, &now, values, at, mods),
            PointerKind::Gone => i.left(),
        }
    }

    /// Whether a layer moves on a running clock, or the engine's values
    /// it reads moved since it was drawn: drawn every frame.
    pub(crate) fn moving(&self) -> bool {
        self.layers.as_ref().is_some_and(|l| {
            l.layers().moving(&self.clocks.read())
                || self.interaction.coasting()
                || (l.values().engine().is_some() && l.values().revision() != self.drawn_values)
        })
    }

    /// Whether a frame now would draw something else than the last one:
    /// a batch came, a layer moves, or a clock one reads was moved.
    pub(crate) fn stale(&self) -> bool {
        self.dirty
            || self.interaction.pending()
            || self.interaction.lit() != self.drawn_lit
            || self
                .layers
                .as_ref()
                .is_some_and(|l| l.values().revision() != self.drawn_values)
            || self
                .layers
                .as_ref()
                .is_some_and(|l| l.layers().stale(&self.drawn, &self.clocks.read()))
    }
}

/// What a pointer did, for a scene's regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PointerKind {
    Move,
    Down,
    Up,
    /// It left the element, or its press was lost.
    Gone,
}

/// An element's scene as its widget and its document share it: updated
/// in place, so a new transform keeps the size the widget last told.
#[derive(Debug)]
pub(crate) struct Painted {
    pub(crate) commands: Commands,
    /// Its layers, once its view sent any ([`LAYERS_ATTR`]): drawn in
    /// place of `commands`.
    pub(crate) layers: Option<architect_scene::ops::Receiver>,
    /// What its layers' animations read: the app's clocks, the same in
    /// every window.
    pub(crate) clocks: architect_scene::Clocks,
    /// The clocks it was last drawn at.
    pub(crate) drawn: architect_scene::Readings,
    /// Wakes its window at its layers' next step and when a clock jumps.
    pub(crate) alarm: Option<Arc<architect_scene::wake::Alarm>>,
    /// The pointer over its layers' regions.
    pub(crate) interaction: architect_scene::region::Interaction,
    /// What it highlighted, and its values' revision, when last drawn.
    pub(crate) drawn_lit: Option<architect_scene::region::Lit>,
    pub(crate) drawn_values: u64,
    pub(crate) transform: Affine,
    pub(crate) element: u64,
    pub(crate) dirty: bool,
    /// Where its element's top left is in the window, CSS pixels: where
    /// an [`OverlayLayer`] draws it.
    pub(crate) origin: (f64, f64),
}

/// What draws a view's scenes over the whole app.
///
/// For an extension granted `overlay`: every scene of the view, at its element's place in the
/// window, unclipped. Its widget ([`OverlayWidget`]) fills the window in
/// the host's own document, above the panes, and takes no input; the
/// scene's element in the pane still has its size and its input.
#[derive(Clone, Default)]
pub struct OverlayLayer {
    pub(crate) scenes: Arc<std::sync::Mutex<Vec<Arc<std::sync::Mutex<Painted>>>>>,
    pub(crate) version: Arc<std::sync::atomic::AtomicU64>,
}

impl OverlayLayer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Something it draws changed.
    pub(crate) fn touch(&self) {
        self.version
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub(crate) fn add(&self, painted: Arc<std::sync::Mutex<Painted>>) {
        self.scenes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(painted);
        self.touch();
    }

    pub(crate) fn remove(&self, painted: &Arc<std::sync::Mutex<Painted>>) {
        self.scenes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|p| !Arc::ptr_eq(p, painted));
        self.touch();
    }

    /// Its widget, for an element filling the window.
    #[must_use]
    pub fn widget(&self) -> OverlayWidget {
        OverlayWidget {
            layer: self.clone(),
            drawn: u64::MAX,
        }
    }
}

/// An [`OverlayLayer`] as a Blitz widget over the whole window.
pub struct OverlayWidget {
    layer: OverlayLayer,
    drawn: u64,
}

impl OverlayWidget {
    /// Whether `test` holds of any scene it draws.
    fn any(&self, test: impl Fn(&Painted) -> bool) -> bool {
        self.layer
            .scenes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .any(|p| test(&p.lock().unwrap_or_else(std::sync::PoisonError::into_inner)))
    }
}

// kurbo's transforms compose with `*`: no overflow to guard.
#[allow(clippy::arithmetic_side_effects)]
impl blitz_dom::Widget for OverlayWidget {
    fn needs_redraw(&self) -> bool {
        self.layer
            .version
            .load(std::sync::atomic::Ordering::Relaxed)
            != self.drawn
            || self.any(Painted::stale)
    }

    fn requires_redraw(&self) -> bool {
        self.any(Painted::moving)
    }

    fn paint(
        &mut self,
        _: &mut dyn anyrender::RenderContext,
        _: &blitz_dom::node::ComputedStyles,
        _: u32,
        _: u32,
        scale: f64,
    ) -> Scene {
        self.drawn = self
            .layer
            .version
            .load(std::sync::atomic::Ordering::Relaxed);
        let scale = if scale > 0.0 { scale } else { 1.0 };
        let mut scene = Scene::new();
        let scenes = self
            .layer
            .scenes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        for painted in scenes {
            let mut p = painted
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let at = Affine::scale(scale) * Affine::translate(p.origin);
            p.draw(&mut scene, at);
        }
        scene
    }
}

/// The host's widget: a scene's content at its transform, scaled to the
/// display, telling the view when its size changes.
pub(crate) struct SceneWidget {
    pub(crate) painted: Arc<std::sync::Mutex<Painted>>,
    pub(crate) overlay: bool,
    pub(crate) events:
        futures::channel::mpsc::UnboundedSender<architect_extensions::wire::ViewEvent>,
    pub(crate) resizes: Arc<std::sync::atomic::AtomicU64>,
    pub(crate) size: Option<(u32, u32)>,
}

impl blitz_dom::Widget for SceneWidget {
    // Drawn again when its view sent something, and every frame its
    // layers move on their own: the view is not asked for those.
    fn needs_redraw(&self) -> bool {
        self.painted
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stale()
    }

    fn requires_redraw(&self) -> bool {
        self.painted
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .moving()
    }

    fn paint(
        &mut self,
        _: &mut dyn anyrender::RenderContext,
        _: &blitz_dom::node::ComputedStyles,
        width: u32,
        height: u32,
        scale: f64,
    ) -> Scene {
        let scale = if scale > 0.0 { scale } else { 1.0 };
        let (drawn, element) = {
            let mut p = self
                .painted
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            p.dirty = false;
            // Drawn over the whole app by its view's `OverlayLayer`.
            let mut scene = Scene::new();
            if !self.overlay {
                scene.push_clip_layer(
                    peniko::Fill::NonZero,
                    Affine::IDENTITY,
                    &peniko::kurbo::Rect::new(0.0, 0.0, f64::from(width), f64::from(height)),
                );
                p.draw(&mut scene, Affine::scale(scale));
                scene.pop_layer();
            }
            // What its regions heard reaches the view once a frame.
            for event in p.interaction.take() {
                let _ = self
                    .events
                    .unbounded_send(architect_extensions::wire::ViewEvent {
                        element: p.element,
                        name: REGION_EVENT.to_owned(),
                        bubbles: false,
                        data: architect_extensions::wire::EventData::Form(region_value(&event)),
                    });
            }
            (scene, p.element)
        };
        if self.size != Some((width, height)) {
            self.size = Some((width, height));
            self.resizes
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let size = serde_json::json!({
                "width": f64::from(width) / scale,
                "height": f64::from(height) / scale,
                "scale": scale,
            });
            let _ = self
                .events
                .unbounded_send(architect_extensions::wire::ViewEvent {
                    element,
                    name: RESIZE_EVENT.to_owned(),
                    bubbles: false,
                    data: architect_extensions::wire::EventData::Form(size.to_string()),
                });
        }
        drawn
    }
}
