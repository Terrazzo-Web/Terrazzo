use std::sync::atomic::AtomicI64;

// Basic
#[derive(Clone, Copy, Debug)]
// Serialization-Deserialization support
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
// HashMap support
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileId(i64);

impl TileId {
    #[cfg(feature = "server")]
    pub fn new() -> Self {
        static NEXT: AtomicI64 = AtomicI64::new(1);
        Self(NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst))
    }

    pub const fn first_tile_id() -> Self {
        Self(0)
    }

    #[cfg(test)]
    pub const fn for_test(id: i64) -> Self {
        Self(id)
    }
}

#[cfg(test)]
mod tests {
    use super::TileId;

    #[test]
    fn serde() {
        assert_eq!("1337", serde_json::to_string(&TileId(1337)).unwrap());
        assert_eq!(TileId(42), serde_json::from_str::<TileId>("42").unwrap());
    }
}
