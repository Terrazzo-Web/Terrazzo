"""Rules for running Stylance."""

def stylance_rule(name, output):
    native.genrule(
        name = name,
        srcs = ["Cargo.toml"] +
               native.glob(["src/**/*.css"], allow_empty = True) +
               native.glob(["src/**/*.scss"]),
        outs = [output],
        # Note: stylance integrates poorly with Bazel.
        # - realpath resolves to the actual path in the source code.
        # - stylance won't look at the symlinked *.css|*.scss files, it will
        #   directly use the files from source code.
        # - In practice, this is the same since we include all of them. We
        #   don't have the ability to compile different scss files with
        #   different inputs. We can only compile one scss file per crate.
        cmd = """
            mkdir -p stylance-tmp
            cleanup() {
                rm -rf stylance-tmp
            }
            trap cleanup EXIT

            for f in $$(find $$(dirname $(location Cargo.toml))); do
                if [ -d "$$f" ]; then
                    continue
                fi
                mkdir -p stylance-tmp/$$(dirname $$f)
                cp $$f stylance-tmp/$$f
            done
            OUTPUT="$$(realpath $$(dirname $@))/$$(basename $@)"
            STYLANCE_CLI="$$(realpath $(execpath //bazel:stylance))"
            (cd stylance-tmp/$$(dirname $(location Cargo.toml)) && "$$STYLANCE_CLI" . --output-file "$$OUTPUT")
        """,
        tools = ["//bazel:stylance"],
    )
