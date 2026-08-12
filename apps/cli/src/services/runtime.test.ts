import { describe, expect, test } from "bun:test";
import Type from "typebox";

import {
  action,
  defineConfig,
  definePlugin,
  source,
  type ActionResult,
  type Item,
} from "@agentboard/core/config";

import { loadExecutableWorkspace } from "./config/workspace.ts";
import { checkWorkspaceHealth, runWorkspace } from "./runtime.ts";
import { renderActionInputs } from "./template.ts";

describe("Workspace runtime orchestration", () => {
  test("runs one in-memory Source and Action end to end", async () => {
    let sourceFactories = 0;
    const renderedInputs: unknown[] = [];

    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      schema: Type.Object({ title: Type.String() }),
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
      schema: Type.Object({ message: Type.String() }),
      runtime: (inputs) => {
        renderedInputs.push(inputs);
        return {
          execute: async (context): Promise<ActionResult> => ({
            outcome: "success",
            stdout: `${context.item.reference_id}:${inputs.message}`,
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
        source: source(sourcePlugin, { title: "Runtime item" }, path),
        actions: [action(actionPlugin, { message: "{{ item.title }}" }, path)],
      }],
    }));

    const first = await runWorkspace(workspace);
    const second = await runWorkspace(workspace);

    expect(sourceFactories).toBe(1);
    expect(renderedInputs).toEqual([
      { message: "{{ item.title }}" },
      { message: "Runtime item" },
      { message: "Runtime item" },
    ]);
    expect(first.sources[0]).toMatchObject({
      id: "issues",
      items: [{ raw: { providerId: 1 } }],
      actions: [{
        itemId: "item-1",
        result: { outcome: "success", stdout: "AB-1:Runtime item", stderr: "" },
      }],
    });
    expect(second.sources[0]?.actions).toHaveLength(1);
  });

  test("keeps Source runtime errors scoped to their Source", async () => {
    const failing = definePlugin(import.meta, {
      kind: "source",
      schema: Type.Object({}),
      runtime: () => ({ collect: () => Promise.reject(new Error("source failed")) }),
      healthCheck: () => undefined,
    });
    const working = definePlugin(import.meta, {
      kind: "source",
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

    const result = await runWorkspace(workspace);

    expect(result.sources[0]).toMatchObject({ id: "broken", error: "source failed" });
    expect(result.sources[1]).toMatchObject({ id: "working", items: [{ id: "item-2" }] });
  });

  test("keeps Action runtime errors scoped to their Item", async () => {
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
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

    const result = await runWorkspace(workspace);

    expect(result.sources[0]?.actions).toEqual([
      { itemId: "one", actionIndex: 0, error: "action failed" },
      {
        itemId: "two",
        actionIndex: 0,
        result: { outcome: "success", stdout: "two", stderr: "" },
      },
    ]);
  });

  test("runs required Source and Action health checks", async () => {
    const calls: string[] = [];
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
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

  test("fails Workspace loading for invalid Action ids and templates", async () => {
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
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

  test("fails Workspace loading when an Action runtime factory fails", async () => {
    const sourcePlugin = definePlugin(import.meta, {
      kind: "source",
      schema: Type.Object({}),
      runtime: () => ({ collect: () => [] }),
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

    await expect(loadExecutableWorkspace(path, defineConfig({
      sources: [{
        id: "issues",
        source: source(sourcePlugin, {}, path),
        actions: [action(actionPlugin, {}, path)],
      }],
    }))).rejects.toThrow("factory failed");
  });

});
