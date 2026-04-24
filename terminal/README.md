# Terrazzo terminal

Terrazzo terminal is a simple web-based terminal built in Rust and Web Assembly 
using the [Terrazzo](https://docs.rs/terrazzo) library.

## Getting started
Pre-requisite:
- [`wasm-pack` CLI](https://rustwasm.github.io/wasm-pack/installer/)
- [`terrazzo-css-cli` CLI](https://github.com/Terrazzo-Web/Terrazzo/tree/main/utils/css/cli)

```
cargo install --locked wasm-pack
cargo install --locked terrazzo-css-cli
```

## Compile from source
Then run `terrazzo-terminal` using
```
cargo run --locked --bin terrazzo-terminal --release --features prod
```

Open the address printed on the terminal to stack hacking
```
Listening on http://127.0.0.1:3001
```

## Install using `cargo install`
```
cargo install --locked terrazzo-terminal --features prod
```

Then start it using
```
terrazzo-terminal
```
