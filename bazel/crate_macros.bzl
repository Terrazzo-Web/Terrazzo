def _make_macros_repo_impl(ctx):
    for mod in ctx.modules:
        for tag in mod.tags.macros_repo:
            crates = use_extension("@rules_rust//crate_universe:extensions.bzl", "crate")
            crates.annotation(
                crate = "server_fn_macro",
                crate_features = tag.server_fn_macro_features,
                repositories = [tag.name],
                rustc_env = {
                    "SERVER_FN_OVERRIDE_KEY": "tbd",
                },
            )
            crates.from_cargo(
                name = tag.name,
                cargo_lockfile = "//:Cargo.lock",
                manifests = ["//:Cargo.toml"],
            )
            use_repo(crates, tag.name)

make_macros_repo = module_extension(
    implementation = _make_macros_repo_impl,
    tag_classes = {
        "macros_repo": tag_class(
            attrs = {
                "name": attr.string(mandatory = True),
                "server_fn_macro_features": attr.string_list(mandatory = True),
            },
        ),
    },
)
