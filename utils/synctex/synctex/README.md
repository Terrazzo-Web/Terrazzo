# terrazzo-synctex

Safe Rust wrapper around the SyncTeX parser.

The crate owns scanner lifetimes, ties query result nodes to their scanner, and keeps raw C pointer
usage inside a small FFI boundary. The PDF viewer is the expected future consumer, but this crate is
currently standalone.
