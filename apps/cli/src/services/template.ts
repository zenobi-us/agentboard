import { Environment } from "minijinja-js";

export function validateActionInputs(value: unknown): void {
  visitStrings(value, (template) => environment().addTemplate("input", template));
}

export function renderActionInputs(
  value: unknown,
  context: Record<string, unknown>,
): unknown {
  if (typeof value === "string") return environment().renderStr(value, context);
  if (Array.isArray(value)) return value.map((item) => renderActionInputs(item, context));
  if (value === null || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value).map(([key, item]) => [key, renderActionInputs(item, context)]),
  );
}

function environment(): Environment {
  const environment = new Environment();
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

function slugify(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}
