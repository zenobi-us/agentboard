import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, test } from "bun:test";
import { Compile } from "typebox/compile";

import {
  discoverPluginPackages,
  findProjectPackageRoot,
  loadPluginPackage,
  loadSelectedPlugins,
} from "./plugins.ts";
import {
  createWorkspaceSchemas,
  loadAllWorkspacePlugins,
  loadDataWorkspace,
  loadExecutableWorkspace,
  loadWorkspacePlugins,
  type LoadedWorkspace,
} from "./config/workspace.ts";
import { loadRunWorkspace } from "../cli/run.ts";
import { loadSchemaWorkspace } from "../cli/schema.ts";

const roots: string[] = [];

function fixture(): string {
  const root = mkdtempSync(join(tmpdir(), "agentboard-plugins-"));
  roots.push(root);
  return root;
}

function pluginSource(kind: "source" | "action"): string {
  return `
    import { definePlugin } from ${JSON.stringify(
      new URL("../../../../pkgs/crates/agentboard-core/src/config.ts", import.meta.url).href,
    )};
    export default definePlugin(import.meta, {
      kind: ${JSON.stringify(kind)},
      schema: {
        type: "object",
        properties: { query: { type: "string" } },
        required: ["query"],
        additionalProperties: false,
      },
      runtime: () => undefined,
      healthCheck: () => undefined,
    });
  `;
}

function configurablePluginSource(
  kind: "source" | "action",
  additionalProperties?: boolean,
): string {
  return `
    import { definePlugin } from ${JSON.stringify(
      new URL("../../../../pkgs/crates/agentboard-core/src/config.ts", import.meta.url).href,
    )};
    export default definePlugin(import.meta, {
      kind: ${JSON.stringify(kind)},
      schema: {
        type: "object",
        properties: {
          query: { type: "string" },
          limit: { type: "integer", default: 50 },
        },
        required: ["query"],
        ${additionalProperties === undefined ? "" : `additionalProperties: ${additionalProperties},`}
      },
      runtime: (config) => config,
      healthCheck: () => undefined,
    });
  `;
}

function packageFixture(
  root: string,
  packagePath: string,
  name: string,
  source: string,
  keywords: string[] = ["agentboard-package"],
): void {
  const directory = join(root, packagePath);
  const entry = join(directory, "index.ts");
  mkdirSync(directory, { recursive: true });
  writeFileSync(entry, source);
  writeFileSync(
    join(directory, "package.json"),
    JSON.stringify({ name, keywords, exports: "./index.ts" }),
  );
}

function workspacePackage(root: string, name: string, directory: string): void {
  const path = join(root, "node_modules", ...name.split("/"));
  mkdirSync(join(path, ".."), { recursive: true });
  symlinkSync(directory, path, "dir");
}

function comparableWorkspace(workspace: LoadedWorkspace) {
  return workspace.sources.map(({ id, source, actions }) => ({
    id,
    source: {
      ...source,
      identity: { ...source.identity, path: "<workspace>" },
    },
    actions: actions.map(({ packageName: _packageName, ...configured }) => ({
      ...configured,
      identity: { ...configured.identity, path: "<workspace>" },
    })),
  }));
}

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

describe("Plugin Package discovery", () => {
  test("finds the nearest project package root and prefers it over global packages", () => {
    const root = fixture();
    const projectRoot = join(root, "packages/app");
    const configPath = join(projectRoot, "agentboard.config.ts");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "workspace" }));
    mkdirSync(projectRoot, { recursive: true });
    writeFileSync(join(projectRoot, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(
      projectRoot,
      "node_modules/@acme/local-plugin",
      "@acme/local-plugin",
      "export default undefined;",
    );
    packageFixture(
      join(root, "global"),
      "@acme/global-plugin",
      "@acme/global-plugin",
      "export default undefined;",
    );
    packageFixture(
      projectRoot,
      "node_modules/not-a-plugin",
      "not-a-plugin",
      "export default undefined;",
      [],
    );

    expect(findProjectPackageRoot(configPath)).toBe(projectRoot);
    const packages = discoverPluginPackages(configPath, join(root, "global"));

    expect(packages.map((item) => item.name)).toEqual([
      "@acme/local-plugin",
      "@acme/global-plugin",
    ]);
  });

  test("local packages override global packages with the same identity", () => {
    const root = fixture();
    const configPath = join(root, "agentboard.config.ts");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(root, "node_modules/local", "same", "export default undefined;");
    packageFixture(join(root, "global"), "same", "same", "export default undefined;");

    const packages = discoverPluginPackages(configPath, join(root, "global"));

    expect(packages.map((item) => item.root)).toEqual([join(root, "node_modules/local")]);
  });

  test("keeps duplicate identities within one discovery root as errors", () => {
    const root = fixture();
    const configPath = join(root, "agentboard.config.ts");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(root, "node_modules/first", "same", "export default undefined;");
    packageFixture(root, "node_modules/second", "same", "export default undefined;");

    expect(() => discoverPluginPackages(configPath, join(root, "global"))).toThrow(
      'duplicate Plugin Package identity "same"',
    );
  });

  test("loads every discovered package for schema generation", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.config.ts");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(root, "node_modules/first", "first", pluginSource("source"));
    packageFixture(root, "node_modules/second", "second", pluginSource("action"));

    const registry = await loadAllWorkspacePlugins(configPath, join(root, "global"));
    const schemas = createWorkspaceSchemas(registry);

    expect([...registry.sources.keys()]).toEqual(["first"]);
    expect([...registry.actions.keys()]).toEqual(["second"]);
    expect(JSON.stringify(schemas.workspace)).toContain("first");
    expect(JSON.stringify(schemas.workspace)).toContain("second");
    expect(Compile(schemas.source).Check({ uses: "first", query: "ready" })).toBe(true);
    expect(Compile(schemas.source).Check({ uses: "first", query: "ready", extra: true })).toBe(false);
    expect(Compile(schemas.source).Check({ uses: "first", query: "ready", id: "reserved" })).toBe(false);
    expect(Compile(schemas.source).Check({ uses: "first", with: { query: "ready" } })).toBe(false);
    expect(Compile(schemas.action).Check({ id: "named", uses: "second", with: { query: "ready" } })).toBe(true);
    expect(Compile(schemas.action).Check({ uses: "second", with: { query: "ready", id: "reserved" } })).toBe(false);
    expect(Compile(schemas.action).Check({ uses: "second", query: "flat", with: { query: "ready" } })).toBe(false);
    expect(Compile(schemas.action).Check({ uses: "unknown", with: { query: "ready" } })).toBe(false);
  });

  test("reserves Source payload id in open generated schemas", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.config.ts");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(root, "node_modules/selected", "selected", configurablePluginSource("source", true));

    const registry = await loadAllWorkspacePlugins(configPath, join(root, "global"));
    const schema = Compile(createWorkspaceSchemas(registry).source);

    expect(schema.Check({ uses: "selected", query: "ready", extra: true })).toBe(true);
    expect(schema.Check({ uses: "selected", query: "ready", id: "reserved" })).toBe(false);
  });

  test("reserves Action payload id in open generated schemas", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.config.ts");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(root, "node_modules/selected", "selected", configurablePluginSource("action", true));

    const registry = await loadAllWorkspacePlugins(configPath, join(root, "global"));
    const schema = Compile(createWorkspaceSchemas(registry).action);

    expect(schema.Check({ uses: "selected", with: { query: "ready", extra: true } })).toBe(true);
    expect(schema.Check({ uses: "selected", with: { query: "ready", id: "reserved" } })).toBe(false);
  });

  test("uses the runtime strict object rule in generated Action schemas", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.config.ts");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(root, "node_modules/selected", "selected", configurablePluginSource("action"));

    const registry = await loadAllWorkspacePlugins(configPath, join(root, "global"));
    const schema = Compile(createWorkspaceSchemas(registry).action);

    expect(schema.Check({ uses: "selected", with: { query: "ready" } })).toBe(true);
    expect(schema.Check({ uses: "selected", with: { query: "ready", extra: true } })).toBe(false);
  });

  test("accepts a descriptor from another core package copy", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.config.ts");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(
      root,
      "node_modules/other-core",
      "other-core",
      `
        const brand = Symbol.for("agentboard.pluginDescriptor");
        export default {
          kind: "source",
          schema: { type: "object" },
          runtime: () => undefined,
          healthCheck: () => undefined,
          meta: { url: import.meta.url },
          [brand]: true,
        };
      `,
    );

    const packages = discoverPluginPackages(configPath, join(root, "global"));
    await expect(loadPluginPackage("other-core", packages)).resolves.toBeDefined();
  });

  test("rejects a package that exports more than one Plugin Descriptor", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.config.ts");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(
      root,
      "node_modules/multiple",
      "multiple",
      `
        import { definePlugin } from ${JSON.stringify(
          new URL("../../../../pkgs/crates/agentboard-core/src/config.ts", import.meta.url).href,
        )};
        const definition = {
          schema: { type: "object" },
          runtime: () => undefined,
          healthCheck: () => undefined,
        };
        export const first = definePlugin(import.meta, { kind: "source", ...definition });
        export const second = definePlugin(import.meta, { kind: "source", ...definition });
      `,
    );

    const packages = discoverPluginPackages(configPath, join(root, "global"));

    await expect(loadPluginPackage("multiple", packages)).rejects.toThrow(
      "must provide exactly one Plugin Descriptor; found 2",
    );
  });

  test("normal workspace loading registers selected packages only", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.config.ts");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(root, "node_modules/selected", "selected", pluginSource("source"));
    packageFixture(root, "node_modules/ignored", "ignored", pluginSource("action"));

    const registry = await loadWorkspacePlugins(configPath, ["selected"], join(root, "global"));
    const schemas = createWorkspaceSchemas(registry);
    const serialized = JSON.stringify(schemas.workspace);

    expect([...registry.sources.keys()]).toEqual(["selected"]);
    expect([...registry.actions.keys()]).toEqual([]);
    expect(serialized).toContain("selected");
    expect(serialized).not.toContain("ignored");
  });

  test("normal data Workspace loading imports selected packages and preserves package metadata", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.toml");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    const selectedMarker = join(root, "selected-imported");
    const ignoredMarker = join(root, "ignored-imported");
    const descriptor = (marker: string) => `
      import { writeFileSync } from "node:fs";
      import { definePlugin } from ${JSON.stringify(
        new URL("../../../../pkgs/crates/agentboard-core/src/config.ts", import.meta.url).href,
      )};
      writeFileSync(${JSON.stringify(marker)}, "imported");
      export default definePlugin(import.meta, {
        kind: "source",
        schema: { type: "object", properties: { value: { type: "string" } }, required: ["value"] },
        runtime: (config) => config,
        healthCheck: () => undefined,
      });
    `;
    packageFixture(root, "node_modules/selected", "selected", descriptor(selectedMarker));
    packageFixture(root, "node_modules/ignored", "ignored", descriptor(ignoredMarker));
    writeFileSync(
      configPath,
      `[[sources]]\nid = "one"\n[sources.source]\nuses = "selected"\nvalue = "runtime"\n`,
    );

    const loaded = await loadDataWorkspace(configPath, join(root, "global"));

    expect([...loaded.registry.sources.keys()]).toEqual(["selected"]);
    expect(loaded.registry.sources.get("selected")?.plugin.meta.packageName).toBe("selected");
    expect(Bun.file(selectedMarker).size).toBeGreaterThan(0);
    expect(Bun.file(ignoredMarker).size).toBe(0);
  });

  test("preserves explicit Source additional property rules in generated schemas", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.json");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(
      root,
      "node_modules/open-source",
      "open-source",
      configurablePluginSource("source", true),
    );
    writeFileSync(configPath, JSON.stringify({
      sources: [{
        id: "one",
        source: { uses: "open-source", query: "runtime", extra: true },
      }],
    }));

    const loaded = await loadDataWorkspace(configPath, join(root, "global"));
    const schemas = createWorkspaceSchemas(loaded.registry);

    expect(loaded.sources[0]?.source.config).toEqual({ query: "runtime", extra: true });
    expect(Compile(schemas.source).Check({ uses: "open-source", query: "runtime", extra: true })).toBe(true);
  });

  test("loads equivalent executable and data Workspace files through the resolved configuration seam", async () => {
    const root = fixture();
    const executablePath = join(root, "agentboard.config.ts");
    const formats = [
      {
        name: "JSON",
        path: join(root, "agentboard.json"),
        data: JSON.stringify({
          sources: [{
            id: "one",
            source: { uses: "source-package", query: "runtime" },
            actions: [{ uses: "action-package", with: { query: "echo ready" } }],
          }],
        }),
      },
      {
        name: "YAML",
        path: join(root, "agentboard.yaml"),
        data: "sources:\n  - id: one\n    source:\n      uses: source-package\n      query: runtime\n    actions:\n      - uses: action-package\n        with:\n          query: echo ready\n",
      },
      {
        name: "TOML",
        path: join(root, "agentboard.toml"),
        data: `[[sources]]\nid = "one"\n[sources.source]\nuses = "source-package"\nquery = "runtime"\n[[sources.actions]]\nuses = "action-package"\n[sources.actions.with]\nquery = "echo ready"\n`,
      },
    ] as const;
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(root, "node_modules/source-package", "source-package", configurablePluginSource("source"));
    packageFixture(root, "node_modules/action-package", "action-package", configurablePluginSource("action"));
    workspacePackage(
      root,
      "@agentboard/core",
      new URL("../../../../pkgs/crates/agentboard-core", import.meta.url).pathname,
    );
    writeFileSync(
      executablePath,
      `
        import { action, defineConfig, source } from "@agentboard/core/config";
        import sourcePlugin from "source-package";
        import actionPlugin from "action-package";
        export default defineConfig({
          sources: [{
            id: "one",
            source: source(sourcePlugin, { query: "runtime" }, import.meta.url),
            actions: [action(actionPlugin, { query: "echo ready" }, import.meta.url)],
          }],
        });
      `,
    );
    const executable = await loadExecutableWorkspace(executablePath);

    for (const format of formats) {
      writeFileSync(format.path, format.data);
      const serialized = await loadDataWorkspace(format.path, join(root, "global"));

      expect(
        comparableWorkspace(serialized),
        format.name,
      ).toEqual(comparableWorkspace(executable));
    }
  });

  test("rejects invalid Plugin payloads in every data Workspace format", async () => {
    const root = fixture();
    const formats = [
      [join(root, "agentboard.json"), JSON.stringify({ sources: [{ id: "one", source: { uses: "selected", query: "ready", extra: true } }] })],
      [join(root, "agentboard.yaml"), "sources:\n  - id: one\n    source:\n      uses: selected\n      query: ready\n      extra: true\n"],
      [join(root, "agentboard.toml"), `[[sources]]\nid = "one"\n[sources.source]\nuses = "selected"\nquery = "ready"\nextra = true\n`],
    ] as const;
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(root, "node_modules/selected", "selected", configurablePluginSource("source"));

    for (const [path, data] of formats) {
      writeFileSync(path, data);
      await expect(loadDataWorkspace(path, join(root, "global"))).rejects.toThrow();
    }
  });

  test("keeps Action payload fields inside with", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.json");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(root, "node_modules/source-package", "source-package", configurablePluginSource("source"));
    packageFixture(root, "node_modules/action-package", "action-package", configurablePluginSource("action"));
    writeFileSync(configPath, JSON.stringify({
      sources: [{
        id: "one",
        source: { uses: "source-package", query: "runtime" },
        actions: [{ uses: "action-package", query: "flat", with: { query: "nested" } }],
      }],
    }));

    await expect(loadDataWorkspace(configPath, join(root, "global"))).rejects.toThrow();
  });

  test("rejects reserved id fields in serialized Plugin payloads", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.json");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(root, "node_modules/source-package", "source-package", configurablePluginSource("source"));
    packageFixture(root, "node_modules/action-package", "action-package", configurablePluginSource("action"));

    for (const data of [
      {
        sources: [{
          id: "one",
          source: { uses: "source-package", query: "runtime", id: "reserved" },
        }],
      },
      {
        sources: [{
          id: "one",
          source: { uses: "source-package", query: "runtime" },
          actions: [{ uses: "action-package", with: { query: "nested", id: "reserved" } }],
        }],
      },
    ]) {
      writeFileSync(configPath, JSON.stringify(data));
      await expect(loadDataWorkspace(configPath, join(root, "global"))).rejects.toThrow(
        'Plugin payload must not define reserved field "id"',
      );
    }
  });

  test("requires with before loading a defaulted Action payload", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.json");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(root, "node_modules/source-package", "source-package", configurablePluginSource("source"));
    packageFixture(
      root,
      "node_modules/action-package",
      "action-package",
      `
        import { definePlugin } from ${JSON.stringify(
          new URL("../../../../pkgs/crates/agentboard-core/src/config.ts", import.meta.url).href,
        )};
        export default definePlugin(import.meta, {
          kind: "action",
          schema: {
            type: "object",
            properties: { timeout: { type: "integer", default: 30 } },
          },
          runtime: (config) => config,
          healthCheck: () => undefined,
        });
      `,
    );
    writeFileSync(configPath, JSON.stringify({
      sources: [{
        id: "one",
        source: { uses: "source-package", query: "runtime" },
        actions: [{ uses: "action-package" }],
      }],
    }));

    await expect(loadDataWorkspace(configPath, join(root, "global"))).rejects.toThrow(
      'action in source one must define "with"',
    );
  });

  test("loads primitive Action payloads from with", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.json");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(root, "node_modules/source-package", "source-package", configurablePluginSource("source"));
    packageFixture(
      root,
      "node_modules/action-package",
      "action-package",
      `
        import { definePlugin } from ${JSON.stringify(
          new URL("../../../../pkgs/crates/agentboard-core/src/config.ts", import.meta.url).href,
        )};
        export default definePlugin(import.meta, {
          kind: "action",
          schema: { type: "string" },
          runtime: (config) => config,
          healthCheck: () => undefined,
        });
      `,
    );
    writeFileSync(configPath, JSON.stringify({
      sources: [{
        id: "one",
        source: { uses: "source-package", query: "runtime" },
        actions: [{ id: "named", uses: "action-package", with: "echo ready" }],
      }],
    }));

    const loaded = await loadDataWorkspace(configPath, join(root, "global"));

    expect(loaded.sources[0]?.actions[0]?.id).toBe("named");
    expect(loaded.sources[0]?.actions[0]?.config).toBe("echo ready");
  });

  test("loads TypeScript defineConfig output through the resolved configuration seam", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.config.ts");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    workspacePackage(
      root,
      "@agentboard/core",
      new URL("../../../../pkgs/crates/agentboard-core", import.meta.url).pathname,
    );
    writeFileSync(
      configPath,
      `
        import { action, defineConfig, definePlugin, source } from "@agentboard/core/config";
        const sourcePlugin = definePlugin(import.meta, {
          kind: "source",
          schema: { type: "object", properties: { query: { type: "string" } }, required: ["query"] },
          runtime: (config) => config,
          healthCheck: () => undefined,
        });
        const actionPlugin = definePlugin(import.meta, {
          kind: "action",
          schema: { type: "object", properties: { command: { type: "string" } }, required: ["command"] },
          runtime: (config) => config,
          healthCheck: () => undefined,
        });
        export default defineConfig({
          sources: [{
            id: "one",
            source: source(sourcePlugin, { query: "runtime" }, import.meta.url),
            actions: [action(actionPlugin, { command: "echo ready" }, import.meta.url)],
          }],
        });
      `,
    );

    const loaded = await loadExecutableWorkspace(configPath);

    expect(loaded.sources[0]?.source.config).toEqual({ query: "runtime" });
    expect(loaded.sources[0]?.actions[0]?.config).toEqual({ command: "echo ready" });
    expect(loaded.sources[0]?.source.identity.role).toBe("source");
    expect(loaded.sources[0]?.actions[0]?.identity.role).toBe("action");
  });

  test("executable Workspace errors include the config source", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.config.ts");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    writeFileSync(configPath, "export default { sources: [{ id: \"one\", actions: {} }] };\n");

    await expect(loadExecutableWorkspace(configPath)).rejects.toThrow(
      `Executable Workspace configuration failed for ${configPath}: source one actions must be an array`,
    );
  });

  test("the production run loader loads executable Workspace configuration", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.config.ts");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    writeFileSync(
      configPath,
      `
        import { definePlugin, source } from ${JSON.stringify(
          new URL("../../../../pkgs/crates/agentboard-core/src/config.ts", import.meta.url).href,
        )};
        const plugin = definePlugin(import.meta, {
          kind: "source",
          schema: { type: "object", properties: { query: { type: "string" } }, required: ["query"] },
          runtime: (config) => config,
          healthCheck: () => undefined,
        });
        export default { sources: [{ id: "one", source: source(plugin, { query: "runtime" }, import.meta.url) }] };
      `,
    );

    const loaded = await loadRunWorkspace(configPath, join(root, "global"));

    expect(loaded.sources).toHaveLength(1);
    expect(loaded.sources[0]?.source.config).toEqual({ query: "runtime" });
  });

  test("the production run loader uses the normal Workspace loader", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.toml");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(root, "node_modules/selected", "selected", pluginSource("source"));
    writeFileSync(
      configPath,
      `[[sources]]\nid = "one"\n[sources.source]\nuses = "selected"\nquery = "runtime"\n`,
    );

    const loaded = await loadRunWorkspace(configPath, join(root, "global"));

    expect(loaded.sources).toHaveLength(1);
    expect(loaded.sources[0]?.source.config).toEqual({ query: "runtime" });
    expect(loaded.sources[0]?.packageName).toBe("selected");
    expect(loaded.registry.sources.get("selected")?.plugin.meta.packageName).toBe("selected");
  });

  test("the production schema loader discovers every Plugin Package", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.toml");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(root, "node_modules/first", "first", pluginSource("source"));
    packageFixture(root, "node_modules/second", "second", pluginSource("action"));

    const registry = await loadSchemaWorkspace(configPath);

    expect([...registry.sources.keys()]).toEqual(["first"]);
    expect([...registry.actions.keys()]).toEqual(["second"]);
  });

  test("schema configuration prefers executable configuration", async () => {
    const root = fixture();
    const executablePath = join(root, "agentboard.config.ts");
    const dataPath = join(root, ".agentboard.toml");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(root, "node_modules/executable", "executable", pluginSource("source"));
    packageFixture(root, "node_modules/data", "data", pluginSource("action"));
    writeFileSync(executablePath, "export default { sources: [] };\n");
    writeFileSync(dataPath, "[[sources]]\nid = \"one\"\n");

    const registry = await loadSchemaWorkspace(executablePath);

    expect([...registry.sources.keys()]).toEqual(["executable"]);
    expect([...registry.actions.keys()]).toEqual(["data"]);
  });

  test("normal data Workspace loading supports YAML", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.yaml");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    packageFixture(root, "node_modules/selected", "selected", pluginSource("source"));
    writeFileSync(
      configPath,
      "sources:\n  - id: one\n    source:\n      uses: selected\n      query: runtime\n",
    );

    const loaded = await loadDataWorkspace(configPath, join(root, "global"));

    expect(loaded.sources[0]?.source.config).toEqual({ query: "runtime" });
  });

  test("normal data Workspace loading reports invalid data at the loader seam", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.json");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    writeFileSync(configPath, JSON.stringify({ sources: [{ id: 42, source: {} }] }));

    await expect(loadDataWorkspace(configPath, join(root, "global"))).rejects.toThrow(
      `Workspace data validation failed for ${configPath}`,
    );
  });

  test("normal data Workspace loading reports missing packages", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.toml");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    writeFileSync(
      configPath,
      `[[sources]]\nid = "one"\n[sources.source]\nuses = "missing"\n`,
    );

    await expect(loadDataWorkspace(configPath, join(root, "global"))).rejects.toThrow(
      'Plugin Package "missing" is not installed; install it with `bun add missing`',
    );
  });

  test("imports selected packages only and maps exact package metadata", async () => {
    const root = fixture();
    const configPath = join(root, "agentboard.config.ts");
    writeFileSync(join(root, "package.json"), JSON.stringify({ name: "project" }));
    const selectedMarker = join(root, "selected-imported");
    const ignoredMarker = join(root, "ignored-imported");
    const descriptor = (marker: string) => `
      import { writeFileSync } from "node:fs";
      import { definePlugin } from ${JSON.stringify(
        new URL("../../../../pkgs/crates/agentboard-core/src/config.ts", import.meta.url).href,
      )};
      writeFileSync(${JSON.stringify(marker)}, "imported");
      export default definePlugin(import.meta, {
        kind: "source",
        schema: { type: "object", properties: { value: { type: "string" } }, required: ["value"] },
        runtime: (config) => config,
        healthCheck: () => undefined,
      });
    `;
    packageFixture(
      root,
      "node_modules/selected",
      "selected",
      descriptor(selectedMarker),
    );
    packageFixture(
      root,
      "node_modules/ignored",
      "ignored",
      descriptor(ignoredMarker),
    );

    const packages = discoverPluginPackages(configPath, join(root, "global"));
    const loaded = await loadSelectedPlugins(["selected"], packages);

    expect(loaded).toHaveLength(1);
    expect(loaded[0]?.plugin.meta.packageName).toBe("selected");
    expect(Bun.file(selectedMarker).size).toBeGreaterThan(0);
    expect(Bun.file(ignoredMarker).size).toBe(0);
  });
});
