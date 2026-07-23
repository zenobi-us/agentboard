import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";

test("Quickstart uses the external demo template", async () => {
  const quickstart = await readFile(
    join(import.meta.dir, "..", "content", "quickstart.md"),
    "utf8",
  );

  expect(quickstart).toContain(
    "gh repo create OWNER/agentboard-quickstart-demo",
  );
  expect(quickstart).toContain(
    "--template zenobi-us/agentboard-demo-template",
  );
  expect(quickstart).toContain(
    "https://github.com/zenobi-us/agentboard-demo-template",
  );
  expect(quickstart).toContain("zellij attach --create agentboard-demo");
  expect(quickstart).toContain("./teardown.sh");
  expect(quickstart).not.toContain("raw.githubusercontent.com");
  expect(quickstart).not.toContain("start-zellij.sh");
});
