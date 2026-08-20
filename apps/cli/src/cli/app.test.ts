import { describe, expect, test } from "bun:test";
import { watchView } from "./app.ts";

describe("Watch Mode view output", () => {
  test("writes terminal escapes and line breaks", async () => {
    const controller = new AbortController();
    const chunks: string[] = [];
    const originalWrite = process.stdout.write;
    const originalIsTTY = process.stdout.isTTY;
    Object.defineProperty(process.stdout, "isTTY", { configurable: true, value: true });
    process.stdout.write = ((chunk: string | Uint8Array) => {
      chunks.push(typeof chunk === "string" ? chunk : new TextDecoder().decode(chunk));
      return true;
    }) as typeof process.stdout.write;
    let renders = 0;
    try {
      await watchView("list", 0, async () => {
        renders += 1;
        if (renders === 2) controller.abort();
        return "view";
      }, controller.signal);
    } finally {
      process.stdout.write = originalWrite;
      Object.defineProperty(process.stdout, "isTTY", { configurable: true, value: originalIsTTY });
    }
    const output = chunks.join("");
    expect(output).toContain("\x1b[2J\x1b[H");
    expect(output).toContain("agentboard list --watch\n");
    expect(output).not.toContain("\\x1b");
    expect(output).not.toContain("\\n");
  });
});
