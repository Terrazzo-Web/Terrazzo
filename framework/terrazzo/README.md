# Terrazzo

Terrazzo is a lightweight, simple and efficient web UI framework based on Rust and WASM.

## Prior art

This project was inspired by frameworks like [Dioxus](https://dioxuslabs.com/learn/0.6/) and [Leptos](https://leptos.dev).

These frameworks are based on Rust and WASM:
- Both the server-side and client-side logic is written in Rust
- Rust↔Javascript interop based on [wasm_bindgen](https://docs.rs/wasm-bindgen/latest/wasm_bindgen/)
  allows creating dynamic web pages using the DOM API.

Like many other frameworks, the core is built around a simple concept: reactivity. When a function
computes a component (i.e: an HTML node), it records which signals are being used. Then, whenever
one of those signals changes, the function is automatically re-evaluated again, and the UI is
updated.

The implicit reactive ownership tree derives from the UI components tree (the DOM), and allows making
the signals arena-allocated. In Rust terms, it means `Signal` can be `Copy` and not just `Clone`,
which greatly improves the ergonomics.

- We don't have to guess how and when to call `.clone()`, especially when signals are used in
  closures.
- We can always pass signals as values so we don't have to deal with references, lifetimes and the
  Rust borrow-checker.

In other words, we can leverage the powerful Rust type system, use one language for both the UI and
the backend server implementation, and get all the benefits of the rich Rust ecosystem.

## Why Terrazzo?

The goal of Terrazzo isn't to replace Dioxus or Leptos. It's a lightweight, bare-bones alternative
that aims to achieve one simple task and do it well: a templating system for UI.

Dioxus and Leptos are incredibly feature-rich, but are also prone to bugs.

### Arena-allocated signals and use-after-free bugs
I believe that making signals `Copy` using arena allocation for the sake of ergonomics is an
anti-pattern.
- With Dioxus, use of signals must obey a strict set of rules can that cannot be enforced otherwise
  by the Rust compiler.
  <https://dioxuslabs.com/learn/0.6/reference/hooks/#rules-of-hooks>
- With Leptos, bugs can arise if signals are used after they are (implicitly) disposed. I feel
  like this is completely missing the point of using Rust, since once of the main selling points of
  this language is precisely to [prevent use-after-free bugs](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html#dangling-references).
  [Appendix: The Life Cycle of a Signal](https://book.leptos.dev/appendix_life_cycle.html?highlight=owner#signals-can-be-used-after-they-are-disposed)

I prefer dealing with the Rust borrow-checker and any other kind of static analysis annoyance, even
if it means I have to add explicit calls to `.clone()` and add some extra boilerplate. This is a
small price to pay if I can avoid wasting time debugging large classes of bugs.

The promise of Rust is that the compiler has your back: if it compiles, it works. Rust code runs
faster than other languages, not because "for-loops" are faster in Rust, but because Rust codebases
are easier to refactor and optimize. You can replace a deep copy with a reference, and that promise
will hold: if it compiles, it works. Else, the Rust compiler will help you figure out when to call
`.clone()`, when to use use ref-counting pointers, or when to guard mutable state with a mutex or
use a cell.

### Hydration bugs
Server-side rendering is a hard-to-use feature. It only works if the server-side code generates the
same page as the client-side code would. In theory, they should always match since the exact same
code runs server- and client-side, it's just an optimization. In practice, it's not necessarily the
case, so avoiding these bugs requires careful debugging and testing.
[Hydration Bugs *(and how to avoid them)*](https://book.leptos.dev/ssr/24_hydration_bugs.html)

### Custom tooling
One of the biggest selling points for Rust is strong tooling, including `cargo`, `rustfmt` and
`clippy`.
- The [Dioxus CLI](https://dioxuslabs.com/learn/0.6/CLI/) is an unnecessary annoyance
- The `rsx! { ... }` and `view! { ... }` macros to write HTML templates look nice at first,
  but don't work with the standard Rust formatter.

## What does Terrazzo look like?

Terrazzo uses two different macros
- The `#[template]` turns a function into a template. Use `#[template(debug = true)]` to see what
  the generated code looks like.
- The `#[html]` adds syntactic sugar to replace function calls where the name matches one of the
  well-known [HTML tags](https://github.com/Terrazzo-Web/Terrazzo/blob/readme/framework/macro/src/arguments.rs#L31-L47)
  into a Rust struct representing an HTML tag.
  Use `#[html(debug = true)]` to see what the generated code looks like.

```
# fn main() {
# #[cfg(feature = "client")] {
# use terrazzo::html;
# use terrazzo::prelude::*;
# use terrazzo::template;
# struct State { value: i32, signal: XSignal<String> }
# impl State { fn click(&self) { println!("Click!"); } }
#[template]
#[html]
pub fn my_main_component() -> XElement {
    let state = State {
        value: 123,
        signal: XSignal::new("signal", "state".to_owned()),
    };
    let state_value = state.value;
    return div(
        class = "main-component",
        style::width = "100%",
        click = move |event| state.click(),
        "text node {state_value}",
        static_component(),
        dynamic_component(state.signal.clone()),
    );
}

#[template(tag = div)]
#[html]
fn static_component() -> XElement {
    tag("static value")
}

#[template(tag = div)]
#[html]
fn dynamic_component(#[signal] value: String) -> XElement {
    tag("Dynamic: ", "{value}")
}
# } // #[cfg(feature = "client")]
# } // fn main()
```

See [demo.rs](https://github.com/Terrazzo-Web/Terrazzo/blob/main/demo/src/demo.rs).
