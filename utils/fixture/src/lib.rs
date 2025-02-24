use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::Weak;

#[derive(Default)]
pub struct Fixture<T> {
    once: OnceLock<FixtureState<T>>,
}

impl<T> Fixture<T> {
    pub const fn new() -> Self {
        Self {
            once: OnceLock::new(),
        }
    }

    pub fn get_or_init(&self, init: impl Fn() -> T + Send + Sync + 'static) -> Arc<T> {
        self.once
            .get_or_init(|| FixtureState::new(init))
            .get_or_init()
    }

    pub fn get(&self) -> Arc<T> {
        self.once.get().expect("Fixture::get").get()
    }
}

struct FixtureState<T> {
    init: Box<dyn Fn() -> T + Send + Sync>,
    current: Mutex<Weak<T>>,
}

impl<T> FixtureState<T> {
    fn new(init: impl Fn() -> T + Send + Sync + 'static) -> Self {
        Self {
            init: Box::new(init),
            current: Default::default(),
        }
    }

    fn get_or_init(&self) -> Arc<T> {
        let mut current = self.current.lock().expect("FixtureState::current::lock");
        if let Some(value) = current.upgrade() {
            return value;
        }
        let value = Arc::new((self.init)());
        *current = Arc::downgrade(&value);
        return value;
    }

    fn get(&self) -> Arc<T> {
        self.current
            .lock()
            .expect("FixtureState::current::lock")
            .upgrade()
            .expect("FixtureState::get")
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicI32;
    use std::sync::atomic::Ordering::SeqCst;

    use super::Fixture;

    #[test]
    fn test_fixture() {
        let a = fixture();
        assert_eq!(1, *a);
        let b = fixture();
        assert_eq!(1, *a);
        drop(a);
        drop(b);
        let c = fixture();
        assert_eq!(2, *c);
    }

    fn fixture() -> Arc<i32> {
        static FIXTURE: Fixture<i32> = Fixture::new();
        static NEXT: AtomicI32 = AtomicI32::new(1);
        FIXTURE.get_or_init(|| NEXT.fetch_add(1, SeqCst))
    }
}
