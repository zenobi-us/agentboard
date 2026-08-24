import { createCliRenderer } from "@opentui/core";
import { createRoot } from "@opentui/react";
import { App } from "./App";

type TuiHost = {
  renderer: Awaited<ReturnType<typeof createCliRenderer>>;
  root: ReturnType<typeof createRoot>;
};

const globalHost = globalThis as typeof globalThis & {
  __agentboardTui?: TuiHost;
};

function disposeExistingTui(): void {
  const previous = globalHost.__agentboardTui;
  if (!previous) return;
  delete globalHost.__agentboardTui;
  previous.root.unmount();
  previous.renderer.destroy();
}

export async function startTui(workspacePath: string, version: string): Promise<void> {
  if (!process.stdin.isTTY || !process.stdout.isTTY) {
    throw new Error("TUI requires interactive stdin and stdout");
  }

  disposeExistingTui();
  const renderer = await createCliRenderer({ exitOnCtrlC: false });
  const root = createRoot(renderer);
  globalHost.__agentboardTui = { renderer, root };

  try {
    root.render(<App workspacePath={workspacePath} version={version} />);
    await new Promise<void>((resolve) => renderer.once("destroy", resolve));
  } finally {
    if (globalHost.__agentboardTui?.renderer === renderer) {
      delete globalHost.__agentboardTui;
    }
    root.unmount();
    renderer.destroy();
  }
}
