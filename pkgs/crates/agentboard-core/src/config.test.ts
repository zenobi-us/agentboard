import { describe, expect, test } from "bun:test";
import Type from "typebox";

import { action, definePlugin, isPluginDescriptor, source } from "./config.ts";

const sourcePlugin = definePlugin(import.meta, {
  kind: "source",
  itemBucketIdentity: () => "memory",
  schema: Type.Object({
    query: Type.String(),
    limit: Type.Optional(Type.Integer({ default: 50 })),
  }),
  runtime: () => ({ collect: () => [] }),
  healthCheck: () => undefined,
});

const actionPlugin = definePlugin(import.meta, {
  kind: "action",
  validate: () => undefined,
  schema: Type.Object({
    command: Type.String(),
    timeout: Type.Optional(Type.String({ default: "30s" })),
  }),
  runtime: () => ({
    execute: ({ inputs }) => ({
      outcome: "success" as const,
      stdout: inputs.command,
      stderr: "",
    }),
  }),
  healthCheck: () => undefined,
});

test("defines typed plugins with module metadata", () => {
  expect(sourcePlugin.kind).toBe("source");
  expect(sourcePlugin.meta.url).toBe(import.meta.url);
});

test("validates Plugin Descriptors through the exported Core contract", () => {
  expect(isPluginDescriptor(sourcePlugin)).toBe(true);
  expect(isPluginDescriptor({ ...sourcePlugin, runtime: undefined })).toBe(false);
  expect(isPluginDescriptor(actionPlugin)).toBe(true);
  expect(isPluginDescriptor({ ...actionPlugin, runtime: undefined })).toBe(false);
});

test("requires health checks from JavaScript Plugin definitions", () => {
  expect(() => definePlugin(import.meta, {
    kind: "source",
    itemBucketIdentity: () => "memory",
    schema: Type.Object({}),
    runtime: () => ({ collect: () => [] }),
  } as never)).toThrow("plugin must define healthCheck()");
});

test("source validates payloads and applies defaults", () => {
  const resolved = source(
    sourcePlugin,
    { id: "issues", query: "open" },
    "/workspace/source.ts",
  );

  expect(resolved.kind).toBe("source");
  expect(resolved.id).toBe("issues");
  expect(resolved.config).toEqual({ query: "open", limit: 50 });
  expect(resolved.identity).toEqual({
    path: "/workspace/source.ts",
    role: "source",
    position: 0,
  });
  expect(Object.keys(resolved)).not.toContain("plugin");
});

test("action validates payloads and keeps its core id outside the payload", () => {
  const resolved = action(
    actionPlugin,
    {
      id: "build",
      command: "bun run build",
    },
    "/workspace/action.ts",
  );

  expect(resolved.kind).toBe("action");
  expect(resolved.id).toBe("build");
  expect(resolved.config).toEqual({ command: "bun run build", timeout: "30s" });
});

test("keeps provisional inline identity local to one configuration node", () => {
  const first = source(sourcePlugin, { query: "first" }, "/workspace/positions.ts");
  const second = source(sourcePlugin, { query: "second" }, "/workspace/positions.ts");

  expect(first.identity).toEqual({
    path: "/workspace/positions.ts",
    role: "source",
    position: 0,
  });
  expect(second.identity).toEqual(first.identity);
});

describe("configuration errors", () => {
  test("rejects unknown fields", () => {
    expect(() =>
      source(
        sourcePlugin,
        { query: "open", unexpected: true } as never,
        "/workspace/unknown-fields.ts",
      ),
    ).toThrow();
  });

  test("rejects missing required fields", () => {
    expect(() =>
      source(sourcePlugin, {} as never, "/workspace/missing-fields.ts"),
    ).toThrow();
  });

  test("rejects a source plugin passed to action", () => {
    expect(() =>
      action(
        sourcePlugin as never,
        { query: "open" } as never,
        "/workspace/source-as-action.ts",
      ),
    ).toThrow(
      "source plugin cannot create action configuration",
    );
  });

  test("rejects an action plugin passed to source", () => {
    expect(() =>
      source(
        actionPlugin as never,
        { command: "build" } as never,
        "/workspace/action-as-source.ts",
      ),
    ).toThrow(
      "action plugin cannot create source configuration",
    );
  });
});
