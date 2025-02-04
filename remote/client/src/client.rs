use trz_gateway_common::declare_identifier;

pub mod certificate;
pub mod connect;

pub struct Client {
    client: reqwest::Client,
}

#[derive(Default)]
pub enum TrustedRoots {
    #[default]
    System,
    Extra(Vec<String>),
    Only(Vec<String>),
}

declare_identifier!(AuthCode);
