use std::hash::Hash;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;

#[derive(Clone, Copy)]
pub struct WithGenerationId<T> {
    pub value: T,
    pub generation_id: usize,
}

impl<T> AsRef<T> for WithGenerationId<T> {
    fn as_ref(&self) -> &T {
        &self.value
    }
}

impl<T> std::ops::Deref for WithGenerationId<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> std::ops::DerefMut for WithGenerationId<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T: std::fmt::Display> std::fmt::Display for WithGenerationId<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.value, f)
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for WithGenerationId<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.value, f)
    }
}

impl<T: PartialEq> PartialEq for WithGenerationId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T: Eq> Eq for WithGenerationId<T> {}

impl<T: PartialOrd> PartialOrd for WithGenerationId<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

impl<T: Ord> Ord for WithGenerationId<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value.cmp(&other.value)
    }
}

impl<T: Hash> Hash for WithGenerationId<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<T> From<T> for WithGenerationId<T> {
    fn from(value: T) -> Self {
        static NEXT_GENERATION_ID: AtomicUsize = AtomicUsize::new(1);
        Self {
            value,
            generation_id: NEXT_GENERATION_ID.fetch_add(1, SeqCst),
        }
    }
}

impl<T: Default> Default for WithGenerationId<T> {
    fn default() -> Self {
        T::default().into()
    }
}
