#![cfg_attr(not(feature = "bazel"), allow(unused_crate_dependencies))]

#[tokio::main]
async fn main() {
    terrazzo_demo::run_server().await
}
