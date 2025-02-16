#[macro_export]
macro_rules! declare_identifier {
    ($name:ident) => {
        #[nameth::nameth]
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name {
            id: std::sync::Arc<str>,
        }

        impl From<String> for $name {
            fn from(id: String) -> Self {
                Self {
                    id: id.into_boxed_str().into(),
                }
            }
        }

        impl From<&str> for $name {
            fn from(id: &str) -> Self {
                id.to_owned().into()
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.id
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;
            fn deref(&self) -> &Self::Target {
                self.as_ref()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(self.as_ref(), f)
            }
        }
    };
}

declare_identifier!(ClientId);

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    #[test]
    fn declare_identifier_compare() {
        declare_identifier!(ConnectionId);
        let c1: ConnectionId = "123".into();
        let c2: ConnectionId = "123".to_string().into();
        assert_eq!(c1, c2);

        let c3: ConnectionId = "124".to_string().into();
        assert_ne!(c1, c3);

        assert!(c1 == c2);
        assert!(c1 < c3);
    }

    #[test]
    fn declare_identifier_hash() {
        declare_identifier!(ConnectionId);
        let c1: ConnectionId = "123".into();
        let c2: ConnectionId = "124".to_string().into();

        let mut map: HashMap<ConnectionId, i32> = HashMap::new();
        map.insert(c1.clone(), 21);
        map.insert(c2.clone(), 34);

        assert_eq!(map[&c1], 21);
        assert_eq!(map[&c2], 34);
    }

    #[test]
    fn declare_identifier_serde() {
        declare_identifier!(ConnectionId);
        let c = ConnectionId::from("ABC123");
        let s = serde_json::to_string(&c).unwrap();
        assert_eq!("\"ABC123\"", s);
        let cc = serde_json::from_str(&s).unwrap();
        assert_eq!(c, cc);
    }
}
