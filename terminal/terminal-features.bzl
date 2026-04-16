"""Generated feature dependency constants."""

DEFAULT_SRCS = [
    "api/client_address.rs",
    "api/client_name.rs",
    "api/mod.rs",
    "api/shared/mod.rs",
    "assets/icons.rs",
    "assets/mod.rs",
    "lib.rs",
    "state/app.rs",
    "state/make_state.rs",
    "state/mod.rs",
    "terminal_id.rs",
    "utils/mod.rs",
    "utils/more_path.rs",
]
BAZEL_DEPS = []
BAZEL_FEATURES = ["bazel"]
BAZEL_SRCS = DEFAULT_SRCS + []
CLIENT_DEPS = [
    "@crates//:stylance",
    "@crates//:wasm-bindgen",
    "@crates//:wasm-bindgen-futures",
]
CLIENT_FEATURES = ["client"]
CLIENT_SRCS = DEFAULT_SRCS + [
    "api/client/login.rs",
    "api/client/mod.rs",
    "api/client/remotes_api.rs",
    "api/client/request.rs",
    "converter/conversion_tabs.rs",
    "converter/ui.rs",
    "frontend/login.rs",
    "frontend/menu.rs",
    "frontend/mod.rs",
    "frontend/remotes.rs",
    "logs/client/engine.rs",
    "logs/client/mod.rs",
    "logs/client/ndjson.rs",
    "logs/client/panel.rs",
    "portforward/manager.rs",
    "portforward/sync_state.rs",
    "portforward/ui.rs",
    "text_editor/autocomplete/ui.rs",
    "text_editor/code_mirror.rs",
    "text_editor/editor.rs",
    "text_editor/folder.rs",
    "text_editor/fsio/ui.rs",
    "text_editor/manager.rs",
    "text_editor/notify/ui.rs",
    "text_editor/path_selector/ui.rs",
    "text_editor/search/state.rs",
    "text_editor/search/ui.rs",
    "text_editor/side/mutation.rs",
    "text_editor/side/ui.rs",
    "text_editor/synchronized_state.rs",
    "text_editor/ui.rs",
]
CONVERTER_DEPS = []
CONVERTER_FEATURES = ["converter"]
CONVERTER_SRCS = DEFAULT_SRCS + [
    "converter/api.rs",
    "converter/mod.rs",
    "frontend/mousemove.rs",
]
REMOTES_UI_DEPS = []
REMOTES_UI_FEATURES = ["remotes-ui"]
REMOTES_UI_SRCS = DEFAULT_SRCS + ["frontend/remotes_ui.rs"]
CONVERTER_CLIENT_DEPS = CLIENT_DEPS + CONVERTER_DEPS + REMOTES_UI_DEPS + ["@crates//:web-sys"]
CONVERTER_CLIENT_FEATURES = ["converter-client"] + CLIENT_FEATURES + CONVERTER_FEATURES + REMOTES_UI_FEATURES
CONVERTER_CLIENT_SRCS = CLIENT_SRCS + CONVERTER_SRCS + REMOTES_UI_SRCS + []
LOGS_PANEL_DEPS = []
LOGS_PANEL_FEATURES = ["logs-panel"]
LOGS_PANEL_SRCS = DEFAULT_SRCS + [
    "backend/client_service/logs_service/callback.rs",
    "backend/client_service/logs_service/dispatch.rs",
    "backend/client_service/logs_service/grpc.rs",
    "backend/client_service/logs_service/mod.rs",
    "backend/client_service/logs_service/response.rs",
    "backend/client_service/logs_service/response/local.rs",
    "backend/client_service/logs_service/response/remote.rs",
    "frontend/mousemove.rs",
    "logs/event.rs",
    "logs/mod.rs",
    "logs/stream.rs",
]
LOGS_PANEL_CLIENT_DEPS = CLIENT_DEPS + LOGS_PANEL_DEPS + ["@crates//:futures"]
LOGS_PANEL_CLIENT_FEATURES = ["logs-panel-client"] + CLIENT_FEATURES + LOGS_PANEL_FEATURES
LOGS_PANEL_CLIENT_SRCS = CLIENT_SRCS + LOGS_PANEL_SRCS + []
PORT_FORWARD_DEPS = []
PORT_FORWARD_FEATURES = ["port-forward"]
PORT_FORWARD_SRCS = DEFAULT_SRCS + [
    "backend/client_service/port_forward_service/bind.rs",
    "backend/client_service/port_forward_service/download.rs",
    "backend/client_service/port_forward_service/grpc.rs",
    "backend/client_service/port_forward_service/mod.rs",
    "backend/client_service/port_forward_service/stream.rs",
    "backend/client_service/port_forward_service/upload.rs",
    "portforward/mod.rs",
    "portforward/schema.rs",
    "portforward/state.rs",
]
PORT_FORWARD_CLIENT_DEPS = CLIENT_DEPS + PORT_FORWARD_DEPS + REMOTES_UI_DEPS + [
    "@crates//:bitflags",
    "@crates//:scopeguard",
    "@crates//:web-sys",
]
PORT_FORWARD_CLIENT_FEATURES = ["port-forward-client"] + CLIENT_FEATURES + PORT_FORWARD_FEATURES + REMOTES_UI_FEATURES
PORT_FORWARD_CLIENT_SRCS = CLIENT_SRCS + PORT_FORWARD_SRCS + REMOTES_UI_SRCS + []
CORRELATION_ID_DEPS = []
CORRELATION_ID_FEATURES = ["correlation-id"]
CORRELATION_ID_SRCS = DEFAULT_SRCS + ["api/server/correlation_id.rs"]
TERMINAL_DEPS = CORRELATION_ID_DEPS
TERMINAL_FEATURES = ["terminal"] + CORRELATION_ID_FEATURES
TERMINAL_SRCS = CORRELATION_ID_SRCS + [
    "api/client/terminal_api/list.rs",
    "api/client/terminal_api/mod.rs",
    "api/client/terminal_api/new_id.rs",
    "api/client/terminal_api/resize.rs",
    "api/client/terminal_api/set_order.rs",
    "api/client/terminal_api/set_title.rs",
    "api/client/terminal_api/stream.rs",
    "api/client/terminal_api/stream/ack.rs",
    "api/client/terminal_api/stream/close.rs",
    "api/client/terminal_api/stream/dispatch.rs",
    "api/client/terminal_api/stream/get.rs",
    "api/client/terminal_api/stream/keepalive.rs",
    "api/client/terminal_api/stream/pipe.rs",
    "api/client/terminal_api/stream/register.rs",
    "api/client/terminal_api/write.rs",
    "api/server/terminal_api/mod.rs",
    "api/server/terminal_api/new_id.rs",
    "api/server/terminal_api/resize.rs",
    "api/server/terminal_api/router.rs",
    "api/server/terminal_api/set_order.rs",
    "api/server/terminal_api/set_title.rs",
    "api/server/terminal_api/stream.rs",
    "api/server/terminal_api/stream/ack.rs",
    "api/server/terminal_api/stream/close.rs",
    "api/server/terminal_api/stream/pipe.rs",
    "api/server/terminal_api/stream/register.rs",
    "api/server/terminal_api/stream/registration.rs",
    "api/server/terminal_api/terminals.rs",
    "api/server/terminal_api/write.rs",
    "api/shared/terminal_schema.rs",
    "backend/client_service/terminal_service/ack.rs",
    "backend/client_service/terminal_service/close.rs",
    "backend/client_service/terminal_service/convert.rs",
    "backend/client_service/terminal_service/grpc.rs",
    "backend/client_service/terminal_service/list.rs",
    "backend/client_service/terminal_service/mod.rs",
    "backend/client_service/terminal_service/new_id.rs",
    "backend/client_service/terminal_service/register.rs",
    "backend/client_service/terminal_service/resize.rs",
    "backend/client_service/terminal_service/set_order.rs",
    "backend/client_service/terminal_service/set_title.rs",
    "backend/client_service/terminal_service/write.rs",
    "backend/throttling_stream.rs",
]
TERMINAL_CLIENT_DEPS = CLIENT_DEPS + TERMINAL_DEPS + [
    "@crates//:futures",
    "@crates//:pin-project",
    "@crates//:scopeguard",
    "@crates//:wasm-streams",
    "@crates//:web-sys",
]
TERMINAL_CLIENT_FEATURES = ["terminal-client"] + CLIENT_FEATURES + TERMINAL_FEATURES
TERMINAL_CLIENT_SRCS = CLIENT_SRCS + TERMINAL_SRCS + []
TEXT_EDITOR_DEPS = []
TEXT_EDITOR_FEATURES = ["text-editor"]
TEXT_EDITOR_SRCS = DEFAULT_SRCS + [
    "backend/client_service/notify_service/callback.rs",
    "backend/client_service/notify_service/convert.rs",
    "backend/client_service/notify_service/dispatch.rs",
    "backend/client_service/notify_service/grpc.rs",
    "backend/client_service/notify_service/mod.rs",
    "backend/client_service/notify_service/request.rs",
    "backend/client_service/notify_service/request/local.rs",
    "backend/client_service/notify_service/request/remote.rs",
    "backend/client_service/notify_service/response.rs",
    "backend/client_service/notify_service/response/local.rs",
    "backend/client_service/notify_service/response/remote.rs",
    "frontend/timestamp.rs",
    "frontend/timestamp/datetime.rs",
    "frontend/timestamp/tick.rs",
    "frontend/timestamp/timer.rs",
    "text_editor/autocomplete/mod.rs",
    "text_editor/autocomplete/server_fn.rs",
    "text_editor/file_path.rs",
    "text_editor/fsio.rs",
    "text_editor/mod.rs",
    "text_editor/notify/mod.rs",
    "text_editor/notify/server_fn.rs",
    "text_editor/path_selector/mod.rs",
    "text_editor/path_selector/schema.rs",
    "text_editor/rust_lang.rs",
    "text_editor/rust_lang/synthetic.rs",
    "text_editor/search/mod.rs",
    "text_editor/search/server_fn.rs",
    "text_editor/side/mod.rs",
    "text_editor/state.rs",
]
TEXT_EDITOR_CLIENT_DEPS = CLIENT_DEPS + REMOTES_UI_DEPS + TEXT_EDITOR_DEPS + [
    "@crates//:futures",
    "@crates//:scopeguard",
    "@crates//:serde-wasm-bindgen",
]
TEXT_EDITOR_CLIENT_FEATURES = ["text-editor-client"] + CLIENT_FEATURES + REMOTES_UI_FEATURES + TEXT_EDITOR_FEATURES
TEXT_EDITOR_CLIENT_SRCS = CLIENT_SRCS + REMOTES_UI_SRCS + TEXT_EDITOR_SRCS + []
CLIENT_ALL_DEPS = CONVERTER_CLIENT_DEPS + LOGS_PANEL_CLIENT_DEPS + PORT_FORWARD_CLIENT_DEPS + TERMINAL_CLIENT_DEPS + TEXT_EDITOR_CLIENT_DEPS
CLIENT_ALL_FEATURES = ["client-all"] + CONVERTER_CLIENT_FEATURES + LOGS_PANEL_CLIENT_FEATURES + PORT_FORWARD_CLIENT_FEATURES + TERMINAL_CLIENT_FEATURES + TEXT_EDITOR_CLIENT_FEATURES
CLIENT_ALL_SRCS = CONVERTER_CLIENT_SRCS + LOGS_PANEL_CLIENT_SRCS + PORT_FORWARD_CLIENT_SRCS + TERMINAL_CLIENT_SRCS + TEXT_EDITOR_CLIENT_SRCS + []
CONCISE_TRACES_DEPS = []
CONCISE_TRACES_FEATURES = ["concise-traces"]
CONCISE_TRACES_SRCS = DEFAULT_SRCS + []
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
SERVER_SRCS = DEFAULT_SRCS + [
    "api/server/common/login.rs",
    "api/server/common/mod.rs",
    "api/server/common/remotes.rs",
    "api/server/mod.rs",
    "assets/install.rs",
    "backend/agent.rs",
    "backend/auth/jwt_timestamp.rs",
    "backend/auth/layer.rs",
    "backend/auth/mod.rs",
    "backend/cli.rs",
    "backend/client_service/convert.rs",
    "backend/client_service/grpc_error.rs",
    "backend/client_service/mod.rs",
    "backend/client_service/port_forward_service/listeners.rs",
    "backend/client_service/remote_fn_service/callback.rs",
    "backend/client_service/remote_fn_service/dispatch.rs",
    "backend/client_service/remote_fn_service/grpc.rs",
    "backend/client_service/remote_fn_service/mod.rs",
    "backend/client_service/remote_fn_service/remote_fn.rs",
    "backend/client_service/remote_fn_service/uplift.rs",
    "backend/client_service/routing.rs",
    "backend/client_service/shared_service/grpc.rs",
    "backend/client_service/shared_service/mod.rs",
    "backend/client_service/shared_service/remotes.rs",
    "backend/config/into_dyn.rs",
    "backend/config/io.rs",
    "backend/config/kill.rs",
    "backend/config/merge.rs",
    "backend/config/mesh.rs",
    "backend/config/mod.rs",
    "backend/config/password.rs",
    "backend/config/pidfile.rs",
    "backend/config/server.rs",
    "backend/config/types.rs",
    "backend/daemonize.rs",
    "backend/mod.rs",
    "backend/protos/mod.rs",
    "backend/root_ca_config.rs",
    "backend/server_config.rs",
    "backend/tls_config.rs",
    "converter/service.rs",
    "converter/service/asn1.rs",
    "converter/service/base64.rs",
    "converter/service/dns.rs",
    "converter/service/json.rs",
    "converter/service/jwt.rs",
    "converter/service/pkcs7.rs",
    "converter/service/timestamps.rs",
    "converter/service/tls_info.rs",
    "converter/service/tls_info/buffered_stream.rs",
    "converter/service/tls_info/indented_writer.rs",
    "converter/service/tls_info/tls_handshake.rs",
    "converter/service/unescaped.rs",
    "converter/service/x509.rs",
    "logs/state.rs",
    "logs/subscription.rs",
    "logs/tracing.rs",
    "portforward/engine.rs",
    "portforward/engine/retry.rs",
    "text_editor/autocomplete/remote.rs",
    "text_editor/autocomplete/service.rs",
    "text_editor/fsio/canonical.rs",
    "text_editor/fsio/fsmetadata.rs",
    "text_editor/fsio/remote.rs",
    "text_editor/fsio/service.rs",
    "text_editor/notify/event_handler.rs",
    "text_editor/notify/service.rs",
    "text_editor/notify/watcher.rs",
    "text_editor/path_selector/service.rs",
    "text_editor/rust_lang/messages.rs",
    "text_editor/rust_lang/remote.rs",
    "text_editor/rust_lang/service.rs",
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
CONVERTER_SERVER_FEATURES = ["converter-server"] + CONVERTER_FEATURES + SERVER_FEATURES
CONVERTER_SERVER_SRCS = CONVERTER_SRCS + SERVER_SRCS + []
DEBUG_DEPS = []
DEBUG_FEATURES = ["debug"]
DEBUG_SRCS = DEFAULT_SRCS + []
DIAGNOSTICS_DEPS = []
DIAGNOSTICS_FEATURES = ["diagnostics"]
DIAGNOSTICS_SRCS = DEFAULT_SRCS + []
LOGS_PANEL_SERVER_DEPS = LOGS_PANEL_DEPS + SERVER_DEPS + ["@crates//:tracing-subscriber"]
LOGS_PANEL_SERVER_FEATURES = ["logs-panel-server"] + LOGS_PANEL_FEATURES + SERVER_FEATURES
LOGS_PANEL_SERVER_SRCS = LOGS_PANEL_SRCS + SERVER_SRCS + []
MAX_LEVEL_DEBUG_DEPS = []
MAX_LEVEL_DEBUG_FEATURES = ["max-level-debug"]
MAX_LEVEL_DEBUG_SRCS = DEFAULT_SRCS + []
MAX_LEVEL_INFO_DEPS = CONCISE_TRACES_DEPS
MAX_LEVEL_INFO_FEATURES = ["max-level-info"] + CONCISE_TRACES_FEATURES
MAX_LEVEL_INFO_SRCS = CONCISE_TRACES_SRCS + []
NO_WASM_BUILD_DEPS = []
NO_WASM_BUILD_FEATURES = ["no_wasm_build"]
NO_WASM_BUILD_SRCS = DEFAULT_SRCS + []
PORT_FORWARD_SERVER_DEPS = PORT_FORWARD_DEPS + SERVER_DEPS
PORT_FORWARD_SERVER_FEATURES = ["port-forward-server"] + PORT_FORWARD_FEATURES + SERVER_FEATURES
PORT_FORWARD_SERVER_SRCS = PORT_FORWARD_SRCS + SERVER_SRCS + []
TERMINAL_SERVER_DEPS = SERVER_DEPS + TERMINAL_DEPS + [
    "//pty",
    "@crates//:dashmap",
    "@crates//:pin-project",
    "@crates//:static_assertions",
    "@crates//:tracing-futures",
]
TERMINAL_SERVER_FEATURES = ["terminal-server"] + SERVER_FEATURES + TERMINAL_FEATURES
TERMINAL_SERVER_SRCS = SERVER_SRCS + TERMINAL_SRCS + []
TEXT_EDITOR_SERVER_DEPS = SERVER_DEPS + TEXT_EDITOR_DEPS + [
    "@crates//:libc",
    "@crates//:notify",
    "@crates//:tokio-stream",
]
TEXT_EDITOR_SERVER_FEATURES = ["text-editor-server"] + SERVER_FEATURES + TEXT_EDITOR_FEATURES
TEXT_EDITOR_SERVER_SRCS = SERVER_SRCS + TEXT_EDITOR_SRCS + []
SERVER_ALL_DEPS = CONVERTER_SERVER_DEPS + LOGS_PANEL_SERVER_DEPS + PORT_FORWARD_SERVER_DEPS + TERMINAL_SERVER_DEPS + TEXT_EDITOR_SERVER_DEPS
SERVER_ALL_FEATURES = ["server-all"] + CONVERTER_SERVER_FEATURES + LOGS_PANEL_SERVER_FEATURES + PORT_FORWARD_SERVER_FEATURES + TERMINAL_SERVER_FEATURES + TEXT_EDITOR_SERVER_FEATURES
SERVER_ALL_SRCS = CONVERTER_SERVER_SRCS + LOGS_PANEL_SERVER_SRCS + PORT_FORWARD_SERVER_SRCS + TERMINAL_SERVER_SRCS + TEXT_EDITOR_SERVER_SRCS + []
PROD_DEPS = MAX_LEVEL_INFO_DEPS + SERVER_ALL_DEPS
PROD_FEATURES = ["prod"] + MAX_LEVEL_INFO_FEATURES + SERVER_ALL_FEATURES
PROD_SRCS = MAX_LEVEL_INFO_SRCS + SERVER_ALL_SRCS + []
RUSTDOC_DEPS = []
RUSTDOC_FEATURES = ["rustdoc"]
RUSTDOC_SRCS = DEFAULT_SRCS + []
