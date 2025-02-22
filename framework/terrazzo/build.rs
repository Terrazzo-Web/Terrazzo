use std::env;

use scopeguard::defer;

const SERVER_FEATURE: &str = "CARGO_FEATURE_SERVER";
const CLIENT_FEATURE: &str = "CARGO_FEATURE_CLIENT";
const RUSTDOC_FEATURE: &str = "CARGO_FEATURE_RUSTDOC";

fn main() {
    if env::var(RUSTDOC_FEATURE).is_ok() {
        return;
    }

    let Ok(server_feature) = env::var(SERVER_FEATURE) else {
        return;
    };
    unsafe { env::remove_var(SERVER_FEATURE) };
    defer!(unsafe { std::env::set_var(SERVER_FEATURE, server_feature) });

    if env::var(CLIENT_FEATURE).is_ok() {
        println!("cargo::warning=Can't enable both 'client' and 'server' features");
    }

    terrazzo_build::build_css();
}
