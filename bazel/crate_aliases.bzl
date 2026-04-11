def _mapped_label(actual, source_prefix, target_prefix):
    actual = str(actual)
    if not actual.startswith(source_prefix):
        fail("expected label to start with {}: {}".format(source_prefix, actual))
    return target_prefix + actual[len(source_prefix):]

def cfg_alias(name, actual, tags = None, **kwargs):
    source_prefix = "@crates__"
    native.alias(
        name = name,
        actual = select({
            "//bazel:opt": _mapped_label(actual, source_prefix, "@crates_opt__"),
            "//conditions:default": _mapped_label(actual, source_prefix, "@crates_plain__"),
        }),
        tags = tags,
        **kwargs
    )
