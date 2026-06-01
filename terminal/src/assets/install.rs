#![cfg(feature = "server")]

use terrazzo::declare_asset;
use terrazzo::declare_scss_asset;

pub fn install_assets() {
    terrazzo::install_assets();
    declare_asset!("/assets/index.html")
        .mime(terrazzo::mime::TEXT_HTML_UTF_8.as_ref())
        .install();
    declare_asset!("/assets/bootstrap.js").install();
    declare_asset!("/assets/jsdeps/dist/jsdeps.js").install();
    declare_asset!("/assets/images/favicon.ico").install();
    declare_scss_asset!("target/css/terrazzo-terminal.scss").install();
    install_icons();
    install_pdfjs();
    install_xterm();
    install_wasm();
}

fn install_icons() {
    #[cfg(not(feature = "client"))]
    fn install_icon(mut asset: terrazzo::static_assets::AssetBuilder) {
        let path = std::path::Path::new("icons").join(asset.asset_name);
        let path = path.as_os_str().to_str().unwrap();
        asset.asset_name = path.into();
        asset.install();
    }

    #[cfg(feature = "client")]
    fn install_icon(_: &'static str) {}

    #[cfg(any(feature = "terminal", feature = "text-editor"))]
    install_icon(super::icons::close_tab());
    install_icon(super::icons::key_icon());
    install_icon(super::icons::menu());

    #[cfg(feature = "terminal")]
    {
        install_icon(super::icons::add_tab());
        install_icon(super::icons::terminal());
    }

    #[cfg(feature = "text-editor")]
    {
        install_icon(super::icons::chevron_double_right());
        install_icon(super::icons::collapse_vert());
        install_icon(super::icons::file());
        install_icon(super::icons::folder());
        install_icon(super::icons::loading());
        install_icon(super::icons::refresh());
        install_icon(super::icons::search());
        install_icon(super::icons::slash());
        install_icon(super::icons::text_editor());
    }

    #[cfg(feature = "converter")]
    {
        install_icon(super::icons::converter());
        install_icon(super::icons::copy());
    }

    #[cfg(any(feature = "converter", feature = "text-editor"))]
    install_icon(super::icons::done());

    #[cfg(feature = "port-forward")]
    {
        install_icon(super::icons::add_port_forward());
        install_icon(super::icons::hub());
        install_icon(super::icons::port_forward_loading());
        install_icon(super::icons::port_forward_pending());
        install_icon(super::icons::port_forward_synchronized());
        install_icon(super::icons::trash());
    }

    #[cfg(feature = "logs-panel")]
    {
        install_icon(super::icons::chevron_bar_up());
        install_icon(super::icons::chevron_bar_down());
    }

    install_icon(super::icons::split_horz());
    install_icon(super::icons::split_vert());
    install_icon(super::icons::close_app());
}

fn install_xterm() {
    declare_asset!("/assets/jsdeps/node_modules/@xterm/xterm/css/xterm.css").install();
}

fn install_pdfjs() {
    #[cfg(feature = "text-editor")]
    {
        declare_asset!("/assets/pdfjs/pdf.min.mjs")
            .asset_name("pdfjs/pdf.min.mjs")
            .mime("text/javascript; charset=utf-8")
            .install();
        declare_asset!("/assets/pdfjs/pdf.worker.min.mjs")
            .asset_name("pdfjs/pdf.worker.min.mjs")
            .mime("text/javascript; charset=utf-8")
            .install();
        declare_asset!("/assets/pdfjs/pdfjs-loader.js")
            .asset_name("pdfjs/pdfjs-loader.js")
            .mime("text/javascript; charset=utf-8")
            .install();
    }
}

fn install_wasm() {
    declare_asset!("/target/assets/wasm/terrazzo_terminal.js")
        .asset_name("wasm/terrazzo_terminal.js")
        .install();
    declare_asset!("/target/assets/wasm/terrazzo_terminal_bg.wasm")
        .asset_name("wasm/terrazzo_terminal_bg.wasm")
        .install();
    #[cfg(all(
        not(feature = "client"),
        any(feature = "terminal", feature = "text-editor")
    ))]
    terrazzo::declare_assets_dir!(
        "wasm/snippets",
        "$CARGO_MANIFEST_DIR/target/assets/wasm/snippets"
    );
}
