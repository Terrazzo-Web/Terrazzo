# Terrazzo

Terrazzo is a lightweight, simple and efficient web UI framework based on Rust and WASM.

# Prior art

This project was inspired by frameworks like Dioxus and Leptos.

These frameworks are based on a simple concept: reactivity. When the function that computes a
component (i.e: an HTML node) reads a signal, that UI component will automagically be re-evaluated
whenever the signal changes. The implicit reactive ownership tree derives from the UI components
tree (the DOM), and allows making the signals arena-allocated. In Rust terms, it means `Signal` can
be `Copy` and not just `Clone`, which greatly improves the ergonomics.

- We don't have to guess how and when to call `.clone()`, especially when signals are used in
  closures.
- We can always pass Signals as values so we don't have to deal with references, lifetimes and the
  Rust borrow-checker.

In other words, we can leverage the powerful Rust type system, use one language for both the UI and
the backend server implementation, and get all the benefits of the rich Rust ecosystem.

# Why Terrazzo?

This project was started out of frustration dealing with frameworks like Dioxus and Leptos.

I believe that making signals `Copy` for the sake of ergonomics is an anti-pattern.
- With Dioxus, using signals must obey a strict set of rules can cannot be enforced otherwise by
  the compiler.
  https://dioxuslabs.com/learn/0.6/reference/hooks/#rules-of-hooks
- With Leptos, bugs can arise if signals are used after they are (implicitely) disposed. I feel
  like this is completely missing the point of using Rust, since the whole reason this language
  exists is precisely to prevents use-after-free bugs.
  https://book.leptos.dev/appendix_life_cycle.html?highlight=owner#signals-can-be-used-after-they-are-disposed

I prefer dealing with the Rust borrow-checker and any other kind of static analysis annoyance, even
if it means I have to add explicit calls to `.clone()` and add some extra boilerplate. This is a
small price to pay if I can avoid wasting time debugging large classes of bugs.

The promise of Rust is that the compiler has your back: if it compiles, it works. Rust code runs
faster than other languages, not because "for-loops" are faster in Rust, but because Rust codebases
are easier to refactor and optimize. You can replace a deep copy with a reference, and that promise
will hold: if it compiles, it works. Else, the Rust compiler will help you figure out when to call
`.clone()`, when to use use ref-counter pointers, or when to guard mutable state with a mutex or
use a cell.

# What does Terrazzo look like?
