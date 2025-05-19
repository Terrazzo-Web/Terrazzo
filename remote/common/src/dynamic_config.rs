use std::sync::RwLock;

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
    pub fn with_notify(mut self, f: impl Into<Box<dyn Fn(&T) + Send + Sync + 'static>>) -> Self {
        self.notify.get_mut().expect("mut notify").push(f.into());
        self
    }

    pub fn add_notify(&self, f: impl Into<Box<dyn Fn(&T) + Send + Sync + 'static>>) {
        self.notify.write().expect("write notify").push(f.into());
    }
}

impl<T: Clone> DynamicConfig<T> {
    pub fn get(&self) -> T {
        self.config
            .read()
            .expect("read lock")
            .as_ref()
            .expect("option always present")
            .to_owned()
    }

    pub fn set(&self, make_new_config: impl FnOnce(T) -> T) {
        let new_config;
        {
            let mut lock = self.config.write().expect("write lock");
            new_config = make_new_config(lock.take().expect("option always present"));
            *lock = Some(new_config.clone());
        }

        #[cfg(debug_assertions)]
        let _ = self.get();

        for notify in &*self.notify.read().expect("read notify") {
            notify(&new_config);
        }
    }
}
