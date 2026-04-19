"""Rules for running Stylance."""

def stylance_rule(name, output):
    native.genrule(
        name = name,
        srcs = ["Cargo.toml"] +
               native.glob(["src/**/*.css"], allow_empty = True) +
               # native.glob(["src/**/*.scss"]),
        outs = [output],
        # Note: stylance integrates poorly with Bazel.
        # - realpath resolves to the actual path in the source code.
        # - stylance won't look at the symlinked *.css|*.scss files, it will
        #   directly use the files from source code.
        # - In practice, this is the same since we include all of them. We
        #   don't have the ability to compile different scss files with
        #   different inputs. We can only compile one scss file per crate.
        cmd = """$(execpath //bazel:stylance) $$(dirname $$(realpath $(location Cargo.toml))) --output-file $@""",
        tools = ["//bazel:stylance"],
    )
