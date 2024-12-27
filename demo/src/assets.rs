#![cfg(feature = "server")]

use terrazzo::declare_asset;
use terrazzo::declare_scss_asset;

pub fn install_assets() {
    terrazzo::install_assets();
    declare_asset!("/assets/index.html")
        .mime(terrazzo::mime::TEXT_HTML_UTF_8.as_ref())
        .install();
    declare_asset!("/assets/bootstrap.js").install();
    declare_asset!("/assets/favicon/favicon.ico").install();
    declare_scss_asset!("target/css/demo.scss").install();
    install_wasm();
}

fn install_wasm() {
    declare_asset!("/assets/wasm/terrazzo_demo.js")
        .asset_name("wasm/terrazzo_demo.js")
        .install();
    declare_asset!("/assets/wasm/terrazzo_demo_bg.wasm")
        .asset_name("wasm/terrazzo_demo_bg.wasm")
        .install();
}
