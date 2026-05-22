use std::num::NonZero;

// Basic
#[derive(Clone, Copy, Debug)]
// Serialization-Deserialization support
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
// HashMap support
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileId(NonZero<i64>);

impl TileId {
    #[cfg(feature = "server")]
    pub fn new() -> Self {
        use std::sync::atomic::AtomicI64;
        use std::sync::atomic::Ordering;
        static NEXT: AtomicI64 = AtomicI64::new(const { TileId::first_tile_id().0.get() + 1 });
        Self(NonZero::new(NEXT.fetch_add(1, Ordering::SeqCst)).unwrap())
    }

    pub const fn first_tile_id() -> Self {
        Self(NonZero::new(1).unwrap())
    }

    #[cfg(all(test, feature = "server"))]
    pub const fn for_test(id: i64) -> Self {
        Self(NonZero::new(id).unwrap())
    }
}

impl std::fmt::Display for TileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZero;

    use super::TileId;

    #[test]
    fn serde() {
        assert_eq!(
            "1337",
            serde_json::to_string(&TileId(NonZero::new(1337).unwrap())).unwrap()
        );
        assert_eq!(
            TileId(NonZero::new(42).unwrap()),
            serde_json::from_str::<TileId>("42").unwrap()
        );
    }
}
