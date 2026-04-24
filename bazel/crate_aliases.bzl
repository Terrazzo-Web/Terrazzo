"""Helpers for aliases that switch crate_universe repositories by config."""

load("@bazel_skylib//lib:selects.bzl", "selects")

def _mapped_label(actual, source_prefix, target_prefix):
    actual = str(actual)
    if not actual.startswith(source_prefix):
        return actual
    return target_prefix + actual[len(source_prefix):]

def cfg_alias(name, actual, tags = None, **kwargs):
    """Creates an alias that switches crate_universe repos by mode and platform.

    Backend targets use `@crates_server_plain__` or `@crates_server_opt__`.
    `wasm32-unknown-unknown` targets use `@crates_client_plain__` or
    `@crates_client_opt__`. Labels outside `@crates__` pass through unchanged.

    Args:
        name: The alias target name to define in the current package.
        actual: The label the alias should point to before crate repo remapping.
        tags: Optional tags to attach to the generated alias target.
        **kwargs: Additional arguments forwarded to `native.alias`.
    """
    actual_str = str(actual)
    source_prefix = "@crates__"
    if not actual_str.startswith(source_prefix):
        native.alias(
            name = name,
            actual = actual,
            tags = tags,
            **kwargs
        )
        return

    if "opt_mode" not in native.existing_rules():
        native.config_setting(
            name = "opt_mode",
            values = {"compilation_mode": "opt"},
        )
    if "wasm_client" not in native.existing_rules():
        selects.config_setting_group(
            name = "wasm_client",
            match_all = [
                "@rules_rust//rust/platform:wasm32-unknown-unknown",
            ],
        )

    native.alias(
        name = name + "__client",
        actual = select({
            ":opt_mode": _mapped_label(actual, source_prefix, "@crates_client_opt__"),
            "//conditions:default": _mapped_label(actual, source_prefix, "@crates_client_plain__"),
        }),
        tags = tags,
    )
    native.alias(
        name = name + "__server",
        actual = select({
            ":opt_mode": _mapped_label(actual, source_prefix, "@crates_server_opt__"),
            "//conditions:default": _mapped_label(actual, source_prefix, "@crates_server_plain__"),
        }),
        tags = tags,
    )

    native.alias(
        name = name,
        actual = select({
            ":wasm_client": ":" + name + "__client",
            "//conditions:default": ":" + name + "__server",
        }),
        tags = tags,
        **kwargs
    )
