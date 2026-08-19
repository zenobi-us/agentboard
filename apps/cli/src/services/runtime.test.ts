import { describe, expect, test } from "bun:test";
import { chmod, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import Type from "typebox";
import qmd from "@agentboard/source-qmd";

import {
  action,
  defineConfig,
  definePlugin,
  source,
  type ActionResult,
  type Item,
} from "@agentboard/core/config";

import { installCancellationHandlers } from "../cli/cancellation.ts";
import { parseRunInterval, runExitStatus } from "../cli/run.ts";
import { loadExecutableWorkspace } from "./config/workspace.ts";
import { checkWorkspaceHealth, runWorkspace, watchWorkspace } from "./runtime.ts";
import { acquireWorkspaceLock } from "./store.ts";
import { renderActionInputs } from "./template.ts";

describe("Workspace runtime orchestration", () => {
  test("runs a QMD Plugin through the Workspace runtime", async () => {
    const root = await mkdtemp(join(tmpdir(), "agentboard-qmd-cli-"));
    const bin = join(root, "qmd");
    const storeRoot = join(root, "store");
    await writeFile(bin, "#!/bin/sh\nprintf '%s' '[{\"path\":\"/notes/AB-1.md\",\"body\":\"---\\nid: AB-1\\ntitle: Do it\\nstatus: ready\\n---\\nBody\"}]'\n");
    await chmod(bin, 0o755);
    const previousPath = process.env["PATH"];
    process.env["PATH"] = root;
    const path = new URL("./qmd-cli.test.ts", import.meta.url).pathname;
    try {
      const workspace = await loadExecutableWorkspace(path, defineConfig({
        sources: [{ id: "tasks", source: source(qmd, { collections: ["tasks"], query: "status:ready" }, path) }],
      }));
      const result = await runWorkspace(workspace, { storeRoot });
      expect(result.sources[0]?.items).toEqual([expect.objectContaining({
        id: "/notes/AB-1.md",
        reference_id: "AB-1",
        source_kind: "qmd",
      })]);
    } finally {
      if (previousPath === undefined) delete process.env["PATH"];
      else process.env["PATH"] = previousPath;
      await rm(root, { recursive: true, force: true });
    }
  });

  test("runs the QMD Plugin through the CLI command", async () => {
    const root = await mkdtemp(join(tmpdir(), "agentboard-qmd-cli-command-"));
    const bin = join(root, "qmd");
    const configPath = join(root, "agentboard.config.ts");
    const cliPath = new URL("../cli/index.ts", import.meta.url).pathname;
    const qmdConfig = new URL("../../../../pkgs/crates/agentboard-source-qmd/src/config.ts", import.meta.url).href;
    const coreConfig = new URL("../../../../pkgs/crates/agentboard-core/src/config.ts", import.meta.url).href;
    await writeFile(bin, "#!/bin/sh\nprintf '%s' '[{\"path\":\"/notes/AB-1.md\",\"body\":\"---\\nid: AB-1\\ntitle: Do it\\nstatus: ready\\n---\\nBody\"}]'\n");
    await chmod(bin, 0o755);
    await writeFile(join(root, "package.json"), "{\"name\":\"qmd-cli-test\"}\n");
    await mkdir(join(root, "node_modules", "@agentboard"), { recursive: true });
    await symlink(
      new URL("../../../../pkgs/crates/agentboard-source-qmd", import.meta.url).pathname,
      join(root, "node_modules", "@agentboard", "source-qmd"),
      "dir",
    );
    await writeFile(configPath, `import { defineConfig, source } from ${JSON.stringify(coreConfig)};\nimport qmd from ${JSON.stringify(qmdConfig)};\nexport default defineConfig({ sources: [{ id: "tasks", source: source(qmd, { collections: ["tasks"], query: "status:ready" }, import.meta.url) }] });\n`);
    const previousPath = process.env["PATH"];
    process.env["PATH"] = root;
    try {
      const child = Bun.spawn([process.execPath, cliPath, "run", configPath], {
        env: { ...process.env, XDG_DATA_HOME: join(root, "data") },
        stdout: "pipe",
        stderr: "pipe",
      });
      const [code, stdout, stderr] = await Promise.all([
        child.exited,
        new Response(child.stdout as ReadableStream<Uint8Array>).text(),
        new Response(child.stderr as ReadableStream<Uint8Array>).text(),
      ]);
      expect(code, `${stdout}\n${stderr}`).toBe(0);
    } finally {
      if (previousPath === undefined) delete process.env["PATH"];
      else process.env["PATH"] = previousPath;
      await rm(root, { recursive: true, force: true });
    }
  });

  test("persists Source Snapshots and skips a Rendered Action after success", async () => {
    const storeRoot = await mkdtemp(join(tmpdir(), "agentboard-store-"));
    let sourceFactories = 0;
    let actionRuntimes = 0;
    const renderedInputs: unknown[] = [];

    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({ kind: Type.String(), title: Type.String() }),
      runtime: async (config, context) => {
        sourceFactories += 1;
        expect(context.sourceId).toBe("issues");
        return {
          collect: async (): Promise<Item[]> => [{
            id: "item-1",
            reference_id: "AB-1",
            title: config.title,
            status: "ready",
            url: "https://example.test/items/1",
            source_id: context.sourceId,
            source_kind: "memory",
            raw: { providerId: 1 },
          }],
        };
      },
      healthCheck: async (_config, context) => {
        expect(context.sourceId).toBe("issues");
      },
    });
    const actionPlugin = definePlugin(import.meta, {
      kind: "action",
      validate: () => undefined,
      pathInputs: ["message"],
      schema: Type.Object({ message: Type.String(), sourceKind: Type.String() }),
      runtime: async () => {
        actionRuntimes += 1;
        return {
          execute: async ({ item, inputs }): Promise<ActionResult> => {
            renderedInputs.push(inputs);
            return {
              outcome: "success",
              stdout: `${item.reference_id}:${inputs.message}:${inputs.sourceKind}`,
              stderr: "",
            };
          },
        };
      },
      healthCheck: async (_config, context) => {
        expect(context.sourceId).toBe("issues");
      },
    });
    const path = new URL("./runtime.test.ts", import.meta.url).pathname;
    const workspace = await loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, { kind: "memory", title: "Runtime item" }, path),
        actions: [action(actionPlugin, {
          message: "$AGENTBOARD_RUNTIME_ROOT/{{ item.title }}",
          sourceKind: "{{ source.source.kind }}",
        }, path)],
      }],
    }));

    const previousRoot = process.env["AGENTBOARD_RUNTIME_ROOT"];
    process.env["AGENTBOARD_RUNTIME_ROOT"] = "/tmp/runtime";
    const first = await runWorkspace(workspace, { storeRoot });
    const second = await runWorkspace(workspace, { storeRoot });

    expect(sourceFactories).toBe(1);
    expect(actionRuntimes).toBe(1);
    expect(renderedInputs).toEqual([
      { message: "/tmp/runtime/Runtime item", sourceKind: "memory" },
    ]);
    expect(first.sources[0]).toMatchObject({
      id: "issues",
      items: [{ raw: { providerId: 1 } }],
      actions: [{
        itemId: "item-1",
        result: { outcome: "success", stdout: "AB-1:/tmp/runtime/Runtime item:memory", stderr: "" },
      }],
    });
    expect(second.sources[0]?.actions).toEqual([{
      itemId: "item-1",
      actionIndex: 0,
      uses: `inline:${path}:action:0`,
      skipped: true,
    }]);
    const files = await Array.fromAsync(new Bun.Glob("**/*").scan({ cwd: storeRoot }));
    expect(files.some((file) => file.endsWith(".snapshots"))).toBe(true);
    const attempts = files.find((file) => file.includes("actions-") && file.endsWith(".jsonl"));
    expect(attempts).toBeDefined();
    expect((await readFile(join(storeRoot, attempts!), "utf8")).trim().split("\n")).toHaveLength(1);
    if (previousRoot === undefined) delete process.env["AGENTBOARD_RUNTIME_ROOT"];
    else process.env["AGENTBOARD_RUNTIME_ROOT"] = previousRoot;
    await rm(storeRoot, { recursive: true, force: true });
  });

  test("Watch Mode reuses one loaded Workspace until cancellation", async () => {
    const controller = new AbortController();
    let sourceCreations = 0;
    let actionRuntimes = 0;
    let collections = 0;
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: async () => {
        sourceCreations += 1;
        return { collect: () => { collections += 1; return []; } };
      },
      healthCheck: () => undefined,
    });
    const actionPlugin = definePlugin(import.meta, {
      kind: "action",
      validate: () => undefined,
      schema: Type.Object({}),
      runtime: async () => {
        actionRuntimes += 1;
        return {
          execute: (): ActionResult => ({ outcome: "success", stdout: "", stderr: "" }),
        };
      },
      healthCheck: () => undefined,
    });
    const path = new URL("./watch.test.ts", import.meta.url).pathname;
    const workspace = await loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, {}, path),
        actions: [action(actionPlugin, {}, path)],
      }],
    }), controller.signal);
    const storeRoot = await mkdtemp(join(tmpdir(), "agentboard-store-"));
    let runs = 0;
    let competingLock: Promise<boolean> | undefined;

    await watchWorkspace(workspace, {
      storeRoot,
      intervalMs: 0,
      onResult: () => {
        runs += 1;
        if (runs === 1) {
          competingLock = acquireWorkspaceLock(workspace, storeRoot).then(
            async (release) => { await release(); return true; },
            () => false,
          );
        }
        if (runs === 2) controller.abort();
      },
    });

    expect({ sourceCreations, actionRuntimes, collections, runs }).toEqual({
      sourceCreations: 1,
      actionRuntimes: 1,
      collections: 2,
      runs: 2,
    });
    expect(await competingLock).toBe(false);
    await rm(storeRoot, { recursive: true, force: true });
  });

  test("marks Watch Mode cancelled when cancellation interrupts the wait", async () => {
    const controller = new AbortController();
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: () => ({ collect: () => [] }),
      healthCheck: () => undefined,
    });
    const path = new URL("./watch-cancelled.test.ts", import.meta.url).pathname;
    const workspace = await loadExecutableWorkspace(path, defineConfig({
      sources: [{ id: "issues", source: source(sourcePlugin, {}, path) }],
    }), controller.signal);
    const storeRoot = await mkdtemp(join(tmpdir(), "agentboard-store-"));

    const resultPromise = watchWorkspace(workspace, { storeRoot, intervalMs: 60_000 });
    await Bun.sleep(0);
    controller.abort(new Error("cancelled"));
    const result = await resultPromise;

    expect(result.cancelled).toBe(true);
    expect(runExitStatus(result)).toBe(130);
    await rm(storeRoot, { recursive: true, force: true });
  });

  test("does not create Source runtimes after Action runtime creation cancels", async () => {
    const controller = new AbortController();
    let sourceRuntimeCreations = 0;
    let actionRuntimeCreations = 0;
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: () => {
        sourceRuntimeCreations += 1;
        return { collect: () => [] };
      },
      healthCheck: () => undefined,
    });
    const actionPlugin = definePlugin(import.meta, {
      kind: "action",
      validate: () => undefined,
      schema: Type.Object({}),
      runtime: async () => {
        actionRuntimeCreations += 1;
        controller.abort(new Error("cancelled"));
        return { execute: (): ActionResult => ({ outcome: "success", stdout: "", stderr: "" }) };
      },
      healthCheck: () => undefined,
    });
    const path = new URL("./runtime-creation-cancelled.test.ts", import.meta.url).pathname;

    await expect(loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, {}, path),
        actions: [
          action(actionPlugin, {}, path),
          action(actionPlugin, {}, path),
        ],
      }],
    }), controller.signal)).rejects.toThrow("cancelled");
    expect(sourceRuntimeCreations).toBe(0);
    expect(actionRuntimeCreations).toBe(1);
  });

  test("keeps Store metadata authoritative when an Action Result has extra fields", async () => {
    const storeRoot = await mkdtemp(join(tmpdir(), "agentboard-store-"));
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: (_config, context) => ({
        collect: (): Item[] => [{
          id: "one",
          reference_id: "one",
          title: "one",
          status: "ready",
          url: "https://example.test/one",
          source_id: context.sourceId,
          source_kind: "memory",
          raw: {},
        }],
      }),
      healthCheck: () => undefined,
    });
    const actionPlugin = definePlugin(import.meta, {
      kind: "action",
      validate: () => undefined,
      schema: Type.Object({}),
      runtime: () => ({
        execute: () => ({
          outcome: "success",
          stdout: "",
          stderr: "",
          source_id: "wrong",
          uses: "wrong",
          rendered_action_hash: "wrong",
        } as ActionResult & Record<string, unknown>),
      }),
      healthCheck: () => undefined,
    });
    const path = new URL("./action-result-metadata.test.ts", import.meta.url).pathname;
    const workspace = await loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, {}, path),
        actions: [action(actionPlugin, {}, path)],
      }],
    }));

    await runWorkspace(workspace, { storeRoot });
    const files = await Array.fromAsync(new Bun.Glob("**/actions-*.jsonl").scan({ cwd: storeRoot }));
    const attempt = JSON.parse(await readFile(join(storeRoot, files[0]!), "utf8")) as Record<string, unknown>;

    expect(attempt["source_id"]).toBe("issues");
    expect(attempt["uses"]).toBe(`inline:${path}:action:0`);
    expect(attempt["rendered_action_hash"]).not.toBe("wrong");
    await rm(storeRoot, { recursive: true, force: true });
  });

  test("starts all Source pipelines before an earlier Source finishes", async () => {
    let startSecond!: () => void;
    const secondStarted = new Promise<void>((resolve) => {
      startSecond = resolve;
    });
    const events: string[] = [];
    const first = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "first",
      schema: Type.Object({}),
      runtime: () => ({
        collect: async () => {
          events.push("first:start");
          await Promise.race([
            secondStarted,
            Bun.sleep(100).then(() => {
              throw new Error("second Source did not start");
            }),
          ]);
          events.push("first:end");
          return [];
        },
      }),
      healthCheck: () => undefined,
    });
    const second = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "second",
      schema: Type.Object({}),
      runtime: () => ({
        collect: () => {
          events.push("second:start");
          startSecond();
          return [];
        },
      }),
      healthCheck: () => undefined,
    });
    const path = new URL("./source-concurrency.test.ts", import.meta.url).pathname;
    const workspace = await loadExecutableWorkspace(path, defineConfig({
      sources: [
        { id: "first", source: source(first, {}, path) },
        { id: "second", source: source(second, {}, path) },
      ],
    }));
    const storeRoot = await mkdtemp(join(tmpdir(), "agentboard-store-"));

    const result = await runWorkspace(workspace, { storeRoot });

    expect(events).toContain("first:start");
    expect(events).toContain("second:start");
    expect(events.at(-1)).toBe("first:end");
    expect(result.sources.map(({ id, error }) => ({ id, error }))).toEqual([
      { id: "first", error: undefined },
      { id: "second", error: undefined },
    ]);
    await rm(storeRoot, { recursive: true, force: true });
  });

  test("keeps Source runtime errors scoped to their Source", async () => {
    const failing = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: () => ({ collect: () => Promise.reject(new Error("source failed")) }),
      healthCheck: () => undefined,
    });
    const working = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: (_config, context) => ({
        collect: async (): Promise<Item[]> => [{
          id: "item-2",
          reference_id: "AB-2",
          title: "Still ran",
          status: "ready",
          url: "https://example.test/items/2",
          source_id: context.sourceId,
          source_kind: "memory",
          raw: {},
        }],
      }),
      healthCheck: () => undefined,
    });
    const path = new URL("./source-error.test.ts", import.meta.url).pathname;
    const workspace = await loadExecutableWorkspace(path, defineConfig({
      sources: [
        { id: "broken", source: source(failing, {}, path) },
        { id: "working", source: source(working, {}, path) },
      ],
    }));

    const storeRoot = await mkdtemp(join(tmpdir(), "agentboard-store-"));
    const result = await runWorkspace(workspace, { storeRoot });

    expect(result.sources[0]).toMatchObject({ id: "broken", error: "source failed" });
    expect(result.sources[1]).toMatchObject({ id: "working", items: [{ id: "item-2" }] });
    await rm(storeRoot, { recursive: true, force: true });
  });

  test("retries when the latest attempt after a success is not successful", async () => {
    const storeRoot = await mkdtemp(join(tmpdir(), "agentboard-store-"));
    let executions = 0;
    let outcome: ActionResult["outcome"] = "success";
    let cachedSuccessIsValid = false;
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: (_config, context) => ({
        collect: (): Item[] => [{
          id: "one",
          reference_id: "one",
          title: "one",
          status: "ready",
          url: "https://example.test/one",
          source_id: context.sourceId,
          source_kind: "memory",
          raw: {},
        }],
      }),
      healthCheck: () => undefined,
    });
    const actionPlugin = definePlugin(import.meta, {
      kind: "action",
      validate: () => undefined,
      schema: Type.Object({}),
      runtime: () => ({
        cachedSuccessIsValid: () => cachedSuccessIsValid,
        execute: (): ActionResult => {
          executions += 1;
          return { outcome, stdout: "", stderr: "" };
        },
      }),
      healthCheck: () => undefined,
    });
    const path = new URL("./latest-attempt.test.ts", import.meta.url).pathname;
    const workspace = await loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, {}, path),
        actions: [action(actionPlugin, {}, path)],
      }],
    }));

    await runWorkspace(workspace, { storeRoot });
    outcome = "cancelled";
    await runWorkspace(workspace, { storeRoot });
    outcome = "success";
    cachedSuccessIsValid = true;
    const third = await runWorkspace(workspace, { storeRoot });

    expect(executions).toBe(3);
    expect(third.sources[0]?.actions[0]?.skipped).not.toBe(true);
    await rm(storeRoot, { recursive: true, force: true });
  });

  test("skips a duplicate Rendered Action identity within one Run", async () => {
    let calls = 0;
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: (_config, context) => ({
        collect: (): Item[] => [1, 2].map((value) => ({
          id: "one",
          reference_id: "one",
          title: `one-${value}`,
          status: "ready",
          url: "https://example.test/one",
          source_id: context.sourceId,
          source_kind: "memory",
          raw: {},
        })),
      }),
      healthCheck: () => undefined,
    });
    const actionPlugin = definePlugin(import.meta, {
      kind: "action",
      validate: () => undefined,
      schema: Type.Object({}),
      runtime: () => ({
        execute: (): ActionResult => {
          calls += 1;
          return { outcome: "success", stdout: "", stderr: "" };
        },
      }),
      healthCheck: () => undefined,
    });
    const path = new URL("./duplicate-identity.test.ts", import.meta.url).pathname;
    const workspace = await loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, {}, path),
        actions: [action(actionPlugin, {}, path)],
      }],
    }));
    const storeRoot = await mkdtemp(join(tmpdir(), "agentboard-store-"));

    const result = await runWorkspace(workspace, { storeRoot });

    expect(calls).toBe(1);
    expect(result.sources[0]?.actions.map((item) => item.skipped ?? false)).toEqual([false, true]);
    await rm(storeRoot, { recursive: true, force: true });
  });

  test("stops the current Item after an Action failure result", async () => {
    const calls: string[] = [];
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: (_config, context) => ({
        collect: async (): Promise<Item[]> => ["one", "two"].map((id) => ({
          id,
          reference_id: id,
          title: id,
          status: "ready",
          url: `https://example.test/${id}`,
          source_id: context.sourceId,
          source_kind: "memory",
          raw: {},
        })),
      }),
      healthCheck: () => undefined,
    });
    const actionPlugin = definePlugin(import.meta, {
      kind: "action",
      validate: () => undefined,
      schema: Type.Object({ name: Type.String() }),
      runtime: () => ({
        execute: async ({ item, inputs }): Promise<ActionResult> => {
          calls.push(`${item.id}:${inputs.name}`);
          return item.id === "one" && inputs.name === "first"
            ? { outcome: "failure", stdout: "", stderr: "failed" }
            : { outcome: "success", stdout: "", stderr: "" };
        },
      }),
      healthCheck: () => undefined,
    });
    const path = new URL("./action-failure.test.ts", import.meta.url).pathname;
    const workspace = await loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, {}, path),
        actions: [
          action(actionPlugin, { name: "first" }, path),
          action(actionPlugin, { name: "second" }, path),
        ],
      }],
    }));
    const storeRoot = await mkdtemp(join(tmpdir(), "agentboard-store-"));

    const result = await runWorkspace(workspace, { storeRoot });

    expect(calls).toEqual(["one:first", "two:first", "two:second"]);
    expect(result.sources[0]?.actions.map((item) => item.result?.outcome)).toEqual([
      "failure",
      "success",
      "success",
    ]);
    await rm(storeRoot, { recursive: true, force: true });
  });

  test("does not start Action execution after cached success validation observes cancellation", async () => {
    const controller = new AbortController();
    let executions = 0;
    let cancelDuringValidation = false;
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: (_config, context) => ({
        collect: (): Item[] => [{
          id: "one",
          reference_id: "one",
          title: "one",
          status: "ready",
          url: "https://example.test/one",
          source_id: context.sourceId,
          source_kind: "memory",
          raw: {},
        }],
      }),
      healthCheck: () => undefined,
    });
    const actionPlugin = definePlugin(import.meta, {
      kind: "action",
      validate: () => undefined,
      schema: Type.Object({}),
      runtime: () => ({
        cachedSuccessIsValid: async () => {
          if (cancelDuringValidation) controller.abort(new Error("cancelled"));
          return false;
        },
        execute: (): ActionResult => {
          executions += 1;
          return { outcome: "success", stdout: "", stderr: "" };
        },
      }),
      healthCheck: () => undefined,
    });
    const path = new URL("./cancel-after-cache-validation.test.ts", import.meta.url).pathname;
    const workspace = await loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, {}, path),
        actions: [action(actionPlugin, {}, path)],
      }],
    }), controller.signal);
    const storeRoot = await mkdtemp(join(tmpdir(), "agentboard-store-"));

    await runWorkspace(workspace, { storeRoot });
    cancelDuringValidation = true;
    const result = await runWorkspace(workspace, { storeRoot });

    expect(executions).toBe(1);
    expect(result.cancelled).toBe(true);
    expect(runExitStatus(result)).toBe(130);
    await rm(storeRoot, { recursive: true, force: true });
  });

  test("persists cancellation and stops new work", async () => {
    const calls: string[] = [];
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: (_config, context) => ({
        collect: (): Item[] => ["one", "two"].map((id) => ({
          id,
          reference_id: id,
          title: id,
          status: "ready",
          url: `https://example.test/${id}`,
          source_id: context.sourceId,
          source_kind: "memory",
          raw: {},
        })),
      }),
      healthCheck: () => undefined,
    });
    const actionPlugin = definePlugin(import.meta, {
      kind: "action",
      validate: () => undefined,
      schema: Type.Object({}),
      runtime: () => ({
        execute: ({ item }): ActionResult => {
          calls.push(item.id);
          return { outcome: "cancelled", stdout: "partial", stderr: "" };
        },
      }),
      healthCheck: () => undefined,
    });
    const path = new URL("./action-cancelled.test.ts", import.meta.url).pathname;
    const workspace = await loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, {}, path),
        actions: [action(actionPlugin, {}, path)],
      }],
    }));
    const storeRoot = await mkdtemp(join(tmpdir(), "agentboard-store-"));

    const result = await runWorkspace(workspace, { storeRoot });

    expect(calls).toEqual(["one"]);
    expect(result.cancelled).toBe(true);
    expect(result.sources[0]?.actions[0]?.result?.outcome).toBe("cancelled");
    await rm(storeRoot, { recursive: true, force: true });
  });

  test("keeps Action execution errors scoped to their Item", async () => {
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: (_config, context) => ({
        collect: async (): Promise<Item[]> => ["one", "two"].map((id) => ({
          id,
          reference_id: id,
          title: id,
          status: "ready",
          url: `https://example.test/${id}`,
          source_id: context.sourceId,
          source_kind: "memory",
          raw: {},
        })),
      }),
      healthCheck: () => undefined,
    });
    const actionPlugin = definePlugin(import.meta, {
      kind: "action",
      validate: () => undefined,
      schema: Type.Object({ itemId: Type.String() }),
      runtime: () => ({
        execute: async ({ item }): Promise<ActionResult> => {
          if (item.id === "one") throw new Error("action failed");
          return { outcome: "success", stdout: item.id, stderr: "" };
        },
      }),
      healthCheck: () => undefined,
    });
    const path = new URL("./action-error.test.ts", import.meta.url).pathname;
    const workspace = await loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, {}, path),
        actions: [action(actionPlugin, { itemId: "{{ item.id }}" }, path)],
      }],
    }));

    const storeRoot = await mkdtemp(join(tmpdir(), "agentboard-store-"));
    const result = await runWorkspace(workspace, { storeRoot });

    expect(result.sources[0]?.actions).toEqual([
      { itemId: "one", actionIndex: 0, uses: `inline:${path}:action:0`, error: "action failed" },
      {
        itemId: "two",
        actionIndex: 0,
        uses: `inline:${path}:action:0`,
        result: { outcome: "success", stdout: "two", stderr: "" },
      },
    ]);
    await rm(storeRoot, { recursive: true, force: true });
  });

  test("maps Run failures to status 1 and cancellation to status 130", () => {
    expect(runExitStatus({
      sources: [{ id: "one", uses: "source", items: [], actions: [], error: "failed" }],
    })).toBe(1);
    expect(runExitStatus({
      sources: [{
        id: "one",
        uses: "source",
        items: [],
        actions: [{ itemId: "one", actionIndex: 0, uses: "action", result: {
          outcome: "failure",
          stdout: "",
          stderr: "failed",
        } }],
      }],
    })).toBe(1);
    expect(runExitStatus({ sources: [], cancelled: true })).toBe(130);
  });

  test("parses Watch Mode intervals as seconds", () => {
    expect(parseRunInterval("30")).toBe(30_000);
    expect(parseRunInterval("1.5s")).toBe(1_500);
    expect(() => parseRunInterval("0")).toThrow("interval must be greater than zero");
  });

  test("cancels on the first interrupt and force-exits on the second", () => {
    const controller = new AbortController();
    const exits: number[] = [];
    const removeHandlers = installCancellationHandlers(controller, ((status: number) => {
      exits.push(status);
      throw new Error("forced exit");
    }) as never);

    process.emit("SIGINT", "SIGINT");
    expect(controller.signal.aborted).toBe(true);
    expect(() => process.emit("SIGINT", "SIGINT")).toThrow("forced exit");
    expect(exits).toEqual([130]);
    removeHandlers();
  });

  test("uses the shared run.lock Store contract", async () => {
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: () => ({ collect: () => [] }),
      healthCheck: () => undefined,
    });
    const path = new URL("./stale-lock.test.ts", import.meta.url).pathname;
    const workspace = await loadExecutableWorkspace(path, defineConfig({
      sources: [{ id: "issues", source: source(sourcePlugin, {}, path) }],
    }));
    const storeRoot = await mkdtemp(join(tmpdir(), "agentboard-store-"));
    const release = await acquireWorkspaceLock(workspace, storeRoot);

    expect(await Bun.file(join(storeRoot, workspace.id, "run.lock")).exists()).toBe(true);
    await expect(acquireWorkspaceLock(workspace, storeRoot)).rejects.toThrow(
      `workspace lock is held at ${join(storeRoot, workspace.id, "run.lock")}`,
    );
    await release();
    await rm(storeRoot, { recursive: true, force: true });
  });

  test("passes one invocation cancellation signal through Source and Action contexts", async () => {
    const controller = new AbortController();
    const signals: AbortSignal[] = [];
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: (_config, context) => {
        signals.push(context.cancellation);
        return {
          collect: (): Item[] => [{
            id: "one",
            reference_id: "one",
            title: "one",
            status: "ready",
            url: "https://example.test/one",
            source_id: context.sourceId,
            source_kind: "memory",
            raw: {},
          }],
        };
      },
      healthCheck: (_config, context) => signals.push(context.cancellation),
    });
    const actionPlugin = definePlugin(import.meta, {
      kind: "action",
      validate: () => undefined,
      schema: Type.Object({}),
      runtime: (_config, context) => {
        signals.push(context.cancellation);
        return {
          execute: (executeContext): ActionResult => {
            signals.push(executeContext.cancellation);
            return { outcome: "success", stdout: "", stderr: "" };
          },
        };
      },
      healthCheck: (_config, context) => signals.push(context.cancellation),
    });
    const path = new URL("./cancellation-context.test.ts", import.meta.url).pathname;
    const workspace = await loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, {}, path),
        actions: [action(actionPlugin, {}, path)],
      }],
    }), controller.signal);
    const storeRoot = await mkdtemp(join(tmpdir(), "agentboard-store-"));

    await checkWorkspaceHealth(workspace);
    await runWorkspace(workspace, { storeRoot });

    expect(signals).toEqual(Array(5).fill(controller.signal));
    await rm(storeRoot, { recursive: true, force: true });
  });

  test("writes Source Collection Status to a safe path and preserves the prior Snapshot after failure", async () => {
    const storeRoot = await mkdtemp(join(tmpdir(), "agentboard-store-"));
    let fail = false;
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: (_config, context) => ({
        collect: (): Item[] => {
          if (fail) throw new Error("collection failed");
          return [{
            id: "one",
            reference_id: "one",
            title: "one",
            status: "ready",
            url: "https://example.test/one",
            source_id: context.sourceId,
            source_kind: "memory",
            raw: {},
          }];
        },
      }),
      healthCheck: () => undefined,
    });
    const path = new URL("./collection-status.test.ts", import.meta.url).pathname;
    const workspace = await loadExecutableWorkspace(path, defineConfig({
      sources: [{ id: "../../issues", source: source(sourcePlugin, {}, path) }],
    }));

    await runWorkspace(workspace, { storeRoot });
    const filesBefore = await Array.fromAsync(new Bun.Glob("**/*.snapshots").scan({ cwd: storeRoot }));
    const boundaryPath = join(storeRoot, filesBefore[0]!);
    const boundaryBefore = await readFile(boundaryPath, "utf8");
    fail = true;
    await runWorkspace(workspace, { storeRoot });
    const statusFiles = await Array.fromAsync(new Bun.Glob("**/collection-status.json").scan({
      cwd: join(storeRoot, workspace.id),
    }));
    expect(statusFiles).toHaveLength(1);
    expect(statusFiles[0]).toMatch(/^sources\/issues-[a-f0-9]{12}\/collection-status\.json$/);
    const status = JSON.parse(await readFile(
      join(storeRoot, workspace.id, statusFiles[0]!),
      "utf8",
    )) as { state: string; error: string };

    expect(status).toMatchObject({ state: "failed", error: "collection failed" });
    expect(await readFile(boundaryPath, "utf8")).toBe(boundaryBefore);
    await rm(storeRoot, { recursive: true, force: true });
  });

  test("does not publish a Source Snapshot after cancellation", async () => {
    const controller = new AbortController();
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: () => ({
        collect: async (): Promise<Item[]> => {
          controller.abort();
          return [];
        },
      }),
      healthCheck: () => undefined,
    });
    const path = new URL("./source-cancelled.test.ts", import.meta.url).pathname;
    const workspace = await loadExecutableWorkspace(path, defineConfig({
      sources: [{ id: "issues", source: source(sourcePlugin, {}, path) }],
    }), controller.signal);
    const storeRoot = await mkdtemp(join(tmpdir(), "agentboard-store-"));

    const result = await runWorkspace(workspace, { storeRoot });

    expect(result.cancelled).toBe(true);
    const files = await Array.fromAsync(new Bun.Glob("**/*").scan({ cwd: storeRoot }));
    expect(files.some((file) => file.endsWith(".snapshots"))).toBe(false);
    const statusPath = files.find((file) => file.endsWith("collection-status.json"));
    expect(statusPath).toBeDefined();
    expect(JSON.parse(await readFile(
      join(storeRoot, statusPath!),
      "utf8",
    ))).toMatchObject({ state: "cancelled" });
    await rm(storeRoot, { recursive: true, force: true });
  });

  test("runs required Source and Action health checks without creating runtimes", async () => {
    const calls: string[] = [];
    let sourceRuntimeCreations = 0;
    let actionRuntimeCreations = 0;
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: () => {
        sourceRuntimeCreations += 1;
        return { collect: () => [] };
      },
      healthCheck: (_config, context) => calls.push(`source:${context.sourceId}`),
    });
    const actionPlugin = definePlugin(import.meta, {
      kind: "action",
      validate: () => undefined,
      schema: Type.Object({}),
      runtime: () => {
        actionRuntimeCreations += 1;
        return {
          execute: (): ActionResult => ({ outcome: "success", stdout: "", stderr: "" }),
        };
      },
      healthCheck: (_config, context) => calls.push(`action:${context.sourceId}`),
    });
    const path = new URL("./health.test.ts", import.meta.url).pathname;
    const workspace = await loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, {}, path),
        actions: [action(actionPlugin, {}, path)],
      }],
    }), undefined, false);

    const results = await checkWorkspaceHealth(workspace);

    expect(calls).toEqual(["source:issues", "action:issues"]);
    expect(sourceRuntimeCreations).toBe(0);
    expect(actionRuntimeCreations).toBe(0);
    expect(results).toEqual([
      { sourceId: "issues", role: "source", uses: `inline:${path}:source:0` },
      { sourceId: "issues", role: "action", uses: `inline:${path}:action:0`, actionIndex: 0 },
    ]);
  });

  test("fails missing and forward named Action input references", () => {
    expect(() => renderActionInputs(
      "{{ actions.missing.inputs.root }}",
      { actions: {} },
    )).toThrow();
    expect(() => renderActionInputs(
      "{% set key = \"worktree\" %}{{ actions[key].inputs.rooot }}",
      { actions: { worktree: { inputs: { root: "/tmp/worktree" } } } },
    )).toThrow("undefined value");
    expect(() => renderActionInputs(
      "{% set key = \"missing\" %}{% set prior = actions[key] %}ok",
      { actions: {} },
    )).toThrow("undefined value");
    expect(() => renderActionInputs(
      "{% set key = \"worktree\" %}{% set prior = actions[key] %}ok",
      { actions: {} },
    )).toThrow("undefined value");
    expect(() => renderActionInputs(
      "{% set key = \"miss\" ~ \"ing\" %}{% set prior = actions[key] %}ok",
      { actions: {} },
    )).toThrow("undefined value");
    expect(() => renderActionInputs(
      "{% set key = item.action_key %}{% set prior = actions[key] %}ok",
      { actions: {}, item: { action_key: "worktree" } },
    )).toThrow("undefined value");
    expect(() => renderActionInputs(
      "{{ actions|attr(\"worktree\")|attr(\"inputs\")|attr(\"rooot\") }}",
      { actions: { worktree: { inputs: { root: "/tmp/worktree" } } } },
    )).toThrow("undefined value");
    expect(() => renderActionInputs(
      "{% set key = \"worktree\" %}{% if actions[key].inputs.rooot %}x{% endif %}",
      { actions: { worktree: { inputs: { root: "/tmp/worktree" } } } },
    )).toThrow("undefined value");
    expect(() => renderActionInputs(
      "{% if actions|attr(\"worktree\")|attr(\"inputs\")|attr(\"rooot\") %}x{% endif %}",
      { actions: { worktree: { inputs: { root: "/tmp/worktree" } } } },
    )).toThrow("undefined value");
    expect(() => renderActionInputs(
      "{{ actions.worktree.inputs.rooot }}",
      { actions: { worktree: { inputs: { root: "/tmp/worktree" } } } },
    )).toThrow("undefined value");
    expect(() => renderActionInputs(
      "{% set prior = actions.worktree %}{{ prior.inputs.rooot }}",
      { actions: { worktree: { inputs: { root: "/tmp/worktree" } } } },
    )).toThrow("undefined value");
    for (const template of [
      "{{ (actions.worktree).inputs.rooot }}",
      "{% with prior = actions.worktree %}{{ prior.inputs.rooot }}{% endwith %}",
      "{% set named = actions %}{{ named.worktree.inputs.rooot }}",
      "{% set named = actions %}{% set prior = named.worktree %}{{ prior.inputs.rooot }}",
      "{% set prior = (actions.worktree) %}{{ prior.inputs.rooot }}",
      "{% for prior in [actions.worktree] %}{{ prior.inputs.rooot }}{% endfor %}",
    ]) {
      expect(() => renderActionInputs(
        template,
        { actions: { worktree: { inputs: { root: "/tmp/worktree" } } } },
      )).toThrow("undefined value");
    }
    expect(renderActionInputs('{{ "actions.missing" }}', { actions: {} })).toBe("actions.missing");
  });

  test("renders dynamic aliases for a preceding named Action", () => {
    for (const [key, context] of [
      ['"worktree"', {}],
      ['"work" ~ "tree"', {}],
      ["item.action_key", { item: { action_key: "worktree" } }],
    ] as const) {
      expect(renderActionInputs(
        `{% set key = ${key} %}
{% set prior = actions[key] %}
{{ prior.inputs.root }}`,
        { actions: { worktree: { inputs: { root: "/ok" } } }, ...context },
      )).toBe("\n\n/ok");
    }
  });

  test("keeps unrelated missing Item fields and plain Action text lenient", () => {
    expect(renderActionInputs("{{ item.missing }}", { item: {} })).toBe("");
    expect(renderActionInputs("{% if item.missing %}x{% endif %}", { item: {} })).toBe("");
    expect(renderActionInputs(
      "{{ actions.worktree.inputs.root }} {{ item.missing }}",
      { actions: { worktree: { inputs: { root: "/ok" } } }, item: {} },
    )).toBe("/ok ");
    expect(renderActionInputs("echo actions.missing", { actions: {} })).toBe("echo actions.missing");
    expect(renderActionInputs(
      "{% raw %}{% if actions.missing.inputs.root %}x{% endif %}{% endraw %}",
      { actions: {} },
    )).toBe("{% if actions.missing.inputs.root %}x{% endif %}");
    expect(renderActionInputs(
      "{# {% if actions.missing.inputs.root %}x{% endif %} #}",
      { actions: {} },
    )).toBe("");
    expect(() => renderActionInputs(
      "{{ actions . missing . inputs . root }}",
      { actions: {} },
    )).toThrow();
  });

  test("expands Action path inputs after rendering", () => {
    const previous = process.env["AGENTBOARD_TEST_ROOT"];
    process.env["AGENTBOARD_TEST_ROOT"] = "/tmp/agentboard-root";
    try {
      expect(renderActionInputs({
        cwd: "$AGENTBOARD_TEST_ROOT/{{ item.id }}",
        repo: "${AGENTBOARD_TEST_ROOT}/repo",
        root: "~/worktrees/{{ item.id }}",
        cmd: "echo $AGENTBOARD_TEST_ROOT",
      }, { item: { id: "AB-1" } }, { pathInputs: ["cwd", "repo", "root"] })).toEqual({
        cwd: "/tmp/agentboard-root/AB-1",
        repo: "/tmp/agentboard-root/repo",
        root: join(process.env["HOME"] ?? "", "worktrees/AB-1"),
        cmd: "echo $AGENTBOARD_TEST_ROOT",
      });
    } finally {
      if (previous === undefined) delete process.env["AGENTBOARD_TEST_ROOT"];
      else process.env["AGENTBOARD_TEST_ROOT"] = previous;
    }
  });

  test("does not expand path-like inputs for unrelated Actions", () => {
    const previous = process.env["AGENTBOARD_TEST_ROOT"];
    process.env["AGENTBOARD_TEST_ROOT"] = "/tmp/agentboard-root";
    try {
      expect(renderActionInputs(
        { cwd: "$AGENTBOARD_TEST_ROOT" },
        {},
        { pathInputs: [] },
      )).toEqual({ cwd: "$AGENTBOARD_TEST_ROOT" });
    } finally {
      if (previous === undefined) delete process.env["AGENTBOARD_TEST_ROOT"];
      else process.env["AGENTBOARD_TEST_ROOT"] = previous;
    }
  });

  test("preserves the documented Action template context", () => {
    const rendered = renderActionInputs({
      source: "{{ source.source.kind }}|{{ source.actions[0].uses }}",
      bracket: '{{ actions["worktree"].inputs["root"] }}',
      statement: "{% set prior = actions.worktree %}{{ prior.inputs.root }}",
      expression: "{{ item.raw.value + 1 }}",
    }, {
      source: {
        id: "issues",
        source: { kind: "memory" },
        actions: [{ uses: "agentboard/worktree" }],
      },
      item: { raw: { value: 1 } },
      actions: { worktree: { inputs: { root: "/tmp/worktree" } } },
    });

    expect(rendered).toEqual({
      source: "memory|agentboard/worktree",
      bracket: "/tmp/worktree",
      statement: "/tmp/worktree",
      expression: "2",
    });
    expect(renderActionInputs(
      "{% if item.ready %}{{ item.title | upper }}{% else %}no{% endif %}",
      { item: { ready: true, title: "ready" } },
    )).toBe("READY");
  });

  test("derives stable inline Plugin identity from Workspace path, role, and position", async () => {
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: () => ({ collect: () => [] }),
      healthCheck: () => undefined,
    });
    const actionPlugin = definePlugin(import.meta, {
      kind: "action",
      validate: () => undefined,
      schema: Type.Object({}),
      runtime: () => ({
        execute: (): ActionResult => ({ outcome: "success", stdout: "", stderr: "" }),
      }),
      healthCheck: () => undefined,
    });
    const path = new URL("./inline-identity.test.ts", import.meta.url).pathname;
    const configuration = () => defineConfig({
      sources: [
        {
          id: "first",
          source: source(sourcePlugin, {}, path),
          actions: [
            action(actionPlugin, {}, path),
            action(actionPlugin, {}, path),
          ],
        },
        {
          id: "second",
          source: source(sourcePlugin, {}, path),
          actions: [action(actionPlugin, {}, path)],
        },
      ],
    });

    const first = await loadExecutableWorkspace(path, configuration());
    const second = await loadExecutableWorkspace(path, configuration());
    const summarize = (workspace: typeof first) => workspace.sources.map((configured) => ({
      source: {
        packageName: configured.packageName,
        identity: configured.source.identity,
      },
      actions: configured.actions.map(({ packageName, identity }) => ({ packageName, identity })),
    }));

    expect(summarize(first)).toEqual(summarize(second));
    expect(summarize(first)).toEqual([
      {
        source: {
          packageName: `inline:${path}:source:0`,
          identity: { path, role: "source", position: 0 },
        },
        actions: [
          {
            packageName: `inline:${path}:action:0`,
            identity: { path, role: "action", position: 0 },
          },
          {
            packageName: `inline:${path}:action:1`,
            identity: { path, role: "action", position: 1 },
          },
        ],
      },
      {
        source: {
          packageName: `inline:${path}:source:1`,
          identity: { path, role: "source", position: 1 },
        },
        actions: [{
          packageName: `inline:${path}:action:2`,
          identity: { path, role: "action", position: 2 },
        }],
      },
    ]);
  });

  test("derives distinct stable Workspace ids from canonical paths", async () => {
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: () => ({ collect: () => [] }),
      healthCheck: () => undefined,
    });
    const configuration = (path: string) => defineConfig({
      sources: [{ id: "issues", source: source(sourcePlugin, {}, path) }],
    });
    const firstPath = "/tmp/first/agentboard.config.ts";
    const secondPath = "/tmp/second/agentboard.config.ts";

    const first = await loadExecutableWorkspace(firstPath, configuration(firstPath));
    const again = await loadExecutableWorkspace(firstPath, configuration(firstPath));
    const second = await loadExecutableWorkspace(secondPath, configuration(secondPath));

    expect(first.id).toBe(again.id);
    expect(first.id).not.toBe(second.id);
    expect(first.id).toMatch(/^agentboard\.config-[a-f0-9]{12}$/);
  });

  test("fails Workspace loading for invalid Action ids and templates", async () => {
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: () => ({ collect: () => [] }),
      healthCheck: () => undefined,
    });
    const actionPlugin = definePlugin(import.meta, {
      kind: "action",
      validate: () => undefined,
      schema: Type.Object({ value: Type.String() }),
      runtime: () => ({
        execute: (): ActionResult => ({ outcome: "success", stdout: "", stderr: "" }),
      }),
      healthCheck: () => undefined,
    });
    const path = new URL("./invalid-runtime.test.ts", import.meta.url).pathname;

    await expect(loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, {}, path),
        actions: [
          action(actionPlugin, { id: "duplicate", value: "ok" }, path),
          action(actionPlugin, { id: "duplicate", value: "ok" }, path),
        ],
      }],
    }))).rejects.toThrow('duplicate Action id "duplicate"');

    await expect(loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, {}, path),
        actions: [action(actionPlugin, { value: "{{" }, path)],
      }],
    }))).rejects.toThrow("syntax error");
  });

  test("fails Workspace loading when Action validation fails", async () => {
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: (_config, context) => ({
        collect: (): Item[] => [{
          id: "one",
          reference_id: "one",
          title: "one",
          status: "ready",
          url: "https://example.test/one",
          source_id: context.sourceId,
          source_kind: "memory",
          raw: {},
        }],
      }),
      healthCheck: () => undefined,
    });
    const actionPlugin = definePlugin(import.meta, {
      kind: "action",
      validate: () => {
        throw new Error("validation failed");
      },
      schema: Type.Object({}),
      runtime: () => ({
        execute: (): ActionResult => ({ outcome: "success", stdout: "", stderr: "" }),
      }),
      healthCheck: () => undefined,
    });
    const path = new URL("./factory-error.test.ts", import.meta.url).pathname;
    await expect(loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, {}, path),
        actions: [action(actionPlugin, {}, path)],
      }],
    }))).rejects.toThrow("validation failed");
  });

  test("fails Workspace loading when Action runtime creation fails", async () => {
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: () => ({ collect: () => [] }),
      healthCheck: () => undefined,
    });
    const actionPlugin = definePlugin(import.meta, {
      kind: "action",
      validate: () => undefined,
      schema: Type.Object({}),
      runtime: () => {
        throw new Error("factory exploded");
      },
      healthCheck: () => undefined,
    });
    const path = new URL("./factory-error.test.ts", import.meta.url).pathname;

    await expect(loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, {}, path),
        actions: [action(actionPlugin, {}, path)],
      }],
    }))).rejects.toThrow("Action runtime creation failed: factory exploded");
  });

  test("rejects malformed Source Items and Action Results", async () => {
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({ malformed: Type.Boolean() }),
      runtime: (config, context) => ({
        collect: (): unknown[] => config.malformed
          ? [{ id: "broken" }]
          : [{
              id: "one",
              reference_id: "one",
              title: "one",
              status: "ready",
              url: "https://example.test/one",
              source_id: context.sourceId,
              source_kind: "memory",
              raw: {},
            }],
      }) as never,
      healthCheck: () => undefined,
    });
    const actionPlugin = definePlugin(import.meta, {
      kind: "action",
      validate: () => undefined,
      schema: Type.Object({}),
      runtime: () => ({
        execute: () => ({ outcome: "bogus", stdout: "", stderr: "" }) as never,
      }),
      healthCheck: () => undefined,
    });
    const path = new URL("./invalid-output.test.ts", import.meta.url).pathname;
    const invalidSource = await loadExecutableWorkspace(path, defineConfig({
      sources: [{ id: "issues", source: source(sourcePlugin, { malformed: true }, path) }],
    }));
    const invalidAction = await loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, { malformed: false }, path),
        actions: [action(actionPlugin, {}, path)],
      }],
    }));
    const storeRoot = await mkdtemp(join(tmpdir(), "agentboard-store-"));

    expect((await runWorkspace(invalidSource, { storeRoot })).sources[0]?.error)
      .toContain("must return normalized Items");
    expect((await runWorkspace(invalidAction, { storeRoot })).sources[0]?.actions[0]?.error)
      .toContain("must return an AgentBoard Action Result");
    await rm(storeRoot, { recursive: true, force: true });
  });

  test("rejects duplicate Source ids before runtime creation", async () => {
    let factories = 0;
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: () => {
        factories += 1;
        return { collect: () => [] };
      },
      healthCheck: () => undefined,
    });
    const path = new URL("./duplicate-source.test.ts", import.meta.url).pathname;

    await expect(loadExecutableWorkspace(path, defineConfig({
      sources: [
        { id: "same", source: source(sourcePlugin, {}, path) },
        { id: "same", source: source(sourcePlugin, {}, path) },
      ],
    }))).rejects.toThrow('duplicate Source id "same"');
    expect(factories).toBe(0);
  });

});
