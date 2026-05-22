use std::sync::Arc;
use std::sync::Mutex;

#[derive(Default)]
pub struct DropList(Vec<Box<dyn FnOnce() + Send>>);

#[derive(Clone, Default)]
pub struct DropListPtr(Arc<Mutex<DropList>>);

impl DropList {
    pub fn reset(&mut self) {
        *self = Self::default()
    }

    pub fn add<F>(&mut self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.0.push(Box::new(f));
    }

    pub fn track<D>(&mut self, d: D)
    where
        D: Drop + Send + 'static,
    {
        self.add(move || drop(d));
    }
}

impl Drop for DropList {
    fn drop(&mut self) {
        for d in std::mem::take(&mut self.0) {
            d()
        }
    }
}

impl DropListPtr {
    pub fn reset(&self) {
        self.0.lock().expect("drop_list").reset();
    }

    pub fn add<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.0.lock().expect("drop_list").add(f);
    }

    pub fn track<D>(&self, d: D)
    where
        D: Drop + Send + 'static,
    {
        self.add(move || drop(d));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicI32;
    use std::sync::atomic::Ordering::SeqCst;

    use autoclone::autoclone;

    #[autoclone]
    #[test]
    fn drop_list() {
        let counter = Arc::new(AtomicI32::new(0));
        let mut drop_list = super::DropList::default();
        drop_list.add(move || {
            autoclone!(counter);
            counter.fetch_add(1, SeqCst);
        });
        drop_list.track(scopeguard::guard(counter.clone(), |c| {
            c.fetch_add(1, SeqCst);
        }));
        drop_list.reset();
        assert_eq!(2, counter.load(SeqCst));

        drop_list.add(move || {
            autoclone!(counter);
            counter.fetch_add(1, SeqCst);
        });
        drop_list.track(scopeguard::guard(counter.clone(), |c| {
            c.fetch_add(1, SeqCst);
        }));
        drop(drop_list);
        assert_eq!(4, counter.load(SeqCst));
    }

    #[autoclone]
    #[test]
    fn drop_list_ptr() {
        let counter = Arc::new(AtomicI32::new(0));
        let drop_list = super::DropListPtr::default();
        drop_list.add(move || {
            autoclone!(counter);
            counter.fetch_add(1, SeqCst);
        });
        drop_list.track(scopeguard::guard(counter.clone(), |c| {
            c.fetch_add(1, SeqCst);
        }));
        drop_list.reset();
        assert_eq!(2, counter.load(SeqCst));

        drop_list.add(move || {
            autoclone!(counter);
            counter.fetch_add(1, SeqCst);
        });
        drop_list.track(scopeguard::guard(counter.clone(), |c| {
            c.fetch_add(1, SeqCst);
        }));
        drop(drop_list);
        assert_eq!(4, counter.load(SeqCst));
    }
}
