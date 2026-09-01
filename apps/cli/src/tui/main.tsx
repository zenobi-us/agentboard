import { createCliRenderer } from "@opentui/core";
import { createRoot } from "@opentui/react";
import { App } from "./App";

type TuiHost = {
  renderer: Awaited<ReturnType<typeof createCliRenderer>>;
  root: ReturnType<typeof createRoot>;
};

const globalHost = globalThis as typeof globalThis & {
  __clankpipeTui?: TuiHost;
};

function disposeExistingTui(): void {
  const previous = globalHost.__clankpipeTui;
  if (!previous) return;
  delete globalHost.__clankpipeTui;
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
  globalHost.__clankpipeTui = { renderer, root };

  try {
    root.render(<App workspacePath={workspacePath} version={version} />);
    await new Promise<void>((resolve) => renderer.once("destroy", resolve));
  } finally {
    if (globalHost.__clankpipeTui?.renderer === renderer) {
      delete globalHost.__clankpipeTui;
    }
    root.unmount();
    renderer.destroy();
  }
}
