#![allow(unused_crate_dependencies)]

use tracing::Level;

fn main() {
    #[cfg(target_arch = "wasm32")]
    compile_error!();

    #[cfg(all(feature = "bazel", feature = "debug"))]
    let run_server = terrazzo_terminal_debug::run_server();
    #[cfg(not(all(feature = "bazel", feature = "debug")))]
    let run_server = terrazzo_terminal::run_server();

    run_server.unwrap_or_else(|error| {
        if tracing::enabled!(Level::ERROR) {
            tracing::error!("{error}")
        } else {
            eprintln!("{error}")
        }
    })
}
