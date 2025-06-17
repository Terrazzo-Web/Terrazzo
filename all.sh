#!/bin/bash

set -e

cargo build --features=client
cargo build --features=server
cargo test --features=client
cargo test --features=server
cargo clippy --features=client,server
cargo clippy --features client
cargo clippy --features client,diagnostics
cargo clippy --features server
cargo doc --all-features
cargo build --bin demo-server --features server,max_level_debug
cargo build --bin demo-server --features server,diagnostics
