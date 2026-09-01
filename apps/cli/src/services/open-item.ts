import { renderActionInputs } from "./template.ts"

export async function openItem(
  command: string,
  context: Record<string, unknown>,
  signal: AbortSignal,
): Promise<void> {
  const rendered = renderActionInputs(command, context)
  if (typeof rendered !== "string" || rendered.trim().length === 0) {
    throw new Error("open command rendered to an empty string")
  }
  if (signal.aborted) throw new Error("open command cancelled")

  const child = Bun.spawn(["sh", "-c", rendered], {
    stdout: "ignore",
    stderr: "ignore",
    detached: true,
  })
  const stop = () => child.kill()
  signal.addEventListener("abort", stop, { once: true })
  try {
    const code = await child.exited
    if (signal.aborted) throw new Error("open command cancelled")
    if (code !== 0) throw new Error(`open command exited with ${code}`)
  } finally {
    signal.removeEventListener("abort", stop)
  }
}
