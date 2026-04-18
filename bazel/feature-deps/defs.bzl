"""Rules for generating feature dependency constants from Cargo features."""

load("//bazel:generated_file.bzl", "generate_file")

def _feature_deps_impl(ctx):
    output = ctx.actions.declare_file("generated.{}-features.bzl".format(ctx.attr.output_name))
    arguments = [
        ctx.file.manifest.path,
        ctx.file.root_rs.path,
        output.path,
    ]
    for dependency, label in sorted(ctx.attr.dependency_aliases.items()):
        arguments.extend(["--dependency-alias", "{}={}".format(dependency, label)])
    for dep in ctx.attr.dependency_exclusion:
        arguments.extend(["--dependency-exclusion", dep])

    ctx.actions.run(
        executable = ctx.executable.tool,
        inputs = [ctx.file.manifest] + ctx.files.all_rs,
        outputs = [output],
        tools = [ctx.executable.tool],
        arguments = arguments,
        mnemonic = "FeatureDeps",
        progress_message = "Generating {}".format(output.short_path),
    )

    return DefaultInfo(files = depset([output]))

_feature_deps = rule(
    implementation = _feature_deps_impl,
    attrs = {
        "output_name": attr.string(
            mandatory = True,
        ),
        "manifest": attr.label(
            allow_single_file = True,
            mandatory = True,
        ),
        "root_rs": attr.label(
            allow_single_file = True,
            mandatory = True,
        ),
        "all_rs": attr.label_list(
            allow_files = True,
            mandatory = True,
        ),
        "dependency_aliases": attr.string_dict(),
        "dependency_exclusion": attr.string_list(),
        "tool": attr.label(
            cfg = "exec",
            default = "//bazel/feature-deps",
            executable = True,
        ),
    },
)

def feature_deps(name = None, manifest = None, root_rs = "src/lib.rs", dependency_aliases = {}, dependency_exclusion = []):
    """Generates a checked-in `{name}-features.bzl` file from a Cargo.toml file.

    Args:
      name: Optional output basename. Defaults to the current package basename.
      manifest: Optional label for the Cargo.toml file. Defaults to `Cargo.toml`.
      root_rs: Crate root source file.
      dependency_aliases: Optional mapping of `dep:` entries to Bazel labels.
      dependency_exclusion: Optional list of `dep:` entries to omit from generated constants.
    """
    if name == None:
        package_name = native.package_name()
        if package_name:
            name = package_name.rsplit("/", 1)[-1]
        else:
            fail("feature_deps(name = None) is not supported in the workspace root package")

    if "/" in name:
        fail("feature_deps name must not contain '/', got {}".format(name))

    if manifest == None:
        manifest = "Cargo.toml"

    _feature_deps(
        name = name,
        output_name = name,
        manifest = manifest,
        root_rs = root_rs,
        all_rs = native.glob(["src/**/*.rs"]),
        dependency_aliases = dependency_aliases,
        dependency_exclusion = dependency_exclusion,
    )

    generate_file(
        name = name + "_update",
        src = ":" + name,
        dest = name + "-features.bzl",
        ignore_whitespace = True,
    )

def base_compute_srcs(features, all_srcs, all_features, excluded_file_id_map):
    """Computes the Rust source files to include for a base feature set.

    Args:
        features: Enabled feature names for the current target.
        all_srcs: All candidate source files for the target.
        all_features: Every known feature name in the manifest.
        excluded_file_id_map: Mapping of feature name to source files excluded
            when that feature is disabled.

    Returns:
        A list of source files from `src/**/*.rs` after excluding files shared by
        all disabled features. If every feature is enabled, returns all matching
        Rust source files.
    """
    features_set = {}
    for feature in features:
        features_set[feature] = True

    seed_feature = None
    for feature in all_features:
        if feature in features_set:
            continue

        seed_feature = feature
        break

    if seed_feature == None:
        return native.glob(["src/**/*.rs"])

    excluded_file_id_map2 = {}
    prev = set()
    for entry in excluded_file_id_map:
        file_ids = set(prev)
        for delta_file_id in entry["delta"]:
            if delta_file_id > 0:
                file_ids.add(delta_file_id)
            else:
                file_ids.remove(-delta_file_id)
        excluded_file_id_map2[entry["feature"]] = list(file_ids)
        prev = file_ids
    excluded_file_id_map = excluded_file_id_map2

    excluded_file_ids = {}
    for file_id in excluded_file_id_map[seed_feature]:
        excluded_file_ids[file_id] = True

    for feature in all_features:
        if feature in features_set:
            continue
        if feature == seed_feature:
            continue
        if not excluded_file_ids:
            break

        next_excluded_file_ids = {}
        for src in excluded_file_id_map[feature]:
            if src in excluded_file_ids:
                next_excluded_file_ids[src] = True
        excluded_file_ids = next_excluded_file_ids

    all_srcs_map = {}
    i = 0
    for src in all_srcs:
        all_srcs_map[i] = src[len(native.package_name()) + 1:]
        i += 1

    excluded_files = []
    for file_id in excluded_file_ids.keys():
        excluded_files.append(all_srcs_map[file_id - 1])

    print("\n\nFor %s\nExclude %s" % (features, str(excluded_files)))
    return native.glob(["src/**/*.rs"], exclude = excluded_files)
