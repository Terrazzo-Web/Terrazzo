#![doc = include_str!("../README.md")]

pub use ::autoclone::autoclone;

#[cfg(feature = "client")]
mod client_impl;
#[cfg(feature = "client")]
pub use self::client_impl::*;

#[cfg(feature = "server")]
mod server_impl;
#[cfg(feature = "server")]
pub use self::server_impl::*;

#[cfg(feature = "client")]
type RefCountPtr<T> = std::rc::Rc<T>;
#[cfg(feature = "client")]
type RefCountWeak<T> = std::rc::Weak<T>;

#[cfg(all(feature = "server", not(feature = "client")))]
type RefCountPtr<T> = std::sync::Arc<T>;
#[cfg(all(feature = "server", not(feature = "client")))]
type RefCountPtr<T> = std::sync::Weak<T>;

#[derive(Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Prc<T: ?Sized>(RefCountPtr<T>);

pub struct Pweak<T: ?Sized>(RefCountWeak<T>);

impl<T> Prc<T> {
    pub fn new(value: T) -> Self {
        Self(RefCountPtr::new(value))
    }
}

impl<T> From<T> for Prc<T> {
    fn from(value: T) -> Self {
        Self(RefCountPtr::from(value))
    }
}

impl From<String> for Prc<str> {
    fn from(value: String) -> Self {
        Self(RefCountPtr::from(value))
    }
}

impl<T: ?Sized> std::ops::Deref for Prc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: std::fmt::Debug + ?Sized> std::fmt::Debug for Prc<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}

impl<T: std::fmt::Display + ?Sized> std::fmt::Display for Prc<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl<T: ?Sized> AsRef<T> for Prc<T> {
    fn as_ref(&self) -> &T {
        &self
    }
}

impl<T: ?Sized> Clone for Prc<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
