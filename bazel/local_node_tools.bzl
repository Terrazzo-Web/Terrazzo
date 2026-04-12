"""Exposes host Node.js executables to Bazel actions as declared tools."""

def _local_node_tools_repo_impl(repository_ctx):
    for tool in ["node", "npm", "npx"]:
        path = repository_ctx.which(tool)
        if path == None:
            fail("Required executable `%s` was not found on PATH." % tool)
        repository_ctx.symlink(path, tool)

    repository_ctx.file(
        "BUILD.bazel",
        """exports_files(["node", "npm", "npx"])""",
    )

local_node_tools = repository_rule(
    implementation = _local_node_tools_repo_impl,
    configure = True,
    local = True,
)

def _local_node_tools_ext_impl(_module_ctx):
    local_node_tools(name = "local_node_tools")

local_node_tools_ext = module_extension(
    implementation = _local_node_tools_ext_impl,
)
