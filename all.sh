#!/bin/bash

set -e

cargo build --features=client
cargo build --features=server
cargo test --features=client
cargo test --features=server
cargo clippy --features=client,server
cargo clippy --features client
cargo clippy --features server
