import { dirname, join, resolve } from "node:path"
import { readFileSync } from "node:fs"
// @ts-expect-error The TUI currently has no local React type package.
import { createContext, useContext } from "react"

type ThemeValue = string
export type ThemeStyle = {
  fg?: ThemeValue
  bg?: ThemeValue
  border?: ThemeValue
}

type ThemeDocument = {
  palette: Record<string, ThemeValue>
  semantic: Record<string, ThemeValue>
  component: Record<string, ThemeStyle>
}

const defaultTheme: ThemeDocument = {
  palette: {
    "neutral.0": "#ffffff",
    "neutral.100": "#e0def4",
    "neutral.500": "#908caa",
    "neutral.800": "#1f1d2e",
    "neutral.900": "#191724",
    "cyan.500": "#31748f",
    "yellow.400": "#f6c177",
    "red.500": "#eb6f92",
    "muted.400": "#6e6a86",
    "overlay.400": "#26233a",
    "rose.400": "#ebbcba",
    "foam.400": "#9ccfd8",
    "iris.400": "#c4a7e7",
  },
  semantic: {
    "text.primary": "{palette.neutral.100}",
    "text.muted": "{palette.muted.400}",
    "text.accent": "{palette.yellow.400}",
    "text.danger": "{palette.red.500}",
    "surface.app": "{palette.neutral.900}",
    "surface.panel": "{palette.neutral.800}",
    "surface.control": "{palette.cyan.500}",
    "surface.control.busy": "{palette.overlay.400}",
    "border.default": "{palette.overlay.400}",
    "state.selected": "{palette.rose.400}",
  },
  component: {
    "app.header": style("text.primary", "surface.app", "border.default"),
    "app.footer": style("text.muted", "surface.app", "border.default"),
    "workspace": style("text.primary", "surface.app", "border.default"),
    "badge.workspace": { bg: "{palette.cyan.500}" },
    "badge.source": { bg: "{palette.foam.400}" },
    "badge.action": { bg: "{palette.iris.400}" },
    "badge.item": { bg: "{palette.rose.400}" },
    "source.heading": style("text.accent", "surface.app", "border.default"),
    "source.item": style("text.primary", "surface.app", "border.default"),
    "source.item.selected": style("state.selected", "surface.app", "state.selected"),
    "source.tree": style("text.muted", "surface.app", "border.default"),
    "source.table": style("text.muted", "surface.app", "border.default"),
    "source.summary": style("text.primary", "surface.app", "border.default"),
    "source.summary.id": style("text.accent", "surface.app", "border.default"),
    "source.summary.action": style("text.muted", "surface.app", "border.default"),
    "item.heading": style("text.accent", "surface.app", "border.default"),
    "definition.label": style("text.muted", "surface.app", "border.default"),
    "settings.modal": style("text.primary", "surface.panel", "border.default"),
    "settings.heading": style("text.accent", "surface.panel", "border.default"),
    "settings.help": style("text.muted", "surface.panel", "border.default"),
    "button.run": style("text.primary", "surface.control", "surface.control"),
    "button.run.busy": style("text.primary", "surface.control.busy", "surface.control.busy"),
    "button.run.disabled": style("text.muted", "surface.panel", "border.default"),
    "button.watch": style("text.primary", "surface.control", "surface.control"),
    "button.watch.busy": style("text.primary", "surface.control.busy", "surface.control.busy"),
    "button.watch.disabled": style("text.muted", "surface.panel", "border.default"),
    "button.list": style("text.primary", "surface.control", "surface.control"),
    "button.list.busy": style("text.primary", "surface.control.busy", "surface.control.busy"),
    "button.list.disabled": style("text.muted", "surface.panel", "border.default"),
    loader: style("text.accent", "surface.app", "border.default"),
    error: style("text.danger", "surface.app", "border.default"),
  },
}

export type Theme = {
  color(token: string): ThemeValue
  component(token: string): ThemeStyle
}

const ThemeContext = createContext<Theme | null>(null)

export function createTheme(overrides: unknown = {}): Theme {
  const source = isRecord(overrides) && isRecord(overrides["theme"]) ? overrides["theme"] : overrides
  const document = merge(defaultTheme, isRecord(source) ? source : {}) as ThemeDocument
  const resolved = new Map<string, string>()

  const color = (token: string): string => {
    const existing = resolved.get(token)
    if (existing) return existing
    const value = getPath(document, token)
    if (typeof value !== "string") throw new Error(`Theme token "${token}" must be a color or alias`)
    const result = resolveValue(value, token, new Set())
    resolved.set(token, result)
    return result
  }

  return {
    color,
    component(token) {
      const value = getPath(document.component, token)
      if (!isRecord(value)) throw new Error(`Theme component "${token}" is not defined`)
      return {
        fg: typeof value["fg"] === "string" ? resolveValue(value["fg"], `${token}.fg`, new Set()) : undefined,
        bg: typeof value["bg"] === "string" ? resolveValue(value["bg"], `${token}.bg`, new Set()) : undefined,
        border: typeof value["border"] === "string" ? resolveValue(value["border"], `${token}.border`, new Set()) : undefined,
      }
    },
  }

  function resolveValue(value: string, token: string, stack: Set<string>): string {
    const alias = /^\{([^{}]+)\}$/.exec(value)?.[1]
    if (!alias) return value
    if (stack.has(token)) throw new Error(`Theme alias cycle at "${token}"`)
    stack.add(token)
    const target = getPath(document, alias)
    if (typeof target !== "string") throw new Error(`Theme alias "${token}" points to missing token "${alias}"`)
    return resolveValue(target, alias, stack)
  }
}

export function loadTheme(workspacePath: string): Theme {
  const path = join(dirname(resolve(workspacePath)), "agentboard.theme.json")
  try {
    return createTheme(JSON.parse(readFileSync(path, "utf8")))
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return createTheme()
    throw new Error(`Theme load failed for ${path}: ${String(error)}`)
  }
}

export function ThemeProvider(props: { theme: Theme; children: React.ReactNode }) {
  return <ThemeContext.Provider value={props.theme}>{props.children}</ThemeContext.Provider>
}

export function useTheme(): Theme {
  const theme = useContext(ThemeContext)
  if (!theme) throw new Error("useTheme must be inside ThemeProvider")
  return theme
}

function style(fg: string, bg: string, border: string): ThemeStyle {
  return { fg: `{semantic.${fg}}`, bg: `{semantic.${bg}}`, border: `{semantic.${border}}` }
}

function isRecord(value: unknown): value is Record<string, any> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
}

function merge(base: unknown, override: unknown): unknown {
  if (!isRecord(base) || !isRecord(override)) return override === undefined ? base : override
  const result: Record<string, unknown> = { ...base }
  for (const [key, value] of Object.entries(override)) result[key] = merge(result[key], value)
  return result
}

function getPath(value: unknown, path: string): unknown {
  const parts = path.split(".")
  let current: unknown = value
  for (let index = 0; index < parts.length; index += 1) {
    if (!isRecord(current)) return undefined
    const remainder = parts.slice(index).join(".")
    if (remainder in current) return current[remainder]
    current = current[parts[index]!]
  }
  return current
}
