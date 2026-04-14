load("@crates//:defs.bzl", "all_crate_deps")
load("@rules_rust//cargo:defs.bzl", "cargo_build_script")
load("@rules_rust_wasm_bindgen//:defs.bzl", "rust_wasm_bindgen")
load("//bazel:playwright_rules.bzl", "playwright_matrix_test")
load("//bazel:rust_rules.bzl", "rust_rules_matrix")
load("//bazel:stylance_rules.bzl", "stylance_rule")

package(default_visibility = ["//visibility:public"])

def terminal_rules(
        prefix = "",
        client_features = [],
        client_deps = [],
        server_features = [],
        server_deps = []):
    rust_rules_matrix(
        package_name = "terminal",
        assets = [
            {
                "targets": glob(["src/**/*.scss"]),
                "copy": True,
            },
        ],
        crate_features = client_features,
        overrides = {
            prefix + "client-lib": {
                "deps": ["//framework/terrazzo:client"],
            },
            prefix + "client-lib-debug": {
                "deps": ["//framework/terrazzo:client-debug"],
            },
            prefix + "client-shared-lib": {
                "deps": ["//framework/terrazzo:client"],
                "generate_tests": False,
                "rule": "shared_library",
                "target_compatible_with": ["@platforms//cpu:wasm32"],
                "crate_name": "terrazzo_terminal",
            },
            prefix + "client-shared-lib-debug": {
                "deps": ["//framework/terrazzo:client-debug"],
                "generate_tests": False,
                "rule": "shared_library",
                "target_compatible_with": ["@platforms//cpu:wasm32"],
                "crate_name": "terrazzo_terminal_debug",
            },
        },
        rustc_env_files = ["rustc.env"],
        deps = client_deps,
    )

    rust_wasm_bindgen(
        name = prefix + "client",
        out_name = "terrazzo_terminal",
        target = "web",
        wasm_file = ":" + prefix + "client-shared-lib",
    )

    rust_wasm_bindgen(
        name = prefix + "client-debug",
        out_name = "terrazzo_terminal",
        target = "web",
        wasm_file = ":" + prefix + "client-shared-lib-debug",
    )

    rust_rules_matrix(
        package_name = "terminal",
        assets = [
            [
                "assets/index.html",
                "assets/bootstrap.js",
                "assets/images/favicon.ico",
                "assets/jsdeps/dist/jsdeps.js",
                "assets/jsdeps/node_modules/@xterm/xterm/css/xterm.css",
            ] + glob(["assets/icons/*.svg"]),
            {
                "targets": [":terminal_scss"],
                "prefix": "target/css",
                "copy": True,
            },
        ],
        crate_features = server_features,
        overrides = {
            prefix + "server-lib": {
                "crate_name": "terrazzo_terminal",
                "assets": [{
                    "targets": [":" + prefix + "client"],
                    "prefix": "target/assets/wasm",
                }],
                "deps": ["//framework/terrazzo:server"],
            },
            prefix + "server-lib-debug": {
                "crate_name": "terrazzo_terminal_debug",
                "assets": [{
                    "targets": [":" + prefix + "client-debug"],
                    "prefix": "target/assets/wasm",
                }],
                "deps": ["//framework/terrazzo:server-debug"],
            },
        },
        rustc_env_files = ["rustc.env"],
        deps = server_deps,
    )

    rust_rules_matrix(
        package_name = "terminal",
        all_crate_deps = None,
        crate_features = ["server"],
        crate_name = "terminal_server",
        crate_root = "src/server.rs",
        overrides = {
            prefix + "server": {
                "deps": [":" + prefix + "server-lib"],
            },
            prefix + "server-debug": {
                "aliases": {
                    ":" + prefix + "server-lib-debug": "terrazzo_terminal",
                },
                "deps": [":" + prefix + "server-lib-debug"],
                "crate_features": ["debug"],
            },
        },
        rule = "binary",
        deps = ["@crates//:tracing"],
    )
