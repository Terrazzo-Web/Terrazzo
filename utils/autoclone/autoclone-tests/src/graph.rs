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
