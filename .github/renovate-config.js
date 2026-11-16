module.exports = {
    extends: ["config:recommended"],
    enabled: true,

    autoApprove: true,
    automerge: true,
    automergeStrategy: "squash",
    gitAuthor: "Renovate Bot <bot@renovateapp.com>",
    // Generated files can be recreated after Renovate rebases a branch.
    gitIgnoredAuthors: ["41898282+github-actions[bot]@users.noreply.github.com"],
    platform: "github",
    repositories: ["Terrazzo-Web/Terrazzo"],
    packageRules: [
        {
            description: "lockFileMaintenance",
            matchUpdateTypes: ["pin", "digest", "patch", "minor", "major", "lockFileMaintenance"]
        },
        {
            matchPackageNames: ["/^.*$/"],
            automerge: true
        }
    ]
};
