import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";

test("Quickstart bootstraps only the demo fixture", async () => {
  const quickstart = await readFile(
    join(import.meta.dir, "..", "content", "quickstart.md"),
    "utf8",
  );

  expect(quickstart).toContain(
    "curl https://raw.githubusercontent.com/zenobi-us/agentboard/refs/heads/main/apps/demo/setup.sh | sh",
  );
  expect(quickstart).toContain("REPO=OWNER/agentboard-quickstart-demo sh");
  expect(quickstart).toContain("downloads and copies only AgentBoard's `apps/demo` directory");
  expect(quickstart).toContain("npx --yes open-terminal");
  expect(quickstart).toContain("installs ESLint, Husky, and lint-staged");
  expect(quickstart).toContain("lints staged `*.html` and `*.css` files");
  expect(quickstart).toContain("Husky runs lint-staged and ESLint");
  expect(quickstart).not.toContain("task-specific test");
  expect(quickstart).not.toContain("gh repo clone zenobi-us/agentboard");
  expect(quickstart).not.toContain("cp -a agentboard/apps/demo");
  expect(quickstart).not.toContain("xdg-terminal-exec");
  expect(quickstart).not.toContain("agentboard-demo-template");
  expect(quickstart).not.toContain("zellij");
  expect(quickstart).not.toContain("./teardown.sh");
});
