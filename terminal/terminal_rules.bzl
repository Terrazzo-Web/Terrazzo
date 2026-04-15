"""Rules to build Terrazzo Terminal"""

load("@rules_rust_wasm_bindgen//:defs.bzl", "rust_wasm_bindgen")
load("//bazel:rust_rules.bzl", "rust_rules_matrix")

def terminal_rules(
        prefix = "",
        client_features = [],
        client_deps = [],
        server_features = [],
        server_deps = []):
    """Defines the client and server Bazel targets for the terminal package.

    Args:
        prefix: String prepended to each generated target name.
        client_features: List of Rust crate features enabled for client targets.
        client_deps: List of additional dependencies for client targets.
        server_features: List of Rust crate features enabled for server library targets.
        server_deps: List of additional dependencies for server library targets.
    """
    if prefix and not prefix.endswith("-"):
        prefix += "-"
    rust_rules_matrix(
        package_name = "terminal",
        assets = [
            native.glob(["src/**/*.js"]),
            {
                "targets": native.glob(["src/**/*.scss"]),
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
                "crate_name": "terrazzo_terminal" + prefix[:-1].replace("-", "_"),
            },
            prefix + "client-shared-lib-debug": {
                "deps": ["//framework/terrazzo:client-debug"],
                "generate_tests": False,
                "rule": "shared_library",
                "target_compatible_with": ["@platforms//cpu:wasm32"],
                "crate_name": "terrazzo_terminal" + prefix[:-1].replace("-", "_") + "_debug",
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

    server_assets_common = [
        [
            "assets/index.html",
            "assets/bootstrap.js",
            "assets/images/favicon.ico",
            "assets/jsdeps/dist/jsdeps.js",
            "assets/jsdeps/node_modules/@xterm/xterm/css/xterm.css",
        ] + native.glob(["assets/icons/*.svg"]),
        {
            "targets": [":terminal_scss"],
            "prefix": "target/css",
            "copy": True,
        },
    ]
    server_assets_release = [{
        "targets": [":" + prefix + "client"],
        "prefix": "target/assets/wasm",
        "copy": True,
    }]
    server_assets_debug = [{
        "targets": [":" + prefix + "client-debug"],
        "prefix": "target/assets/wasm",
        "copy": True,
    }]

    rust_rules_matrix(
        package_name = "terminal",
        assets = server_assets_common,
        crate_features = server_features,
        overrides = {
            prefix + "server-lib": {
                "crate_name": "terrazzo_terminal" + prefix[:-1].replace("-", "_"),
                "assets": server_assets_release,
                "deps": ["//framework/terrazzo:server"],
            },
            prefix + "server-lib-debug": {
                "crate_name": "terrazzo_terminal" + prefix[:-1].replace("-", "_") + "_debug",
                "assets": server_assets_debug,
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
                "aliases": {
                    ":" + prefix + "server-lib": "terrazzo_terminal",
                },
                "deps": [":" + prefix + "server-lib"],
            },
            prefix + "server-debug": {
                "assets": server_assets_common + server_assets_debug,
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
