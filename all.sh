#!/bin/bash

set -e

cargo build --features=client
cargo build --features=server
cargo test --features=client
cargo test --features=server
cargo clippy --features=client,server
cargo clippy --features client,debug,diagnostics
cargo clippy --features client --release
cargo clippy --features server,debug,diagnostics
cargo clippy --features server --release
cargo doc --all-features
cargo build --bin demo-server --features server,max_level_debug,debug,diagnostics
cargo build --bin demo-server --features server
