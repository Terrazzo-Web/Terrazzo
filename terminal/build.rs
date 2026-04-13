use std::env;

use heck::ToKebabCase as _;
use heck::ToShoutySnakeCase as _;
use terrazzo_build as _;
use tonic_prost_build as _;

#[path = "build/client.rs"]
mod client;
#[path = "build/protos.rs"]
mod protos;

#[derive(Clone, Copy, Debug)]
enum Feature {
    DocsRs,
    Client,
    Server,
    MaxLevelDebug,
    MaxLevelInfo,
    Diagnostics,
    NoWasmBuild,
    Debug,

    Terminal,
    TerminalClient,
    TerminalServer,

    TextEditor,
    TextEditorClient,
    TextEditorServer,
    TextEditorSearch,

    Converter,
    ConverterClient,
    ConverterServer,

    PortForward,
    PortForwardClient,
    PortForwardServer,

    LogsPanel,
    LogsPanelClient,
    LogsPanelServer,
}

impl Feature {
    fn is_set(self) -> bool {
        std::env::var(self.env_name()).is_ok()
    }

    fn feature_name(self) -> String {
        format!("{self:?}").to_kebab_case()
    }

    fn env_name(self) -> String {
        format!("CargoFeature{self:?}").to_shouty_snake_case()
    }

    fn disable(self) -> Option<impl Drop> {
        let env_name = self.env_name();
        let value = std::env::var(&env_name).ok()?;
        unsafe { env::remove_var(&env_name) };
        Some(scopeguard::guard((), |_| unsafe {
            std::env::set_var(env_name, value)
        }))
    }

    fn propagate(self, wasm_pack_options: &mut Vec<String>) {
        if self.is_set() {
            self.add(wasm_pack_options);
        }
    }

    fn add(self, wasm_pack_options: &mut Vec<String>) {
        wasm_pack_options.extend(["--features".into(), self.feature_name()]);
    }
}

fn main() {
    client::main();
    if Feature::Server.is_set() {
        protos::main();
    };
}
