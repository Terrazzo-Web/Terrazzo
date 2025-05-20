use std::collections::HashMap;
use std::collections::hash_map;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::RwLock;

use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;

use crate::is_global::IsGlobal;

#[derive(Default)]
pub struct DynamicConfig<T> {
    config: std::sync::RwLock<Option<T>>,
    notify: Arc<Registry<Box<dyn Fn(&T) + Send + Sync + 'static>>>,
    on_drop: std::sync::Mutex<Vec<Box<dyn FnOnce() + Send + Sync + 'static>>>,
}

impl<T> From<T> for DynamicConfig<T> {
    fn from(config: T) -> Self {
        Self {
            config: RwLock::new(Some(config)),
            notify: Default::default(),
            on_drop: Default::default(),
        }
    }
}

impl<T> DynamicConfig<T> {
    pub fn add_notify(
        &self,
        f: impl Fn(&T) + IsGlobal,
    ) -> RegistryHandle<Box<dyn Fn(&T) + Send + Sync + 'static>> {
        self.notify.add(Box::new(f))
    }
}

impl<T> DynamicConfig<T> {
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.with(T::clone)
    }

    pub fn set(&self, make_new_config: impl FnOnce(T) -> T)
    where
        T: Clone,
    {
        let new_config;
        {
            let mut lock = self.config.write().expect("write lock");
            new_config = make_new_config(lock.take().expect("option not present"));
            *lock = Some(new_config.clone());
        }

        #[cfg(debug_assertions)]
        let _ = self.get();

        self.notify.with(|notify| {
            for notify in notify {
                notify(&new_config);
            }
        });
    }

    pub fn derive<U: Clone + IsGlobal>(
        self: &Arc<Self>,
        to: impl Fn(&T) -> U + IsGlobal,
        from: impl Fn(T, &U) -> Option<T> + IsGlobal,
    ) -> Arc<DynamicConfig<U>>
    where
        T: Clone + IsGlobal,
    {
        let main = self.clone();
        let derived = Arc::new(DynamicConfig::from(self.with(|m: &T| to(m))));
        let on_derived_change = derived.add_notify({
            let main_weak = Arc::downgrade(&main);
            move |d: &U| {
                if let Some(main) = main_weak.upgrade() {
                    if let Some(m) = from(main.get(), d) {
                        main.set(|_| m);
                    }
                }
            }
        });
        main.on_drop
            .lock()
            .unwrap()
            .push(Box::new(move || drop(on_derived_change)));
        let on_main_change = main.add_notify(Box::new({
            let derived_weak = Arc::downgrade(&derived);
            move |m: &T| {
                if let Some(derived) = derived_weak.upgrade() {
                    derived.set(|_| to(m))
                }
            }
        }));
        derived
            .on_drop
            .lock()
            .unwrap()
            .push(Box::new(move || drop(on_main_change)));
        return derived;
    }

    pub fn if_change<U>(
        from: impl Fn(&T, &U) -> Option<T> + 'static,
    ) -> impl Fn(T, &U) -> Option<T> + 'static
    where
        T: Eq,
    {
        move |old_t, u| from(&old_t, u).filter(|new_t| *new_t != old_t)
    }
}

impl<T> DynamicConfig<Arc<T>> {
    pub fn if_ptr_change<U>(
        from: impl Fn(&Arc<T>, &U) -> Option<Arc<T>> + 'static,
    ) -> impl Fn(Arc<T>, &U) -> Option<Arc<T>> + 'static {
        move |old_t, u| from(&old_t, u).filter(|new_t| !Arc::ptr_eq(new_t, &old_t))
    }
}

impl<T> DynamicConfig<T> {
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(self
            .config
            .read()
            .expect("read lock")
            .as_ref()
            .expect("option not present"))
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
    fn derive_eq() {
        let main = Arc::new(DynamicConfig::from("hello".to_owned()));
        let derived = main.derive(
            |main| Box::new(main.to_uppercase()),
            DynamicConfig::if_change(|_m, d: &Box<String>| Some(d.to_lowercase())),
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
            DynamicConfig::if_ptr_change(|m: &Arc<String>, _| Some(m.clone())),
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
