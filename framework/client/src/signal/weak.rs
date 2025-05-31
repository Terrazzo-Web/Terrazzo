use super::XSignal;
use super::XSignalInner;
use crate::utils::Ptr;
use crate::utils::PtrWeak;

pub struct XSignalWeak<T>(PtrWeak<XSignalInner<T>>);

impl<T> XSignalWeak<T> {
    pub fn upgrade(&self) -> Option<XSignal<T>> {
        Some(XSignal(self.0.upgrade()?))
    }
}

impl<T> XSignal<T> {
    pub fn downgrade(&self) -> XSignalWeak<T> {
        XSignalWeak(Ptr::downgrade(&self.0))
    }
}

impl<T> Clone for XSignalWeak<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
