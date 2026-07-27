---
description: Review an issue branch with the Matt Pocock workflow
argument-hint: "<issue-number>"
---
Review `origin/main...HEAD` against GitHub issue #$1 and repository standards.

Load and follow the Matt Pocock `code-review` skill as the governing workflow. Use `origin/main` as the fixed point.

Act as an independent reviewer. Do not edit files or commit changes.

- Use `gh` to read the issue, pull request, and comments.
- Before reviewing, claim the issue: remove `agentboard:ready-for-review`, then add `agentboard:reviewing`.
- Verify the written acceptance criteria directly.
- Run relevant Moon validation.
- Post both Standards and Spec findings plus validation evidence on the pull request.

If blocking findings exist:

- Remove `agentboard:reviewing`.
- Add `agentboard:changes-requested`.
- Add `ready-for-agent` last so the implementation pipeline can retry.

If review passes:

- Remove `agentboard:reviewing` and `agentboard:changes-requested`.
- Add `agentboard:review-complete`.

If blocked:

- Comment with the blocker on the pull request.
- Remove `agentboard:reviewing`.
- Restore `agentboard:ready-for-review`.

Do not merge.
