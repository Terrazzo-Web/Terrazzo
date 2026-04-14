"""Rules for generating files and having them checked-in."""

load("@rules_shell//shell:sh_binary.bzl", "sh_binary")

def generate_file(name, src, dest, ignore_whitespace = False):
    sh_binary(
        name = name,
        srcs = ["//bazel:generated_file.sh"],
        data = [
            src,
        ],
        args = [
            "$(location %s)" % src,
            native.package_name() + "/" + dest,
            "true" if ignore_whitespace else "false",
        ],
        tags=["auto-generated"]
    )
