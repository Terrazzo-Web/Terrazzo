module.exports = {
    extends: ["config:base"],
    autoApprove: true,
    automerge: true,
    automergeStrategy: 'squash',
    gitAuthor: 'Renovate Bot <bot@renovateapp.com>',
    onboarding: false,
    platform: 'github',
    repositories: ['Terrazzo-Web/Terrazzo'],
    packageRules: [
        {
            description: 'lockFileMaintenance',
            matchUpdateTypes: [
                'pin',
                'digest',
                'patch',
                'minor',
                'major',
                'lockFileMaintenance',
            ],
        },
        {
            packagePatterns: ["^.*$"],
            automerge: true
        }
    ],
};
