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
