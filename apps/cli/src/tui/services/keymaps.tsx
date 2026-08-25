import type { KeyEvent } from "@opentui/core"
// @ts-expect-error The TUI currently has no local React type package.
import { createContext, useContext, useEffect, useRef } from "react"
import { useKeyboard } from "@opentui/react"
import type { AnyActorRef } from "xstate"

export type CommandCode = string
export type KeyStroke = readonly string[]
export type Keymap = Readonly<Record<CommandCode, KeyStroke>>

type Scope = {
  actor: AnyActorRef
  bindings: Keymap
}

type KeymapRegistry = {
  register(scope: Scope): () => void
  dispatch(event: KeyEvent): void
}

const defaultKeymap: Keymap = {
  "app.quit": ["ctrl", "q"],
  "app.refresh": ["ctrl", "r"],
  "app.open-settings": ["ctrl", "s"],
  "modal.close": ["escape"],
}

const KeymapRegistryContext = createContext<KeymapRegistry | null>(null)

function aliasKey(key: string): string {
  return ({ esc: "escape", enter: "return" } as Record<string, string>)[key] ?? key
}

function canonicalStroke(stroke: KeyStroke): string {
  const modifiers = new Set(stroke.slice(0, -1).map((key) => aliasKey(key.toLowerCase())))
  const name = aliasKey(stroke.at(-1)?.toLowerCase() ?? "")
  return [...["ctrl", "meta", "shift", "option", "super", "hyper"].filter((key) => modifiers.has(key)), name].join("+")
}

function eventStroke(event: KeyEvent): string {
  return canonicalStroke([
    ...(event.ctrl ? ["ctrl"] : []),
    ...(event.meta ? ["meta"] : []),
    ...(event.shift ? ["shift"] : []),
    ...(event.option ? ["option"] : []),
    ...(event.super ? ["super"] : []),
    ...(event.hyper ? ["hyper"] : []),
    event.name,
  ])
}

function findCommand(bindings: Keymap, stroke: string): CommandCode | undefined {
  return (Object.entries(bindings) as [CommandCode, KeyStroke][]).find(
    ([, binding]) => canonicalStroke(binding) === stroke,
  )?.[0]
}

export function KeymapProvider(props: { children: React.ReactNode }) {
  const scopes = useRef<Scope[]>([])

  const registry = useRef<KeymapRegistry>({
    register(scope: Scope) {
      scopes.current.push(scope)
      return () => {
        scopes.current = scopes.current.filter((candidate: Scope) => candidate !== scope)
      }
    },
    dispatch(event: KeyEvent) {
      if (event.eventType === "release") return
      const stroke = eventStroke(event)

      for (let index = scopes.current.length - 1; index >= 0; index -= 1) {
        const scope = scopes.current[index]
        const code = findCommand(scope.bindings, stroke)
        if (!code) continue
        scope.actor.send({ type: "COMMAND", code })
        return
      }
    },
  }).current

  useKeyboard(registry.dispatch)

  return (
    <KeymapRegistryContext.Provider value={registry}>
      {props.children}
    </KeymapRegistryContext.Provider>
  )
}

export function KeymapScope(props: {
  actor: AnyActorRef
  bindings?: Keymap
  children: React.ReactNode
}) {
  const registry = useContext(KeymapRegistryContext)
  if (!registry) throw new Error("KeymapScope must be inside KeymapProvider")

  useEffect(
    () => registry.register({ actor: props.actor, bindings: props.bindings ?? defaultKeymap }),
    [props.actor, props.bindings, registry],
  )

  return props.children
}

export const appKeymap: Keymap = {
  "app.quit": ["ctrl", "q"],
  "app.refresh": ["ctrl", "r"],
  "app.run": ["r"],
  "app.watch": ["w"],
  "app.list": ["l"],
  "app.view-workspace": ["1"],
  "app.view-items": ["2"],
  "app.open-settings": ["ctrl", "s"],
}

export const workspaceKeymap: Keymap = {
  "workspace.next": ["down"],
  "workspace.previous": ["up"],
  "workspace.open-source": ["return"],
  "workspace.open-item": ["tab"],
}

export const sourceKeymap: Keymap = {
  "source.back": ["escape"],
  "source.next": ["down"],
  "source.previous": ["up"],
  "source.open-item": ["return"],
}

export const itemKeymap: Keymap = {
  "item.back": ["escape"],
  "item.run": ["r"],
}

export const modalKeymap: Keymap = {
  "modal.close": ["escape"],
}
