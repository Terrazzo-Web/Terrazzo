# autoclone

`autoclone` is a small proc macro that clones variables before they are captured
by a `move` closure or `async` block.

It is useful in Rust UI and async code where the same `Rc`, `Arc`, signal,
callback handle, or other cloneable value needs to be moved into several nested
closures while still being usable afterward.

## The problem

Without `autoclone`, each `move` closure needs a separate binding outside the
closure:

```rust
fn without_autoclone() {
    let message = "hello".to_owned();

    let message_for_callback = message.clone();
    do_with_owned(move || {
        drop(message_for_callback);
    });

    let _still_available = message.clone();
}

fn do_with_owned(callback: impl FnOnce()) {
    callback();
}
```

That works, but it gets noisy quickly when there are multiple values or nested
callbacks.

## With autoclone

Annotate the function with `#[autoclone]`, then put `autoclone!(...)` at the top
of the closure or async block that needs owned clones.

```rust
use autoclone::autoclone;

#[autoclone]
fn with_autoclone() {
    let message = "hello".to_owned();

    do_with_owned(move || {
        autoclone!(message);
        drop(message);
    });

    let _still_available = message.clone();
}

fn do_with_owned(callback: impl FnOnce()) {
    callback();
}
```

The macro rewrites the closure roughly like this:

```rust
fn expanded() {
    let message = "hello".to_owned();

    do_with_owned({
        let message = message.to_owned();
        move || {
            drop(message);
        }
    });

    let _still_available = message.clone();
}
```

`autoclone!(...)` is syntax consumed by the attribute macro. There is no runtime
macro call after expansion.

## Multiple values

Clone several captures at once:

```rust
use autoclone::autoclone;

#[autoclone]
fn multiple_values() {
    let greeting = "hello".to_owned();
    let subject = "world".to_owned();

    do_with_owned(move || {
        autoclone!(greeting, subject);
        println!("{greeting}, {subject}");
    });

    println!("outside: {greeting}, {subject}");
}
```

This keeps the list of cloned values close to the closure body, where it is
easiest to update while editing.

## Nested closures

Each closure or async block declares the values it needs:

```rust
use autoclone::autoclone;

#[autoclone]
fn nested() {
    let first = "hello".to_owned();
    let second = "world".to_owned();

    do_with_owned(move || {
        autoclone!(first, second);

        do_with_owned(move || {
            autoclone!(first, second);
            println!("inner: {first} {second}");
        });

        println!("outer: {first} {second}");
    });

    println!("after: {first} {second}");
}
```

Without `autoclone`, the same code needs new names for every level:

```rust
fn nested_without_autoclone() {
    let first = "hello".to_owned();
    let second = "world".to_owned();

    let outer_first = first.clone();
    let outer_second = second.clone();
    do_with_owned(move || {
        let inner_first = outer_first.clone();
        let inner_second = outer_second.clone();
        do_with_owned(move || {
            println!("inner: {inner_first} {inner_second}");
        });

        println!("outer: {outer_first} {outer_second}");
    });

    println!("after: {first} {second}");
}
```

## Async blocks

The macro also handles `async move` blocks:

```rust
use autoclone::autoclone;

#[autoclone]
fn spawn_work(sender: std::sync::Arc<String>) {
    let future = async move {
        autoclone!(sender);
        println!("{sender}");
    };

    let _can_still_use_sender = sender.clone();
    drop(future);
}
```

## Debugging expansion

Use `#[autoclone(debug = true)]` to print the before/after expansion at compile
time.

```rust
#[autoclone(debug = true)]
fn inspect_me() {
    let value = "hello".to_owned();
    do_with_owned(move || {
        autoclone!(value);
        drop(value);
    });
}
```

By default, `#[autoclone]` emits a compile error if it does not find any
`autoclone!(...)` marker. Use `#[autoclone(allow_unused = true)]` when applying
the attribute conditionally or during a refactor.

## Notes

- Values are cloned with `.to_owned()`.
- `autoclone!(...)` markers are only read when they appear at the beginning of a
  closure or block.
- The marker list accepts identifiers, for example
  `autoclone!(state, callback, sender);`.
- The crate also exposes `#[envelope]`, a separate helper for wrapping a type in
  a shared pointer.
