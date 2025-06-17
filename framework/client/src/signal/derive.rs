use std::fmt::Debug;

use super::XSignal;
use super::depth::Depth;
use crate::debug_correlation_id::DebugCorrelationId;
use crate::prelude::OrElseLog as _;
use crate::string::XString;
use crate::tracing::debug;
use crate::tracing::debug_span;
use crate::tracing::trace;
use crate::tracing::warn;

impl<T> XSignal<T> {
    pub fn derive<U>(
        &self,
        name: impl Into<XString>,
        to: impl Fn(&T) -> U + 'static,
        from: impl Fn(&T, &U) -> Option<T> + 'static,
    ) -> XSignal<U>
    where
        T: Debug + 'static,
        U: Debug + Eq + 'static,
    {
        let main = self;
        let main_weak = main.downgrade();
        let main_name =
            DebugCorrelationId::new(|| format!("Main:{}", main.0.producer.name()).into());

        let derived = XSignal::new(
            name,
            to(main.0.current_value.lock().or_throw("main").value()),
        );
        let derived_weak = derived.downgrade();
        let derived_name =
            DebugCorrelationId::new(|| format!("Derived:{}", derived.0.producer.name()).into());

        let span = debug_span! { "Derived signal", main = %main_name, derived = %derived_name };
        let _span = span.clone().entered();
        debug!(main = %main_name, derived = %derived_name, "Make");

        {
            // Update main when derived changes
            let main_weak = main_weak.clone();
            let derived_weak = derived_weak.clone();
            let span = span.clone();
            trace!("Derived updates Main");
            let derived_producer = &derived.0.producer;
            let consumer =
                derived_producer.register(derived_name, Depth::zero(), move |_version| {
                    let _span = span.enter();
                    debug!("Derived updated");
                    let Some(derived) = derived_weak.upgrade() else {
                        warn!("Derived is dropped but triggered");
                        debug_assert!(false);
                        return;
                    };
                    let Some(main) = main_weak.upgrade() else {
                        warn!("Main is dropped but subscribed");
                        debug_assert!(false);
                        return;
                    };
                    let t = from(
                        main.0.current_value.lock().or_throw("main").value(),
                        derived.0.current_value.lock().or_throw("derived").value(),
                    );
                    if let Some(t) = t {
                        main.force(t);
                    }
                });
            // If main is dropped there is no need to try to update it.
            let mut on_main_drop = main.0.on_drop.lock().or_throw("on_drop");
            on_main_drop.push(Box::new(move || drop(consumer)));
        }

        {
            // Update derived when main changes
            let main_weak = main_weak.clone();
            let derived_weak = derived_weak.clone();
            let span = span.clone();
            trace!("Main updates Derived");
            let main_producer = &main.0.producer;
            let consumer = main_producer.register(main_name, Depth::zero(), move |_version| {
                let _span = span.enter();
                debug!("Main updated");
                let Some(main) = main_weak.upgrade() else {
                    warn!("Main is dropped but triggered");
                    debug_assert!(false);
                    return;
                };
                let Some(derived) = derived_weak.upgrade() else {
                    warn!("Derived is dropped but subscribed");
                    debug_assert!(false);
                    return;
                };
                let t = to(main.0.current_value.lock().or_throw("main").value());
                derived.set(t);
            });
            // If derived is dropped there is no need to try to update it.
            let mut on_drop = derived.0.on_drop.lock().or_throw("on_drop");
            on_drop.push(Box::new(move || drop(consumer)));
        }

        return derived;
    }

    pub fn view<U>(&self, name: impl Into<XString>, to: impl Fn(&T) -> U + 'static) -> XSignal<U>
    where
        T: Debug + 'static,
        U: Debug + Eq + 'static,
    {
        self.derive(name, to, |_, _| None)
    }
}

/// A handy utility function to add diffing logic when using [derived] signals.
///
/// ```
/// # use std::cell::Cell;
/// # use autoclone::autoclone;
/// # use terrazzo_client::prelude::*;
/// # #[autoclone]
/// # fn main() {
/// let main = XSignal::new("main", "1".to_owned());
/// let compute_derived = Ptr::new(Cell::new(0));
/// let compute_main = Ptr::new(Cell::new(0));
///
/// let derived_nodiff = main.derive(
///     "derived",
///     /* to: */
///     move |main| {
/// #       autoclone!(compute_derived);
///         compute_derived.set(compute_derived.get() + 1);
///         main.parse::<i32>().unwrap()
///     },
///     /* from: */
///     move |_main: &String, derived: &i32| {
/// #       autoclone!(compute_main);
///         compute_main.set(compute_main.get() + 1);
///         Some(derived.to_string())
///     },
/// );
///
/// # assert_eq!("1", main.get_value_untracked());
/// # assert_eq!(1, derived_nodiff.get_value_untracked());
/// # assert_eq!(1, compute_derived.take());
/// # assert_eq!(0, compute_main.take());
///
/// // 1. Updating `main` updates `derived`
/// // 2. Which updates `main` again
/// // 3. Which updates `derived` but to the same value
/// main.set("2");
/// # assert_eq!("2", main.get_value_untracked());
/// # assert_eq!(2, derived_nodiff.get_value_untracked());
/// assert_eq!(2, compute_derived.take());
/// assert_eq!(1, compute_main.take());
///
/// // Updating `main` to the same value is a no-op.
/// main.set("2");
/// # assert_eq!("2", main.get_value_untracked());
/// # assert_eq!(2, derived_nodiff.get_value_untracked());
/// assert_eq!(0, compute_derived.take());
/// assert_eq!(0, compute_main.take());
///
/// // Reset ...
/// # drop(derived_nodiff);
/// # main.set("1");
///
/// let derived_diff = main.derive(
///     "derived",
///     /* to: */
///     move |main| {
///         autoclone!(compute_derived);
///         compute_derived.set(compute_derived.get() + 1);
///         main.parse::<i32>().unwrap()
///     },
///     /* from: */
///     if_change(move |_main: &String, derived: &i32| {
///         autoclone!(compute_main);
///         compute_main.set(compute_main.get() + 1);
///         Some(derived.to_string())
///     }),
/// );
///
/// # assert_eq!("1", main.get_value_untracked());
/// # assert_eq!(1, derived_diff.get_value_untracked());
/// # compute_derived.set(0);
/// # compute_main.set(0);
///
/// // Updating `main` updates `derived`, which updates `main` again but to the same value.
/// main.set("2");
/// # assert_eq!("2", main.get_value_untracked());
/// # assert_eq!(2, derived_diff.get_value_untracked());
/// assert_eq!(1, compute_derived.take());
/// assert_eq!(1, compute_main.take());
///
/// // Updating `main` to the same value is a no-op.
/// main.set("2");
/// # assert_eq!("2", main.get_value_untracked());
/// # assert_eq!(2, derived_diff.get_value_untracked());
/// assert_eq!(0, compute_derived.take());
/// assert_eq!(0, compute_main.take());
/// # }
/// ```
///
/// [derived]: crate::prelude::XSignal::derive
pub fn if_change<T: Eq, U>(
    from: impl Fn(&T, &U) -> Option<T> + 'static,
) -> impl Fn(&T, &U) -> Option<T> + 'static {
    move |old_t, u| from(old_t, u).filter(|new_t| new_t != old_t)
}
