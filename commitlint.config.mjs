// Commitlint configuration used in CI to enforce commit message style.
export default {
  extends: ['@commitlint/config-conventional'],
  ignores: [
    // Common non-conventional messages that can appear in PRs.
    (message) => message.startsWith('Merge '),
    (message) => message.startsWith('Revert '),
    (message) => message.startsWith('fixup! '),
    (message) => message.startsWith('squash! '),
  ],
  rules: {
    // Keep this aligned with the project's history and git-cliff parsing.
    'type-enum': [
      2,
      'always',
      [
        'build',
        'chore',
        'ci',
        'docs',
        'doc',
        'feat',
        'fix',
        'perf',
        'refactor',
        'revert',
        'style',
        'test',
        'feature',
      ],
    ],
  },
};

