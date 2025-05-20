use std::fmt::Debug;
use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::Arc;
use std::sync::LockResult;
use std::sync::RwLock;

use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;

use crate::is_global::IsGlobal;

#[derive(Default)]
pub struct DynamicConfig<T> {
    config: std::sync::RwLock<Option<T>>,
    notify: std::sync::RwLock<Vec<Box<dyn Fn(&T) + Send + Sync + 'static>>>,
}

impl<T> From<T> for DynamicConfig<T> {
    fn from(config: T) -> Self {
        Self {
            config: RwLock::new(Some(config)),
            notify: RwLock::default(),
        }
    }
}

impl<T> DynamicConfig<T> {
    pub fn with_notify(mut self, f: impl Fn(&T) + IsGlobal) -> Self {
        add_notify_impl(self.notify.get_mut(), f);
        self
    }

    pub fn add_notify(&self, f: impl Fn(&T) + IsGlobal) {
        add_notify_impl(self.notify.write(), f);
    }
}

fn add_notify_impl<T>(
    notify: LockResult<
        impl Deref<Target = Vec<Box<dyn Fn(&T) + Send + Sync + 'static>>> + DerefMut,
    >,
    f: impl Fn(&T) + IsGlobal,
) {
    notify.expect("write notify").push(Box::new(f));
}

impl<T: Clone> DynamicConfig<T> {
    pub fn get(&self) -> T {
        self.with(T::clone)
    }

    pub fn set(&self, make_new_config: impl FnOnce(T) -> T) {
        let new_config;
        {
            let mut lock = self.config.write().expect("write lock");
            new_config = make_new_config(lock.take().expect("option not present"));
            *lock = Some(new_config.clone());
        }

        #[cfg(debug_assertions)]
        let _ = self.get();

        for notify in &*self.notify.read().expect("read notify") {
            notify(&new_config);
        }
    }

    pub fn derive<U: Clone + IsGlobal>(
        self: Arc<Self>,
        to: impl Fn(&T) -> U + IsGlobal,
        from: impl Fn(T, &U) -> Option<T> + IsGlobal,
    ) -> Arc<DynamicConfig<U>>
    where
        T: IsGlobal,
    {
        let main = self.clone();
        let derived = DynamicConfig::from(self.with(|m: &T| to(m)));
        let derived = derived.with_notify({
            let main = main.clone();
            move |d: &U| {
                if let Some(m) = from(main.get(), d) {
                    main.set(|_| m);
                }
            }
        });
        let derived = Arc::new(derived);
        main.add_notify(Box::new({
            let derived = derived.clone();
            move |m: &T| derived.set(|_| to(m))
        }));
        return derived;
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
