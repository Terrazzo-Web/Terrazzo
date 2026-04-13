"""Helpers for aliases that switch crate_universe repositories by config."""

load("@bazel_skylib//lib:selects.bzl", "selects")

def _mapped_label(actual, source_prefix, target_prefix):
    actual = str(actual)
    if not actual.startswith(source_prefix):
        return actual
    return target_prefix + actual[len(source_prefix):]

def cfg_alias(name, actual, tags = None, **kwargs):
    """Creates an alias that switches crate_universe repos by mode and platform.

    Backend targets use `@crates_client__` or `@crates_opt_client__`.
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

    if "opt_backend" not in native.existing_rules():
        native.config_setting(
            name = "opt_backend",
            values = {"compilation_mode": "opt"},
        )
    if "fastbuild_backend" not in native.existing_rules():
        native.config_setting(
            name = "fastbuild_backend",
            values = {"compilation_mode": "fastbuild"},
        )
    if "dbg_backend" not in native.existing_rules():
        native.config_setting(
            name = "dbg_backend",
            values = {"compilation_mode": "dbg"},
        )
    if "opt_frontend" not in native.existing_rules():
        selects.config_setting_group(
            name = "opt_frontend",
            match_all = [
                ":opt_backend",
                "@rules_rust//rust/platform:wasm32-unknown-unknown",
            ],
        )
    if "fastbuild_frontend" not in native.existing_rules():
        selects.config_setting_group(
            name = "fastbuild_frontend",
            match_all = [
                ":fastbuild_backend",
                "@rules_rust//rust/platform:wasm32-unknown-unknown",
            ],
        )
    if "dbg_frontend" not in native.existing_rules():
        selects.config_setting_group(
            name = "dbg_frontend",
            match_all = [
                ":dbg_backend",
                "@rules_rust//rust/platform:wasm32-unknown-unknown",
            ],
        )
    native.alias(
        name = name,
        actual = select({
            ":opt_frontend": _mapped_label(actual, source_prefix, "@crates_opt_client__"),
            ":fastbuild_frontend": _mapped_label(actual, source_prefix, "@crates_client__"),
            ":dbg_frontend": _mapped_label(actual, source_prefix, "@crates_client__"),
            ":opt_backend": _mapped_label(actual, source_prefix, "@crates_opt_client__"),
            "//conditions:default": _mapped_label(actual, source_prefix, "@crates_client__"),
        }),
        tags = tags,
        **kwargs
    )
