import { homedir } from "node:os";
import { Environment } from "minijinja-js";

export interface RenderActionInputsOptions {
  readonly pathInputs?: readonly string[];
}

export function validateActionInputs(value: unknown): void {
  visitStrings(value, (template) => environment().addTemplate("input", template));
}

export function renderActionInputs(
  value: unknown,
  context: Record<string, unknown>,
  options: RenderActionInputsOptions = {},
): unknown {
  return render(value, context, new Set(options.pathInputs ?? []));
}

function render(
  value: unknown,
  context: Record<string, unknown>,
  pathInputs: ReadonlySet<string>,
  key?: string,
): unknown {
  if (typeof value === "string") {
    const rendered = environment().renderStr(value, context);
    return key !== undefined && pathInputs.has(key) ? expandPath(rendered) : rendered;
  }
  if (Array.isArray(value)) return value.map((item) => render(item, context, pathInputs));
  if (value === null || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value).map(([itemKey, item]) => [
      itemKey,
      render(item, context, pathInputs, itemKey),
    ]),
  );
}

function environment(): Environment {
  const environment = new Environment();
  environment.undefinedBehavior = "strict";
  environment.addFilter("slugify", slugify);
  return environment;
}

function visitStrings(value: unknown, visit: (value: string) => void): void {
  if (typeof value === "string") {
    visit(value);
    return;
  }
  if (Array.isArray(value)) {
    for (const item of value) visitStrings(item, visit);
    return;
  }
  if (value !== null && typeof value === "object") {
    for (const item of Object.values(value)) visitStrings(item, visit);
  }
}

function expandPath(value: string): string {
  let expanded = value === "~" || value.startsWith("~/")
    ? `${homedir()}${value.slice(1)}`
    : value;
  expanded = expanded.replace(/\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)/g, (
    match,
    braced: string | undefined,
    plain: string | undefined,
  ) => process.env[braced ?? plain ?? ""] ?? match);
  return expanded;
}

function slugify(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}
