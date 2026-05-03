"""Rules for updating vendored SyncTeX sources."""

load("//bazel:generated_file.bzl", "generate_file")

_SYNCTEX_COMMIT = "917617707955cde0c2fae127130d9d3129303cbc"
_SYNCTEX_RAW_URL = "https://raw.githubusercontent.com/jlaurens/synctex/%s" % _SYNCTEX_COMMIT

_SYNCTEX_VENDOR_FILES = [
    "LICENSE",
    "synctex_parser.c",
    "synctex_parser.h",
    "synctex_parser_advanced.h",
    "synctex_parser_readme.md",
    "synctex_parser_utils.c",
    "synctex_parser_utils.h",
    "synctex_version.h",
]

def _target_name(path):
    return "synctex_vendor_" + path.replace(".", "_").replace("-", "_")

def synctex_vendor_sources(name = None):
    """Build targets to keep vendored SyncTeX sources up-to-date.

    Args:
      name: Unused.
    """
    for path in _SYNCTEX_VENDOR_FILES:
        target_name = _target_name(path)
        generated_path = "generated/vendor/synctex/" + path
        vendored_path = "vendor/synctex/" + path

        native.genrule(
            name = target_name,
            outs = [generated_path],
            cmd = "curl --fail --location --silent --show-error --output $@ %s/%s" % (_SYNCTEX_RAW_URL, path),
        )

        generate_file(
            name = target_name + "_update",
            src = ":" + target_name,
            dest = vendored_path,
        )
