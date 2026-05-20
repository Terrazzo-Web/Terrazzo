use serde::Deserialize;
use serde::Deserializer;
use serde::Serializer;

pub fn serialize<S>(domains: &[String], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.collect_seq(domains)
}

pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Domains {
        Single(String),
        Multiple(Vec<String>),
    }

    Ok(match Domains::deserialize(deserializer)? {
        Domains::Single(domain) => vec![domain],
        Domains::Multiple(domains) => domains,
    })
}
