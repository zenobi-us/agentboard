import { mkdir, writeFile } from "node:fs/promises";
import { dirname, resolve, basename } from "node:path";
import { app } from "./app.ts";

export async function initializeWorkspace(path: string): Promise<void> {
  const target = resolve(path);
  if (await Bun.file(target).exists()) throw new Error(`Workspace already exists at ${target}`);
  await mkdir(dirname(target), { recursive: true });
  await writeFile(target, "sources = []\n");
  console.log(target);
}

const workspace = app.sub("workspace");
const init = workspace
  .sub("init")
  .args([{ name: "path", type: "string", default: ".agentboard.toml" }])
  .meta({ description: "Create an empty Workspace" })
  .run(({ args }) => initializeWorkspace(args.path));

async function listWorkspaces(): Promise<void> {
  const root = resolve(process.env["XDG_CONFIG_HOME"] ?? `${process.env["HOME"] ?? "."}/.config`, "agentboard");
  const names = (await Array.fromAsync(new Bun.Glob("*.toml").scan({ cwd: root })))
    .sort()
    .map((path) => basename(path, ".toml"));
  console.log(JSON.stringify(names));
}

const list = workspace
  .sub("list")
  .meta({ description: "List configured Workspaces" })
  .run(listWorkspaces);

const edit = workspace
  .sub("edit")
  .args([{ name: "path", type: "string", default: ".agentboard.toml" }])
  .meta({ description: "Edit a Workspace" })
  .run(({ args }) => {
    const editor = process.env["EDITOR"];
    if (!editor) throw new Error("EDITOR is not set");
    const child = Bun.spawnSync([editor, resolve(args.path)], { stdin: "inherit", stdout: "inherit", stderr: "inherit" });
    if (child.exitCode !== 0) process.exitCode = child.exitCode;
  });

export const workspaceCmd = workspace.command(init).command(list).command(edit);

export const workspacesCmd = app
  .sub("workspaces")
  .meta({ description: "List configured Workspaces" })
  .run(listWorkspaces);
