import { describe, expect, test } from "bun:test";
import { invokedCliName, printLegacyDeprecation, watchView } from "./app.ts";

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
    expect(output).toContain("clankpipe list --watch\n");
    expect(output).not.toContain("\\x1b");
    expect(output).not.toContain("\\n");
  });
});

describe("CLI branding", () => {
  test("recognizes both executable names", () => {
    expect(invokedCliName("/usr/local/bin/clankpipe")).toBe("clankpipe");
    expect(invokedCliName("/usr/local/bin/agentboard")).toBe("agentboard");
  });

  test("prints the compatibility deprecation message only for AgentBoard", () => {
    const originalError = console.error;
    const messages: string[] = [];
    console.error = (message: string) => messages.push(message);
    try {
      printLegacyDeprecation("/usr/local/bin/clankpipe");
      expect(messages).toEqual([]);
      printLegacyDeprecation("/usr/local/bin/agentboard");
      expect(messages).toEqual(["agentboard is deprecated; use clankpipe instead."]);
    } finally {
      console.error = originalError;
    }
  });
});
