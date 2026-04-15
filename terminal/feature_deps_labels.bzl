"""Helpers for translating generated Cargo feature deps into Bazel labels."""

def crate_feature_deps(deps):
    labels = {
        "terrazzo-pty": "//pty",
        "trz-gateway-client": "//remote/client",
        "trz-gateway-common": "//remote/common",
        "trz-gateway-server": "//remote/server:acme",
    }

    return [labels.get(dep, "@crates//:" + dep) for dep in deps]

def dedupe(items):
    result = []
    for item in items:
        if item not in result:
            result.append(item)
    return result
