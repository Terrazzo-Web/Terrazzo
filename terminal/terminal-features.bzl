"""Generated feature dependency constants."""

BAZEL_DEPS = []
CLIENT_DEPS = [
    "@crates//:stylance",
    "@crates//:wasm-bindgen",
    "@crates//:wasm-bindgen-futures",
]
CONVERTER_DEPS = []
REMOTES_UI_DEPS = []
CONVERTER_CLIENT_DEPS = CLIENT_DEPS + CONVERTER_DEPS + REMOTES_UI_DEPS + ["@crates//:web-sys"]
LOGS_PANEL_DEPS = []
LOGS_PANEL_CLIENT_DEPS = CLIENT_DEPS + LOGS_PANEL_DEPS + ["@crates//:futures"]
PORT_FORWARD_DEPS = []
PORT_FORWARD_CLIENT_DEPS = CLIENT_DEPS + PORT_FORWARD_DEPS + REMOTES_UI_DEPS + [
    "@crates//:bitflags",
    "@crates//:scopeguard",
    "@crates//:web-sys",
]
CORRELATION_ID_DEPS = []
TERMINAL_DEPS = CORRELATION_ID_DEPS
TERMINAL_CLIENT_DEPS = CLIENT_DEPS + TERMINAL_DEPS + [
    "@crates//:futures",
    "@crates//:pin-project",
    "@crates//:scopeguard",
    "@crates//:wasm-streams",
    "@crates//:web-sys",
]
TEXT_EDITOR_DEPS = []
TEXT_EDITOR_CLIENT_DEPS = CLIENT_DEPS + REMOTES_UI_DEPS + TEXT_EDITOR_DEPS + [
    "@crates//:futures",
    "@crates//:scopeguard",
    "@crates//:serde-wasm-bindgen",
]
CLIENT_ALL_DEPS = CONVERTER_CLIENT_DEPS + LOGS_PANEL_CLIENT_DEPS + PORT_FORWARD_CLIENT_DEPS + TERMINAL_CLIENT_DEPS + TEXT_EDITOR_CLIENT_DEPS
CONCISE_TRACES_DEPS = []
SERVER_DEPS = [
    "//remote/client",
    "//remote/common",
    "@crates//:axum-extra",
    "@crates//:base64",
    "@crates//:clap",
    "@crates//:const_format",
    "@crates//:futures",
    "@crates//:humantime",
    "@crates//:inventory",
    "@crates//:jsonwebtoken",
    "@crates//:nix",
    "@crates//:openssl",
    "@crates//:pbkdf2",
    "@crates//:pin-project",
    "@crates//:prost",
    "@crates//:prost-types",
    "@crates//:rpassword",
    "@crates//:scopeguard",
    "@crates//:sha2",
    "@crates//:tokio",
    "@crates//:toml",
    "@crates//:tonic",
    "@crates//:tonic-prost",
    "@crates//:tower",
    "@crates//:tower-http",
    "@crates//:tracing",
    "@crates//:uuid",
]
CONVERTER_SERVER_DEPS = CONVERTER_DEPS + SERVER_DEPS + [
    "@crates//:cms",
    "@crates//:hickory-client",
    "@crates//:oid-registry",
    "@crates//:regex",
    "@crates//:rustls-native-certs",
    "@crates//:serde_yaml_ng",
    "@crates//:simple_asn1",
    "@crates//:tls-parser",
    "@crates//:tokio-rustls",
    "@crates//:unescaper",
    "@crates//:url",
    "@crates//:x509-parser",
]
DEBUG_DEPS = []
DIAGNOSTICS_DEPS = []
LOGS_PANEL_SERVER_DEPS = LOGS_PANEL_DEPS + SERVER_DEPS + ["@crates//:tracing-subscriber"]
MAX_LEVEL_DEBUG_DEPS = []
MAX_LEVEL_INFO_DEPS = CONCISE_TRACES_DEPS
NO_WASM_BUILD_DEPS = []
PORT_FORWARD_SERVER_DEPS = PORT_FORWARD_DEPS + SERVER_DEPS
TERMINAL_SERVER_DEPS = SERVER_DEPS + TERMINAL_DEPS + [
    "//pty",
    "@crates//:dashmap",
    "@crates//:pin-project",
    "@crates//:static_assertions",
    "@crates//:tracing-futures",
]
TEXT_EDITOR_SERVER_DEPS = SERVER_DEPS + TEXT_EDITOR_DEPS + [
    "@crates//:libc",
    "@crates//:notify",
    "@crates//:tokio-stream",
]
SERVER_ALL_DEPS = CONVERTER_SERVER_DEPS + LOGS_PANEL_SERVER_DEPS + PORT_FORWARD_SERVER_DEPS + TERMINAL_SERVER_DEPS + TEXT_EDITOR_SERVER_DEPS
PROD_DEPS = MAX_LEVEL_INFO_DEPS + SERVER_ALL_DEPS
RUSTDOC_DEPS = []
