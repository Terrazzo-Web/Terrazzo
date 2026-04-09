"""Rules for preparing Playwright dependencies for Bazel tests."""

def _playwright_setup_impl(ctx):
    output_dir = ctx.actions.declare_directory(ctx.label.name)

    ctx.actions.run_shell(
        inputs = [
            ctx.file.package_json,
            ctx.file.package_lock,
        ],
        outputs = [output_dir],
        arguments = [
            output_dir.path,
            ctx.file.package_json.path,
            ctx.file.package_lock.path,
        ],
        command = """set -euo pipefail

output_dir="$1"
package_json="$2"
package_lock="$3"

mkdir -p "$output_dir"
cp "$package_json" "$output_dir/package.json"
cp "$package_lock" "$output_dir/package-lock.json"

cd "$output_dir"
export PLAYWRIGHT_BROWSERS_PATH="$output_dir/ms-playwright"

echo "npm install"
npm install

echo "npx playwright install --with-deps chromium"
npx playwright install --with-deps chromium
""",
        execution_requirements = {
            "local": "1",
            "no-sandbox": "1",
        },
        mnemonic = "PlaywrightSetup",
        progress_message = "Preparing Playwright dependencies for %s" % ctx.label,
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
    },
)
