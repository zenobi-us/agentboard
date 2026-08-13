import { homedir } from "node:os";
import { Environment } from "minijinja-js";

export interface RenderActionInputsOptions {
  readonly pathInputs?: readonly string[];
}

export function validateActionInputs(value: unknown): void {
  visitStrings(value, (template) => environment(false).addTemplate("input", template));
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
    const rendered = environment(referencesActions(value)).renderStr(value, context);
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

function referencesActions(template: string): boolean {
  for (const block of template.matchAll(/{[{%#]([\s\S]*?)(?:}}|%}|#})/g)) {
    if (block[0].startsWith("{#")) continue;
    if (hasActionsIdentifier(block[1]!)) return true;
  }
  return false;
}

function hasActionsIdentifier(expression: string): boolean {
  let quote = "";
  for (let index = 0; index < expression.length;) {
    const character = expression[index]!;
    if (quote) {
      if (character === "\\") index += 2;
      else {
        if (character === quote) quote = "";
        index += 1;
      }
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
      index += 1;
      continue;
    }
    if (/[A-Za-z_]/.test(character)) {
      let end = index + 1;
      while (end < expression.length && /[A-Za-z0-9_]/.test(expression[end]!)) end += 1;
      if (expression.slice(index, end) === "actions") return true;
      index = end;
      continue;
    }
    index += 1;
  }
  return false;
}

function environment(strict: boolean): Environment {
  const environment = new Environment();
  environment.undefinedBehavior = strict ? "strict" : "lenient";
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
