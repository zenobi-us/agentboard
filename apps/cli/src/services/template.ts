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
    validateStrictActionExpressions(value, context);
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

const access = String.raw`(?:\s*\.\s*[A-Za-z_]\w*|\s*\[\s*(?:["'][^"']+["']|[A-Za-z_]\w*)\s*\])`;

function validateStrictActionExpressions(
  template: string,
  context: Record<string, unknown>,
): void {
  const validation = transformActionExpressions(template);
  if (validation === template) return;
  let missing = false;
  const strict = environment();
  strict.addFilter("requiredActionReference", (value: unknown) => {
    if (value === undefined) missing = true;
    return value;
  });
  strict.renderStr(validation, context);
  if (missing) throw new Error("undefined value in named Action reference");
}

function transformActionExpressions(template: string): string {
  let output = "";
  let offset = 0;
  while (offset < template.length) {
    const start = template.indexOf("{", offset);
    if (start < 0) return output + template.slice(offset);
    output += template.slice(offset, start);
    const opener = template.slice(start, start + 2);
    if (opener === "{#") {
      const end = template.indexOf("#}", start + 2);
      if (end < 0) return output + template.slice(start);
      output += template.slice(start, end + 2);
      offset = end + 2;
      continue;
    }
    if (opener !== "{{" && opener !== "{%") {
      output += template[start];
      offset = start + 1;
      continue;
    }
    const closer = opener === "{{" ? "}}" : "%}";
    const end = template.indexOf(closer, start + 2);
    if (end < 0) return output + template.slice(start);
    const expression = template.slice(start + 2, end);
    if (opener === "{%" && /^\s*-?\s*raw\b/.test(expression)) {
      const rawEnd = /{%\s*-?\s*endraw\s*-?\s*%}/g;
      rawEnd.lastIndex = end + 2;
      const match = rawEnd.exec(template);
      if (!match) return output + template.slice(start);
      output += template.slice(start, match.index + match[0].length);
      offset = match.index + match[0].length;
      continue;
    }
    const statement = opener === "{%" ? strictActionStatement(expression) : undefined;
    output += opener === "{{" && /\bactions\b/.test(expression)
      ? `{{ (${expression})|requiredActionReference }}`
      : statement === undefined
        ? template.slice(start, end + 2)
        : `{% ${statement} %}`;
    offset = end + 2;
  }
  return output;
}

function strictActionStatement(expression: string): string | undefined {
  const match = expression.match(/^(\s*-?\s*(?:if|elif)\s+)([\s\S]*?)(\s*-?\s*)$/);
  if (!match || !/\bactions\b/.test(match[2]!)) return undefined;
  return `${match[1]}(${match[2]})|requiredActionReference${match[3]}`;
}

function validateActionReferences(template: string, context: Record<string, unknown>): void {
  const aliases = new Map<string, string[]>([["actions", []]]);
  const stringVariables = new Map<string, string>();
  const evaluationContext = { ...context };
  for (const expression of activeExpressions(template)) {
    const roots = [...aliases.keys()].map(escapeRegExp).join("|");
    const referencePattern = new RegExp(
      String.raw`\(?\s*\b(${roots})${access}*(?:\s*\))?${access}*`,
      "g",
    );
    for (const match of unquotedMatches(expression, referencePattern)) {
      const prefix = aliases.get(match[1]!)!;
      const path = [...prefix, ...actionPath(match[0], stringVariables)];
      if (path.length > 0) validateActionReference(context, path);
    }

    const assignment = expression.match(new RegExp(
      String.raw`\b(?:set|with)\s+([A-Za-z_]\w*)\s*=\s*\(?\s*(${roots})(${access}*)\s*\)?`,
    ));
    const iteration = expression.match(new RegExp(
      String.raw`\bfor\s+([A-Za-z_]\w*)\s+in\s+\[\s*\(?\s*(${roots})(${access}*)\s*\)?\s*\]`,
    ));
    for (const declaration of [assignment, iteration]) {
      if (!declaration) continue;
      aliases.set(declaration[1]!, [
        ...aliases.get(declaration[2]!)!,
        ...actionPath(declaration[3]!, stringVariables),
      ]);
    }

    const valueAssignment = expression.match(
      /^\s*-?\s*set\s+([A-Za-z_]\w*)\s*=\s*([\s\S]*?)\s*-?\s*$/,
    );
    if (valueAssignment) {
      try {
        const value = environment().evalExpr(valueAssignment[2]!, evaluationContext);
        evaluationContext[valueAssignment[1]!] = value;
        if (typeof value === "string") stringVariables.set(valueAssignment[1]!, value);
      } catch {
        // The normal MiniJinja render reports expression errors with template context.
      }
    }
  }
}

function* activeExpressions(template: string): Generator<string> {
  let offset = 0;
  while (offset < template.length) {
    const start = template.indexOf("{", offset);
    if (start < 0) return;
    const opener = template.slice(start, start + 2);
    if (opener === "{#") {
      const end = template.indexOf("#}", start + 2);
      offset = end < 0 ? template.length : end + 2;
      continue;
    }
    if (opener !== "{{" && opener !== "{%") {
      offset = start + 1;
      continue;
    }
    const closer = opener === "{{" ? "}}" : "%}";
    const end = template.indexOf(closer, start + 2);
    if (end < 0) return;
    const expression = template.slice(start + 2, end);
    if (opener === "{%" && /^\s*-?\s*raw\b/.test(expression)) {
      const rawEnd = /{%\s*-?\s*endraw\s*-?\s*%}/g;
      rawEnd.lastIndex = end + 2;
      const match = rawEnd.exec(template);
      offset = match ? match.index + match[0].length : template.length;
      continue;
    }
    yield expression;
    offset = end + 2;
  }
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
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

function actionPath(reference: string, stringVariables: ReadonlyMap<string, string>): string[] {
  return [...reference.matchAll(
    /\.\s*([A-Za-z_]\w*)|\[\s*(?:["']([^"']+)["']|([A-Za-z_]\w*))\s*\]/g,
  )].map((match) => match[1] ?? match[2] ?? stringVariables.get(match[3]!) ?? match[3]!);
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
