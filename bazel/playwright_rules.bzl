"""Rules for preparing and running Playwright dependencies for Bazel tests."""

load("@rules_shell//shell:sh_test.bzl", "sh_test")

def _playwright_setup_impl(ctx):
    output_dir = ctx.actions.declare_directory(ctx.label.name)

    ctx.actions.run(
        inputs = [
            ctx.file.package_json,
            ctx.file.package_lock,
            ctx.file._node,
            ctx.file._npm,
        ],
        outputs = [output_dir],
        arguments = [
            output_dir.path,
            ctx.file.package_json.path,
            ctx.file.package_lock.path,
            ctx.file._node.path,
            ctx.file._npm.path,
        ],
        executable = ctx.executable._setup_script,
        mnemonic = "PlaywrightSetup",
        progress_message = "Preparing Playwright dependencies for %s" % ctx.label,
        tools = [
            ctx.executable._setup_script,
            ctx.file._node,
            ctx.file._npm,
        ],
    )

    return [DefaultInfo(files = depset([output_dir]))]

playwright_setup = rule(
    implementation = _playwright_setup_impl,
    attrs = {
        "package_json": attr.label(
            allow_single_file = True,
            default = "//:package.json",
        ),
        "package_lock": attr.label(
            allow_single_file = True,
            default = "//:package-lock.json",
        ),
        "_setup_script": attr.label(
            allow_single_file = True,
            cfg = "exec",
            default = "//bazel:playwright_setup.sh",
            executable = True,
        ),
        "_node": attr.label(
            allow_single_file = True,
            cfg = "exec",
            default = "@local_node_tools//:node",
        ),
        "_npm": attr.label(
            allow_single_file = True,
            cfg = "exec",
            default = "@local_node_tools//:npm",
        ),
    },
)

def playwright_test(name, server, test, **kwargs):
    sh_test(
        name = name,
        srcs = ["//bazel:playwright_test.sh"],
        args = [
            "$(location %s)" % server,
            "$(rootpath //bazel:playwright_setup)",
            "$(rootpath @local_node_tools//:node)",
            "$(rootpath @local_node_tools//:npx)",
            "$(rootpath %s)" % test,
        ],
        data = [
            server,
            test,
            "//bazel:playwright_setup",
            "@local_node_tools//:node",
            "@local_node_tools//:npx",
        ],
        **kwargs
    )
