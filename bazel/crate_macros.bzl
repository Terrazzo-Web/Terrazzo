"""Rules for generating MODULE.bazel fragments."""

load("//bazel:generated_file.bzl", "generate_file")

def _crate_macro_module_impl(ctx):
    out = ctx.actions.declare_file(ctx.attr.name + ".MODULE.bazel.generated")

    features = ", ".join(['"%s"' % feature for feature in ctx.attr.module_features])
    content = """{name} = use_extension("@rules_rust//crate_universe:extensions.bzl", "crate")
{name}.annotation(
    crate = "server_fn_macro",
    crate_features = [{features}],
    repositories = ["{name}"],
    rustc_env = {{
        "SERVER_FN_OVERRIDE_KEY": "tbd",
    }},
)
{name}.from_cargo(
    name = "{name}",
    cargo_lockfile = "//:Cargo.lock",
    manifests = ["//:Cargo.toml"],
)
use_repo({name}, "{name}")
""".format(
        name = ctx.attr.name,
        features = features,
    )

    ctx.actions.write(out, content)

    return DefaultInfo(files = depset([out]))

_crate_macro_module = rule(
    implementation = _crate_macro_module_impl,
    attrs = {
        "module_features": attr.string_list(
            mandatory = True,
        ),
    },
)

def crate_macro_module(name, features):
    _crate_macro_module(
        name = name,
        module_features = features,
    )
    generate_file(
        name = name + "_update",
        src = ":" + name,
        dest = name + ".MODULE.bazel",
        ignore_whitespace = True,
    )
