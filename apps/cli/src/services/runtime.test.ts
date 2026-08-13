import { describe, expect, test } from "bun:test";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import Type from "typebox";

import {
  action,
  defineConfig,
  definePlugin,
  source,
  type ActionResult,
  type Item,
} from "@agentboard/core/config";

import { installCancellationHandlers } from "../cli/cancellation.ts";
import { loadExecutableWorkspace } from "./config/workspace.ts";
import { checkWorkspaceHealth, runWorkspace } from "./runtime.ts";
import { renderActionInputs } from "./template.ts";

describe("Workspace runtime orchestration", () => {
  test("persists Source Snapshots and skips a Rendered Action after success", async () => {
    const storeRoot = await mkdtemp(join(tmpdir(), "agentboard-store-"));
    let sourceFactories = 0;
    const renderedInputs: unknown[] = [];

    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({ kind: Type.String(), title: Type.String() }),
      runtime: (config, context) => {
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
      schema: Type.Object({ message: Type.String(), sourceKind: Type.String() }),
      runtime: (inputs) => {
        renderedInputs.push(inputs);
        return {
          execute: async (context): Promise<ActionResult> => ({
            outcome: "success",
            stdout: `${context.item.reference_id}:${inputs.message}:${inputs.sourceKind}`,
            stderr: "",
          }),
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
          message: "{{ item.title }}",
          sourceKind: "{{ source.source.kind }}",
        }, path)],
      }],
    }));

    const first = await runWorkspace(workspace, { storeRoot });
    const second = await runWorkspace(workspace, { storeRoot });

    expect(sourceFactories).toBe(1);
    expect(renderedInputs).toEqual([
      { message: "Runtime item", sourceKind: "memory" },
      { message: "Runtime item", sourceKind: "memory" },
    ]);
    expect(first.sources[0]).toMatchObject({
      id: "issues",
      items: [{ raw: { providerId: 1 } }],
      actions: [{
        itemId: "item-1",
        result: { outcome: "success", stdout: "AB-1:Runtime item:memory", stderr: "" },
      }],
    });
    expect(second.sources[0]?.actions).toEqual([{
      itemId: "item-1",
      actionIndex: 0,
      skipped: true,
    }]);
    const files = await Array.fromAsync(new Bun.Glob("**/*").scan({ cwd: storeRoot }));
    expect(files.some((file) => file.endsWith(".snapshots"))).toBe(true);
    const attempts = files.find((file) => file.includes("actions-") && file.endsWith(".jsonl"));
    expect(attempts).toBeDefined();
    expect((await readFile(join(storeRoot, attempts!), "utf8")).trim().split("\n")).toHaveLength(1);
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
      schema: Type.Object({ name: Type.String() }),
      runtime: (inputs) => ({
        execute: async (context): Promise<ActionResult> => {
          calls.push(`${context.item.id}:${inputs.name}`);
          return context.item.id === "one" && inputs.name === "first"
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
      schema: Type.Object({}),
      runtime: () => ({
        execute: (context): ActionResult => {
          calls.push(context.item.id);
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

  test("keeps Action runtime errors scoped to their Item", async () => {
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
      schema: Type.Object({ itemId: Type.String() }),
      runtime: () => ({
        execute: async (context): Promise<ActionResult> => {
          if (context.item.id === "one") throw new Error("action failed");
          return { outcome: "success", stdout: context.item.id, stderr: "" };
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
      { itemId: "one", actionIndex: 0, error: "action failed" },
      {
        itemId: "two",
        actionIndex: 0,
        result: { outcome: "success", stdout: "two", stderr: "" },
      },
    ]);
    await rm(storeRoot, { recursive: true, force: true });
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

  test("recovers a Workspace lock left by a dead process", async () => {
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
    const lockRoot = join(storeRoot, workspace.id, "run-locks");
    await mkdir(lockRoot, { recursive: true });
    await writeFile(join(lockRoot, "999999999-stale"), "999999999");

    const result = await runWorkspace(workspace, { storeRoot });

    expect(result.cancelled).toBeUndefined();
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
      schema: Type.Object({}),
      runtime: (_inputs, context) => {
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
    expect(await Array.fromAsync(new Bun.Glob("**/*").scan({ cwd: storeRoot }))).toEqual([]);
    await rm(storeRoot, { recursive: true, force: true });
  });

  test("runs required Source and Action health checks", async () => {
    const calls: string[] = [];
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      itemBucketIdentity: () => "memory",
      schema: Type.Object({}),
      runtime: () => ({ collect: () => [] }),
      healthCheck: (_config, context) => calls.push(`source:${context.sourceId}`),
    });
    const actionPlugin = definePlugin(import.meta, {
      kind: "action",
      schema: Type.Object({}),
      runtime: () => ({
        execute: (): ActionResult => ({ outcome: "success", stdout: "", stderr: "" }),
      }),
      healthCheck: (_config, context) => calls.push(`action:${context.sourceId}`),
    });
    const path = new URL("./health.test.ts", import.meta.url).pathname;
    const workspace = await loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, {}, path),
        actions: [action(actionPlugin, {}, path)],
      }],
    }));

    const results = await checkWorkspaceHealth(workspace);

    expect(calls).toEqual(["source:issues", "action:issues"]);
    expect(results).toEqual([
      { sourceId: "issues", role: "source" },
      { sourceId: "issues", role: "action", actionIndex: 0 },
    ]);
  });

  test("fails missing and forward named Action input references", () => {
    expect(() => renderActionInputs(
      "{{ actions.missing.inputs.root }}",
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

  test("keeps an Action factory error scoped to its Item", async () => {
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
      schema: Type.Object({}),
      runtime: () => {
        throw new Error("factory failed");
      },
      healthCheck: () => undefined,
    });
    const path = new URL("./factory-error.test.ts", import.meta.url).pathname;
    const workspace = await loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, {}, path),
        actions: [action(actionPlugin, {}, path)],
      }],
    }));
    const storeRoot = await mkdtemp(join(tmpdir(), "agentboard-store-"));

    const result = await runWorkspace(workspace, { storeRoot });

    expect(result.sources[0]?.actions).toEqual([{
      itemId: "one",
      actionIndex: 0,
      error: "factory failed",
    }]);
    await rm(storeRoot, { recursive: true, force: true });
  });

});
