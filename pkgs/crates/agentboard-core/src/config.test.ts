import { describe, expect, test } from "bun:test";
import Type from "typebox";

import { action, definePlugin, source } from "./config.ts";

const sourcePlugin = definePlugin(import.meta, {
  kind: "source",
  itemBucketIdentity: () => "memory",
  schema: Type.Object({
    query: Type.String(),
    limit: Type.Optional(Type.Integer({ default: 50 })),
  }),
  runtime: (config) => config,
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
    create: () => ({
      execute: () => ({ outcome: "success" as const, stdout: "", stderr: "" }),
    }),
  }),
  healthCheck: () => undefined,
});

test("defines typed plugins with module metadata", () => {
  expect(sourcePlugin.kind).toBe("source");
  expect(sourcePlugin.meta.url).toBe(import.meta.url);
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

test("gives each inline configuration node a position", () => {
  const first = source(
    sourcePlugin,
    { query: "first" },
    "/workspace/positions.ts",
  );
  const second = source(
    sourcePlugin,
    { query: "second" },
    "/workspace/positions.ts",
  );

  expect(first.identity.position).toBe(0);
  expect(second.identity.position).toBe(1);
});

test("separates distinct inline plugins by configuration path and role", () => {
  const firstPlugin = definePlugin(import.meta, {
    kind: "source",
    itemBucketIdentity: () => "memory",
    schema: Type.Object({ query: Type.String() }),
    runtime: (config) => config,
    healthCheck: () => undefined,
  });
  const secondPlugin = definePlugin(import.meta, {
    kind: "source",
    itemBucketIdentity: () => "memory",
    schema: Type.Object({ query: Type.String() }),
    runtime: (config) => config,
    healthCheck: () => undefined,
  });

  const first = source(firstPlugin, { query: "first" }, "/workspace/agentboard.config.ts");
  const second = source(secondPlugin, { query: "second" }, "/workspace/agentboard.config.ts");

  expect(first.identity).toEqual({
    path: "/workspace/agentboard.config.ts",
    role: "source",
    position: 0,
  });
  expect(second.identity).toEqual({
    path: "/workspace/agentboard.config.ts",
    role: "source",
    position: 1,
  });
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
