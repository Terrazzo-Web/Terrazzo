# Terrazzo

**Terrazzo** is a lightweight web UI framework for Rust and WebAssembly. It
lets you write browser UI, event handlers, reactive state, reusable widgets, and
server/static-asset glue in Rust while still using normal Rust formatting and
normal Rust ownership.

The umbrella `terrazzo` crate re-exports:

- `#[html]` and `#[template]` from `terrazzo-macro`
- client-side rendering types such as `XElement`, `XSignal`, `MutableSignal`,
  attributes, batches, and widgets
- server-side static asset helpers such as `declare_scss_asset!`
- utility macros such as `autoclone!`

See the demo in `demo/src/demo.rs` and its modules for runnable examples.

## Design

Terrazzo is intentionally smaller than frameworks such as Dioxus or Leptos. Its
core job is templated DOM construction with reactive updates. A template records
which signals it reads, and when one of those signals changes only the dependent
template is re-evaluated.

Terrazzo does not make signals `Copy` through arena allocation. Signals are
ordinary owned Rust values, so moving them into closures generally means calling
`.clone()` or using `autoclone!`. That is slightly more explicit, but it keeps
Rust's ownership model visible and avoids relying on implicit signal lifetimes.

Terrazzo does not require a custom CLI for authoring UI. Templates are Rust
functions, so `rustfmt`, `cargo`, clippy, and Bazel all continue to work on the
code you write.

## Basic component

Terrazzo uses two macros together:

- `#[html]` rewrites calls such as `div(...)`, `button(...)`, and `input(...)`
  into DOM element builders.
- `#[template]` turns a function into a reactive template. Use
  `#[template(debug = true)]` or `#[html(debug = true)]` when you need to inspect
  generated code.

```rust
use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use web_sys::MouseEvent;

#[autoclone]
#[html]
#[template(tag = div)]
fn counter(#[signal] count: i32, count_signal: XSignal<i32>) -> XElement {
    tag(
        class = "counter",
        button(
            click = move |_ev: MouseEvent| {
                autoclone!(count_signal);
                count_signal.update(|count| Some(*count - 1));
            },
            "-",
        ),
        span("Count: {count}"),
        button(
            click = move |_ev: MouseEvent| {
                autoclone!(count_signal);
                count_signal.update(|count| Some(*count + 1));
            },
            "+",
        ),
    )
}
```

`#[signal] count: i32` means this template re-runs when the signal passed for
`count` changes. Push signal reads as low as possible in the DOM tree so updates
only refresh the smallest useful part of the page.

## Tag templates

A tag template is a template whose outer element is fixed by the template
attribute. The special `tag(...)` call creates that outer element.

```rust
#[html]
#[template(tag = div)]
fn panel(title: &str) -> XElement {
    tag(
        class = "panel",
        h2("{title}"),
        div(class = "panel-body", "Content"),
    )
}
```

Tag templates are useful when a component always renders the same root tag and
you want a compact return expression. They are also common for helper templates
that need a stable root element, such as `#[template(tag = img)]`.

## Attribute templates

Templates can also return an attribute value instead of an element. Use
`#[template(wrap = true)]` for reactive attributes, classes, styles, titles, and
other values.

```rust
#[template(wrap = true)]
fn active_class(#[signal] active: bool) -> XAttributeValue {
    active.then_some("active")
}

#[template(wrap = true)]
fn width(#[signal] percent: i32) -> XAttributeValue {
    format!("{percent}%")
}

#[html]
fn row(active: XSignal<bool>, percent: XSignal<i32>) -> XElement {
    div(
        class = "row",
        class %= active_class(active),
        style::width %= width(percent),
        "Resizable row",
    )
}
```

The `%=` form attaches a dynamic value. The template re-runs when one of its
`#[signal]` inputs changes.

## Attributes and styles

Attributes are written as named arguments. Raw Rust keywords can be escaped with
`r#`.

```rust
#[html]
fn form() -> XElement {
    input(
        r#type = "text",
        name = "project",
        value = "Terrazzo",
    )
}
```

CSS properties use the `style::` namespace:

```rust
#[html]
fn boxy() -> XElement {
    div(
        style::display = "flex",
        style::font_family = "Arial",
        style::width = "100%",
        "Styled from Rust",
    )
}
```

You can also append full style fragments with `style = "...;"`, which is useful
when a helper returns several CSS declarations.

## Optional attributes

Use `|=` when an attribute or style may be absent. Any value convertible into
`Option` works, and `None` means no attribute is emitted.

```rust
#[html]
fn maybe_disabled(disabled: bool) -> XElement {
    button(
        disabled |= disabled.then_some("disabled"),
        title |= (!disabled).then_some("Ready"),
        style::visibility |= disabled.then_some("hidden"),
        "Run",
    )
}
```

This is the same mechanism used by dynamic attribute templates that return
`Option`, such as `active.then_some("active")`.

## Conditionally compiled attributes

Because templates are Rust syntax, `#[cfg(...)]` can be used directly on
attributes, styles, events, and children.

```rust
#[html]
fn build_marker() -> XElement {
    div(
        #[cfg(feature = "bazel")]
        class = "bazel",
        #[cfg(not(feature = "bazel"))]
        class = "cargo",
        #[cfg(feature = "debug")]
        data_mode = "debug",
        "Build-specific markup",
    )
}
```

This is useful for test-only classes, debug-only labels, feature-gated handlers,
and product builds where diagnostics should disappear entirely.

## Events

DOM events are written as attributes whose value is a closure. Common event
names such as `click`, `change`, `keydown`, `mouseover`, `mouseout`, and
`dblclick` are typed by the macro.

```rust
use terrazzo::widgets::more_event::MoreEvent as _;
use web_sys::HtmlInputElement;
use web_sys::KeyboardEvent;

#[html]
fn editor_name(name: XSignal<String>) -> XElement {
    input(
        change = move |ev: web_sys::Event| {
            let input: HtmlInputElement =
                ev.current_target_element("name").or_throw("name input");
            name.set(input.value());
        },
        keydown = move |ev: KeyboardEvent| {
            if ev.key() == "Enter" {
                ev.prevent_default();
            }
        },
    )
}
```

Event closures are normal Rust closures. When a `move` closure needs to capture
the same signal or pointer more than once, annotate the function with
`#[autoclone]` and place `autoclone!(value);` at the top of the closure.

## CSS modules and static styles

Terrazzo projects commonly keep SCSS beside the Rust module that uses it:

```rust
terrazzo_css::import_style!(style, "tabs.scss");

#[html]
fn tabs() -> XElement {
    div(class = style::TABS, "Tabs")
}
```

`terrazzo_css::import_style!` reads the SCSS file at compile time and exposes
hashed class-name constants. The Bazel `scss_rule` or the CSS CLI bundles the
rewritten SCSS into a static asset. Server builds can install SCSS/CSS assets
with `declare_scss_asset!`.

## Keys and lifecycle hooks

Use the special `key` attribute to keep DOM identity stable when lists reorder or
when a component should be replaced only when a key changes.

```rust
#[html]
fn item(id: u64, label: &str) -> XElement {
    div(key = "{id}", "{label}")
}
```

Templates also support render hooks:

```rust
#[html]
fn measured() -> XElement {
    div(
        before_render = |_: &Element| diagnostics::info!("before render"),
        after_render = |_: &Element| diagnostics::info!("after render"),
        "Measure me",
    )
}
```

## Widgets and utilities

The client runtime includes reusable widgets and helpers under
`terrazzo::widgets`, including tabs, editable fields, select controls, debounce,
cancellable timers, resize events, element capture, and link helpers. The crate
also exposes `drop_list` utilities for cleanup registration.

## Build features

The crate is split by feature:

- `client`: client-side DOM rendering, signals, widgets, and macros.
- `server`: static asset helpers and server-side support.
- `debug`: debug variants used by local/dev builds.
- `diagnostics`: verbose names and diagnostics-friendly output.

Most applications build separate client WASM and server binaries with the same
Rust source tree but different feature sets.
