//! The few controls the extensions page draws (buttons, badges, cards,
//! alerts, a text field, a switch), with app-shell's theme (`var(--as-…)`)
//! and inline styles only: no stylesheet to load, the same in a browser
//! and in Blitz. Their props are the ones the page was written against
//! (Architect's `architect-ui`), so it reads as it did.

use dioxus::prelude::*;

/// A button's look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Outline,
    Ghost,
    Destructive,
}

/// A button's size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSize {
    #[default]
    Default,
    Small,
}

#[component]
pub fn Button(
    #[props(default)] variant: ButtonVariant,
    #[props(default)] size: ButtonSize,
    #[props(default)] on_click: Option<Callback<MouseEvent>>,
    #[props(default)] disabled: bool,
    children: Element,
) -> Element {
    let (bg, fg, border) = match variant {
        ButtonVariant::Primary => ("var(--as-accent)", "var(--as-accent-text)", "transparent"),
        ButtonVariant::Outline => ("transparent", "var(--as-text)", "var(--as-border)"),
        ButtonVariant::Ghost => ("transparent", "var(--as-text)", "transparent"),
        ButtonVariant::Destructive => ("var(--as-danger)", "#fff", "transparent"),
    };
    let (h, pad, font) = match size {
        ButtonSize::Default => (32, 12, 13),
        ButtonSize::Small => (26, 8, 12),
    };
    let opacity = if disabled { "0.5" } else { "1" };
    rsx! {
        button {
            class: "as-hov",
            disabled,
            style: "display: inline-flex; align-items: center; justify-content: center; gap: 6px; height: {h}px; padding: 0 {pad}px; \
                    border: 1px solid {border}; border-radius: var(--as-radius); background-color: {bg}; color: {fg}; \
                    font-family: var(--as-font); font-size: {font}px; font-weight: 500; opacity: {opacity}; cursor: pointer; white-space: nowrap;",
            onclick: move |e| {
                if !disabled && let Some(f) = on_click {
                    f.call(e);
                }
            },
            {children}
        }
    }
}

/// A badge's look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeVariant {
    #[default]
    Primary,
    Secondary,
    Outline,
    Destructive,
}

#[component]
pub fn Badge(#[props(default)] variant: BadgeVariant, children: Element) -> Element {
    let (bg, fg, border) = match variant {
        BadgeVariant::Primary => ("var(--as-accent)", "var(--as-accent-text)", "transparent"),
        BadgeVariant::Secondary => ("var(--as-raised)", "var(--as-text)", "transparent"),
        BadgeVariant::Outline => ("transparent", "var(--as-muted)", "var(--as-border)"),
        BadgeVariant::Destructive => ("var(--as-danger)", "#fff", "transparent"),
    };
    rsx! {
        span {
            style: "display: inline-flex; align-items: center; height: 20px; padding: 0 8px; border-radius: 10px; \
                    border: 1px solid {border}; background-color: {bg}; color: {fg}; font-size: 11px; font-weight: 600; white-space: nowrap;",
            {children}
        }
    }
}

#[component]
pub fn Card(children: Element) -> Element {
    rsx! {
        div {
            style: "display: flex; flex-direction: column; gap: 12px; padding: 16px; border: 1px solid var(--as-border); \
                    border-radius: 12px; background-color: var(--as-surface); color: var(--as-text);",
            {children}
        }
    }
}

#[component]
pub fn CardHeader(children: Element) -> Element {
    rsx! { div { style: "display: flex; flex-direction: column; gap: 4px;", {children} } }
}

#[component]
pub fn CardTitle(children: Element) -> Element {
    rsx! { div { style: "font-size: 15px; font-weight: 600;", {children} } }
}

#[component]
pub fn CardDescription(children: Element) -> Element {
    rsx! { div { style: "font-size: 13px; color: var(--as-muted);", {children} } }
}

#[component]
pub fn CardContent(children: Element) -> Element {
    rsx! { div { style: "display: flex; flex-direction: column; font-size: 13px;", {children} } }
}

#[component]
pub fn CardFooter(children: Element) -> Element {
    rsx! { div { style: "display: flex; align-items: center;", {children} } }
}

/// An alert's look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlertVariant {
    #[default]
    Default,
    Destructive,
}

#[component]
pub fn Alert(#[props(default)] variant: AlertVariant, children: Element) -> Element {
    let (border, fg) = match variant {
        AlertVariant::Default => ("var(--as-border)", "var(--as-text)"),
        AlertVariant::Destructive => ("var(--as-danger)", "var(--as-danger)"),
    };
    rsx! {
        div {
            role: "alert",
            style: "display: flex; flex-direction: column; gap: 4px; padding: 10px 12px; border: 1px solid {border}; \
                    border-radius: var(--as-radius); color: {fg}; font-size: 13px;",
            {children}
        }
    }
}

#[component]
pub fn AlertTitle(children: Element) -> Element {
    rsx! { div { style: "font-weight: 600;", {children} } }
}

#[component]
pub fn AlertDescription(children: Element) -> Element {
    rsx! { div { style: "color: var(--as-muted);", {children} } }
}

/// A one-line text field, bound to `value`.
#[component]
pub fn Input(value: Signal<String>, #[props(default)] placeholder: String) -> Element {
    let mut value = value;
    rsx! {
        input {
            r#type: "text",
            value: "{value}",
            placeholder: "{placeholder}",
            "aria-label": "{placeholder}",
            style: "width: 100%; box-sizing: border-box; height: 32px; padding: 0 10px; border: 1px solid var(--as-border); \
                    border-radius: var(--as-radius); background-color: var(--as-bg); color: var(--as-text); \
                    font-family: var(--as-font); font-size: 13px; outline: none;",
            oninput: move |e| value.set(e.value()),
        }
    }
}

/// An on/off switch over `checked`; `on_change` hears the new state.
#[component]
pub fn Switch(
    checked: Signal<bool>,
    #[props(default)] on_change: Option<Callback<bool>>,
) -> Element {
    let on = checked();
    let (track, left) = if on {
        ("var(--as-accent)", 16)
    } else {
        ("var(--as-border)", 2)
    };
    rsx! {
        div {
            role: "switch",
            "aria-checked": "{on}",
            style: "position: relative; flex: none; width: 34px; height: 20px; border-radius: 10px; background-color: {track}; cursor: pointer;",
            onclick: move |_| {
                let next = !on;
                let mut checked = checked;
                checked.set(next);
                if let Some(f) = on_change {
                    f.call(next);
                }
            },
            div { style: "position: absolute; top: 2px; left: {left}px; width: 16px; height: 16px; border-radius: 8px; background-color: #fff;" }
        }
    }
}

/// A row of choices, one on: `options` are `(value, label)`.
#[component]
pub fn SegmentedControl(
    value: String,
    on_change: Callback<String>,
    options: Vec<(String, String)>,
) -> Element {
    rsx! {
        div {
            role: "radiogroup",
            style: "display: inline-flex; flex-wrap: wrap; gap: 2px; padding: 2px; border-radius: var(--as-radius); background-color: var(--as-raised);",
            for (v, label) in options {
                {
                    let on = v == value;
                    let (bg, fg) = if on { ("var(--as-bg)", "var(--as-text)") } else { ("transparent", "var(--as-muted)") };
                    rsx! {
                        div {
                            key: "{v}",
                            role: "radio",
                            "aria-checked": "{on}",
                            style: "height: 24px; padding: 0 10px; display: flex; align-items: center; border-radius: var(--as-radius); \
                                    background-color: {bg}; color: {fg}; font-size: 12px; cursor: pointer; white-space: nowrap;",
                            onclick: move |_| on_change.call(v.clone()),
                            "{label}"
                        }
                    }
                }
            }
        }
    }
}

/// Where a list would be, when it has nothing.
#[component]
pub fn EmptyState(message: String) -> Element {
    rsx! {
        div { style: "padding: 24px 12px; text-align: center; font-size: 13px; color: var(--as-muted);", "{message}" }
    }
}

/// A list's row: its label, a detail under it, a tag at its end.
#[component]
pub fn ListRow(
    label: String,
    #[props(default)] detail: String,
    #[props(default)] tag: String,
    #[props(default)] on_click: Option<Callback<()>>,
) -> Element {
    rsx! {
        div {
            class: "as-hov",
            role: "button",
            style: "display: flex; align-items: center; gap: 8px; min-height: 40px; padding: 6px 10px; border-radius: var(--as-radius); cursor: pointer;",
            onclick: move |_| {
                if let Some(f) = on_click {
                    f.call(());
                }
            },
            div { style: "flex: 1 1 0px; min-width: 0; display: flex; flex-direction: column; overflow: hidden;",
                span { style: "font-size: 13px; color: var(--as-text); white-space: nowrap; overflow: hidden;", "{label}" }
                if !detail.is_empty() {
                    span { style: "font-size: 12px; color: var(--as-muted); white-space: nowrap; overflow: hidden;", "{detail}" }
                }
            }
            if !tag.is_empty() {
                Badge { variant: BadgeVariant::Outline, "{tag}" }
            }
        }
    }
}

/// A label and its value on one line.
#[component]
pub fn KeyValueRow(label: String, value: String, #[props(default = true)] mono: bool) -> Element {
    let font = if mono {
        "ui-monospace, SFMono-Regular, Menlo, monospace"
    } else {
        "inherit"
    };
    rsx! {
        div { style: "display: flex; gap: 12px; padding: 4px 0; font-size: 12px;",
            span { style: "flex: none; width: 80px; color: var(--as-muted);", "{label}" }
            span { style: "flex: 1 1 0px; min-width: 0; overflow: hidden; font-family: {font}; color: var(--as-text);", "{value}" }
        }
    }
}

/// A choice of the platform's own, bound to `value`.
#[component]
pub fn NativeSelect(
    value: Signal<String>,
    #[props(default)] on_change: Option<Callback<FormEvent>>,
    children: Element,
) -> Element {
    let mut value = value;
    rsx! {
        select {
            value: "{value}",
            style: "height: 30px; padding: 0 8px; border: 1px solid var(--as-border); border-radius: var(--as-radius); \
                    background-color: var(--as-bg); color: var(--as-text); font-family: var(--as-font); font-size: 13px;",
            onchange: move |e| {
                value.set(e.value());
                if let Some(f) = on_change {
                    f.call(e);
                }
            },
            {children}
        }
    }
}

#[component]
pub fn NativeSelectOption(value: String, children: Element) -> Element {
    rsx! { option { value: "{value}", {children} } }
}

/// A section's heading.
#[component]
pub fn SectionHeader(label: String) -> Element {
    rsx! {
        div { style: "padding: 8px 0 4px 0; font-size: 12px; font-weight: 600; letter-spacing: 0.02em; text-transform: uppercase; color: var(--as-heading);",
            "{label}"
        }
    }
}
