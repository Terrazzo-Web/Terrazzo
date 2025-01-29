use std::path::PathBuf;

use trz_gateway_common::declare_identifier;

pub mod certificate;
pub mod connect;

pub struct Client {
    client: reqwest::Client,
    base_url: String,
    certificate_store_path: Option<PathBuf>,
    trusted_roots: TrustedRoots,
}

#[derive(Default)]
pub enum TrustedRoots {
    #[default]
    System,
    Extra(Vec<String>),
    Only(Vec<String>),
}

declare_identifier!(AuthCode);
