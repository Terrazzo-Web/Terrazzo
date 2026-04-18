"""Generated feature dependency constants."""

load("//bazel/feature-deps:defs.bzl", "base_compute_srcs")

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
    {"feature": "client-all", "delta": []},
    {"feature": "logs-panel-client", "delta": []},
    {"feature": "logs-panel-server", "delta": []},
    {"feature": "converter-client", "delta": []},
    {"feature": "max-level-info", "delta": []},
    {"feature": "rustdoc", "delta": []},
    {"feature": "converter-server", "delta": []},
    {"feature": "server-all", "delta": []},
    {"feature": "prod", "delta": []},
    {"feature": "text-editor-client", "delta": []},
    {"feature": "diagnostics", "delta": []},
    {"feature": "remotes-ui", "delta": []},
    {"feature": "debug", "delta": []},
    {"feature": "terminal-client", "delta": []},
    {"feature": "port-forward-client", "delta": []},
    {"feature": "terminal-server", "delta": []},
    {"feature": "bazel", "delta": []},
    {"feature": "no_wasm_build", "delta": []},
    {"feature": "max-level-debug", "delta": []},
    {"feature": "concise-traces", "delta": []},
    {"feature": "correlation-id", "delta": []},
    {"feature": "text-editor-server", "delta": []},
    {"feature": "port-forward-server", "delta": []},
    {"feature": "port-forward", "delta": [77, 78, 79, 80, 81, 82, 161, 162, 163, 164, 165, 166, 167]},
    {"feature": "logs-panel", "delta": [-167, -166, -165, -164, -163, -162, -161, -82, -81, -80, -79, -78, -77, 59, 60, 61, 62, 63, 64, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159]},
    {"feature": "converter", "delta": [-159, -158, -157, -156, -155, -154, -153, -152, -151, -150, -64, -63, -62, -61, -60, -59, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138]},
    {"feature": "client", "delta": [-138, -137, -136, -135, -134, -133, -132, -131, -130, -129, -128, -127, -126, -125, -124, -123, -122, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 140, 141, 142, 143, 144, 145, 146, 147, 148, 151, 152, 153, 180, 181, 182, 183, 184, 185]},
    {"feature": "terminal", "delta": [-153, -152, -151, -148, -147, -146, -145, -144, -143, -142, -141, -140, -7, -6, -5, -4, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 169, 170, 171, 172, 173, 174, 175]},
    {"feature": "server", "delta": [-185, -184, -183, -182, -181, -180, -21, -20, -19, -18, -17, -16, -15, -14, -13, -12, -11, -10, -9, -8, 23, 24, 25, 26, 27, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 162]},
    {"feature": "text-editor", "delta": [-175, -174, -173, -172, -171, -170, -169, -162, -137, -136, -135, -134, -133, -132, -131, -130, -129, -128, -127, -126, -125, -120, -119, -118, -117, -116, -115, -114, -113, -112, -111, -110, -109, -108, -107, -106, -105, -104, -103, -102, -101, -100, -99, -98, -97, -96, -95, -94, -93, -92, -91, -90, -89, -88, -87, -86, -85, -84, -83, -82, -81, -80, -79, -78, -77, -76, -65, -64, -63, -62, -61, -60, -59, -58, -57, -56, -55, -54, -53, -52, -51, -50, -49, -40, -39, -38, -37, -36, -35, -34, -33, -32, -31, -30, -29, -28, -27, -26, -25, -24, -23, 146, 147, 148, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228]},
]
_ALL_SRCS = ["terminal/src/lib.rs", "terminal/src/api/mod.rs", "terminal/src/api/client/mod.rs", "terminal/src/api/client/login.rs", "terminal/src/api/client/remotes_api.rs", "terminal/src/api/client/request.rs", "terminal/src/api/client/terminal_api/mod.rs", "terminal/src/api/client/terminal_api/list.rs", "terminal/src/api/client/terminal_api/new_id.rs", "terminal/src/api/client/terminal_api/resize.rs", "terminal/src/api/client/terminal_api/set_order.rs", "terminal/src/api/client/terminal_api/set_title.rs", "terminal/src/api/client/terminal_api/stream.rs", "terminal/src/api/client/terminal_api/stream/ack.rs", "terminal/src/api/client/terminal_api/stream/close.rs", "terminal/src/api/client/terminal_api/stream/dispatch.rs", "terminal/src/api/client/terminal_api/stream/get.rs", "terminal/src/api/client/terminal_api/stream/keepalive.rs", "terminal/src/api/client/terminal_api/stream/pipe.rs", "terminal/src/api/client/terminal_api/stream/register.rs", "terminal/src/api/client/terminal_api/write.rs", "terminal/src/api/server/mod.rs", "terminal/src/api/server/common/mod.rs", "terminal/src/api/server/common/login.rs", "terminal/src/api/server/common/remotes.rs", "terminal/src/api/server/correlation_id.rs", "terminal/src/api/server/terminal_api/mod.rs", "terminal/src/api/server/terminal_api/new_id.rs", "terminal/src/api/server/terminal_api/resize.rs", "terminal/src/api/server/terminal_api/router.rs", "terminal/src/api/server/terminal_api/set_order.rs", "terminal/src/api/server/terminal_api/set_title.rs", "terminal/src/api/server/terminal_api/stream.rs", "terminal/src/api/server/terminal_api/stream/ack.rs", "terminal/src/api/server/terminal_api/stream/close.rs", "terminal/src/api/server/terminal_api/stream/pipe.rs", "terminal/src/api/server/terminal_api/stream/register.rs", "terminal/src/api/server/terminal_api/stream/registration.rs", "terminal/src/api/server/terminal_api/terminals.rs", "terminal/src/api/server/terminal_api/write.rs", "terminal/src/api/shared/mod.rs", "terminal/src/api/shared/terminal_schema.rs", "terminal/src/api/client_address.rs", "terminal/src/api/client_name.rs", "terminal/src/assets/mod.rs", "terminal/src/assets/icons.rs", "terminal/src/assets/install.rs", "terminal/src/backend/mod.rs", "terminal/src/backend/agent.rs", "terminal/src/backend/auth/mod.rs", "terminal/src/backend/auth/jwt_timestamp.rs", "terminal/src/backend/auth/layer.rs", "terminal/src/backend/auth/tests.rs", "terminal/src/backend/cli.rs", "terminal/src/backend/client_service/mod.rs", "terminal/src/backend/client_service/convert.rs", "terminal/src/backend/client_service/grpc_error.rs", "terminal/src/backend/client_service/logs_service/mod.rs", "terminal/src/backend/client_service/logs_service/callback.rs", "terminal/src/backend/client_service/logs_service/dispatch.rs", "terminal/src/backend/client_service/logs_service/grpc.rs", "terminal/src/backend/client_service/logs_service/response.rs", "terminal/src/backend/client_service/logs_service/response/local.rs", "terminal/src/backend/client_service/logs_service/response/remote.rs", "terminal/src/backend/client_service/notify_service/mod.rs", "terminal/src/backend/client_service/notify_service/callback.rs", "terminal/src/backend/client_service/notify_service/convert.rs", "terminal/src/backend/client_service/notify_service/dispatch.rs", "terminal/src/backend/client_service/notify_service/grpc.rs", "terminal/src/backend/client_service/notify_service/request.rs", "terminal/src/backend/client_service/notify_service/request/local.rs", "terminal/src/backend/client_service/notify_service/request/remote.rs", "terminal/src/backend/client_service/notify_service/response.rs", "terminal/src/backend/client_service/notify_service/response/local.rs", "terminal/src/backend/client_service/notify_service/response/remote.rs", "terminal/src/backend/client_service/port_forward_service/mod.rs", "terminal/src/backend/client_service/port_forward_service/bind.rs", "terminal/src/backend/client_service/port_forward_service/download.rs", "terminal/src/backend/client_service/port_forward_service/grpc.rs", "terminal/src/backend/client_service/port_forward_service/listeners.rs", "terminal/src/backend/client_service/port_forward_service/stream.rs", "terminal/src/backend/client_service/port_forward_service/upload.rs", "terminal/src/backend/client_service/remote_fn_service/mod.rs", "terminal/src/backend/client_service/remote_fn_service/callback.rs", "terminal/src/backend/client_service/remote_fn_service/dispatch.rs", "terminal/src/backend/client_service/remote_fn_service/grpc.rs", "terminal/src/backend/client_service/remote_fn_service/remote_fn.rs", "terminal/src/backend/client_service/remote_fn_service/uplift.rs", "terminal/src/backend/client_service/routing.rs", "terminal/src/backend/client_service/shared_service/mod.rs", "terminal/src/backend/client_service/shared_service/grpc.rs", "terminal/src/backend/client_service/shared_service/remotes.rs", "terminal/src/backend/client_service/terminal_service/mod.rs", "terminal/src/backend/client_service/terminal_service/ack.rs", "terminal/src/backend/client_service/terminal_service/close.rs", "terminal/src/backend/client_service/terminal_service/convert.rs", "terminal/src/backend/client_service/terminal_service/grpc.rs", "terminal/src/backend/client_service/terminal_service/list.rs", "terminal/src/backend/client_service/terminal_service/new_id.rs", "terminal/src/backend/client_service/terminal_service/register.rs", "terminal/src/backend/client_service/terminal_service/resize.rs", "terminal/src/backend/client_service/terminal_service/set_order.rs", "terminal/src/backend/client_service/terminal_service/set_title.rs", "terminal/src/backend/client_service/terminal_service/write.rs", "terminal/src/backend/config/mod.rs", "terminal/src/backend/config/into_dyn.rs", "terminal/src/backend/config/io.rs", "terminal/src/backend/config/kill.rs", "terminal/src/backend/config/merge.rs", "terminal/src/backend/config/mesh.rs", "terminal/src/backend/config/password.rs", "terminal/src/backend/config/pidfile.rs", "terminal/src/backend/config/server.rs", "terminal/src/backend/config/types.rs", "terminal/src/backend/daemonize.rs", "terminal/src/backend/protos/mod.rs", "terminal/src/backend/root_ca_config.rs", "terminal/src/backend/server_config.rs", "terminal/src/backend/throttling_stream.rs", "terminal/src/backend/tls_config.rs", "terminal/src/converter/mod.rs", "terminal/src/converter/api.rs", "terminal/src/converter/conversion_tabs.rs", "terminal/src/converter/service.rs", "terminal/src/converter/service/asn1.rs", "terminal/src/converter/service/base64.rs", "terminal/src/converter/service/dns.rs", "terminal/src/converter/service/json.rs", "terminal/src/converter/service/jwt.rs", "terminal/src/converter/service/pkcs7.rs", "terminal/src/converter/service/timestamps.rs", "terminal/src/converter/service/tls_info.rs", "terminal/src/converter/service/tls_info/buffered_stream.rs", "terminal/src/converter/service/tls_info/indented_writer.rs", "terminal/src/converter/service/tls_info/tls_handshake.rs", "terminal/src/converter/service/unescaped.rs", "terminal/src/converter/service/x509.rs", "terminal/src/converter/ui.rs", "terminal/src/frontend/mod.rs", "terminal/src/frontend/login.rs", "terminal/src/frontend/menu.rs", "terminal/src/frontend/mousemove.rs", "terminal/src/frontend/remotes.rs", "terminal/src/frontend/remotes_ui.rs", "terminal/src/frontend/timestamp.rs", "terminal/src/frontend/timestamp/datetime.rs", "terminal/src/frontend/timestamp/tick.rs", "terminal/src/frontend/timestamp/timer.rs", "terminal/src/logs/mod.rs", "terminal/src/logs/client/mod.rs", "terminal/src/logs/client/engine.rs", "terminal/src/logs/client/ndjson.rs", "terminal/src/logs/client/panel.rs", "terminal/src/logs/event.rs", "terminal/src/logs/state.rs", "terminal/src/logs/stream.rs", "terminal/src/logs/subscription.rs", "terminal/src/logs/tests.rs", "terminal/src/logs/tracing.rs", "terminal/src/portforward/mod.rs", "terminal/src/portforward/engine.rs", "terminal/src/portforward/engine/retry.rs", "terminal/src/portforward/manager.rs", "terminal/src/portforward/schema.rs", "terminal/src/portforward/state.rs", "terminal/src/portforward/sync_state.rs", "terminal/src/portforward/ui.rs", "terminal/src/processes/mod.rs", "terminal/src/processes/close.rs", "terminal/src/processes/io.rs", "terminal/src/processes/list.rs", "terminal/src/processes/resize.rs", "terminal/src/processes/set_title.rs", "terminal/src/processes/stream.rs", "terminal/src/processes/write.rs", "terminal/src/state/mod.rs", "terminal/src/state/app.rs", "terminal/src/state/make_state.rs", "terminal/src/terminal/mod.rs", "terminal/src/terminal/attach.rs", "terminal/src/terminal/javascript.rs", "terminal/src/terminal/terminal_tab.rs", "terminal/src/terminal/terminal_tabs.rs", "terminal/src/terminal/terminal_tabs/add_tab.rs", "terminal/src/terminal/terminal_tabs/move_tab.rs", "terminal/src/terminal_id.rs", "terminal/src/text_editor/mod.rs", "terminal/src/text_editor/autocomplete/mod.rs", "terminal/src/text_editor/autocomplete/remote.rs", "terminal/src/text_editor/autocomplete/server_fn.rs", "terminal/src/text_editor/autocomplete/service.rs", "terminal/src/text_editor/autocomplete/ui.rs", "terminal/src/text_editor/code_mirror.rs", "terminal/src/text_editor/editor.rs", "terminal/src/text_editor/file_path.rs", "terminal/src/text_editor/folder.rs", "terminal/src/text_editor/fsio.rs", "terminal/src/text_editor/fsio/canonical.rs", "terminal/src/text_editor/fsio/fsmetadata.rs", "terminal/src/text_editor/fsio/remote.rs", "terminal/src/text_editor/fsio/service.rs", "terminal/src/text_editor/fsio/ui.rs", "terminal/src/text_editor/manager.rs", "terminal/src/text_editor/notify/mod.rs", "terminal/src/text_editor/notify/event_handler.rs", "terminal/src/text_editor/notify/server_fn.rs", "terminal/src/text_editor/notify/service.rs", "terminal/src/text_editor/notify/ui.rs", "terminal/src/text_editor/notify/watcher.rs", "terminal/src/text_editor/path_selector/mod.rs", "terminal/src/text_editor/path_selector/schema.rs", "terminal/src/text_editor/path_selector/service.rs", "terminal/src/text_editor/path_selector/ui.rs", "terminal/src/text_editor/rust_lang.rs", "terminal/src/text_editor/rust_lang/messages.rs", "terminal/src/text_editor/rust_lang/remote.rs", "terminal/src/text_editor/rust_lang/service.rs", "terminal/src/text_editor/rust_lang/synthetic.rs", "terminal/src/text_editor/search/mod.rs", "terminal/src/text_editor/search/server_fn.rs", "terminal/src/text_editor/search/state.rs", "terminal/src/text_editor/search/ui.rs", "terminal/src/text_editor/side/mod.rs", "terminal/src/text_editor/side/mutation.rs", "terminal/src/text_editor/side/ui.rs", "terminal/src/text_editor/state.rs", "terminal/src/text_editor/synchronized_state.rs", "terminal/src/text_editor/ui.rs", "terminal/src/utils/mod.rs", "terminal/src/utils/async_throttle.rs", "terminal/src/utils/more_path.rs"]

def compute_srcs(features):
    return base_compute_srcs(features, _ALL_SRCS, _ALL_FEATURES, _EXCLUSION_MAP)
