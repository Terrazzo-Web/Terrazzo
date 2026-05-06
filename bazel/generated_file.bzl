"""Rules for generating files and having them checked-in."""

load("@rules_shell//shell:sh_binary.bzl", "sh_binary")

_GENERATED_FILE_SH = Label("//bazel:generated_file.sh")

def generate_file(name, src, dest, ignore_whitespace = False, runner = _GENERATED_FILE_SH):
    sh_binary(
        name = name,
        srcs = [runner],
        data = [src],
        args = [
            "$(location %s)" % src,
            native.package_name() + "/" + dest,
            "true" if ignore_whitespace else "false",
        ],
        tags = ["auto-generated"],
    )
