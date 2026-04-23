"""Generated feature dependency constants."""

load("//bazel/feature_deps:feature_deps_rules.bzl", "base_compute_srcs")

_ALL_FEATURES = ["bazel", "client", "client-all", "client-prod", "concise-traces", "converter", "converter-client", "converter-server", "correlation-id", "debug", "diagnostics", "logs-panel", "logs-panel-client", "logs-panel-server", "max-level-debug", "max-level-info", "no_wasm_build", "port-forward", "port-forward-client", "port-forward-server", "prod", "remotes-ui", "rustdoc", "server", "server-all", "streaming-remote-fn", "terminal", "terminal-client", "terminal-server", "text-editor", "text-editor-client", "text-editor-server"]
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
CONVERTER_CLIENT_DEPS = CLIENT_DEPS + CONVERTER_DEPS + REMOTES_UI_DEPS + [
    "@crates//:futures",
    "@crates//:web-sys",
]
CONVERTER_CLIENT_FEATURES = CLIENT_FEATURES + CONVERTER_FEATURES + REMOTES_UI_FEATURES + ["converter-client"]
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
TEXT_EDITOR_CLIENT_DEPS = CLIENT_DEPS + REMOTES_UI_DEPS + TEXT_EDITOR_DEPS + [
    "@crates//:futures",
    "@crates//:scopeguard",
    "@crates//:serde-wasm-bindgen",
]
TEXT_EDITOR_CLIENT_FEATURES = CLIENT_FEATURES + REMOTES_UI_FEATURES + TEXT_EDITOR_FEATURES + ["text-editor-client"]
CLIENT_ALL_DEPS = CONVERTER_CLIENT_DEPS + LOGS_PANEL_CLIENT_DEPS + PORT_FORWARD_CLIENT_DEPS + TERMINAL_CLIENT_DEPS + TEXT_EDITOR_CLIENT_DEPS
CLIENT_ALL_FEATURES = CONVERTER_CLIENT_FEATURES + LOGS_PANEL_CLIENT_FEATURES + PORT_FORWARD_CLIENT_FEATURES + TERMINAL_CLIENT_FEATURES + TEXT_EDITOR_CLIENT_FEATURES + ["client-all"]
CONCISE_TRACES_DEPS = []
CONCISE_TRACES_FEATURES = ["concise-traces"]
MAX_LEVEL_INFO_DEPS = CONCISE_TRACES_DEPS
MAX_LEVEL_INFO_FEATURES = CONCISE_TRACES_FEATURES + ["max-level-info"]
CLIENT_PROD_DEPS = CLIENT_ALL_DEPS + MAX_LEVEL_INFO_DEPS
CLIENT_PROD_FEATURES = CLIENT_ALL_FEATURES + MAX_LEVEL_INFO_FEATURES + ["client-prod"]
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
CONVERTER_SERVER_FEATURES = CONVERTER_FEATURES + SERVER_FEATURES + ["converter-server"]
DEBUG_DEPS = []
DEBUG_FEATURES = ["debug"]
DIAGNOSTICS_DEPS = []
DIAGNOSTICS_FEATURES = ["diagnostics"]
STREAMING_REMOTE_FN_DEPS = ["@crates//:tracing-futures"]
STREAMING_REMOTE_FN_FEATURES = ["streaming-remote-fn"]
LOGS_PANEL_SERVER_DEPS = LOGS_PANEL_DEPS + SERVER_DEPS + STREAMING_REMOTE_FN_DEPS + ["@crates//:tracing-subscriber"]
LOGS_PANEL_SERVER_FEATURES = LOGS_PANEL_FEATURES + SERVER_FEATURES + STREAMING_REMOTE_FN_FEATURES + ["logs-panel-server"]
MAX_LEVEL_DEBUG_DEPS = []
MAX_LEVEL_DEBUG_FEATURES = ["max-level-debug"]
NO_WASM_BUILD_DEPS = []
NO_WASM_BUILD_FEATURES = ["no_wasm_build"]
PORT_FORWARD_SERVER_DEPS = PORT_FORWARD_DEPS + SERVER_DEPS
PORT_FORWARD_SERVER_FEATURES = PORT_FORWARD_FEATURES + SERVER_FEATURES + ["port-forward-server"]
TERMINAL_SERVER_DEPS = SERVER_DEPS + TERMINAL_DEPS + [
    "//pty",
    "@crates//:dashmap",
    "@crates//:pin-project",
    "@crates//:static_assertions",
    "@crates//:tracing-futures",
]
TERMINAL_SERVER_FEATURES = SERVER_FEATURES + TERMINAL_FEATURES + ["terminal-server"]
TEXT_EDITOR_SERVER_DEPS = SERVER_DEPS + TEXT_EDITOR_DEPS + [
    "@crates//:libc",
    "@crates//:notify",
    "@crates//:tokio-stream",
]
TEXT_EDITOR_SERVER_FEATURES = SERVER_FEATURES + TEXT_EDITOR_FEATURES + ["text-editor-server"]
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
    {"feature": "logs-panel", "delta": [316, 4, 326, 5]},
    {"feature": "streaming-remote-fn", "delta": [-334, 5, -322, 4, 174, 3, 182, 5]},
    {"feature": "converter", "delta": [-190, 5, -178, 3, 108, 3, 116, 3, 258, 2, 264, 15]},
    {"feature": "port-forward", "delta": [-292, 15, -260, 2, -120, 3, -112, 3, 148, 4, 158, 2, 336, 3, 344, 4]},
    {"feature": "text-editor", "delta": [-350, 4, -340, 3, -160, 2, -154, 4, 126, 4, 136, 6, 308, 3, 392, 16, 426, 15, 460, 10]},
    {"feature": "client", "delta": [-478, 10, -454, 15, -422, 16, -146, 6, -132, 4, 3, 6, 17, 294, 2, 300, 4, 317, 321, 376, 2, 382, 4]},
    {"feature": "terminal", "delta": [-319, -315, -312, 7, -296, 2, -11, -8, 2, -1, 58, 13, 200, 5, 212, 6, 352, 3, 360, 4]},
    {"feature": "server", "delta": [-388, 4, -378, 2, -38, 13, -9, 46, 4, 57, 94, 53, 211, 224, 11, 248, 5, 266, 13, 339]},
]

def compute_srcs(features):
    return base_compute_srcs(features, _ALL_FEATURES, _EXCLUSION_MAP)
