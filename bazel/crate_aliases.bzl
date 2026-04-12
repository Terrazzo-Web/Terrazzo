"""Helpers for aliases that switch crate_universe repositories by config."""

def _mapped_label(actual, source_prefix, target_prefix):
    actual = str(actual)
    if not actual.startswith(source_prefix):
        return actual
    return target_prefix + actual[len(source_prefix):]

def cfg_alias(name, actual, tags = None, **kwargs):
    """Creates an alias that switches crate_universe repos by compilation mode.

    In `opt` mode, labels under `@crates__` are remapped to `@crates_opt__`.
    In all other modes, they are remapped to `@crates_plain__`. Labels outside
    that prefix are passed through unchanged.

    Args:
        name: The alias target name to define in the current package.
        actual: The label the alias should point to before crate repo remapping.
        tags: Optional tags to attach to the generated alias target.
        **kwargs: Additional arguments forwarded to `native.alias`.
    """
    if "compilation_mode_opt" not in native.existing_rules():
        native.config_setting(
            name = "compilation_mode_opt",
            values = {"compilation_mode": "opt"},
        )
    source_prefix = "@crates__"
    native.alias(
        name = name,
        actual = select({
            ":compilation_mode_opt": _mapped_label(actual, source_prefix, "@crates_opt__"),
            "//conditions:default": _mapped_label(actual, source_prefix, "@crates_plain__"),
        }),
        tags = tags,
        **kwargs
    )
