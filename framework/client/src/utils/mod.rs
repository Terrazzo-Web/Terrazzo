//! Utils

use wasm_bindgen::JsValue;

pub mod or_else_log;

/// [Send]+[Sync] wrapper.
///
/// Safe because Javascript is single-threaded.
pub struct UiThreadSafe<T: AsRef<JsValue>>(T);

/// Safe because Javascript is single-threaded.
unsafe impl<T: AsRef<JsValue>> Send for UiThreadSafe<T> {}

/// Safe because Javascript is single-threaded.
unsafe impl<T: AsRef<JsValue>> Sync for UiThreadSafe<T> {}

impl<T: AsRef<JsValue>> std::ops::Deref for UiThreadSafe<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: AsRef<JsValue>, R> AsRef<R> for UiThreadSafe<T>
where
    T: AsRef<R>,
{
    fn as_ref(&self) -> &R {
        self.0.as_ref()
    }
}

impl<T: AsRef<JsValue>> From<T> for UiThreadSafe<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}
