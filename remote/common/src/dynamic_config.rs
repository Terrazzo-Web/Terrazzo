use std::collections::HashMap;
use std::collections::hash_map;
use std::convert::Infallible;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::RwLock;

use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;

use self::has_diff::HasDiff;
use crate::is_global::IsGlobal;
use crate::unwrap_infallible::UnwrapInfallible as _;

#[derive(Default)]
pub struct DynamicConfig<T, M = mode::RW> {
    config: std::sync::RwLock<Option<T>>,
    notify: Arc<Registry<Box<dyn Fn(&T) + Send + Sync + 'static>>>,
    on_drop: std::sync::Mutex<Vec<Box<dyn FnOnce() + Send + Sync + 'static>>>,
    _mode: PhantomData<M>,
}

pub mod mode {
    pub use super::mode_impl::Mode;
    pub use super::mode_impl::RO;
    pub use super::mode_impl::RW;
}

mod mode_impl {
    use crate::is_global::IsGlobal;

    pub trait Mode: IsGlobal {
        fn mode() -> ModeImpl;
    }

    pub enum RW {}
    pub enum RO {}

    #[derive(Debug, PartialEq, Eq)]
    pub enum ModeImpl {
        RW,
        RO,
    }

    impl Mode for RW {
        fn mode() -> ModeImpl {
            ModeImpl::RW
        }
    }
    impl Mode for RO {
        fn mode() -> ModeImpl {
            ModeImpl::RO
        }
    }
}

impl<T> From<T> for DynamicConfig<T, mode::RW> {
    fn from(config: T) -> Self {
        from_impl(config)
    }
}

fn from_impl<T, M>(config: T) -> DynamicConfig<T, M>
where
    M: mode::Mode,
{
    DynamicConfig {
        config: RwLock::new(Some(config)),
        notify: Default::default(),
        on_drop: Default::default(),
        _mode: PhantomData,
    }
}

impl<T> DynamicConfig<T, mode::RW> {
    pub fn add_notify(
        &self,
        f: impl Fn(&T) + IsGlobal,
    ) -> RegistryHandle<Box<dyn Fn(&T) + Send + Sync + 'static>> {
        add_notify(self, f)
    }
}

fn add_notify<T>(
    this: &DynamicConfig<T, impl mode::Mode>,
    f: impl Fn(&T) + IsGlobal,
) -> RegistryHandle<Box<dyn Fn(&T) + Send + Sync + 'static>> {
    this.notify.add(Box::new(f))
}

impl<T, M> DynamicConfig<T, M>
where
    M: mode::Mode,
{
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.with(T::clone)
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(self
            .config
            .read()
            .expect("read lock")
            .as_ref()
            .expect("option not present"))
    }

    pub fn view_diff<U>(
        self: &Arc<Self>,
        to: impl Fn(&T) -> U + IsGlobal,
    ) -> Arc<DynamicConfig<U, mode::RO>>
    where
        T: Clone + IsGlobal,
        U: Clone + IsGlobal + HasDiff,
    {
        derive_impl(self.clone(), to, |_, _| Option::<T>::None, U::is_same)
    }

    pub fn view<U>(
        self: &Arc<Self>,
        to: impl Fn(&T) -> U + IsGlobal,
    ) -> Arc<DynamicConfig<U, mode::RO>>
    where
        T: Clone + IsGlobal,
        U: Clone + IsGlobal,
    {
        derive_impl(self.clone(), to, |_, _| Option::<T>::None, always_changed)
    }
}

impl<T> DynamicConfig<T, mode::RW> {
    pub fn set(&self, make_new_config: impl FnOnce(&T) -> T)
    where
        T: Clone,
    {
        self.try_set(|t| Ok::<_, Infallible>(make_new_config(t)))
            .unwrap_infallible()
    }

    pub fn try_set<E>(&self, make_new_config: impl FnOnce(&T) -> Result<T, E>) -> Result<(), E>
    where
        T: Clone,
    {
        self.try_set_impl(make_new_config, always_changed)
    }

    pub fn silent_set(&self, make_new_config: impl FnOnce(&T) -> T)
    where
        T: Clone,
    {
        self.silent_try_set(|t| Ok::<_, Infallible>(make_new_config(t)))
            .unwrap_infallible()
    }

    pub fn silent_try_set<E>(
        &self,
        make_new_config: impl FnOnce(&T) -> Result<T, E>,
    ) -> Result<(), E>
    where
        T: Clone,
    {
        {
            let mut lock = self.config.write().expect("write lock");
            let new_config = make_new_config(lock.as_ref().expect("option not present"))?;
            *lock = Some(new_config);
        }

        #[cfg(debug_assertions)]
        let _ = self.get();

        Ok(())
    }

    pub fn derive<U>(
        self: &Arc<Self>,
        to: impl Fn(&T) -> U + IsGlobal,
        from: impl Fn(T, &U) -> Option<T> + IsGlobal,
    ) -> Arc<DynamicConfig<U, mode::RW>>
    where
        T: Clone + IsGlobal,
        U: Clone + IsGlobal + HasDiff,
    {
        derive_impl(self.clone(), to, from, U::is_same)
    }
}

fn derive_impl<T, U, MU>(
    main: Arc<DynamicConfig<T, impl mode::Mode>>,
    to: impl Fn(&T) -> U + IsGlobal,
    from: impl Fn(T, &U) -> Option<T> + IsGlobal,
    is_same: impl FnOnce(&U, &U) -> bool + IsGlobal + Copy,
) -> Arc<DynamicConfig<U, MU>>
where
    T: Clone + IsGlobal,
    U: Clone + IsGlobal,
    MU: mode::Mode,
{
    let derived: Arc<DynamicConfig<U, MU>> = Arc::new(from_impl(main.with(|m| to(m))));
    if MU::mode() == mode_impl::ModeImpl::RW {
        let on_derived_change = add_notify(&derived, {
            let main_weak = Arc::downgrade(&main);
            move |d| {
                if let Some(main) = main_weak.upgrade() {
                    if let Some(m) = from(main.get(), d) {
                        main.try_set_impl(|_| Ok(m), always_changed)
                            .unwrap_infallible();
                    }
                }
            }
        });
        main.on_drop
            .lock()
            .unwrap()
            .push(Box::new(move || drop(on_derived_change)));
    }
    let on_main_change = add_notify(&main, {
        let derived_weak = Arc::downgrade(&derived);
        move |m| {
            if let Some(derived) = derived_weak.upgrade() {
                derived
                    .try_set_impl(|_old| Ok(to(m)), is_same)
                    .unwrap_infallible()
            }
        }
    });
    derived
        .on_drop
        .lock()
        .unwrap()
        .push(Box::new(move || drop(on_main_change)));
    return derived;
}

impl<T> DynamicConfig<T> {
    pub fn if_change<U>(
        from: impl Fn(&T, &U) -> T + 'static,
    ) -> impl Fn(T, &U) -> Option<T> + 'static
    where
        T: Eq,
    {
        move |old_t, u| {
            let new_t = from(&old_t, u);
            if new_t != old_t { Some(new_t) } else { None }
        }
    }

    pub fn if_ptr_change<U>(
        from: impl Fn(&Arc<T>, &U) -> Arc<T> + 'static,
    ) -> impl Fn(Arc<T>, &U) -> Option<Arc<T>> + 'static {
        move |old_t, u| {
            let new_t = from(&old_t, u);
            if !Arc::ptr_eq(&new_t, &old_t) {
                Some(new_t)
            } else {
                None
            }
        }
    }
}

impl<T, M> DynamicConfig<T, M>
where
    M: mode::Mode,
{
    fn try_set_impl<E>(
        &self,
        make_new_config: impl FnOnce(&T) -> Result<T, E>,
        is_same: impl FnOnce(&T, &T) -> bool,
    ) -> Result<(), E>
    where
        T: Clone,
    {
        let new_config;
        {
            let mut lock = self.config.write().expect("write lock");
            let old_config = lock.as_ref().expect("option not present");
            new_config = make_new_config(old_config)?;
            if is_same(old_config, &new_config) {
                return Ok(());
            }
            *lock = Some(new_config.clone());
        }

        #[cfg(debug_assertions)]
        let _ = self.get();

        self.notify.with(|notify| {
            for notify in notify {
                notify(&new_config);
            }
        });
        Ok(())
    }
}

impl<T: Debug> std::fmt::Debug for DynamicConfig<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.with(|config| {
            f.debug_struct("DynamicConfig")
                .field("config", config)
                .finish()
        })
    }
}

impl<T: Serialize> Serialize for DynamicConfig<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.with(|v| v.serialize(serializer))
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for DynamicConfig<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(T::deserialize(deserializer)?.into())
    }
}

struct Registry<T>(std::sync::RwLock<RegistryInner<T>>);

struct RegistryInner<T> {
    next: i32,
    table: HashMap<i32, T>,
}

#[must_use]
pub struct RegistryHandle<T> {
    registry: Arc<Registry<T>>,
    key: i32,
}

impl<T> Registry<T> {
    fn add(self: &Arc<Self>, value: T) -> RegistryHandle<T> {
        let mut lock = self.write();
        let RegistryInner { next, table } = &mut *lock;
        *next += 1;
        let prev = table.insert(*next, value);
        assert!(prev.is_none());
        return RegistryHandle {
            registry: self.clone(),
            key: *next,
        };
    }

    fn read(&self) -> std::sync::RwLockReadGuard<RegistryInner<T>> {
        self.0.read().expect("registry")
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<RegistryInner<T>> {
        self.0.write().expect("registry")
    }
}

impl<T> Registry<T> {
    pub fn with<R>(&self, f: impl Fn(hash_map::Values<'_, i32, T>) -> R) -> R {
        f(self.read().table.values())
    }
}

impl<T> Default for Registry<T> {
    fn default() -> Self {
        Self(RwLock::new(RegistryInner {
            next: 0,
            table: HashMap::new(),
        }))
    }
}

impl<T> Drop for RegistryHandle<T> {
    fn drop(&mut self) {
        let mut lock = self.registry.write();
        let removed = lock.table.remove(&self.key);
        debug_assert!(removed.is_some());
    }
}

pub mod has_diff {
    use std::ops::Deref;
    use std::sync::Arc;

    use serde::Deserialize;
    use serde::Serialize;

    pub trait HasDiff {
        #[expect(unused)]
        fn is_same(lhs: &Self, rhs: &Self) -> bool {
            false
        }

        fn is_diff(lhs: &Self, rhs: &Self) -> bool {
            !Self::is_same(lhs, rhs)
        }
    }

    impl<T: Eq> HasDiff for T {
        fn is_same(lhs: &Self, rhs: &Self) -> bool {
            PartialEq::eq(lhs, rhs)
        }
    }

    #[derive(Default, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct DiffArc<T>(Arc<T>);

    impl<T> HasDiff for DiffArc<T> {
        fn is_same(lhs: &Self, rhs: &Self) -> bool {
            Arc::ptr_eq(&lhs.0, &rhs.0)
        }
    }

    impl<T> From<T> for DiffArc<T> {
        fn from(value: T) -> Self {
            Self(value.into())
        }
    }

    impl<T> From<Arc<T>> for DiffArc<T> {
        fn from(value: Arc<T>) -> Self {
            Self(value)
        }
    }

    impl<T> Deref for DiffArc<T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl<T> Clone for DiffArc<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }

    impl<T: std::fmt::Debug> std::fmt::Debug for DiffArc<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.0, f)
        }
    }

    #[derive(Clone, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct DiffOption<T>(Option<T>);

    impl<T: HasDiff> HasDiff for DiffOption<T> {
        fn is_same(lhs: &Self, rhs: &Self) -> bool {
            match (&lhs.0, &rhs.0) {
                (None, None) => true,
                (None, Some(_)) => false,
                (Some(_), None) => false,
                (Some(lhs), Some(rhs)) => T::is_same(lhs, rhs),
            }
        }
    }

    impl<T> From<T> for DiffOption<T> {
        fn from(value: T) -> Self {
            Self(value.into())
        }
    }

    impl<T> Default for DiffOption<T> {
        fn default() -> Self {
            Self(Option::default())
        }
    }

    impl<T> From<Option<T>> for DiffOption<T> {
        fn from(value: Option<T>) -> Self {
            Self(value)
        }
    }

    impl<T> Deref for DiffOption<T> {
        type Target = Option<T>;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl<T: std::fmt::Debug> std::fmt::Debug for DiffOption<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.0, f)
        }
    }

    #[derive(Clone, Default, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct DiffItem<T>(T);

    impl<T> HasDiff for DiffItem<T> {}

    impl<T> From<T> for DiffItem<T> {
        fn from(value: T) -> Self {
            Self(value)
        }
    }

    impl<T> Deref for DiffItem<T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl<T: std::fmt::Debug> std::fmt::Debug for DiffItem<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.0, f)
        }
    }
}

fn always_changed<T>(_: &T, _: &T) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;

    use super::DynamicConfig;

    #[test]
    fn set() {
        let cfg = DynamicConfig::from("hello".to_owned());
        let () = cfg.set(|old| format!("{old} world"));
        assert_eq!("hello world", cfg.get());
    }

    #[test]
    fn try_set() {
        let cfg = DynamicConfig::from("hello".to_owned());
        let Ok(()): Result<(), ()> = cfg.try_set(|old| Ok(format!("{old} world"))) else {
            panic!();
        };
        assert_eq!("hello world", cfg.get());

        let Err(()): Result<(), ()> = cfg.try_set(|_| Err(())) else {
            panic!();
        };
        assert_eq!("hello world", cfg.get());
    }

    #[test]
    fn add_notify() {
        let cfg = DynamicConfig::from("hello".to_owned());
        let last = Arc::new(Mutex::new(None));
        let last2 = last.clone();
        let notify =
            cfg.add_notify(move |current| *last2.lock().unwrap() = Some(current.to_owned()));

        let () = cfg.set(|old| format!("{old} world"));
        assert_eq!(Some("hello world"), last.lock().unwrap().as_deref());
        assert_eq!("hello world", cfg.get());

        let () = cfg.set(|old| format!("{old}!"));
        assert_eq!(Some("hello world!"), last.lock().unwrap().as_deref());
        assert_eq!("hello world!", cfg.get());

        drop(notify);
        let () = cfg.set(|old| format!("{old}!!"));
        assert_eq!(Some("hello world!"), last.lock().unwrap().as_deref());
        assert_eq!("hello world!!!", cfg.get());
    }

    #[test]
    fn derive() {
        let main = Arc::new(DynamicConfig::from("hello".to_owned()));
        let derived = main.derive(
            |main| Box::new(main.to_uppercase()),
            |m, d| {
                let new_main = d.to_lowercase();
                if new_main != m { Some(new_main) } else { None }
            },
        );
        assert_eq!("hello", main.get());
        assert_eq!("HELLO", *derived.get());

        derived.set(|_| Box::new("HELLO_WORLD".to_owned()));
        assert_eq!("hello_world", main.get());
        assert_eq!("HELLO_WORLD", *derived.get());

        assert_eq!(1, main.notify.read().table.len());
        assert_eq!(1, derived.notify.read().table.len());

        drop(derived);
        assert_eq!(0, main.notify.read().table.len());
    }

    #[test]
    fn view() {
        let main = Arc::new(DynamicConfig::from("hello".to_owned()));
        let view = main.view(|main| Box::new(main.to_uppercase()));
        assert_eq!("hello", main.get());
        assert_eq!("HELLO", *view.get());

        main.set(|_| "Hello World".to_owned());
        assert_eq!("Hello World", main.get());
        assert_eq!("HELLO WORLD", *view.get());
    }

    #[test]
    fn derive_eq() {
        let main = Arc::new(DynamicConfig::from("hello".to_owned()));
        let derived = main.derive(
            |main| Box::new(main.to_uppercase()),
            DynamicConfig::if_change(|_m, d: &Box<String>| d.to_lowercase()),
        );
        assert_eq!("hello", main.get());
        assert_eq!("HELLO", *derived.get());

        derived.set(|_| Box::new("HELLO_WORLD".to_owned()));
        assert_eq!("hello_world", main.get());
        assert_eq!("HELLO_WORLD", *derived.get());
    }

    #[test]
    fn derive_ptr() {
        let main = Arc::new(DynamicConfig::from(Arc::new("hello".to_string())));
        let derived = main.derive(
            |main| Arc::new(main.to_uppercase()),
            DynamicConfig::if_ptr_change(|m: &Arc<String>, _| m.clone()),
        );
        assert_eq!("hello", *main.get());
        assert_eq!("HELLO", *derived.get());

        derived.set(|_| Arc::new("HELLO_WORLD".to_owned()));
        assert_eq!("hello", *main.get());
        assert_eq!("HELLO_WORLD", *derived.get());

        main.set(|_| Arc::new("hello2".to_owned()));
        assert_eq!("hello2", *main.get());
        assert_eq!("HELLO2", *derived.get());
    }
}
