import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";

test("Quickstart uses the repository demo fixture", async () => {
  const quickstart = await readFile(
    join(import.meta.dir, "..", "content", "quickstart.md"),
    "utf8",
  );

  expect(quickstart).toContain("cp -a agentboard/apps/demo");
  expect(quickstart).toContain(
    "gh repo create OWNER/agentboard-quickstart-demo",
  );
  expect(quickstart).toContain("--source=.");
  expect(quickstart).toContain("./setup.sh");
  expect(quickstart).toContain("xdg-terminal-exec");
  expect(quickstart).not.toContain("agentboard-demo-template");
  expect(quickstart).not.toContain("zellij");
  expect(quickstart).not.toContain("./teardown.sh");
});
