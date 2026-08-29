// @ts-expect-error The TUI currently has no local React type package.
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react"
import { useTheme } from "../theme/theme.tsx"

type ToastVariant = "info" | "success" | "warning" | "error"

export type ToastOptions = {
  message: string
  variant?: ToastVariant
  duration?: number
}

type Toast = ToastOptions & { id: number }

type ToastContextValue = {
  toast(options: ToastOptions): number
  dismiss(id: number): void
}

const ToastContext = createContext<ToastContextValue | null>(null)
const defaultDuration = 4000

export function ToastProvider(props: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([])
  const nextId = useRef(0)
  const timers = useRef(new Map<number, ReturnType<typeof setTimeout>>())

  useEffect(() => () => {
    for (const timer of timers.current.values()) clearTimeout(timer)
    timers.current.clear()
  }, [])

  const dismiss = useCallback((id: number) => {
    const timer = timers.current.get(id)
    if (timer) clearTimeout(timer)
    timers.current.delete(id)
    setToasts((current: Toast[]) => current.filter((item: Toast) => item.id !== id))
  }, [])

  const toast = useCallback((options: ToastOptions): number => {
    const id = nextId.current++
    setToasts((current: Toast[]) => [...current, { ...options, id }])

    const duration = options.duration ?? defaultDuration
    if (duration > 0) {
      timers.current.set(id, setTimeout(() => dismiss(id), duration))
    }

    return id
  }, [dismiss])
  const context = useMemo(() => ({ toast, dismiss }), [toast, dismiss])

  return (
    <ToastContext.Provider value={context}>
      <box position="relative" flexGrow={1}>
        {props.children}
        <ToastViewport toasts={toasts} dismiss={dismiss} />
      </box>
    </ToastContext.Provider>
  )
}

export function useToast(): ToastContextValue {
  const value = useContext(ToastContext)
  if (!value) throw new Error("useToast must be inside ToastProvider")
  return value
}

function ToastViewport(props: { toasts: Toast[]; dismiss(id: number): void }) {
  if (props.toasts.length === 0) return null

  return (
    <box position="absolute" right={1} bottom={1} zIndex={20} flexDirection="column">
      {props.toasts.map((toast) => (
        <ToastCard key={toast.id} toast={toast} onDismiss={() => props.dismiss(toast.id)} />
      ))}
    </box>
  )
}

function ToastCard(props: { toast: Toast; onDismiss(): void }) {
  const theme = useTheme()
  const style = theme.component("error")
  const colors: Record<ToastVariant, string | undefined> = {
    info: theme.color("palette.foam.400"),
    success: theme.color("palette.cyan.500"),
    warning: theme.color("palette.yellow.400"),
    error: style.fg,
  }
  const color = colors[props.toast.variant ?? "info"]

  return (
    <box
      flexDirection="row"
      marginTop={1}
      paddingX={1}
      border={true}
      borderStyle="single"
      borderColor={color}
      backgroundColor={style.bg}
    >
      <text fg={color}>{props.toast.message}</text>
      <box marginLeft={1} onMouseDown={props.onDismiss}>
        <text fg={color}>×</text>
      </box>
    </box>
  )
}
