"""Generated feature dependency constants."""

load("//bazel/feature_deps:feature_deps_rules.bzl", "base_compute_srcs")

_ALL_FEATURES = ["bazel", "client", "client-all", "client-prod", "concise-traces", "converter", "converter-client", "converter-server", "correlation-id", "debug", "diagnostics", "logs-panel", "logs-panel-client", "logs-panel-server", "max-level-debug", "max-level-info", "no_wasm_build", "port-forward", "port-forward-client", "port-forward-server", "prod", "remote-fn", "remote-fn-streaming", "remote-fn-unary", "remotes-ui", "rustdoc", "server", "server-all", "terminal", "terminal-client", "terminal-server", "text-editor", "text-editor-client", "text-editor-server", "tiles-state", "tiles-state-client", "tiles-state-server"]
BAZEL_DEPS = []
BAZEL_FEATURES = ["bazel"]
CLIENT_DEPS = [
    "//utils/css/css",
    "@crates//:wasm-bindgen",
    "@crates//:wasm-bindgen-futures",
]
CLIENT_FEATURES = ["client"]
CONVERTER_DEPS = []
CONVERTER_FEATURES = ["converter"]
REMOTES_UI_DEPS = []
REMOTES_UI_FEATURES = ["remotes-ui"]
TILES_STATE_DEPS = []
TILES_STATE_FEATURES = ["tiles-state"]
TILES_STATE_CLIENT_DEPS = TILES_STATE_DEPS + ["@crates//:futures"]
TILES_STATE_CLIENT_FEATURES = TILES_STATE_FEATURES + ["tiles-state-client"]
CONVERTER_CLIENT_DEPS = CLIENT_DEPS + CONVERTER_DEPS + REMOTES_UI_DEPS + TILES_STATE_CLIENT_DEPS + ["@crates//:web-sys"]
CONVERTER_CLIENT_FEATURES = CLIENT_FEATURES + CONVERTER_FEATURES + REMOTES_UI_FEATURES + TILES_STATE_CLIENT_FEATURES + ["converter-client"]
LOGS_PANEL_DEPS = []
LOGS_PANEL_FEATURES = ["logs-panel"]
LOGS_PANEL_CLIENT_DEPS = CLIENT_DEPS + LOGS_PANEL_DEPS + [
    "@crates//:futures",
    "@crates//:scopeguard",
]
LOGS_PANEL_CLIENT_FEATURES = CLIENT_FEATURES + LOGS_PANEL_FEATURES + ["logs-panel-client"]
PORT_FORWARD_DEPS = []
PORT_FORWARD_FEATURES = ["port-forward"]
PORT_FORWARD_CLIENT_DEPS = CLIENT_DEPS + PORT_FORWARD_DEPS + REMOTES_UI_DEPS + [
    "@crates//:bitflags",
    "@crates//:scopeguard",
    "@crates//:web-sys",
]
PORT_FORWARD_CLIENT_FEATURES = CLIENT_FEATURES + PORT_FORWARD_FEATURES + REMOTES_UI_FEATURES + ["port-forward-client"]
CORRELATION_ID_DEPS = []
CORRELATION_ID_FEATURES = ["correlation-id"]
TERMINAL_DEPS = CORRELATION_ID_DEPS
TERMINAL_FEATURES = CORRELATION_ID_FEATURES + ["terminal"]
TERMINAL_CLIENT_DEPS = CLIENT_DEPS + TERMINAL_DEPS + [
    "@crates//:futures",
    "@crates//:pin-project",
    "@crates//:scopeguard",
    "@crates//:wasm-streams",
    "@crates//:web-sys",
]
TERMINAL_CLIENT_FEATURES = CLIENT_FEATURES + TERMINAL_FEATURES + ["terminal-client"]
TEXT_EDITOR_DEPS = []
TEXT_EDITOR_FEATURES = ["text-editor"]
TEXT_EDITOR_CLIENT_DEPS = CLIENT_DEPS + REMOTES_UI_DEPS + TEXT_EDITOR_DEPS + TILES_STATE_CLIENT_DEPS + [
    "@crates//:base64",
    "@crates//:futures",
    "@crates//:scopeguard",
    "@crates//:serde-wasm-bindgen",
    "@crates//:web-sys",
]
TEXT_EDITOR_CLIENT_FEATURES = CLIENT_FEATURES + REMOTES_UI_FEATURES + TEXT_EDITOR_FEATURES + TILES_STATE_CLIENT_FEATURES + ["text-editor-client"]
CLIENT_ALL_DEPS = CONVERTER_CLIENT_DEPS + LOGS_PANEL_CLIENT_DEPS + PORT_FORWARD_CLIENT_DEPS + TERMINAL_CLIENT_DEPS + TEXT_EDITOR_CLIENT_DEPS
CLIENT_ALL_FEATURES = CONVERTER_CLIENT_FEATURES + LOGS_PANEL_CLIENT_FEATURES + PORT_FORWARD_CLIENT_FEATURES + TERMINAL_CLIENT_FEATURES + TEXT_EDITOR_CLIENT_FEATURES + ["client-all"]
CONCISE_TRACES_DEPS = []
CONCISE_TRACES_FEATURES = ["concise-traces"]
MAX_LEVEL_INFO_DEPS = CONCISE_TRACES_DEPS
MAX_LEVEL_INFO_FEATURES = CONCISE_TRACES_FEATURES + ["max-level-info"]
CLIENT_PROD_DEPS = CLIENT_ALL_DEPS + MAX_LEVEL_INFO_DEPS
CLIENT_PROD_FEATURES = CLIENT_ALL_FEATURES + MAX_LEVEL_INFO_FEATURES + ["client-prod"]
REMOTE_FN_DEPS = []
REMOTE_FN_FEATURES = ["remote-fn"]
REMOTE_FN_UNARY_DEPS = REMOTE_FN_DEPS + ["@crates//:inventory"]
REMOTE_FN_UNARY_FEATURES = REMOTE_FN_FEATURES + ["remote-fn-unary"]
SERVER_DEPS = [
    "//remote/client",
    "//remote/common",
    "@crates//:axum-extra",
    "@crates//:base64",
    "@crates//:clap",
    "@crates//:futures",
    "@crates//:humantime",
    "@crates//:jsonwebtoken",
    "@crates//:nix",
    "@crates//:notify",
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
SERVER_FEATURES = ["server"]
TILES_STATE_SERVER_DEPS = TILES_STATE_DEPS + ["@crates//:const_format"]
TILES_STATE_SERVER_FEATURES = TILES_STATE_FEATURES + ["tiles-state-server"]
CONVERTER_SERVER_DEPS = CONVERTER_DEPS + REMOTE_FN_UNARY_DEPS + SERVER_DEPS + TILES_STATE_SERVER_DEPS + [
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
CONVERTER_SERVER_FEATURES = CONVERTER_FEATURES + REMOTE_FN_UNARY_FEATURES + SERVER_FEATURES + TILES_STATE_SERVER_FEATURES + ["converter-server"]
DEBUG_DEPS = []
DEBUG_FEATURES = ["debug"]
DIAGNOSTICS_DEPS = []
DIAGNOSTICS_FEATURES = ["diagnostics"]
REMOTE_FN_STREAMING_DEPS = REMOTE_FN_DEPS + [
    "@crates//:inventory",
    "@crates//:tracing-futures",
]
REMOTE_FN_STREAMING_FEATURES = REMOTE_FN_FEATURES + ["remote-fn-streaming"]
LOGS_PANEL_SERVER_DEPS = LOGS_PANEL_DEPS + REMOTE_FN_STREAMING_DEPS + SERVER_DEPS + ["@crates//:tracing-subscriber"]
LOGS_PANEL_SERVER_FEATURES = LOGS_PANEL_FEATURES + REMOTE_FN_STREAMING_FEATURES + SERVER_FEATURES + ["logs-panel-server"]
MAX_LEVEL_DEBUG_DEPS = []
MAX_LEVEL_DEBUG_FEATURES = ["max-level-debug"]
NO_WASM_BUILD_DEPS = []
NO_WASM_BUILD_FEATURES = ["no_wasm_build"]
PORT_FORWARD_SERVER_DEPS = PORT_FORWARD_DEPS + REMOTE_FN_UNARY_DEPS + SERVER_DEPS
PORT_FORWARD_SERVER_FEATURES = PORT_FORWARD_FEATURES + REMOTE_FN_UNARY_FEATURES + SERVER_FEATURES + ["port-forward-server"]
TERMINAL_SERVER_DEPS = SERVER_DEPS + TERMINAL_DEPS + [
    "//pty",
    "@crates//:dashmap",
    "@crates//:pin-project",
    "@crates//:static_assertions",
    "@crates//:tracing-futures",
]
TERMINAL_SERVER_FEATURES = SERVER_FEATURES + TERMINAL_FEATURES + ["terminal-server"]
TEXT_EDITOR_SERVER_DEPS = REMOTE_FN_UNARY_DEPS + SERVER_DEPS + TEXT_EDITOR_DEPS + TILES_STATE_SERVER_DEPS + [
    "@crates//:libc",
    "@crates//:lru",
    "@crates//:notify",
    "@crates//:tokio-stream",
]
TEXT_EDITOR_SERVER_FEATURES = REMOTE_FN_UNARY_FEATURES + SERVER_FEATURES + TEXT_EDITOR_FEATURES + TILES_STATE_SERVER_FEATURES + ["text-editor-server"]
SERVER_ALL_DEPS = CONVERTER_SERVER_DEPS + LOGS_PANEL_SERVER_DEPS + PORT_FORWARD_SERVER_DEPS + TERMINAL_SERVER_DEPS + TEXT_EDITOR_SERVER_DEPS
SERVER_ALL_FEATURES = CONVERTER_SERVER_FEATURES + LOGS_PANEL_SERVER_FEATURES + PORT_FORWARD_SERVER_FEATURES + TERMINAL_SERVER_FEATURES + TEXT_EDITOR_SERVER_FEATURES + ["server-all"]
PROD_DEPS = MAX_LEVEL_INFO_DEPS + SERVER_ALL_DEPS
PROD_FEATURES = MAX_LEVEL_INFO_FEATURES + SERVER_ALL_FEATURES + ["prod"]
RUSTDOC_DEPS = []
RUSTDOC_FEATURES = ["rustdoc"]
_EXCLUSION_MAP = [
    {"feature": "bazel", "delta": []},
    {"feature": "client-all", "delta": []},
    {"feature": "client-prod", "delta": []},
    {"feature": "concise-traces", "delta": []},
    {"feature": "converter-client", "delta": []},
    {"feature": "converter-server", "delta": []},
    {"feature": "correlation-id", "delta": []},
    {"feature": "debug", "delta": []},
    {"feature": "diagnostics", "delta": []},
    {"feature": "logs-panel-client", "delta": []},
    {"feature": "logs-panel-server", "delta": []},
    {"feature": "max-level-debug", "delta": []},
    {"feature": "max-level-info", "delta": []},
    {"feature": "no_wasm_build", "delta": []},
    {"feature": "port-forward-client", "delta": []},
    {"feature": "port-forward-server", "delta": []},
    {"feature": "prod", "delta": []},
    {"feature": "remotes-ui", "delta": []},
    {"feature": "rustdoc", "delta": []},
    {"feature": "server-all", "delta": []},
    {"feature": "terminal-client", "delta": []},
    {"feature": "terminal-server", "delta": []},
    {"feature": "text-editor-client", "delta": []},
    {"feature": "text-editor-server", "delta": []},
    {"feature": "tiles-state", "delta": []},
    {"feature": "tiles-state-client", "delta": []},
    {"feature": "tiles-state-server", "delta": []},
    {"feature": "remote-fn-streaming", "delta": [150, 9]},
    {"feature": "remote-fn", "delta": [168, 6, 503]},
    {"feature": "remote-fn-unary", "delta": [-501, -166, 9]},
    {"feature": "converter", "delta": [-178, 6, 246, 2, 252, 15]},
    {"feature": "logs-panel", "delta": [-280, 15, -248, 2, 304, 4, 314, 5]},
    {"feature": "port-forward", "delta": [-322, 5, -310, 4, 134, 4, 144, 2, 324, 3, 332, 4]},
    {"feature": "text-editor", "delta": [-338, 4, -328, 3, -146, 2, -140, 4, 112, 4, 122, 6, 296, 3, 374, 14, 404, 15, 438, 14]},
    {"feature": "client", "delta": [-456, 10, -432, 15, -400, 14, -132, 6, -118, 4, 3, 6, 17, 282, 2, 288, 4, 305, 309, 358, 2, 364, 4]},
    {"feature": "terminal", "delta": [-464, 4, -307, -303, -300, 7, -284, 2, -11, -8, 2, -1, 58, 13, 188, 5, 200, 6, 340, 3, 348, 4]},
    {"feature": "server", "delta": [-370, 4, -360, 2, -38, 13, -9, 46, 4, 57, 94, 47, 199, 212, 11, 236, 5, 254, 13, 327]},
]

def compute_srcs(features):
    return base_compute_srcs(features, _ALL_FEATURES, _EXCLUSION_MAP)
