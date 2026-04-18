"""Generated feature dependency constants."""

load("//bazel/feature_deps:feature_deps_rules.bzl", "base_compute_srcs")

_ALL_FEATURES = ["bazel", "client", "client-all", "concise-traces", "converter", "converter-client", "converter-server", "correlation-id", "debug", "diagnostics", "logs-panel", "logs-panel-client", "logs-panel-server", "max-level-debug", "max-level-info", "no_wasm_build", "port-forward", "port-forward-client", "port-forward-server", "prod", "remotes-ui", "rustdoc", "server", "server-all", "terminal", "terminal-client", "terminal-server", "text-editor", "text-editor-client", "text-editor-server"]
BAZEL_DEPS = []
BAZEL_FEATURES = ["bazel"]
CLIENT_DEPS = [
    "@crates//:stylance",
    "@crates//:wasm-bindgen",
    "@crates//:wasm-bindgen-futures",
]
CLIENT_FEATURES = ["client"]
CONVERTER_DEPS = []
CONVERTER_FEATURES = ["converter"]
REMOTES_UI_DEPS = []
REMOTES_UI_FEATURES = ["remotes-ui"]
CONVERTER_CLIENT_DEPS = CLIENT_DEPS + CONVERTER_DEPS + REMOTES_UI_DEPS + ["@crates//:web-sys"]
CONVERTER_CLIENT_FEATURES = CLIENT_FEATURES + CONVERTER_FEATURES + REMOTES_UI_FEATURES + ["converter-client"]
LOGS_PANEL_DEPS = []
LOGS_PANEL_FEATURES = ["logs-panel"]
LOGS_PANEL_CLIENT_DEPS = CLIENT_DEPS + LOGS_PANEL_DEPS + ["@crates//:futures"]
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
LOGS_PANEL_SERVER_DEPS = LOGS_PANEL_DEPS + SERVER_DEPS + ["@crates//:tracing-subscriber"]
LOGS_PANEL_SERVER_FEATURES = LOGS_PANEL_FEATURES + SERVER_FEATURES + ["logs-panel-server"]
MAX_LEVEL_DEBUG_DEPS = []
MAX_LEVEL_DEBUG_FEATURES = ["max-level-debug"]
MAX_LEVEL_INFO_DEPS = CONCISE_TRACES_DEPS
MAX_LEVEL_INFO_FEATURES = CONCISE_TRACES_FEATURES + ["max-level-info"]
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
    {"feature": "converter", "delta": [122, 17]},
    {"feature": "logs-panel", "delta": [-138, 17, 59, 6, 150, 10]},
    {"feature": "port-forward", "delta": [-159, 10, -64, 6, 77, 6, 161, 7]},
    {"feature": "text-editor", "delta": [-167, 7, -82, 6, 66, 10, 146, 3, 188, 41]},
    {"feature": "client", "delta": [-228, 41, -75, 10, 4, 18, 140, 6, 151, 3, 180, 6]},
    {"feature": "terminal", "delta": [-153, 3, -148, 9, -7, 4, 28, 13, 94, 11, 169, 7]},
    {"feature": "server", "delta": [-185, 6, -21, 14, 23, 5, 49, 45, 105, 16, 125, 13, 162, 1]},
]
_ALL_SRCS = ["src/lib.rs", "src/api/mod.rs", "src/api/client/mod.rs", "src/api/client/login.rs", "src/api/client/remotes_api.rs", "src/api/client/request.rs", "src/api/client/terminal_api/mod.rs", "src/api/client/terminal_api/list.rs", "src/api/client/terminal_api/new_id.rs", "src/api/client/terminal_api/resize.rs", "src/api/client/terminal_api/set_order.rs", "src/api/client/terminal_api/set_title.rs", "src/api/client/terminal_api/stream.rs", "src/api/client/terminal_api/stream/ack.rs", "src/api/client/terminal_api/stream/close.rs", "src/api/client/terminal_api/stream/dispatch.rs", "src/api/client/terminal_api/stream/get.rs", "src/api/client/terminal_api/stream/keepalive.rs", "src/api/client/terminal_api/stream/pipe.rs", "src/api/client/terminal_api/stream/register.rs", "src/api/client/terminal_api/write.rs", "src/api/server/mod.rs", "src/api/server/common/mod.rs", "src/api/server/common/login.rs", "src/api/server/common/remotes.rs", "src/api/server/correlation_id.rs", "src/api/server/terminal_api/mod.rs", "src/api/server/terminal_api/new_id.rs", "src/api/server/terminal_api/resize.rs", "src/api/server/terminal_api/router.rs", "src/api/server/terminal_api/set_order.rs", "src/api/server/terminal_api/set_title.rs", "src/api/server/terminal_api/stream.rs", "src/api/server/terminal_api/stream/ack.rs", "src/api/server/terminal_api/stream/close.rs", "src/api/server/terminal_api/stream/pipe.rs", "src/api/server/terminal_api/stream/register.rs", "src/api/server/terminal_api/stream/registration.rs", "src/api/server/terminal_api/terminals.rs", "src/api/server/terminal_api/write.rs", "src/api/shared/mod.rs", "src/api/shared/terminal_schema.rs", "src/api/client_address.rs", "src/api/client_name.rs", "src/assets/mod.rs", "src/assets/icons.rs", "src/assets/install.rs", "src/backend/mod.rs", "src/backend/agent.rs", "src/backend/auth/mod.rs", "src/backend/auth/jwt_timestamp.rs", "src/backend/auth/layer.rs", "src/backend/auth/tests.rs", "src/backend/cli.rs", "src/backend/client_service/mod.rs", "src/backend/client_service/convert.rs", "src/backend/client_service/grpc_error.rs", "src/backend/client_service/logs_service/mod.rs", "src/backend/client_service/logs_service/callback.rs", "src/backend/client_service/logs_service/dispatch.rs", "src/backend/client_service/logs_service/grpc.rs", "src/backend/client_service/logs_service/response.rs", "src/backend/client_service/logs_service/response/local.rs", "src/backend/client_service/logs_service/response/remote.rs", "src/backend/client_service/notify_service/mod.rs", "src/backend/client_service/notify_service/callback.rs", "src/backend/client_service/notify_service/convert.rs", "src/backend/client_service/notify_service/dispatch.rs", "src/backend/client_service/notify_service/grpc.rs", "src/backend/client_service/notify_service/request.rs", "src/backend/client_service/notify_service/request/local.rs", "src/backend/client_service/notify_service/request/remote.rs", "src/backend/client_service/notify_service/response.rs", "src/backend/client_service/notify_service/response/local.rs", "src/backend/client_service/notify_service/response/remote.rs", "src/backend/client_service/port_forward_service/mod.rs", "src/backend/client_service/port_forward_service/bind.rs", "src/backend/client_service/port_forward_service/download.rs", "src/backend/client_service/port_forward_service/grpc.rs", "src/backend/client_service/port_forward_service/listeners.rs", "src/backend/client_service/port_forward_service/stream.rs", "src/backend/client_service/port_forward_service/upload.rs", "src/backend/client_service/remote_fn_service/mod.rs", "src/backend/client_service/remote_fn_service/callback.rs", "src/backend/client_service/remote_fn_service/dispatch.rs", "src/backend/client_service/remote_fn_service/grpc.rs", "src/backend/client_service/remote_fn_service/remote_fn.rs", "src/backend/client_service/remote_fn_service/uplift.rs", "src/backend/client_service/routing.rs", "src/backend/client_service/shared_service/mod.rs", "src/backend/client_service/shared_service/grpc.rs", "src/backend/client_service/shared_service/remotes.rs", "src/backend/client_service/terminal_service/mod.rs", "src/backend/client_service/terminal_service/ack.rs", "src/backend/client_service/terminal_service/close.rs", "src/backend/client_service/terminal_service/convert.rs", "src/backend/client_service/terminal_service/grpc.rs", "src/backend/client_service/terminal_service/list.rs", "src/backend/client_service/terminal_service/new_id.rs", "src/backend/client_service/terminal_service/register.rs", "src/backend/client_service/terminal_service/resize.rs", "src/backend/client_service/terminal_service/set_order.rs", "src/backend/client_service/terminal_service/set_title.rs", "src/backend/client_service/terminal_service/write.rs", "src/backend/config/mod.rs", "src/backend/config/into_dyn.rs", "src/backend/config/io.rs", "src/backend/config/kill.rs", "src/backend/config/merge.rs", "src/backend/config/mesh.rs", "src/backend/config/password.rs", "src/backend/config/pidfile.rs", "src/backend/config/server.rs", "src/backend/config/types.rs", "src/backend/daemonize.rs", "src/backend/protos/mod.rs", "src/backend/root_ca_config.rs", "src/backend/server_config.rs", "src/backend/throttling_stream.rs", "src/backend/tls_config.rs", "src/converter/mod.rs", "src/converter/api.rs", "src/converter/conversion_tabs.rs", "src/converter/service.rs", "src/converter/service/asn1.rs", "src/converter/service/base64.rs", "src/converter/service/dns.rs", "src/converter/service/json.rs", "src/converter/service/jwt.rs", "src/converter/service/pkcs7.rs", "src/converter/service/timestamps.rs", "src/converter/service/tls_info.rs", "src/converter/service/tls_info/buffered_stream.rs", "src/converter/service/tls_info/indented_writer.rs", "src/converter/service/tls_info/tls_handshake.rs", "src/converter/service/unescaped.rs", "src/converter/service/x509.rs", "src/converter/ui.rs", "src/frontend/mod.rs", "src/frontend/login.rs", "src/frontend/menu.rs", "src/frontend/mousemove.rs", "src/frontend/remotes.rs", "src/frontend/remotes_ui.rs", "src/frontend/timestamp.rs", "src/frontend/timestamp/datetime.rs", "src/frontend/timestamp/tick.rs", "src/frontend/timestamp/timer.rs", "src/logs/mod.rs", "src/logs/client/mod.rs", "src/logs/client/engine.rs", "src/logs/client/ndjson.rs", "src/logs/client/panel.rs", "src/logs/event.rs", "src/logs/state.rs", "src/logs/stream.rs", "src/logs/subscription.rs", "src/logs/tests.rs", "src/logs/tracing.rs", "src/portforward/mod.rs", "src/portforward/engine.rs", "src/portforward/engine/retry.rs", "src/portforward/manager.rs", "src/portforward/schema.rs", "src/portforward/state.rs", "src/portforward/sync_state.rs", "src/portforward/ui.rs", "src/processes/mod.rs", "src/processes/close.rs", "src/processes/io.rs", "src/processes/list.rs", "src/processes/resize.rs", "src/processes/set_title.rs", "src/processes/stream.rs", "src/processes/write.rs", "src/state/mod.rs", "src/state/app.rs", "src/state/make_state.rs", "src/terminal/mod.rs", "src/terminal/attach.rs", "src/terminal/javascript.rs", "src/terminal/terminal_tab.rs", "src/terminal/terminal_tabs.rs", "src/terminal/terminal_tabs/add_tab.rs", "src/terminal/terminal_tabs/move_tab.rs", "src/terminal_id.rs", "src/text_editor/mod.rs", "src/text_editor/autocomplete/mod.rs", "src/text_editor/autocomplete/remote.rs", "src/text_editor/autocomplete/server_fn.rs", "src/text_editor/autocomplete/service.rs", "src/text_editor/autocomplete/ui.rs", "src/text_editor/code_mirror.rs", "src/text_editor/editor.rs", "src/text_editor/file_path.rs", "src/text_editor/folder.rs", "src/text_editor/fsio.rs", "src/text_editor/fsio/canonical.rs", "src/text_editor/fsio/fsmetadata.rs", "src/text_editor/fsio/remote.rs", "src/text_editor/fsio/service.rs", "src/text_editor/fsio/ui.rs", "src/text_editor/manager.rs", "src/text_editor/notify/mod.rs", "src/text_editor/notify/event_handler.rs", "src/text_editor/notify/server_fn.rs", "src/text_editor/notify/service.rs", "src/text_editor/notify/ui.rs", "src/text_editor/notify/watcher.rs", "src/text_editor/path_selector/mod.rs", "src/text_editor/path_selector/schema.rs", "src/text_editor/path_selector/service.rs", "src/text_editor/path_selector/ui.rs", "src/text_editor/rust_lang.rs", "src/text_editor/rust_lang/messages.rs", "src/text_editor/rust_lang/remote.rs", "src/text_editor/rust_lang/service.rs", "src/text_editor/rust_lang/synthetic.rs", "src/text_editor/search/mod.rs", "src/text_editor/search/server_fn.rs", "src/text_editor/search/state.rs", "src/text_editor/search/ui.rs", "src/text_editor/side/mod.rs", "src/text_editor/side/mutation.rs", "src/text_editor/side/ui.rs", "src/text_editor/state.rs", "src/text_editor/synchronized_state.rs", "src/text_editor/ui.rs", "src/utils/mod.rs", "src/utils/async_throttle.rs", "src/utils/more_path.rs"]

def compute_srcs(features):
    return base_compute_srcs(features, _ALL_SRCS, _ALL_FEATURES, _EXCLUSION_MAP)
