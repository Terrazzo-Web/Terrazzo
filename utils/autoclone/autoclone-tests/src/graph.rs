#![cfg(test)]

use autoclone::graph;

#[test]
fn simple_graph() {
    let run = |name| make_app::run(name);
    let _ = run("App".to_string());
}

#[allow(unused)]
#[graph]
mod make_app {

    pub struct App {
        name: String,
        comp1: Comp1,
        comp2: Comp2,
    }

    #[derive(Default)]
    struct Comp1;

    #[derive(Default)]
    struct Comp2;

    pub fn run(name: String, comp1: Comp1, comp2: Comp2) -> App {
        App { name, comp1, comp2 }
    }

    fn comp1() -> Comp1 {
        Comp1
    }

    fn comp2() -> Comp2 {
        Comp2
    }
}

#[test]
fn shared_owned_node() {
    assert_eq!(
        shared_values::run(),
        ("value".to_owned(), "value".to_owned())
    );
}

#[graph]
mod shared_values {
    pub fn run(value: String, wrapped: String) -> (String, String) {
        (value, wrapped)
    }

    fn value() -> String {
        "value".to_owned()
    }

    fn wrapped(value: Result<String, ()>) -> String {
        value.unwrap()
    }
}

#[test]
fn borrowed_node_and_last_use_need_no_clone() {
    assert_eq!(borrowed_values::run(), 10);
}

#[graph]
mod borrowed_values {
    struct Value(usize);

    pub fn run(borrowed: usize, value: Value) -> usize {
        borrowed + value.0
    }

    fn value() -> Value {
        Value(5)
    }

    fn borrowed(value: &Value) -> usize {
        value.0
    }
}

#[test]
fn shared_input() {
    assert_eq!(shared_inputs::run("value".to_owned()), "valuevalue");
}

#[graph]
mod shared_inputs {
    pub fn run(first: String, second: String) -> String {
        first + &second
    }

    fn first(input: String) -> String {
        input
    }

    fn second(input: String) -> String {
        input
    }
}

#[tokio::test]
async fn inferred_result_propagates_errors() {
    assert_eq!(inferred_result::run(false).await, Ok(15));
    assert_eq!(
        inferred_result::run(true).await,
        Err("component failed".to_owned())
    );
}

#[graph]
mod inferred_result {
    pub fn run(value: usize, doubled: usize) -> usize {
        value + doubled
    }

    async fn value(fail: bool) -> Result<usize, String> {
        if fail {
            Err("component failed".to_owned())
        } else {
            Ok(5)
        }
    }

    fn doubled(value: usize) -> Result<usize, String> {
        Ok(value * 2)
    }
}

#[tokio::test]
async fn explicit_result_converts_component_errors() {
    assert_eq!(explicit_result::run(0).await, Ok(3));
    assert_eq!(
        explicit_result::run(1).await,
        Err(explicit_result::Error::First)
    );
    assert_eq!(
        explicit_result::run(2).await,
        Err(explicit_result::Error::Second)
    );
}

#[graph]
mod explicit_result {
    #[derive(Debug, PartialEq)]
    pub enum Error {
        First,
        Second,
    }
    struct First;
    struct Second;

    impl From<First> for Error {
        fn from(_: First) -> Self {
            Self::First
        }
    }
    impl From<Second> for Error {
        fn from(_: Second) -> Self {
            Self::Second
        }
    }

    pub async fn run(first: usize, second: usize) -> Result<usize, Error> {
        Ok(first + second)
    }
    async fn first(fail: usize) -> Result<usize, First> {
        if fail == 1 { Err(First) } else { Ok(1) }
    }
    fn second(fail: usize) -> Result<usize, Second> {
        if fail == 2 { Err(Second) } else { Ok(2) }
    }
}

#[tokio::test]
async fn independent_components_are_polled_concurrently() {
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
    let mut future = std::pin::pin!(parallel_components::run(barrier.clone(), barrier));
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    // A sequential expansion would remain pending forever at the first barrier.
    for _ in 0..4 {
        if let std::task::Poll::Ready(value) =
            std::future::Future::poll(future.as_mut(), &mut context)
        {
            assert_eq!(value, 3);
            return;
        }
    }
    panic!("independent components did not make concurrent progress");
}

#[graph]
mod parallel_components {
    use std::sync::Arc;

    use tokio::sync::Barrier;

    pub fn run(first: usize, second: usize) -> usize {
        first + second
    }
    async fn first(left: Arc<Barrier>) -> usize {
        left.wait().await;
        1
    }
    async fn second(right: Arc<Barrier>) -> usize {
        right.wait().await;
        2
    }
}

#[test]
fn fallible_components_are_polled_concurrently() {
    for fail in [false, true] {
        let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
        let result = poll_joined(fallible_parallel::run(barrier.clone(), barrier, fail));
        assert_eq!(
            result,
            if fail {
                Err("failed".to_owned())
            } else {
                Ok(3)
            }
        );
    }
}

fn poll_joined<T>(future: impl std::future::Future<Output = T>) -> T {
    let mut future = std::pin::pin!(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    for _ in 0..4 {
        if let std::task::Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
    panic!("independent components did not make concurrent progress");
}

#[graph]
mod fallible_parallel {
    use std::sync::Arc;

    use tokio::sync::Barrier;

    pub fn run(first: &usize, second: usize) -> usize {
        first + second
    }
    async fn first(left: Arc<Barrier>) -> usize {
        left.wait().await;
        1
    }
    async fn second(right: Arc<Barrier>, fail: bool) -> Result<usize, String> {
        right.wait().await;
        if fail {
            Err("failed".to_owned())
        } else {
            Ok(2)
        }
    }
}

#[test]
fn joined_results_convert_errors_in_component_order() {
    for (fail_first, fail_second, expected) in [
        (false, false, Ok(3)),
        (true, false, Err(joined_results::Error::First)),
        (false, true, Err(joined_results::Error::Second)),
        (true, true, Err(joined_results::Error::First)),
    ] {
        let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
        assert_eq!(
            poll_joined(joined_results::run(
                barrier.clone(),
                fail_first,
                barrier,
                fail_second
            )),
            expected,
        );
    }
}

#[graph]
mod joined_results {
    use std::sync::Arc;

    use tokio::sync::Barrier;

    #[derive(Debug, PartialEq)]
    pub enum Error {
        First,
        Second,
    }
    struct First;
    struct Second;
    impl From<First> for Error {
        fn from(_: First) -> Self {
            Self::First
        }
    }
    impl From<Second> for Error {
        fn from(_: Second) -> Self {
            Self::Second
        }
    }

    pub fn run(first: usize, second: usize) -> Result<usize, Error> {
        Ok(first + second)
    }
    async fn first(left: Arc<Barrier>, fail_first: bool) -> Result<usize, First> {
        left.wait().await;
        if fail_first { Err(First) } else { Ok(1) }
    }
    async fn second(right: Arc<Barrier>, fail_second: bool) -> Result<usize, Second> {
        right.wait().await;
        if fail_second { Err(Second) } else { Ok(2) }
    }
}

#[test]
fn graph_preserves_impl_trait_inputs() {
    assert_eq!(impl_trait_input::run(&42), "42:42");
}

#[graph]
mod impl_trait_input {
    pub fn run(first: String, second: String) -> String {
        format!("{first}:{second}")
    }

    fn first(value: &impl ToString) -> String {
        value.to_string()
    }

    fn second(value: &impl ToString) -> String {
        value.to_string()
    }
}

#[test]
fn named_graph_inputs() {
    assert_eq!(
        named_entries::MakeApp {
            name: "app".to_owned(),
            count: 2
        }
        .run(),
        "app:2"
    );
    assert_eq!(
        named_entries::MakeLabel {
            name: "label".to_owned()
        }
        .run(),
        "label"
    );
    assert_eq!(
        shared_values::Run {}.run(),
        ("value".to_owned(), "value".to_owned())
    );
    assert_eq!(impl_trait_input::Run { value: &42 }.run(), "42:42");
    let text = "borrowed".to_owned();
    assert_eq!(
        named_entries::Borrow {
            text: &text,
            fallback: "fallback"
        }
        .run(),
        "borrowed"
    );
    assert_eq!(
        named_entries::BorrowElided { text: &text }.run(),
        "borrowed"
    );
    assert_eq!(named_entries::Identity { value: 17 }.run(), 17);
    assert_eq!(named_entries::ArrayLength { values: [1, 2, 3] }.run(), 3);
}

#[graph]
mod named_entries {
    pub fn make_app(name: String, count: usize) -> String {
        format!("{name}:{count}")
    }
    pub fn make_label(name: String) -> String {
        name
    }
    pub fn borrow<'a>(text: &'a str, fallback: &'a str) -> &'a str {
        if text.is_empty() { fallback } else { text }
    }
    pub fn borrow_elided(text: &str) -> &str {
        text
    }
    pub fn identity<T>(value: T) -> T {
        value
    }
    pub fn array_length<const N: usize>(values: [u8; N]) -> usize {
        values.len()
    }
}

#[tokio::test]
async fn named_async_graph_inputs() {
    assert_eq!(inferred_result::Run { fail: false }.run().await, Ok(15));
    assert_eq!(
        inferred_result::Run { fail: true }.run().await,
        Err("component failed".to_owned())
    );
    assert_eq!(explicit_result::Run { fail: 0 }.run().await, Ok(3));
}
