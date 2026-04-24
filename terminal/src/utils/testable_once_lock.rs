pub struct TestableOnceLock<T> {
    #[cfg(not(test))]
    once_lock: std::sync::OnceLock<T>,

    #[cfg(test)]
    mutex: std::sync::Mutex<Option<T>>,
}

#[cfg(not(test))]
mod prod {
    use super::TestableOnceLock;

    impl<T> TestableOnceLock<T> {
        pub const fn new() -> Self {
            Self {
                once_lock: std::sync::OnceLock::new(),
            }
        }

        pub fn get(&self) -> Option<&T> {
            self.once_lock.get()
        }

        pub fn set(&self, value: T)
        where
            T: std::fmt::Debug,
        {
            self.once_lock.set(value).unwrap()
        }
    }
}

#[cfg(test)]
mod test {
    use std::sync::Mutex;

    use super::TestableOnceLock;

    impl<T> TestableOnceLock<T> {
        pub const fn new() -> Self {
            Self {
                mutex: Mutex::new(None),
            }
        }

        pub fn get(&self) -> std::sync::MutexGuard<'_, Option<T>> {
            self.mutex.lock().unwrap()
        }

        pub fn set(&self, value: T) {
            *self.get() = Some(value);
        }
    }
}
