"""Rules for generating feature dependency constants from Cargo features."""

load("@rules_rust//rust:defs.bzl", "rust_binary")
load("//bazel:generated_file.bzl", "generate_file")

def _feature_deps_impl(ctx):
    output = ctx.actions.declare_file("generated.{}-features.bzl".format(ctx.attr.output_name))
    arguments = [
        ctx.file.path.path,
        output.path,
    ]
    for dep in ctx.attr.exclude_deps:
        arguments.extend(["--exclude-dep", dep])

    ctx.actions.run(
        executable = ctx.executable.tool,
        inputs = [ctx.file.path],
        outputs = [output],
        tools = [ctx.executable.tool],
        arguments = arguments,
        mnemonic = "FeatureDeps",
        progress_message = "Generating {}".format(output.short_path),
    )

    return DefaultInfo(files = depset([output]))

_feature_deps = rule(
    implementation = _feature_deps_impl,
    attrs = {
        "output_name": attr.string(
            mandatory = True,
        ),
        "path": attr.label(
            allow_single_file = True,
            mandatory = True,
        ),
        "exclude_deps": attr.string_list(),
        "tool": attr.label(
            cfg = "exec",
            default = "//bazel/feature-deps:feature-deps",
            executable = True,
        ),
    },
)

def feature_deps_tool():
    rust_binary(
        name = "feature-deps",
        crate_name = "feature_deps",
        crate_root = "src/main.rs",
        srcs = native.glob(["src/**/*.rs"]),
        deps = [
            "@crates//:clap",
            "@crates//:heck",
            "@crates//:toml",
        ],
    )

def feature_deps(name = None, path = None, exclude_deps = []):
    if name == None:
        package_name = native.package_name()
        if package_name:
            name = package_name.rsplit("/", 1)[-1]
        else:
            fail("feature_deps(name = None) is not supported in the workspace root package")

    if "/" in name:
        fail("feature_deps name must not contain '/', got {}".format(name))

    if path == None:
        path = "Cargo.toml"

    _feature_deps(
        name = name,
        output_name = name,
        exclude_deps = exclude_deps,
        path = path,
    )

    generate_file(
        name = name + "_update",
        src = ":" + name,
        dest = name + "-features.bzl",
        ignore_whitespace = True,
    )
