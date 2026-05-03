# terrazzo-synctex-sys

Low-level Rust FFI bindings for the SyncTeX parser.

The C sources in `vendor/synctex` are vendored from:

- repository: <https://github.com/jlaurens/synctex>
- commit: `917617707955cde0c2fae127130d9d3129303cbc`
- upstream license: MIT-style license in `vendor/synctex/LICENSE`

The public Rust declarations mirror `synctex_parser.h`. Safe Rust code should depend on
`terrazzo-synctex` instead of calling this crate directly.
