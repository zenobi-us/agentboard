import { describe, expect, test } from "bun:test";
import pkg from "../../package.json";

const entry = new URL("./index.ts", import.meta.url).pathname;
const builtCli = new URL("../../dist/cli", import.meta.url).pathname;
const builtLegacyCli = new URL("../../dist/agentboard", import.meta.url).pathname;

async function run(...args: string[]): Promise<{ code: number; stdout: string; stderr: string }> {
  const child = Bun.spawn([process.execPath, entry, ...args], { stdout: "pipe", stderr: "pipe" });
  return {
    code: await child.exited,
    stdout: await new Response(child.stdout).text(),
    stderr: await new Response(child.stderr).text(),
  };
}

async function runBuilt(path: string, ...args: string[]): Promise<{ code: number; stdout: string; stderr: string }> {
  const child = Bun.spawn([process.execPath, path, ...args], { stdout: "pipe", stderr: "pipe" });
  return {
    code: await child.exited,
    stdout: await new Response(child.stdout).text(),
    stderr: await new Response(child.stderr).text(),
  };
}

describe("Bun CLI command surface", () => {
  test("publishes ClankPipe and AgentBoard executable names", () => {
    expect(pkg.bin).toEqual({ clankpipe: "dist/cli", agentboard: "dist/agentboard" });
  });

  test("exposes the public command tree", async () => {
    const result = await run("--help");
    expect(result.code).toBe(0);
    expect(result.stdout).toContain("clankpipe");
    expect(result.stdout).toContain("workspace");
    expect(result.stdout).toContain("show");
    expect(result.stdout).toContain("run");
  });

  test("runs both built executable names and deprecates only AgentBoard", async () => {
    const clankpipe = await runBuilt(builtCli, "--help");
    expect(clankpipe.code).toBe(0);
    expect(clankpipe.stdout).toContain("clankpipe");
    expect(clankpipe.stderr).toBe("");

    const agentboard = await runBuilt(builtLegacyCli, "--help");
    expect(agentboard.code).toBe(0);
    expect(agentboard.stdout).toContain("clankpipe");
    expect(agentboard.stderr).toContain("agentboard is deprecated; use clankpipe instead.");
  });

  test("uses the package path for an external Workspace", async () => {
    const result = await run("run", "/tmp/agentboard-missing-workspace.toml", "--dry-run");
    expect(result.code).not.toBe(0);
  });

  test("lists an empty Store as JSON and accepts global output flags", async () => {
    const workspace = `/tmp/agentboard-empty-${process.pid}.toml`;
    await Bun.write(workspace, "sources = []\n");
    const result = await run("--quiet", "--color", "never", "list", workspace, "--json");
    expect(result.code).toBe(0);
    expect(JSON.parse(result.stdout)).toEqual([]);
  });

  test("supports Watch Mode flags for Store views", async () => {
    const list = await run("list", "--help");
    const show = await run("show", "--help");
    expect(list.stdout).toContain("--watch");
    expect(list.stdout).toContain("--interval");
    expect(show.stdout).toContain("--watch");
    expect(show.stdout).toContain("--interval");
  });

  test("emits structured run output without human summary", async () => {
    const workspace = `/tmp/agentboard-run-output-${process.pid}.toml`;
    await Bun.write(workspace, "sources = []\n");
    const result = await run("run", workspace, "--dry-run", "--output-format", "json");
    expect(result.code).toBe(0);
    expect(JSON.parse(result.stdout)).toEqual({ sources: [] });
    expect(result.stderr).toBe("");
  });

  test("rejects redirected Store Watch Mode", async () => {
    const workspace = `/tmp/agentboard-watch-${process.pid}.toml`;
    await Bun.write(workspace, "sources = []\n");
    for (const command of ["list", "show"]) {
      const args = command === "list" ? [command, workspace, "--watch"] : [command, workspace, "missing", "--watch"];
      const result = await run(...args);
      expect(result.code).not.toBe(0);
      expect(result.stderr + result.stdout).toContain("Watch Mode requires terminal stdout");
    }
  });

  test("rejects JSON output in Store Watch Mode", async () => {
    const workspace = `/tmp/agentboard-watch-json-${process.pid}.toml`;
    await Bun.write(workspace, "sources = []\n");
    for (const command of ["list", "show"]) {
      const args = command === "list"
        ? [command, workspace, "--watch", "--json"]
        : [command, workspace, "missing", "--watch", "--json"];
      const result = await run(...args);
      expect(result.code).not.toBe(0);
      expect(result.stderr + result.stdout).toContain("--watch cannot be combined with --json");
    }
  });

  test("keeps dashboard read-only and requires a terminal", async () => {
    const workspace = `/tmp/agentboard-dashboard-${process.pid}.toml`;
    await Bun.write(workspace, "sources = []\n");
    const result = await run("dashboard", workspace);
    expect(result.code).not.toBe(0);
    expect(result.stderr + result.stdout).toContain("Dashboard requires interactive stdin and stdout");
  });
});
