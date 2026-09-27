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
