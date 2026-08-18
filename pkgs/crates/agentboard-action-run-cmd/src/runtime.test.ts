import { expect, test } from "bun:test";
import plugin from "./config.ts";
import { runtime } from "./runtime.ts";

test("executes rendered command inputs with action identity environment", async () => {
  const result = await runtime({ cmd: "ignored", cwd: null, healthcheck: null, healthcheck_interval: "1s", healthcheck_timeout: "1s" }).execute({
    workspaceId: "workspace",
    sourceId: "source",
    item: { id: "item" } as never,
    inputs: { cmd: "printf '%s|%s|%s' \"$AGENTBOARD_WORKSPACE_ID\" \"$AGENTBOARD_SOURCE_ID\" \"$AGENTBOARD_ITEM_ID\"", cwd: null, healthcheck: null, healthcheck_interval: "1s", healthcheck_timeout: "1s" },
    cancellation: new AbortController().signal,
  });
  expect(result).toMatchObject({ outcome: "success", stdout: "workspace|source|item" });
});

test("caps launch and healthcheck output at 64 KiB", async () => {
  const result = await runtime({ cmd: "ignored", cwd: null, healthcheck: null, healthcheck_interval: "1ms", healthcheck_timeout: "10ms" }).execute({
    workspaceId: "workspace",
    sourceId: "source",
    item: { id: "item" } as never,
    inputs: { cmd: "head -c 70000 /dev/zero | tr '\\0' l", cwd: null, healthcheck: "printf probe; exit 1", healthcheck_interval: "1ms", healthcheck_timeout: "3ms" },
    cancellation: new AbortController().signal,
  });
  expect(result.outcome).toBe("failure");
  expect(new TextEncoder().encode(result.stdout).byteLength).toBeLessThanOrEqual(64 * 1024);
  expect(result.stdout.endsWith("probe")).toBe(true);
});

test("validates healthcheck durations during Plugin configuration", () => {
  expect(() => plugin.validate!({ cmd: "true", healthcheck_interval: "0s", healthcheck_timeout: "1s" })).toThrow("invalid duration 0s");
  expect(() => plugin.validate!({ cmd: "true", healthcheck_interval: "1s", healthcheck_timeout: "never" })).toThrow("invalid duration never");
});

test("returns partial launch output when cancellation interrupts a healthcheck", async () => {
  const controller = new AbortController();
  const promise = runtime({ cmd: "printf launched", cwd: null, healthcheck: null, healthcheck_interval: "1s", healthcheck_timeout: "1s" }).execute({
    workspaceId: "workspace",
    sourceId: "source",
    item: {} as never,
    inputs: { cmd: "printf launched", cwd: null, healthcheck: "sleep 10", healthcheck_interval: "1s", healthcheck_timeout: "30s" },
    cancellation: controller.signal,
  });
  await Bun.sleep(20);
  controller.abort();
  expect(await promise).toMatchObject({ outcome: "cancelled", stdout: "launched" });
});
