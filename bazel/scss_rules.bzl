"""Rules for running Terrazzo SCSS CLI."""

def scss_rule(name, output):
    native.genrule(
        name = name,
        srcs = ["Cargo.toml"] +
               native.glob(["src/**/*.css"], allow_empty = True) +
               native.glob(["src/**/*.scss"]),
        outs = [output],
        cmd = """$(execpath //utils/css/cli) $$(dirname $$(realpath $(location Cargo.toml))) --output-file $@""",
        tools = ["//utils/css/cli"],
    )
