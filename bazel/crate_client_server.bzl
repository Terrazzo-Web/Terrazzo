"""Rules for generating client/server crate MODULE.bazel fragments."""

load("//bazel:generated_file.bzl", "generate_file")

def _format_string_list(values):
    return ", ".join(['"%s"' % value for value in values])

def _format_list_block(values):
    return "[%s]" % "\n".join(['"%s",' % value for value in values])

def _crate_client_server_module_impl(ctx):
    out = ctx.actions.declare_file(ctx.attr.name + ".MODULE.bazel.generated")

    server_fn_features = _format_string_list(ctx.attr.server_fn_features)
    server_fn_deps = _format_list_block(ctx.attr.server_fn_deps)
    tracing_features = _format_list_block(ctx.attr.tracing_features)

    content = """{name} = use_extension("@rules_rust//crate_universe:extensions.bzl", "crate")
{name}.annotation(
    crate = "server_fn",
    crate_features = [{server_fn_features}],
    repositories = ["{name}"],
    deps = {server_fn_deps},
)
{name}.annotation(
    crate = "tracing",
    crate_features = {tracing_features},
    repositories = ["{name}"],
)
{name}.from_cargo(
    name = "{name}",
    cargo_lockfile = "//bazel:{name}.lock",
    manifests = ["//:Cargo.toml"],
)
use_repo({name}, "{name}")
""".format(
        name = ctx.attr.name,
        server_fn_features = server_fn_features,
        server_fn_deps = server_fn_deps,
        tracing_features = tracing_features,
    )

    ctx.actions.write(out, content)

    return DefaultInfo(files = depset([out]))

_crate_client_server_module = rule(
    implementation = _crate_client_server_module_impl,
    attrs = {
        "server_fn_features": attr.string_list(
            mandatory = True,
        ),
        "server_fn_deps": attr.string_list(),
        "tracing_features": attr.string_list(),
    },
)

def crate_client_server_module(name, server_fn_features, server_fn_deps = [], tracing_features = []):
    _crate_client_server_module(
        name = name,
        server_fn_features = server_fn_features,
        server_fn_deps = server_fn_deps,
        tracing_features = tracing_features,
    )
    generate_file(
        name = name + "_update",
        src = ":" + name,
        dest = name + ".MODULE.bazel",
        ignore_whitespace = True,
    )
