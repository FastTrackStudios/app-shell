//! Embeds drawn in the app's window: the view half of
//! `architect_extensions::embeds`.

use std::rc::Rc;

use dioxus::prelude::*;
use serde_json::Value;

use crate::interface::ExtensionUi;
use architect_extensions::contrib::{Claim, EmbedInput};
use architect_extensions::embeds::{EmbedSpec, Embeds};
use architect_extensions::linked::EmbedContext;

/// A page embed, as [`EmbedPages`] is asked for it.
#[derive(Clone)]
pub struct EmbedAsk {
    pub spec: EmbedSpec,
    /// What its runs may reach: the extension's capabilities.
    pub gate: architect_extensions::host::Gate,
    /// What it is given, live.
    pub input: Signal<EmbedInput>,
    /// A guest's content (a share link's note, #629): the page runs
    /// on a host that reaches nothing of the person's, `gate` unused.
    pub guest: bool,
}

impl std::fmt::Debug for EmbedAsk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbedAsk")
            .field("spec", &self.spec)
            .finish_non_exhaustive()
    }
}

/// Draws a page embed: `architect-pages` provides it
/// (`architect_pages::ui::use_embed_pages`), a sub-document sized to
/// its content. `None` leaves it to the extension's own view.
#[derive(Clone)]
pub struct EmbedPages(pub Rc<dyn Fn(&EmbedAsk) -> Option<Element>>);

/// The block `claim` claims, drawn by the extension that claims it,
/// inline and content-sized; `fallback` (the plain block) when none
/// does, or once it unloads.
///
/// `source` is the block's text (a fence's body, a note's), `props`
/// the host's properties for it (a note's frontmatter, a fence's info
/// string); the view is drawn again when either changes. The
/// `ExtensionUi` in context says what is loaded.
///
/// The person's own content only: a guest's (a share link's note) is
/// [`GuestEmbedView`]'s, which never draws with the person's reach.
#[component]
pub fn EmbedView(
    claim: Claim,
    source: String,
    #[props(default)] props: Value,
    #[props(default)] fallback: Option<Element>,
) -> Element {
    embed_view(&claim, source, props, fallback, false)
}

/// A guest's content (a share link's note, #629) that `claim` claims:
/// as [`EmbedView`], but never drawn with the person's reach. A page
/// embed draws it, on a host with no action, no host-service call and
/// no vault (`architect_pages::host::AppHost::guest`). A linked view
/// draws it only where the app's [`GuestEmbedPolicy`](architect_extensions::embeds::GuestEmbedPolicy)
/// (in context) lets its own linked extensions and its host is a
/// guest's; a process view never does. Otherwise `fallback` shows. A
/// component of its own, not a prop, so no caller can forget to say
/// so.
#[component]
pub fn GuestEmbedView(
    claim: Claim,
    source: String,
    #[props(default)] props: Value,
    #[props(default)] fallback: Option<Element>,
) -> Element {
    embed_view(&claim, source, props, fallback, true)
}

// A component's body: its hooks run in the component that calls it.
fn embed_view(
    claim: &Claim,
    source: String,
    props: Value,
    fallback: Option<Element>,
    guest: bool,
) -> Element {
    let now = EmbedInput {
        claim: claim.to_string(),
        source,
        props,
    };
    let mut input = use_signal(|| now.clone());
    use_effect(use_reactive!(|now| {
        if *input.peek() != now {
            input.set(now);
        }
    }));
    let plain = move || fallback.clone().unwrap_or_else(|| rsx! {});
    let Some(ui) = try_use_context::<ExtensionUi>() else {
        return plain();
    };
    let _ = ui.revision();
    let host = ui.host();
    let Some(spec) = Embeds::new(host.clone()).resolve(claim) else {
        return plain();
    };
    if guest {
        return draw_guest(&host, spec, input, plain);
    }
    draw(&host, spec, input, plain)
}

/// `spec` drawn for a guest's content, given `input`: by a page (its
/// own or its view's), on a host that reaches nothing of the person's
/// (`architect_pages::host::AppHost::guest`); by the app's own linked
/// view where the app's policy and its host allow it
/// (`GuestEmbedPolicy::draws_linked`); else `plain`.
///
/// The policy is the embedding app's, not this function's, because
/// only the app knows what its process holds. A linked view runs in
/// the app's process and a process view over its own connection: on a
/// desk both hold the person's session and cannot be kept from their
/// vaults, so by default neither draws a guest's content. An app
/// built for guests (no vault, no session, a host that refuses every
/// vault request) holds nothing to keep a view from, and its linked
/// views are the app itself.
fn draw_guest(
    host: &architect_extensions::host::Host,
    spec: EmbedSpec,
    input: Signal<EmbedInput>,
    plain: impl Fn() -> Element,
) -> Element {
    if host.get(&spec.extension).is_none() {
        return plain();
    }
    let key = format!("{}/{}", spec.extension, spec.embed.id);
    if spec.page.is_none()
        && try_consume_context::<architect_extensions::embeds::GuestEmbedPolicy>()
            .unwrap_or_default()
            .draws_linked(host)
        && let Some(view) = spec.embed.view.as_deref()
        && let Some(root) = host.linked_view(&spec.extension, view)
    {
        let max = spec.max_height;
        return rsx! {
            div { class: "embed", "data-embed": "{key}", "data-guest": "true",
                style: "{BOX} max-height: {max}px; overflow-y: auto;",
                LinkedEmbed { key: "{key}", root, input }
            }
        };
    }
    let Some(pages) = try_consume_context::<EmbedPages>() else {
        return plain();
    };
    let ask = EmbedAsk {
        // Unused for a guest: nothing passes.
        gate: std::sync::Arc::new(|_| Err("a guest's content runs nothing".to_owned())),
        input,
        spec,
        guest: true,
    };
    (pages.0)(&ask).map_or_else(plain, |page| {
        rsx! {
            div { class: "embed", "data-embed": "{key}", "data-guest": "true", style: "{BOX}", {page} }
        }
    })
}

/// An extension's embed `id` mounted by an editor's widget decoration
/// (`architect_extensions::editor`): drawn as an inline embed is, given `props` (the
/// widget's) and `source` (empty); nothing when the extension or the
/// embed is gone.
#[component]
pub fn WidgetView(extension: String, id: String, #[props(default)] props: Value) -> Element {
    let now = EmbedInput {
        claim: Claim::EmbedType("widget".to_owned()).to_string(),
        source: String::new(),
        props,
    };
    let mut input = use_signal(|| now.clone());
    use_effect(use_reactive!(|now| {
        if *input.peek() != now {
            input.set(now);
        }
    }));
    let Some(ui) = try_use_context::<ExtensionUi>() else {
        return rsx! {};
    };
    let _ = ui.revision();
    let host = ui.host();
    let Some(spec) = Embeds::new(host.clone()).of(&extension, &id) else {
        return rsx! {};
    };
    draw(&host, spec, input, || rsx! {})
}

/// A base's view of a type an extension adds (`[[base_views]]`, type
/// `kind`), drawn by it and given `input` (the view's spec and rows),
/// up to `max_height` points tall; `fallback` when no loaded extension
/// adds the type, or once it unloads.
#[component]
pub fn BaseViewEmbed(
    kind: String,
    input: architect_extensions::contrib::BaseViewInput,
    #[props(default = u32::MAX)] max_height: u32,
    #[props(default)] fallback: Option<Element>,
) -> Element {
    let now = EmbedInput {
        claim: Claim::EmbedType("base_view".to_owned()).to_string(),
        source: input.base.clone(),
        props: serde_json::to_value(&input).unwrap_or_default(),
    };
    let mut live = use_signal(|| now.clone());
    use_effect(use_reactive!(|now| {
        if *live.peek() != now {
            live.set(now);
        }
    }));
    let plain = move || fallback.clone().unwrap_or_else(|| rsx! {});
    let Some(ui) = try_use_context::<ExtensionUi>() else {
        return plain();
    };
    let _ = ui.revision();
    let host = ui.host();
    let Some(mut spec) = Embeds::new(host.clone()).base_view(&kind) else {
        return plain();
    };
    spec.max_height = max_height;
    draw(&host, spec, live, plain)
}

/// `spec` drawn, given `input`: a page's sub-document, a linked view in
/// the host's document, or a process's; `plain` when none can be.
// `spec` is consumed only with feature `remote` (a process's embed).
#[allow(clippy::needless_pass_by_value)]
fn draw(
    host: &architect_extensions::host::Host,
    spec: EmbedSpec,
    input: Signal<EmbedInput>,
    plain: impl Fn() -> Element,
) -> Element {
    let Some(loaded) = host.get(&spec.extension) else {
        return plain();
    };
    let key = format!("{}/{}", spec.extension, spec.embed.id);
    let max = spec.max_height;
    if spec.page.is_some()
        && let Some(pages) = try_consume_context::<EmbedPages>()
    {
        let ask = EmbedAsk {
            gate: architect_extensions::host::gate(&loaded.manifest),
            input,
            spec: spec.clone(),
            guest: false,
        };
        if let Some(page) = (pages.0)(&ask) {
            return rsx! {
                div { class: "embed", "data-embed": "{key}", style: "{BOX}", {page} }
            };
        }
    }
    if let Some(view) = spec.embed.view.as_deref()
        && let Some(root) = host.linked_view(&spec.extension, view)
    {
        // In the host's own document: laid out by it, its events its.
        return rsx! {
            div { class: "embed", "data-embed": "{key}",
                style: "{BOX} max-height: {max}px; overflow-y: auto;",
                LinkedEmbed { key: "{key}", root, input }
            }
        };
    }
    #[cfg(all(feature = "remote", unix))]
    if let Some(view) = spec.embed.view.clone()
        && let Some(processes) = crate::use_processes()
    {
        return rsx! {
            div { class: "embed", "data-embed": "{key}", style: "{BOX}",
                crate::remote::RemoteEmbed {
                    key: "{key}",
                    processes,
                    extension: spec.extension,
                    view,
                    input,
                    max_height: max,
                }
            }
        };
    }
    plain()
}

/// An embed's box in the host's flow: as wide as the text, as tall as
/// what is in it.
const BOX: &str = "display: flex; flex-direction: column; width: 100%; flex-shrink: 0;";

/// A linked-in view, given its input.
#[component]
fn LinkedEmbed(root: fn() -> Element, input: Signal<EmbedInput>) -> Element {
    use_context_provider(|| EmbedContext(input));
    root()
}
