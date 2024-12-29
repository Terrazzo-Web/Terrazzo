# Terrazzo

Terrazzo is a lightweight, simple and efficient web UI framework based on Rust and WASM.

# Components

## Terrazzo library
[Terrazzo](framework/terrazzo/README.md)

This is the umbrella library that re-exports the macros and runtime library, as well as some reusable widgets and tooling.

## Terrazzo macros
Syntax sugar to generate dynamic HTML nodes in Rust.

[Terrazzo macros](framework/macro/README.md)

## Terrazzo client
Runtime rendering library.

[Terrazzo client](framework/client/README.md)

## Terrazzo build
Build scripts to compile and package the WASM assembly.

[Terrazzo build](framework/build/README.md)

# Demo
Terrazzo `"Hello, World!"` demo. Shows how to use macros and signals to build a dynamic web page with Terrazzo.

[Terrazzo demo](demo/README.md)

# Utils

## Autoclone
The [autoclone!](autoclone/autoclone/README.md) syntax sugar.

## Nameth
The [`#[nameth]`](nameth/nameth/README.md) macro.
