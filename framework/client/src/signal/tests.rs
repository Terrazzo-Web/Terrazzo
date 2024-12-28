#![cfg(test)]

use std::cell::Cell;

use autoclone::autoclone;

use super::XSignal;
use crate::signal::derive::if_change;

#[test]
fn derive() {
    setup_logs();
    let main = XSignal::new("main", 1i32);
    let derived = main.derive(
        "derived",
        |main: &i32| (main + 1) as usize,
        if_change(|_main: &i32, derived: &usize| Some((derived - 1) as i32)),
    );

    assert_eq!(1, main.get_value_untracked());
    assert_eq!(2, derived.get_value_untracked());

    derived.set(3usize);
    assert_eq!(2, main.get_value_untracked());
    assert_eq!(3, derived.get_value_untracked());

    main.set(5i32);
    assert_eq!(5, main.get_value_untracked());
    assert_eq!(6, derived.get_value_untracked());
}

#[autoclone]
#[test]
fn derive2() {
    setup_logs();
    let main = XSignal::new("main", "1".to_owned());
    let to_exec = std::rc::Rc::new(Cell::new(0));
    let from_exec = std::rc::Rc::new(Cell::new(0));
    let derived = main.derive(
        "derived",
        /* to: */
        move |main| {
            autoclone!(to_exec);
            to_exec.set(to_exec.get() + 1);
            main.parse::<i32>().unwrap()
        },
        /* from: */
        if_change(move |_main: &String, derived: &i32| {
            autoclone!(from_exec);
            from_exec.set(from_exec.get() + 1);
            Some(derived.to_string())
        }),
    );

    assert_eq!("1", main.get_value_untracked());
    assert_eq!(1, derived.get_value_untracked());
    assert_eq!(1, to_exec.take());
    assert_eq!(0, from_exec.take());

    derived.set(2);
    assert_eq!("2", main.get_value_untracked());
    assert_eq!(2, derived.get_value_untracked());

    // Updating derived updates main, which updates derived again but to the same value.
    assert_eq!(1, to_exec.take());
    assert_eq!(1, from_exec.take());

    main.set("3");
    assert_eq!("3", main.get_value_untracked());
    assert_eq!(3, derived.get_value_untracked());

    // Updating main updates derived, which updates main again but to the same value.
    assert_eq!(1, to_exec.get());
    assert_eq!(1, from_exec.get());
}

#[autoclone]
#[test]
fn derive_diff() {
    setup_logs();
    let main = XSignal::new("main", "1".to_owned());
    let compute_derived = std::rc::Rc::new(Cell::new(0));
    let compute_main = std::rc::Rc::new(Cell::new(0));

    let derived_nodiff = main.derive(
        "derived",
        /* to: */
        move |main| {
            autoclone!(compute_derived);
            compute_derived.set(compute_derived.get() + 1);
            main.parse::<i32>().unwrap()
        },
        /* from: */
        move |_main: &String, derived: &i32| {
            autoclone!(compute_main);
            compute_main.set(compute_main.get() + 1);
            Some(derived.to_string())
        },
    );

    assert_eq!("1", main.get_value_untracked());
    assert_eq!(1, derived_nodiff.get_value_untracked());
    assert_eq!(1, compute_derived.take());
    assert_eq!(0, compute_main.take());

    // 1. Updating `main` updates `derived`
    // 2. Which updates `main` again
    // 3. Which updates `derived` but to the same value
    main.set("2");
    assert_eq!("2", main.get_value_untracked());
    assert_eq!(2, derived_nodiff.get_value_untracked());
    assert_eq!(2, compute_derived.take());
    assert_eq!(1, compute_main.take());

    // Updating `main` to the same value is a no-op.
    main.set("2");
    assert_eq!("2", main.get_value_untracked());
    assert_eq!(2, derived_nodiff.get_value_untracked());
    assert_eq!(0, compute_derived.take());
    assert_eq!(0, compute_main.take());

    // Reset
    drop(derived_nodiff);
    main.set("1");

    let derived_diff = main.derive(
        "derived",
        /* to: */
        move |main| {
            autoclone!(compute_derived);
            compute_derived.set(compute_derived.get() + 1);
            main.parse::<i32>().unwrap()
        },
        /* from: */
        if_change(move |_main: &String, derived: &i32| {
            autoclone!(compute_main);
            compute_main.set(compute_main.get() + 1);
            Some(derived.to_string())
        }),
    );

    assert_eq!("1", main.get_value_untracked());
    assert_eq!(1, derived_diff.get_value_untracked());
    compute_derived.set(0);
    compute_main.set(0);

    // Updating `main` updates `derived`, which updates `main` again but to the same value.
    main.set("2");
    assert_eq!("2", main.get_value_untracked());
    assert_eq!(2, derived_diff.get_value_untracked());
    assert_eq!(1, compute_derived.take());
    assert_eq!(1, compute_main.take());

    // Updating `main` to the same value is a no-op.
    main.set("2");
    assert_eq!("2", main.get_value_untracked());
    assert_eq!(2, derived_diff.get_value_untracked());
    assert_eq!(0, compute_derived.take());
    assert_eq!(0, compute_main.take());
}

#[test]
fn drop_main() {
    setup_logs();
    let main = XSignal::new("main", 1i32);
    let derived = main.derive(
        "derived",
        |main: &i32| (main + 1) as usize,
        if_change(|_main: &i32, derived: &usize| Some((derived - 1) as i32)),
    );

    assert_eq!(1, derived.0.producer.consumers().count());
    drop(main);
    assert_eq!(0, derived.0.producer.consumers().count());
    derived.set(3usize);
    assert_eq!(0, derived.0.producer.consumers().count());
    assert_eq!(3, derived.get_value_untracked());
}

#[test]
fn drop_derived() {
    setup_logs();
    let main = XSignal::new("main", 1i32);
    let derived = main.derive(
        "derived",
        |main: &i32| (main + 1) as usize,
        if_change(|_main: &i32, derived: &usize| Some((derived - 1) as i32)),
    );

    assert_eq!(1, main.0.producer.consumers().count());
    drop(derived);
    assert_eq!(0, main.0.producer.consumers().count());
    main.set(3i32);
    assert_eq!(0, main.0.producer.consumers().count());
    assert_eq!(3, main.get_value_untracked());
}

fn setup_logs() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(true)
        .with_file(true)
        .with_line_number(true)
        .with_target(false)
        .try_init();
}
