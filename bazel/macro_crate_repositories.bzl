"""Shared macro crate repository setup for bzlmod."""

load("@bazel_tools//tools/build_defs/repo:git.bzl", "new_git_repository")
load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
load(
    "@rules_rust//crate_universe:defs.bzl",
    "crate",
)
load(
    "@rules_rust//crate_universe/private:common_utils.bzl",
    "new_cargo_bazel_fn",
)
load("@rules_rust//crate_universe/private:crates_repository.bzl", "SUPPORTED_PLATFORM_TRIPLES")
load(
    "@rules_rust//crate_universe/private:crates_vendor.bzl",
    "generate_config_file",
    "generate_splicing_manifest",
)
load(
    "@rules_rust//crate_universe/private:generate_utils.bzl",
    "CARGO_BAZEL_GENERATOR_SHA256",
    "CARGO_BAZEL_GENERATOR_URL",
    "GENERATOR_ENV_VARS",
    "determine_repin",
    "execute_generator",
    generate_render_config = "render_config",
)
load("@rules_rust//crate_universe/private:local_crate_mirror.bzl", "local_crate_mirror")
load(
    "@rules_rust//crate_universe/private:splicing_utils.bzl",
    "splice_workspace_manifest",
    generate_splicing_config = "splicing_config",
)
load("@rules_rust//crate_universe/private:urls.bzl", "CARGO_BAZEL_SHA256S", "CARGO_BAZEL_URLS")
load("@rules_rust//rust/platform:triple.bzl", "get_host_triple")

_CARGO_LOCKFILE = Label("//:Cargo.lock")
_MANIFESTS = [Label("//:Cargo.toml")]
_HOST_CARGO = "/Users/richard/.cargo/bin/cargo"
_HOST_RUSTC = "/Users/richard/.cargo/bin/rustc"

def _generate_repo_impl(repository_ctx):
    for path, contents in repository_ctx.attr.contents.items():
        repository_ctx.file(path, contents)
    repository_ctx.file("WORKSPACE.bazel", """workspace(name = "{}")""".format(repository_ctx.name))

_generate_repo = repository_rule(
    implementation = _generate_repo_impl,
    attrs = {
        "contents": attr.string_dict(mandatory = True),
    },
)

def _get_generator(module_ctx, host_triple):
    use_environ = False
    for var in GENERATOR_ENV_VARS:
        if var in module_ctx.os.environ:
            use_environ = True

    if use_environ:
        generator_sha256 = module_ctx.os.environ.get(CARGO_BAZEL_GENERATOR_SHA256)
        generator_url = module_ctx.os.environ.get(CARGO_BAZEL_GENERATOR_URL)
    elif len(CARGO_BAZEL_URLS) == 0:
        return module_ctx.path(Label("@cargo_bazel_bootstrap//:BUILD.bazel")).dirname.get_child("cargo-bazel")
    else:
        generator_sha256 = CARGO_BAZEL_SHA256S.get(host_triple.str)
        generator_url = CARGO_BAZEL_URLS.get(host_triple.str)

    if not generator_url:
        fail("No generator URL was found for `{}`".format(host_triple.str))

    output = module_ctx.path("cargo-bazel.exe" if "win" in module_ctx.os.name else "cargo-bazel")
    download_kwargs = {
        "executable": True,
        "output": output,
        "url": generator_url,
    }
    if generator_sha256:
        download_kwargs["sha256"] = generator_sha256
    module_ctx.download(**download_kwargs)
    return output

def _get_host_cargo_rustc(module_ctx, host_triple, host_tools_repo):
    _ = module_ctx
    _ = host_triple
    _ = host_tools_repo
    return (_HOST_CARGO, _HOST_RUSTC)

def _generate_hub_and_spokes(
        module_ctx,
        cargo_bazel_fn,
        cfg,
        annotations,
        render_config,
        splicing_config,
        lockfile,
        skip_cargo_lockfile_overwrite,
        strip_internal_dependencies_from_cargo_lockfile,
        cargo_lockfile = None,
        manifests = {},
        packages = {}):
    tag_path = module_ctx.path(cfg.name)

    config_file = tag_path.get_child("config.json")
    module_ctx.file(
        config_file,
        executable = False,
        content = generate_config_file(
            module_ctx,
            mode = "remote",
            annotations = annotations,
            generate_build_scripts = cfg.generate_build_scripts,
            supported_platform_triples = cfg.supported_platform_triples,
            generate_target_compatible_with = True,
            repository_name = cfg.name,
            output_pkg = cfg.name,
            workspace_name = cfg.name,
            generate_binaries = cfg.generate_binaries,
            render_config = render_config,
            repository_ctx = module_ctx,
        ),
    )

    splicing_manifest = tag_path.get_child("splicing_manifest.json")
    module_ctx.file(
        splicing_manifest,
        executable = False,
        content = generate_splicing_manifest(
            packages = packages,
            splicing_config = splicing_config,
            cargo_config = cfg.cargo_config,
            manifests = manifests,
            manifest_to_path = module_ctx.path,
        ),
    )

    repin = not lockfile or determine_repin(
        repository_ctx = module_ctx,
        repository_name = cfg.name,
        cargo_bazel_fn = cargo_bazel_fn,
        lockfile_path = lockfile,
        config = config_file,
        splicing_manifest = splicing_manifest,
    )

    nonhermetic_root_bazel_workspace_dir = module_ctx.path(Label("@@//:MODULE.bazel")).dirname

    kwargs = {}
    if repin:
        splice_outputs = splice_workspace_manifest(
            repository_ctx = module_ctx,
            cargo_bazel_fn = cargo_bazel_fn,
            cargo_lockfile = cargo_lockfile,
            splicing_manifest = splicing_manifest,
            config_path = config_file,
            output_dir = tag_path.get_child("splicing-output"),
            debug_workspace_dir = tag_path.get_child("splicing-workspace"),
            skip_cargo_lockfile_overwrite = cfg.skip_cargo_lockfile_overwrite,
            nonhermetic_root_bazel_workspace_dir = nonhermetic_root_bazel_workspace_dir,
            repository_name = cfg.name,
        )

        if cargo_lockfile == None:
            cargo_lockfile = splice_outputs.cargo_lock

        if lockfile == None:
            lockfile = tag_path.get_child("cargo-bazel-lock.json")
            module_ctx.file(lockfile, "")

        kwargs["metadata"] = splice_outputs.metadata

        for path_to_track in splice_outputs.extra_paths_to_track:
            if path_to_track.startswith(str(nonhermetic_root_bazel_workspace_dir)):
                module_ctx.watch(path_to_track)

    paths_to_track_file = tag_path.get_child("paths_to_track.json")
    warnings_output_file = tag_path.get_child("warnings_output.json")

    execute_generator(
        cargo_bazel_fn = cargo_bazel_fn,
        config = config_file,
        splicing_manifest = splicing_manifest,
        lockfile_path = lockfile,
        cargo_lockfile_path = cargo_lockfile,
        repository_dir = tag_path,
        nonhermetic_root_bazel_workspace_dir = nonhermetic_root_bazel_workspace_dir,
        paths_to_track_file = paths_to_track_file,
        warnings_output_file = warnings_output_file,
        skip_cargo_lockfile_overwrite = skip_cargo_lockfile_overwrite,
        strip_internal_dependencies_from_cargo_lockfile = strip_internal_dependencies_from_cargo_lockfile,
        **kwargs
    )

    for path in json.decode(module_ctx.read(paths_to_track_file)):
        module_ctx.watch(path)

    for warning in json.decode(module_ctx.read(warnings_output_file)):
        print("WARN: {}".format(warning))

    crates_dir = tag_path.get_child(cfg.name)
    _generate_repo(
        name = cfg.name,
        contents = {
            "BUILD.bazel": module_ctx.read(crates_dir.get_child("BUILD.bazel")),
            "alias_rules.bzl": module_ctx.read(crates_dir.get_child("alias_rules.bzl")),
            "defs.bzl": module_ctx.read(crates_dir.get_child("defs.bzl")),
        },
    )

    contents = json.decode(module_ctx.read(lockfile))
    for crate_info in contents["crates"].values():
        repo = crate_info["repository"]
        if repo == None:
            continue

        name = crate_info["name"]
        version = crate_info["version"]
        crate_repo_name = "{repo_name}__{name}-{version}".format(
            repo_name = cfg.name,
            name = name,
            version = version.replace("+", "-"),
        )

        if "Http" in repo:
            http_repo = repo["Http"]
            http_archive(
                name = crate_repo_name,
                patch_args = http_repo.get("patch_args", None),
                patch_tool = http_repo.get("patch_tool", None),
                patches = http_repo.get("patches", None),
                remote_patch_strip = 1,
                sha256 = http_repo.get("sha256", None),
                type = "tar.gz",
                urls = [http_repo["url"]],
                strip_prefix = "%s-%s" % (name, version),
                build_file_content = module_ctx.read(crates_dir.get_child("BUILD.%s-%s.bazel" % (name, version))),
            )
        elif "Git" in repo:
            git_repo = repo["Git"]
            git_kwargs = {}
            for key, value in git_repo["commitish"].items():
                git_kwargs["commit" if key == "Rev" else key.lower()] = value
            new_git_repository(
                name = crate_repo_name,
                init_submodules = True,
                patch_args = git_repo.get("patch_args", None),
                patch_tool = git_repo.get("patch_tool", None),
                patches = git_repo.get("patches", None),
                shallow_since = git_repo.get("shallow_since", None),
                remote = git_repo["remote"],
                build_file_content = module_ctx.read(crates_dir.get_child("BUILD.%s-%s.bazel" % (name, version))),
                strip_prefix = git_repo.get("strip_prefix", None),
                **git_kwargs
            )
        elif "Path" in repo:
            path_kwargs = {}
            if len(CARGO_BAZEL_URLS) == 0:
                path_kwargs["generator"] = "@cargo_bazel_bootstrap//:cargo-bazel"
            local_crate_mirror(
                name = crate_repo_name,
                options_json = json.encode({
                    "config": render_config,
                    "crate_context": crate_info,
                    "platform_conditions": contents["conditions"],
                    "supported_platform_triples": cfg.supported_platform_triples,
                }),
                path = repo["Path"]["path"],
                **path_kwargs
            )
        else:
            fail("Invalid repo metadata for crate %s-%s" % (name, version))

def _macro_client_annotations(_repo_name):
    return {
        "server_fn_macro": [crate.annotation(
            rustc_env = {
                "SERVER_FN_OVERRIDE_KEY": "tbd",
            },
        )],
    }

def _macro_server_annotations(_repo_name):
    return {
        "server_fn_macro": [crate.annotation(
            crate_features = [
                "axum",
                "ssr",
            ],
            rustc_env = {
                "SERVER_FN_OVERRIDE_KEY": "tbd",
            },
        )],
    }

def _macro_repo(module_ctx, generator, host_triple, repo_name, callback):
    module_ctx.watch(_CARGO_LOCKFILE)
    for manifest in _MANIFESTS:
        module_ctx.watch(manifest)

    cargo_path, rustc_path = _get_host_cargo_rustc(module_ctx, host_triple, None)
    cargo_bazel_fn = new_cargo_bazel_fn(
        repository_ctx = module_ctx,
        cargo_bazel_path = generator,
        cargo_path = cargo_path,
        rustc_path = rustc_path,
        isolated = True,
    )

    _generate_hub_and_spokes(
        module_ctx = module_ctx,
        cargo_bazel_fn = cargo_bazel_fn,
        cfg = struct(
            cargo_config = None,
            cargo_lockfile = _CARGO_LOCKFILE,
            generate_binaries = False,
            generate_build_scripts = True,
            host_tools = None,
            isolated = True,
            lockfile = None,
            manifests = _MANIFESTS,
            name = repo_name,
            skip_cargo_lockfile_overwrite = False,
            strip_internal_dependencies_from_cargo_lockfile = False,
            supported_platform_triples = SUPPORTED_PLATFORM_TRIPLES,
        ),
        annotations = callback(repo_name),
        cargo_lockfile = module_ctx.path(_CARGO_LOCKFILE),
        lockfile = None,
        manifests = {str(module_ctx.path(manifest)): str(manifest) for manifest in _MANIFESTS},
        packages = {},
        render_config = json.decode(generate_render_config(
            regen_command = "bazel mod show_repo '{}'".format(repo_name),
        )),
        skip_cargo_lockfile_overwrite = False,
        splicing_config = json.decode(generate_splicing_config()),
        strip_internal_dependencies_from_cargo_lockfile = False,
    )

def _macro_crate_repositories_ext_impl(module_ctx):
    host_triple = get_host_triple(module_ctx, abi = {
        "aarch64-unknown-linux": "musl",
        "x86_64-unknown-linux": "musl",
    })
    generator = _get_generator(module_ctx, host_triple)

    _macro_repo(module_ctx, generator, host_triple, "macro_client", _macro_client_annotations)
    _macro_repo(module_ctx, generator, host_triple, "macro_server", _macro_server_annotations)

macro_crate_repositories_ext = module_extension(
    implementation = _macro_crate_repositories_ext_impl,
)
