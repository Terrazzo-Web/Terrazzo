"""Rust rules bundle

See
- https://bazelbuild.github.io/rules_rust/rust.html
"""

load("@bazel_skylib//rules:build_test.bzl", "build_test")
load("@crates//:defs.bzl", "all_crate_deps")
load("@rules_rust//cargo:defs.bzl", "extract_cargo_lints")
load(
    "@rules_rust//rust:defs.bzl",
    "rust_binary",
    "rust_clippy",
    "rust_library",
    "rust_proc_macro",
    "rust_shared_library",
    "rust_test",
    "rustfmt_test",
)
load(":utils.bzl", "make_rules_matrix")

CARGO_ROOT = "cargo_root"

def rust_rules_matrix(overrides = {}, **kwargs):
    """Rust rules bundle

    Args:
      overrides: Map of parameters to generate a matrix of rust_rules
      **kwargs: Additional arguments
    """
    make_rules_matrix(rust_rules, overrides, **kwargs)

def rust_rules(
        name,
        package_name = None,
        deps = [],
        deps_proc_macro = [],
        deps_dev = [],
        deps_dev_proc_macro = [],
        data = [],
        rule = "library",
        crate_features = [],
        crate_features_dev = None,
        rustc_env_files = [],
        assets = [],
        generate_tests = True,
        **kwargs):
    """Rust rules bundle

    Args:
      name: Name of the Bazel target
      package_name: Name of the Rust package in the crate universe
      deps: Additional dependencies
      deps_proc_macro: Additional macro dependencies
      deps_dev: Additional dependencies for tests
      deps_dev_proc_macro: Additional macro dependencies for tests
      data: Data deps
      rule: one of the https://bazelbuild.github.io/rules_rust/rust.html#rules
      crate_features: List of features enabled
      crate_features_dev: List of features enabled for tests
      rustc_env_files: Environment variables
      assets: Either a list of asset maps or a shorthand list of target strings,
        which expands to [{"targets": <list>}]
      generate_tests: Whether to generate rust test and clippy targets
      **kwargs: Additional arguments
    """
    _rust_rules_impl(
        name,
        package_name,
        deps,
        deps_proc_macro,
        deps_dev,
        deps_dev_proc_macro,
        data,
        rule,
        crate_features,
        crate_features_dev,
        rustc_env_files,
        assets,
        generate_tests,
        **kwargs
    )

def _rust_rules_impl(
        name = "!!",
        package_name = "!!",
        deps = "!!",
        deps_proc_macro = "!!",
        deps_dev = "!!",
        deps_dev_proc_macro = "!!",
        data = "!!",
        rule = "!!",
        crate_features = "!!",
        crate_features_dev = "!!",
        rustc_env_files = "!!",
        assets = ["!!"],
        generate_tests = "!!",
        **kwargs):
    if package_name == None:
        package_name = name

    if crate_features_dev == None:
        crate_features_dev = crate_features

    rust_srcs = native.glob(["src/**/*.rs"])

    asset_copy_targets = ["Cargo.toml"] + rust_srcs
    asset_link_targets = []
    i = 0
    for asset in assets:
        # TODO: document the format of assets parameter in method rust_rules()
        if type(asset) == "string":
            asset = [asset]
        if type(asset) == "list":
            asset = {"targets": asset}
        if type(asset["targets"]) == "string":
            asset["targets"] = [asset["targets"]]

        if "prefix" in asset:
            asset_prefix = asset["prefix"]
        else:
            asset_prefix = None
        asset_copy = "copy" in asset and asset["copy"]
        if asset_prefix == None:
            if asset_copy:
                asset_copy_targets += asset["targets"]
            else:
                asset_link_targets += asset["targets"]
            continue
        if asset_prefix[0] == "/":
            fail("asset_prefix should be a relative path, got " + asset_prefix)

        i += 1
        asset_target = name + "-asset-" + str(i)
        if asset_copy:
            asset_copy_targets.append(":" + asset_target)
        else:
            asset_link_targets.append(":" + asset_target)
        _link_assets_to_dir(
            name = asset_target,
            srcs = asset["targets"],
            out_dir = "{}/{}/{}".format(CARGO_ROOT, name, asset_prefix),
            visibility = ["//visibility:private"],
            tags = ["manual"],
        )

    mirror = name + "-mirror"
    _mirror_sources(
        name = mirror,
        rust_target = name,
        asset_copy_targets = asset_copy_targets,
        asset_link_targets = asset_link_targets,
        visibility = ["//visibility:private"],
        tags = ["manual"],
    )
    native.filegroup(
        name = mirror + "-rs",
        srcs = [":" + mirror],
        output_group = "rs_files",
        visibility = ["//visibility:private"],
        tags = ["manual"],
    )
    native.filegroup(
        name = mirror + "-data",
        srcs = [":" + mirror],
        output_group = "data_files",
        visibility = ["//visibility:private"],
        tags = ["manual"],
    )
    native.filegroup(
        name = mirror + "-manifest",
        srcs = [":" + mirror],
        output_group = "manifest_file",
        visibility = ["//visibility:private"],
        tags = ["manual"],
    )

    if rule == "library":
        rule = rust_library
    elif rule == "binary":
        rule = rust_binary
    elif rule == "proc_macro":
        rule = rust_proc_macro
    elif rule == "shared_library":
        rule = rust_shared_library
    else:
        fail("Unknown rust target rule: " + rule)

    rule(
        name = name,
        srcs = [":" + mirror + "-rs"],
        lint_config = ":" + name + "-lints",
        deps = deps + all_crate_deps(package_name = package_name, normal = True),
        proc_macro_deps = deps_proc_macro + all_crate_deps(
            package_name = package_name,
            proc_macro = True,
        ),
        compile_data = [
            ":" + mirror + "-data",
            ":" + mirror + "-manifest",
        ],
        data = data,
        rustc_env_files = rustc_env_files + [":" + name + "-manifest-dir-env"],
        crate_features = crate_features + ["bazel"],
        **kwargs
    )

    if generate_tests:
        build_test(
            name = name + "-build-test",
            targets = [":" + name],
        )

        rust_test(
            name = name + "-test",
            crate = ":" + name,
            lint_config = ":" + name + "-lints",
            deps = deps_dev + all_crate_deps(
                package_name = package_name,
                normal_dev = True,
            ),
            proc_macro_deps = deps_dev_proc_macro + all_crate_deps(
                package_name = package_name,
                proc_macro_dev = True,
            ),
            data = data,
            crate_features = crate_features_dev + ["bazel"],
        )

        rustfmt_test(
            name = name + "-rustfmt",
            targets = [
                ":" + name,
                ":" + name + "-test",
            ],
        )

        rust_clippy(
            name = name + "-clippy",
            testonly = True,
            deps = [
                ":" + name,
                ":" + name + "-test",
            ],
        )

        build_test(
            name = name + "-clippy-build-test",
            targets = [":" + name + "-clippy"],
        )

    extract_cargo_lints(
        name = name + "-lints",
        manifest = "Cargo.toml",
        workspace = "//:Cargo.toml",
        visibility = ["//visibility:private"],
    )

    _manifest_dir_env(
        name = name + "-manifest-dir-env",
        manifest = ":" + mirror + "-manifest",
        out = name + "-manifest-dir.env",
        visibility = ["//visibility:private"],
        tags = ["manual"],
    )

def _mirror_sources_impl(ctx):
    package_name = ctx.build_file_path.rsplit("/", 1)[0]
    prefix = CARGO_ROOT + "/" + ctx.attr.rust_target + "/"
    lines = ["set -e"]
    copied_files = []
    rs_files = []
    data_files = []
    manifest_file = None

    for copy in [True, False]:
        if copy:
            asset_targets = ctx.files.asset_copy_targets
        else:
            asset_targets = ctx.files.asset_link_targets
        for f in asset_targets:
            if f.short_path.startswith(package_name):
                rel = f.short_path[len(package_name):]
            else:
                fail("Unexpected short_path:{} does not start with {} (path:{})".format(f.short_path, package_name, f.path))

            if f.extension == "rs":
                copied = ctx.actions.declare_file(prefix + rel)
                rs_files.append(copied)
            else:
                if f.is_directory:
                    copied = ctx.actions.declare_directory(prefix + rel)
                else:
                    copied = ctx.actions.declare_file(prefix + rel)
                if copied.basename == "Cargo.toml":
                    manifest_file = copied
                else:
                    data_files.append(copied)

            lines.append('mkdir -p "$(dirname "{}")"'.format(copied.path))
            if copy:
                lines.append('cp "{}" "{}"'.format(f.path, copied.path))
            else:
                lines.append('ln -s "$(realpath "{}")" "{}"'.format(f.path, copied.path))
            copied_files.append(copied)

    ctx.actions.run_shell(
        inputs = ctx.files.asset_copy_targets + ctx.files.asset_link_targets,
        outputs = copied_files,
        command = "\n".join(lines),
    )

    return [
        DefaultInfo(files = depset(copied_files)),
        OutputGroupInfo(
            rs_files = depset(rs_files),
            data_files = depset(data_files),
            manifest_file = depset([manifest_file]),
        ),
    ]

_mirror_sources = rule(
    implementation = _mirror_sources_impl,
    attrs = {
        "rust_target": attr.string(),
        "asset_copy_targets": attr.label_list(allow_files = True),
        "asset_link_targets": attr.label_list(allow_files = True),
    },
)

def _manifest_dir_env_impl(ctx):
    manifest = ctx.file.manifest
    out = ctx.outputs.out

    command = "printf 'CARGO_MANIFEST_DIR=%s\\n' \"$(dirname \"$(realpath \"{}\")\")\" > \"{}\"".format(
        manifest.path,
        out.path,
    )
    ctx.actions.run_shell(
        inputs = [manifest],
        outputs = [out],
        command = command,
    )
    return [DefaultInfo(files = depset([out]))]

_manifest_dir_env = rule(
    implementation = _manifest_dir_env_impl,
    attrs = {
        "manifest": attr.label(allow_single_file = True, mandatory = True),
        "out": attr.output(mandatory = True),
    },
)

def _link_assets_to_dir_impl(ctx):
    outputs = []
    commands = ["set -e"]

    for src in ctx.files.srcs:
        out_path = "{}/{}".format(ctx.attr.out_dir, src.basename)
        if src.is_directory:
            out = ctx.actions.declare_directory(out_path)
        else:
            out = ctx.actions.declare_file(out_path)
        commands.append('mkdir -p "$(dirname "{}")"'.format(out.path))
        commands.append('ln -s $(realpath "{}") "{}"'.format(src.path, out.path))
        outputs.append(out)

    ctx.actions.run_shell(
        inputs = ctx.files.srcs,
        outputs = outputs,
        command = "\n".join(commands),
    )

    return [DefaultInfo(files = depset(outputs))]

_link_assets_to_dir = rule(
    implementation = _link_assets_to_dir_impl,
    attrs = {
        "out_dir": attr.string(mandatory = True),
        "srcs": attr.label_list(mandatory = True),
    },
)
