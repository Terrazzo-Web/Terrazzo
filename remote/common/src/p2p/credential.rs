// Basic
#[derive(Clone, Default, PartialEq, Eq)]
// Serialization / Deserialization
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Credential(String);

impl From<Credential> for String {
    fn from(credential: Credential) -> Self {
        credential.0
    }
}

impl From<String> for Credential {
    fn from(credential: String) -> Self {
        Self(credential)
    }
}

impl From<&str> for Credential {
    fn from(credential: &str) -> Self {
        Self(credential.to_owned())
    }
}

impl Credential {
    pub fn into_string(self) -> String {
        self.0
    }

    pub fn as_string(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Credential")
            .field(&(!self.0.is_empty()).then_some("[REDACTED]"))
            .finish()
    }
}
