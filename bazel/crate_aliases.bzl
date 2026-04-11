def _mapped_label(actual, source_prefix, target_prefix):
    actual = str(actual)
    if not actual.startswith(source_prefix):
        return actual
    return target_prefix + actual[len(source_prefix):]

def cfg_alias(name, actual, tags = None, **kwargs):
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
