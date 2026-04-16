"""Rules for generating feature dependency constants from Cargo features."""

load("@rules_rust//rust:defs.bzl", "rust_binary")
load("//bazel:generated_file.bzl", "generate_file")

def _feature_deps_impl(ctx):
    output = ctx.actions.declare_file("generated.{}-features.bzl".format(ctx.attr.output_name))
    arguments = [
        ctx.file.path.path,
        output.path,
    ]
    for dependency, label in sorted(ctx.attr.dependency_aliases.items()):
        arguments.extend(["--dependency-alias", "{}={}".format(dependency, label)])
    for dep in ctx.attr.dependency_exclusion:
        arguments.extend(["--dependency-exclusion", dep])

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
        "dependency_aliases": attr.string_dict(),
        "dependency_exclusion": attr.string_list(),
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
            "@crates//:thiserror",
            "@crates//:toml",
        ],
    )

def feature_deps(name = None, path = None, dependency_aliases = {}, dependency_exclusion = []):
    """Generates a checked-in `{name}-features.bzl` file from a Cargo.toml file.

    Args:
      name: Optional output basename. Defaults to the current package basename.
      path: Optional label for the Cargo.toml file. Defaults to `Cargo.toml`.
      dependency_aliases: Optional mapping of `dep:` entries to Bazel labels.
      dependency_exclusion: Optional list of `dep:` entries to omit from generated constants.
    """
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
        dependency_aliases = dependency_aliases,
        dependency_exclusion = dependency_exclusion,
        path = path,
    )

    generate_file(
        name = name + "_update",
        src = ":" + name,
        dest = name + "-features.bzl",
        ignore_whitespace = True,
    )
