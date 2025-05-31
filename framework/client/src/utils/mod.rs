//! Utils

use std::sync::Arc;

pub mod or_else_log;

#[derive(Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ptr<T>(std::sync::Arc<T>)
where
    T: ?Sized;

pub struct PtrWeak<T>(std::sync::Weak<T>)
where
    T: ?Sized;

impl<T> From<T> for Ptr<T> {
    fn from(value: T) -> Self {
        Self(Arc::from(value))
    }
}

impl From<&str> for Ptr<str> {
    fn from(value: &str) -> Self {
        Self(Arc::from(value))
    }
}

impl<T> From<Box<T>> for Ptr<T>
where
    T: ?Sized,
{
    fn from(value: Box<T>) -> Self {
        Self(Arc::from(value))
    }
}

impl From<String> for Ptr<str> {
    fn from(value: String) -> Self {
        Self(Arc::from(value))
    }
}

impl<T> Ptr<T> {
    pub fn new(value: T) -> Ptr<T> {
        Ptr(Arc::new(value))
    }

    pub fn try_unwrap(this: Self) -> Result<T, Self> {
        Arc::try_unwrap(this.0).map_err(Ptr)
    }
}

impl<T> Ptr<T>
where
    T: ?Sized,
{
    pub fn downgrade(this: &Self) -> PtrWeak<T> {
        PtrWeak(Arc::downgrade(&this.0))
    }

    pub fn strong_count(this: &Self) -> usize {
        Arc::strong_count(&this.0)
    }
}

impl<T> Clone for Ptr<T>
where
    T: ?Sized,
{
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Ptr<T>
where
    T: ?Sized,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}

impl<T: std::fmt::Display> std::fmt::Display for Ptr<T>
where
    T: ?Sized,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl<T> std::fmt::Pointer for Ptr<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Pointer::fmt(&self.0, f)
    }
}

impl<T> std::ops::Deref for Ptr<T>
where
    T: ?Sized,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> AsRef<T> for Ptr<T>
where
    T: ?Sized,
{
    fn as_ref(&self) -> &T {
        &self.0.as_ref()
    }
}

impl<T> PtrWeak<T>
where
    T: ?Sized,
{
    pub fn upgrade(&self) -> Option<Ptr<T>> {
        self.0.upgrade().map(Ptr)
    }
}

impl<T> Clone for PtrWeak<T>
where
    T: ?Sized,
{
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
