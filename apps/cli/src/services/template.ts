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
    validateActionReferences(value, context);
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

const access = String.raw`(?:\s*\.\s*[A-Za-z_]\w*|\s*\[\s*["'][^"']+["']\s*\])`;

function validateActionReferences(template: string, context: Record<string, unknown>): void {
  const aliases = new Map<string, string[]>();
  for (const block of template.matchAll(/{[{%]([\s\S]*?)(?:}}|%})/g)) {
    const expression = block[1]!;
    for (const match of unquotedMatches(
      expression,
      new RegExp(String.raw`\(?\s*\bactions${access}+(?:\s*\))?${access}*`, "g"),
    )) validateActionReference(context, actionPath(match[0]));

    const assignment = expression.match(new RegExp(
      String.raw`\b(?:set|with)\s+([A-Za-z_]\w*)\s*=\s*actions(${access}*)`,
    ));
    const iteration = expression.match(new RegExp(
      String.raw`\bfor\s+([A-Za-z_]\w*)\s+in\s+\[\s*actions(${access}*)\s*\]`,
    ));
    for (const declaration of [assignment, iteration]) {
      if (declaration) aliases.set(declaration[1]!, actionPath(declaration[2]!));
    }

    for (const [alias, prefix] of aliases) {
      for (const match of unquotedMatches(
        expression,
        new RegExp(String.raw`\b${alias}${access}+`, "g"),
      )) validateActionReference(context, [...prefix, ...actionPath(match[0])]);
    }
  }
}

function* unquotedMatches(expression: string, pattern: RegExp): Generator<RegExpMatchArray> {
  for (const match of expression.matchAll(pattern)) {
    if (!insideQuotes(expression, match.index)) yield match;
  }
}

function insideQuotes(value: string, end: number): boolean {
  let quote = "";
  for (let index = 0; index < end; index += 1) {
    if (value[index] === "\\") index += 1;
    else if (quote) {
      if (value[index] === quote) quote = "";
    } else if (value[index] === '"' || value[index] === "'") quote = value[index]!;
  }
  return quote !== "";
}

function actionPath(reference: string): string[] {
  return [...reference.matchAll(/\.\s*([A-Za-z_]\w*)|\[\s*["']([^"']+)["']\s*\]/g)]
    .map((match) => match[1] ?? match[2]!);
}

function validateActionReference(context: Record<string, unknown>, path: readonly string[]): void {
  let value = context["actions"];
  for (const key of path) {
    if (value === null || typeof value !== "object" || !Object.hasOwn(value, key)) {
      throw new Error(`undefined value: actions.${path.join(".")}`);
    }
    value = (value as Record<string, unknown>)[key];
  }
}

function environment(): Environment {
  const environment = new Environment();
  environment.undefinedBehavior = "lenient";
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
