"""Helpers for aliases that switch crate_universe repositories by config."""

load("@bazel_skylib//lib:selects.bzl", "selects")

def _mapped_label(actual, source_prefix, target_prefix):
    actual = str(actual)
    if not actual.startswith(source_prefix):
        return actual
    return target_prefix + actual[len(source_prefix):]

def cfg_alias(name, actual, tags = None, **kwargs):
    """Creates an alias that switches crate_universe repos by mode and platform.

    Backend targets use `@crates_server__` or `@crates_opt_server__`.
    `wasm32-unknown-unknown` targets use `@crates_client__` or
    `@crates_opt_client__`. Labels outside `@crates__` pass through unchanged.

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

    if "opt_server" not in native.existing_rules():
        native.config_setting(
            name = "opt_server",
            values = {"compilation_mode": "opt"},
        )
    if "fastbuild_server" not in native.existing_rules():
        native.config_setting(
            name = "fastbuild_server",
            values = {"compilation_mode": "fastbuild"},
        )
    if "dbg_server" not in native.existing_rules():
        native.config_setting(
            name = "dbg_server",
            values = {"compilation_mode": "dbg"},
        )
    if "opt_client" not in native.existing_rules():
        selects.config_setting_group(
            name = "opt_client",
            match_all = [
                ":opt_server",
                "@rules_rust//rust/platform:wasm32-unknown-unknown",
            ],
        )
    if "fastbuild_client" not in native.existing_rules():
        selects.config_setting_group(
            name = "fastbuild_client",
            match_all = [
                ":fastbuild_server",
                "@rules_rust//rust/platform:wasm32-unknown-unknown",
            ],
        )
    if "dbg_client" not in native.existing_rules():
        selects.config_setting_group(
            name = "dbg_client",
            match_all = [
                ":dbg_server",
                "@rules_rust//rust/platform:wasm32-unknown-unknown",
            ],
        )
    native.alias(
        name = name,
        actual = select({
            ":opt_client": _mapped_label(actual, source_prefix, "@crates_opt_client__"),
            ":fastbuild_client": _mapped_label(actual, source_prefix, "@crates_client__"),
            ":dbg_client": _mapped_label(actual, source_prefix, "@crates_client__"),
            ":opt_server": _mapped_label(actual, source_prefix, "@crates_opt_server__"),
            "//conditions:default": _mapped_label(actual, source_prefix, "@crates_server__"),
        }),
        tags = tags,
        **kwargs
    )
