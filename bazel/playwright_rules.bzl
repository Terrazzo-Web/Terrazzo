"""Rules for preparing and running Playwright dependencies for Bazel tests."""

load("@rules_shell//shell:sh_test.bzl", "sh_test")
load(":utils.bzl", "make_rules_matrix")

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

def playwright_matrix_test(overrides = {}, **kwargs):
    """Creates a matrix of Playwright tests.

    Args:
      overrides: Mapping of attribute names to lists of values used to expand
        multiple `playwright_test` targets via `make_rules_matrix`.
      **kwargs: Base arguments forwarded to each generated `playwright_test`.
    """
    make_rules_matrix(playwright_test, overrides, **kwargs)

def _target_with_suffix(target, suffix):
    if ":" in target:
        return target + suffix

    fail("Expected a target label with an explicit target name, got %s" % target)

def playwright_test(name, server, test, target_server = None, extra_data = [], tags = [], **kwargs):
    """Defines a Playwright test.

    Args:
      name: Name of the Bazel test target.
      server: Label of the server binary or launcher started by the test wrapper.
      test: Label of the Playwright test entrypoint to execute.
      target_server: Optional server binary managed by the launcher.
      extra_data: Additional runtime files needed by the Playwright test.
      tags: Additional Bazel tags to apply to the generated test.
      **kwargs: Additional arguments forwarded to `sh_test`.
    """
    terrazzo_server = target_server if target_server else server
    terrazzo_server_manifest = _target_with_suffix(terrazzo_server, "-mirror-manifest")
    terrazzo_server_data = _target_with_suffix(terrazzo_server, "-mirror-data")

    data = [
        server,
        test,
        terrazzo_server_data,
        terrazzo_server_manifest,
        "//bazel:playwright_setup",
        "@local_node_tools//:node",
        "@local_node_tools//:npx",
    ]
    if target_server:
        data.append(target_server)
    data.extend(extra_data)

    sh_test(
        name = name,
        srcs = ["//bazel:playwright_test.sh"],
        args = [
            "$(rootpath %s)" % server,
            "$(rootpath %s)" % target_server if target_server else "-",
            "$(rootpath %s)" % terrazzo_server_manifest,
            "$(rootpath //bazel:playwright_setup)",
            "$(rootpath @local_node_tools//:node)",
            "$(rootpath @local_node_tools//:npx)",
            "$(rootpath %s)" % test,
        ],
        data = data,
        tags = tags + ["playwright"],
        **kwargs
    )
