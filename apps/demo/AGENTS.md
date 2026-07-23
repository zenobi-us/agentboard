# AgentBoard demo agent instructions

- Implement and review each issue against its written acceptance criteria.
- Do not bypass Git hooks with `--no-verify`.
- Husky runs lint-staged before every commit.
- Staged `*.html` files are checked by ESLint with `@html-eslint/eslint-plugin`.
- Staged `*.css` files are checked by ESLint with `@eslint/css`.
- Fix hook failures before committing.
- Review agents should run `bun run lint` and report the result with their acceptance-criteria review.
