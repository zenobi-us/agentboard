import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";

test("Quickstart bootstraps only the demo fixture", async () => {
  const quickstart = await readFile(
    join(import.meta.dir, "..", "content", "quickstart.md"),
    "utf8",
  );

  expect(quickstart).toContain("./apps/demo/setup.sh");
  expect(quickstart).toContain(
    "curl https://raw.githubusercontent.com/zenobi-us/clankpipe/refs/heads/main/apps/demo/setup.sh | sh",
  );
  expect(quickstart).toContain("env REPO=OWNER/clankpipe-quickstart-demo sh");
  expect(quickstart).toContain("TARGET_DIR=\"$HOME/clankpipe-quickstart-demo\"");
  expect(quickstart).toContain("downloads and copies only ClankPipe's `apps/demo` directory");
  expect(quickstart).toContain("CLANKPIPE_LAUNCHER=gnome-terminal");
  expect(quickstart).toContain("launches Pi through the selected launcher");
  expect(quickstart).toContain("installs ESLint, Husky, and lint-staged");
  expect(quickstart).toContain("lints staged `*.html` and `*.css` files");
  expect(quickstart).toContain("Husky runs lint-staged and ESLint");
  expect(quickstart).not.toContain("task-specific test");
  expect(quickstart).not.toContain("gh repo clone zenobi-us/clankpipe");
  expect(quickstart).not.toContain("cp -a clankpipe/apps/demo");
  expect(quickstart).not.toContain("open-terminal");
  expect(quickstart).not.toContain("xdg-terminal-exec");
  expect(quickstart).not.toContain("clankpipe-demo-template");
  expect(quickstart).toContain("Zellij");
  expect(quickstart).toContain("Herdr");
  expect(quickstart).not.toContain("./teardown.sh");
});
