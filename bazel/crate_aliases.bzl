"""Helpers for aliases that switch crate_universe repositories by config."""

def _mapped_label(actual, source_prefix, target_prefix):
    actual = str(actual)
    if not actual.startswith(source_prefix):
        return actual
    return target_prefix + actual[len(source_prefix):]

def cfg_alias(name, actual, tags = None, **kwargs):
    """Creates an alias that switches crate_universe repos by mode and platform.

    Backend targets use `@crates_plain_backend__` or `@crates_opt_backend__`.
    `wasm32-unknown-unknown` targets use `@crates_plain_frontend__` or
    `@crates_opt_frontend__`. Labels outside `@crates__` pass through unchanged.

    Args:
        name: The alias target name to define in the current package.
        actual: The label the alias should point to before crate repo remapping.
        tags: Optional tags to attach to the generated alias target.
        **kwargs: Additional arguments forwarded to `native.alias`.
    """
    if "opt_backend" not in native.existing_rules():
        native.config_setting(
            name = "opt_backend",
            values = {"compilation_mode": "opt"},
        )
    source_prefix = "@crates__"
    native.alias(
        name = name,
        actual = select({
            "@rules_rust//rust/platform:wasm32-unknown-unknown": select({
                ":opt_backend": _mapped_label(actual, source_prefix, "@crates_opt_frontend__"),
                "//conditions:default": _mapped_label(actual, source_prefix, "@crates_plain_frontend__"),
            }),
            "//conditions:default": select({
                ":opt_backend": _mapped_label(actual, source_prefix, "@crates_opt_backend__"),
                "//conditions:default": _mapped_label(actual, source_prefix, "@crates_plain_backend__"),
            }),
        }),
        tags = tags,
        **kwargs
    )
