use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;

pub struct Fixture<T> {
    new: Box<dyn Fn() -> T + Send + Sync>,
    current: Mutex<Weak<T>>,
}

impl<T> Fixture<T> {
    pub fn new(new: impl Fn() -> T + Send + Sync + 'static) -> Self {
        Self {
            new: Box::new(new),
            current: Mutex::new(Weak::new()),
        }
    }

    pub fn get(&self) -> Arc<T> {
        let mut current = self.current.lock().expect("fixture::current::lock");
        if let Some(current) = current.upgrade() {
            return current;
        }
        let new = Arc::new((self.new)());
        *current = Arc::downgrade(&new);
        return new;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicI32;
    use std::sync::atomic::Ordering::SeqCst;
    use std::sync::OnceLock;
    use std::thread;
    use std::time::Duration;

    use super::Fixture;

    fn fixture() -> &'static Fixture<i32> {
        static FIXTURE: OnceLock<Fixture<i32>> = OnceLock::new();
        static NEXT: AtomicI32 = AtomicI32::new(1);
        let init: Box<dyn Fn() -> i32 + Send + Sync> = Box::new(|| NEXT.fetch_add(1, SeqCst));
        FIXTURE.get_or_init(|| Fixture::new(init))
    }

    #[test]
    fn a() {
        let fixture = fixture().get();
        assert_eq!(1, *fixture);
        thread::sleep(Duration::from_secs(1));
        drop(fixture);
    }

    #[test]
    fn b() {
        let fixture = fixture().get();
        assert_eq!(1, *fixture);
        thread::sleep(Duration::from_secs(1));
        drop(fixture);
    }

    #[test]
    fn c() {
        thread::sleep(Duration::from_secs(2));
        let fixture = fixture().get();
        assert_eq!(2, *fixture);
        drop(fixture);
    }
}
