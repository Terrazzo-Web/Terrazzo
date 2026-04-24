use std::env;
use std::path::PathBuf;

use terrazzo_build::BuildOptions;

use crate::Feature;

pub fn main() {
    if Feature::NoWasmBuild.is_set() {
        return;
    }
    if Feature::DocsRs.is_set() {
        return;
    }

    let Some(disable_server_feature) = Feature::Server.disable() else {
        return;
    };
    let disable_server_features = (
        Feature::TerminalServer.disable(),
        Feature::TextEditorServer.disable(),
        Feature::ConverterServer.disable(),
        Feature::PortForwardServer.disable(),
        Feature::LogsPanelServer.disable(),
    );

    if Feature::Client.is_set() {
        println!("cargo::warning=Can't enable both 'client' and 'server' features");
    }

    let cargo_manifest_dir: PathBuf = env::var("CARGO_MANIFEST_DIR").unwrap().into();
    let server_dir = cargo_manifest_dir.join("target");
    std::fs::create_dir_all(server_dir.join("assets")).expect("server_dir");
    let client_dir = cargo_manifest_dir;

    let mut wasm_pack_options = vec!["--no-default-features".into()];
    Feature::MaxLevelDebug.propagate(&mut wasm_pack_options);
    Feature::MaxLevelInfo.propagate(&mut wasm_pack_options);
    Feature::Diagnostics.propagate(&mut wasm_pack_options);
    Feature::Debug.propagate(&mut wasm_pack_options);
    if Feature::Terminal.is_set() {
        Feature::TerminalClient.add(&mut wasm_pack_options);
    }
    if Feature::TextEditor.is_set() {
        Feature::TextEditorClient.add(&mut wasm_pack_options);
    }
    if Feature::TextEditorSearch.is_set() {
        Feature::TextEditorSearch.add(&mut wasm_pack_options);
    }
    if Feature::Converter.is_set() {
        Feature::ConverterClient.add(&mut wasm_pack_options);
    }
    if Feature::PortForward.is_set() {
        Feature::PortForwardClient.add(&mut wasm_pack_options);
    }
    if Feature::LogsPanel.is_set() {
        Feature::LogsPanelClient.add(&mut wasm_pack_options);
    }
    let wasm_pack_options = wasm_pack_options
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>();
    let wasm_pack_options = &wasm_pack_options;
    terrazzo_build::build(BuildOptions {
        client_dir,
        server_dir,
        wasm_pack_options,
    })
    .unwrap();
    terrazzo_build::build_css();

    drop(disable_server_feature);
    drop(disable_server_features);
}
